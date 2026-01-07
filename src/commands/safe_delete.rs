//! SafeDelete command implementation
//!
//! Provides safe deletion of Vantiq resources with:
//! - Dry-run mode to preview what would be deleted
//! - Automatic backup before deletion
//! - Confirmation prompts
//! - Limits to prevent accidental mass deletion

use crate::cli::{OutputFormat, SafeDeleteArgs};
use crate::config::Config;
use crate::error::{Result, VqxError};
use crate::profile::ProfileManager;
use crate::underlying::{CliOptions, UnderlyingCli};
use chrono::Local;
use console::style;
use dialoguer::Confirm;
use serde::Serialize;
use serde_json::Value;
use std::fs;
use std::path::PathBuf;
use tracing::{info, warn};

/// Default limit for deleteMatching to prevent accidental mass deletion
const DEFAULT_DELETE_LIMIT: usize = 100;

/// Result of a safe delete operation
#[derive(Debug, Serialize)]
pub struct SafeDeleteResult {
    pub success: bool,
    pub dry_run: bool,
    pub resource_type: String,
    pub target: String,
    pub items_found: usize,
    pub items_deleted: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub backup_path: Option<PathBuf>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// Run the safe-delete command
pub async fn run(
    args: &SafeDeleteArgs,
    config: &Config,
    profile_name: Option<&str>,
    output_format: OutputFormat,
    verbose: bool,
) -> Result<SafeDeleteResult> {
    info!(
        resource = %args.resource,
        target = %args.target,
        dry_run = args.dry_run,
        "Running safe-delete"
    );

    let cli = UnderlyingCli::new(config.cli_path.clone())
        .with_timeout(config.timeout())
        .with_retries(config.max_retries, config.retry_delay_ms);

    let options = build_cli_options(profile_name)?;

    // Determine if this is a single delete or deleteMatching
    let is_matching = args.target.starts_with('{');

    if verbose {
        println!();
        println!("{}", style("Safe Delete").bold().red());
        println!("{}", style("─".repeat(40)).dim());
        println!("Resource type: {}", style(&args.resource).yellow());
        println!(
            "Target: {}",
            if is_matching {
                style("(query)").dim().to_string()
            } else {
                style(&args.target).cyan().to_string()
            }
        );
        if args.dry_run {
            println!("Mode: {}", style("DRY RUN").yellow().bold());
        }
        println!();
    }

    // Step 1: Find what would be deleted
    let items = find_items(&cli, &options, &args.resource, &args.target, is_matching).await?;
    let items_count = items.len();

    if items_count == 0 {
        let result = SafeDeleteResult {
            success: true,
            dry_run: args.dry_run,
            resource_type: args.resource.clone(),
            target: args.target.clone(),
            items_found: 0,
            items_deleted: 0,
            backup_path: None,
            error: None,
        };
        display_result(&result, output_format, verbose);
        return Ok(result);
    }

    // Step 2: Check limits for deleteMatching
    if is_matching && items_count > DEFAULT_DELETE_LIMIT && !args.force {
        let error_msg = format!(
            "Found {} items to delete, which exceeds the limit of {}. Use --force to override.",
            items_count, DEFAULT_DELETE_LIMIT
        );
        warn!("{}", error_msg);
        return Ok(SafeDeleteResult {
            success: false,
            dry_run: args.dry_run,
            resource_type: args.resource.clone(),
            target: args.target.clone(),
            items_found: items_count,
            items_deleted: 0,
            backup_path: None,
            error: Some(error_msg),
        });
    }

    // Display items to be deleted
    if verbose || args.dry_run {
        println!(
            "{} Found {} item(s) to delete:",
            style("→").cyan(),
            items_count
        );
        for item in &items {
            if let Some(name) = item.get("name").and_then(|v| v.as_str()) {
                println!("  - {}", style(name).yellow());
            } else if let Some(id) = item.get("_id").and_then(|v| v.as_str()) {
                println!("  - {}", style(id).dim());
            }
        }
        println!();
    }

    // If dry-run, stop here
    if args.dry_run {
        println!(
            "{} Dry run complete. No items were deleted.",
            style("✓").green()
        );
        return Ok(SafeDeleteResult {
            success: true,
            dry_run: true,
            resource_type: args.resource.clone(),
            target: args.target.clone(),
            items_found: items_count,
            items_deleted: 0,
            backup_path: None,
            error: None,
        });
    }

    // Step 3: Confirmation
    if !args.yes {
        let prompt = format!(
            "Are you sure you want to delete {} {}(s)?",
            items_count, args.resource
        );
        let confirmed = Confirm::new()
            .with_prompt(prompt)
            .default(false)
            .interact()
            .map_err(|e| VqxError::Other(format!("Confirmation failed: {}", e)))?;

        if !confirmed {
            println!("{} Operation cancelled.", style("✗").yellow());
            return Ok(SafeDeleteResult {
                success: false,
                dry_run: false,
                resource_type: args.resource.clone(),
                target: args.target.clone(),
                items_found: items_count,
                items_deleted: 0,
                backup_path: None,
                error: Some("Operation cancelled by user".to_string()),
            });
        }
    }

    // Step 4: Create backup
    let backup_path = if !args.no_backup {
        Some(create_backup(&args.resource, &items)?)
    } else {
        None
    };

    if let Some(ref path) = backup_path {
        println!(
            "{} Backup saved to: {}",
            style("✓").green(),
            style(path.display()).dim()
        );
    }

    // Step 5: Execute deletion
    let deleted_count = if is_matching {
        delete_matching(&cli, &options, &args.resource, &args.target).await?
    } else {
        delete_single(&cli, &options, &args.resource, &args.target).await?
    };

    let result = SafeDeleteResult {
        success: true,
        dry_run: false,
        resource_type: args.resource.clone(),
        target: args.target.clone(),
        items_found: items_count,
        items_deleted: deleted_count,
        backup_path,
        error: None,
    };

    display_result(&result, output_format, verbose);
    Ok(result)
}

/// Find items that match the target
async fn find_items(
    cli: &UnderlyingCli,
    options: &CliOptions,
    resource: &str,
    target: &str,
    is_matching: bool,
) -> Result<Vec<Value>> {
    let exec_result = if is_matching {
        // Use select with query
        let mut args = vec![resource.to_string()];
        args.push("-qual".to_string());
        args.push(target.to_string());
        cli.execute(options, "select", args).await?
    } else {
        // Find single item
        cli.execute(options, "find", [resource, target]).await?
    };

    if !exec_result.success() {
        // If not found, return empty
        if exec_result.stderr.contains("not found")
            || exec_result.stderr.contains("No results")
            || exec_result.stdout.trim().is_empty()
        {
            return Ok(vec![]);
        }
        return Err(VqxError::CliExecutionFailed {
            code: exec_result.code(),
            message: exec_result.stderr,
        });
    }

    // Parse JSON output
    let stdout = exec_result.stdout.trim();
    if stdout.is_empty() {
        return Ok(vec![]);
    }

    let parsed: Value = serde_json::from_str(stdout)
        .map_err(|e| VqxError::Other(format!("Failed to parse response: {}", e)))?;

    match parsed {
        Value::Array(arr) => Ok(arr),
        Value::Object(_) => Ok(vec![parsed]),
        _ => Ok(vec![]),
    }
}

/// Create a backup of items before deletion
fn create_backup(resource: &str, items: &[Value]) -> Result<PathBuf> {
    let timestamp = Local::now().format("%Y%m%d_%H%M%S");
    let backup_dir = dirs::data_local_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("vqx")
        .join("backups");

    fs::create_dir_all(&backup_dir)
        .map_err(|e| VqxError::Other(format!("Failed to create backup directory: {}", e)))?;

    let filename = format!("{}_{}.json", resource, timestamp);
    let backup_path = backup_dir.join(filename);

    let backup_data = serde_json::to_string_pretty(items)
        .map_err(|e| VqxError::Other(format!("Failed to serialize backup: {}", e)))?;

    fs::write(&backup_path, backup_data)
        .map_err(|e| VqxError::Other(format!("Failed to write backup: {}", e)))?;

    info!(path = %backup_path.display(), "Backup created");
    Ok(backup_path)
}

/// Delete a single item
async fn delete_single(
    cli: &UnderlyingCli,
    options: &CliOptions,
    resource: &str,
    resource_id: &str,
) -> Result<usize> {
    let exec_result = cli
        .execute(options, "delete", [resource, resource_id])
        .await?;

    if exec_result.success() {
        Ok(1)
    } else {
        Err(VqxError::CliExecutionFailed {
            code: exec_result.code(),
            message: exec_result.stderr,
        })
    }
}

/// Delete items matching a query
async fn delete_matching(
    cli: &UnderlyingCli,
    options: &CliOptions,
    resource: &str,
    query: &str,
) -> Result<usize> {
    let exec_result = cli
        .execute(options, "deleteMatching", [resource, query])
        .await?;

    if exec_result.success() {
        // Try to parse the count from output
        let count = exec_result
            .stdout
            .lines()
            .find_map(|line| {
                if line.contains("deleted") {
                    line.split_whitespace()
                        .find_map(|word| word.parse::<usize>().ok())
                } else {
                    None
                }
            })
            .unwrap_or(1);
        Ok(count)
    } else {
        Err(VqxError::CliExecutionFailed {
            code: exec_result.code(),
            message: exec_result.stderr,
        })
    }
}

/// Build CLI options from profile
fn build_cli_options(profile_name: Option<&str>) -> Result<CliOptions> {
    if let Some(name) = profile_name {
        let manager = ProfileManager::new()?;
        let profile = manager.get_resolved(name)?;
        Ok(CliOptions::from_profile(&profile))
    } else {
        Ok(CliOptions::default())
    }
}

/// Display the result
fn display_result(result: &SafeDeleteResult, output_format: OutputFormat, verbose: bool) {
    match output_format {
        OutputFormat::Json => {
            if let Ok(json) = serde_json::to_string_pretty(result) {
                println!("{}", json);
            }
        }
        OutputFormat::Text | OutputFormat::Csv => {
            if verbose {
                println!();
                println!("{}", style("─".repeat(40)).dim());
            }

            if result.success {
                if result.dry_run {
                    println!(
                        "{} Would delete {} item(s)",
                        style("✓").green().bold(),
                        result.items_found
                    );
                } else if result.items_deleted > 0 {
                    println!(
                        "{} Successfully deleted {} item(s)",
                        style("✓").green().bold(),
                        result.items_deleted
                    );
                } else {
                    println!("{} No items to delete", style("✓").green().bold());
                }
            } else {
                println!("{} Delete failed", style("✗").red().bold());
                if let Some(ref err) = result.error {
                    eprintln!("{}", style(err).red());
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_matching_query() {
        assert!("{\"name\": \"test\"}".starts_with('{'));
        assert!(!"MyResource".starts_with('{'));
    }
}
