//! Promote command implementation
//!
//! Promotes resources from one Vantiq environment to another.
//! Workflow: export from source -> diff (optional) -> confirm -> import to target -> test (optional)

use crate::cli::{OutputFormat, PromoteArgs};
use crate::config::Config;
use crate::error::{Result, VqxError};
use crate::profile::ProfileManager;
use crate::underlying::{CliOptions, UnderlyingCli};
use console::style;
use dialoguer::Confirm;
use serde::Serialize;
use std::path::PathBuf;
use tempfile::TempDir;
use tracing::info;
use walkdir::WalkDir;

/// Result of a promote operation
#[derive(Debug, Serialize)]
pub struct PromoteResult {
    pub success: bool,
    pub source_profile: String,
    pub target_profile: String,
    pub exported: bool,
    pub imported: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub test_result: Option<TestResult>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// Test execution result
#[derive(Debug, Serialize)]
pub struct TestResult {
    pub success: bool,
    pub test_type: String,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub output: Option<String>,
}

/// Run the promote command
pub async fn run(
    args: &PromoteArgs,
    config: &Config,
    _profile_name: Option<&str>,
    output_format: OutputFormat,
    verbose: bool,
) -> Result<PromoteResult> {
    info!(
        from = %args.from,
        to = %args.to,
        "Running promote"
    );

    // Validate profiles exist
    let manager = ProfileManager::new()?;
    let source_profile = manager.get_resolved(&args.from)?;
    let target_profile = manager.get_resolved(&args.to)?;

    if !source_profile.has_auth() {
        return Err(VqxError::ProfileInvalid {
            message: format!("Source profile '{}' has no authentication", args.from),
        });
    }
    if !target_profile.has_auth() {
        return Err(VqxError::ProfileInvalid {
            message: format!("Target profile '{}' has no authentication", args.to),
        });
    }

    let cli = UnderlyingCli::new(config.cli_path.clone())
        .with_timeout(config.timeout())
        .with_retries(config.max_retries, config.retry_delay_ms);

    // Display promotion info
    if !matches!(output_format, OutputFormat::Json) {
        println!();
        println!("{}", style("Promote").bold().magenta());
        println!("{}", style("─".repeat(50)).dim());
        println!(
            "  From:   {} ({})",
            style(&args.from).cyan(),
            style(&source_profile.url).dim()
        );
        println!(
            "  To:     {} ({})",
            style(&args.to).yellow(),
            style(&target_profile.url).dim()
        );
        if let Some(ref ts) = args.testsuite {
            println!("  Test:   testsuite '{}'", style(ts).green());
        }
        if let Some(ref proc) = args.procedure {
            println!("  Test:   procedure '{}'", style(proc).green());
        }
        println!();
    }

    // Create temporary directory for export
    let temp_dir = TempDir::new()
        .map_err(|e| VqxError::Other(format!("Failed to create temp directory: {}", e)))?;
    let export_path = temp_dir.path().to_path_buf();

    // Step 1: Export from source
    println!("{} Exporting from source...", style("→").cyan());
    let source_options = CliOptions::from_profile(&source_profile);
    let export_result = cli
        .export(
            &source_options,
            Some("metadata"),
            Some(export_path.to_str().unwrap()),
            None,
            None,
            None,
            None,
            false,
        )
        .await?;

    if !export_result.success() {
        return Ok(PromoteResult {
            success: false,
            source_profile: args.from.clone(),
            target_profile: args.to.clone(),
            exported: false,
            imported: false,
            test_result: None,
            error: Some(format!("Export failed: {}", export_result.stderr)),
        });
    }

    // Count exported files
    let file_count = count_json_files(&export_path);
    println!(
        "{} Exported {} resource file(s)",
        style("✓").green(),
        file_count
    );

    // Step 2: Show diff (if not skipped)
    if !args.no_diff {
        println!();
        println!("{} Comparing with target...", style("→").cyan());

        // Export from target for comparison
        let target_temp = TempDir::new()
            .map_err(|e| VqxError::Other(format!("Failed to create temp directory: {}", e)))?;
        let target_export_path = target_temp.path();

        let target_options = CliOptions::from_profile(&target_profile);
        let target_export_result = cli
            .export(
                &target_options,
                Some("metadata"),
                Some(target_export_path.to_str().unwrap()),
                None,
                None,
                None,
                None,
                false,
            )
            .await?;

        if target_export_result.success() {
            // Show simple diff summary
            let source_files = list_json_files(&export_path);
            let target_files = list_json_files(&target_export_path.to_path_buf());

            let new_files: Vec<_> = source_files
                .iter()
                .filter(|f| !target_files.contains(f))
                .collect();
            let removed_files: Vec<_> = target_files
                .iter()
                .filter(|f| !source_files.contains(f))
                .collect();

            if verbose {
                if !new_files.is_empty() {
                    println!("  {} New resources:", style("+").green());
                    for f in &new_files {
                        println!("    {}", style(f).green());
                    }
                }
                if !removed_files.is_empty() {
                    println!("  {} Removed resources:", style("-").red());
                    for f in &removed_files {
                        println!("    {}", style(f).red());
                    }
                }
            }

            println!(
                "{} {} new, {} removed, {} potentially modified",
                style("✓").green(),
                new_files.len(),
                removed_files.len(),
                source_files.len().saturating_sub(new_files.len())
            );
        } else {
            println!(
                "{} Could not compare (target export failed)",
                style("⚠").yellow()
            );
        }
    }

    // Step 3: Confirmation
    if !args.yes {
        println!();
        let prompt = format!(
            "Promote {} resources from '{}' to '{}'?",
            file_count, args.from, args.to
        );
        let confirmed = Confirm::new()
            .with_prompt(prompt)
            .default(false)
            .interact()
            .map_err(|e| VqxError::Other(format!("Confirmation failed: {}", e)))?;

        if !confirmed {
            println!("{} Operation cancelled.", style("✗").yellow());
            return Ok(PromoteResult {
                success: false,
                source_profile: args.from.clone(),
                target_profile: args.to.clone(),
                exported: true,
                imported: false,
                test_result: None,
                error: Some("Operation cancelled by user".to_string()),
            });
        }
    }

    // Step 4: Import to target
    println!();
    println!("{} Importing to target...", style("→").cyan());
    let target_options = CliOptions::from_profile(&target_profile);
    let import_result = cli
        .import(
            &target_options,
            Some("metadata"),
            Some(export_path.to_str().unwrap()),
            None,
            None,
            None,
            None,
        )
        .await?;

    if !import_result.success() {
        return Ok(PromoteResult {
            success: false,
            source_profile: args.from.clone(),
            target_profile: args.to.clone(),
            exported: true,
            imported: false,
            test_result: None,
            error: Some(format!("Import failed: {}", import_result.stderr)),
        });
    }

    println!("{} Import completed successfully", style("✓").green());

    // Step 5: Run tests (if specified)
    let test_result = if !args.no_test {
        if let Some(ref testsuite) = args.testsuite {
            println!();
            println!(
                "{} Running test suite '{}'...",
                style("→").cyan(),
                testsuite
            );
            let result = cli.run_testsuite(&target_options, testsuite, None).await?;

            let success = result.success();
            if success {
                println!("{} Test suite passed", style("✓").green());
            } else {
                println!("{} Test suite failed", style("✗").red());
                if !result.stderr.is_empty() {
                    eprintln!("{}", style(&result.stderr).red());
                }
            }

            Some(TestResult {
                success,
                test_type: "testsuite".to_string(),
                name: testsuite.clone(),
                output: Some(result.stdout),
            })
        } else if let Some(ref procedure) = args.procedure {
            println!();
            println!("{} Running procedure '{}'...", style("→").cyan(), procedure);
            let result = cli.run_procedure(&target_options, procedure, &[]).await?;

            let success = result.success();
            if success {
                println!("{} Procedure completed", style("✓").green());
            } else {
                println!("{} Procedure failed", style("✗").red());
            }

            Some(TestResult {
                success,
                test_type: "procedure".to_string(),
                name: procedure.clone(),
                output: Some(result.stdout),
            })
        } else {
            None
        }
    } else {
        None
    };

    // Determine overall success
    let test_passed = test_result.as_ref().map(|t| t.success).unwrap_or(true);

    let result = PromoteResult {
        success: test_passed,
        source_profile: args.from.clone(),
        target_profile: args.to.clone(),
        exported: true,
        imported: true,
        test_result,
        error: None,
    };

    display_result(&result, output_format);
    Ok(result)
}

/// Count JSON files in a directory
fn count_json_files(dir: &PathBuf) -> usize {
    if !dir.exists() {
        return 0;
    }

    WalkDir::new(dir)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| {
            e.path()
                .extension()
                .map(|ext| ext == "json")
                .unwrap_or(false)
        })
        .count()
}

/// List JSON file names in a directory (relative paths)
fn list_json_files(dir: &PathBuf) -> Vec<String> {
    if !dir.exists() {
        return vec![];
    }

    WalkDir::new(dir)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| {
            e.path()
                .extension()
                .map(|ext| ext == "json")
                .unwrap_or(false)
        })
        .filter_map(|e| {
            e.path()
                .strip_prefix(dir)
                .ok()
                .map(|p| p.to_string_lossy().to_string())
        })
        .collect()
}

/// Display the result
fn display_result(result: &PromoteResult, output_format: OutputFormat) {
    match output_format {
        OutputFormat::Json => {
            if let Ok(json) = serde_json::to_string_pretty(result) {
                println!("{}", json);
            }
        }
        OutputFormat::Text | OutputFormat::Csv => {
            println!();
            println!("{}", style("─".repeat(50)).dim());

            if result.success {
                println!(
                    "{} Promotion completed: {} → {}",
                    style("✓").green().bold(),
                    style(&result.source_profile).cyan(),
                    style(&result.target_profile).yellow()
                );
            } else {
                println!(
                    "{} Promotion failed: {} → {}",
                    style("✗").red().bold(),
                    &result.source_profile,
                    &result.target_profile
                );
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
    fn test_list_json_files_nonexistent() {
        let files = list_json_files(&PathBuf::from("/nonexistent/path"));
        assert!(files.is_empty());
    }
}
