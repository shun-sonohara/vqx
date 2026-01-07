//! Sync command implementation
//!
//! Provides bidirectional synchronization between local directories and Vantiq servers.
//!
//! Subcommands:
//! - `sync pull`: Export from remote to local directory
//! - `sync push`: Import from local to remote with diff preview and confirmation
//!
//! The sync command builds on export/import but adds:
//! - Automatic diff preview before push
//! - Confirmation prompts
//! - Backup creation
//! - JSON normalization

use crate::cli::{OutputFormat, SyncCommands, SyncPullArgs, SyncPushArgs};
use crate::commands::diff::{self, DiffResult};
use crate::config::Config;
use crate::error::{Result, VqxError};
use crate::normalizer::ResourceNormalizer;
use crate::profile::ProfileManager;
use crate::underlying::{CliOptions, UnderlyingCli};
use console::style;
use dialoguer::Confirm;
use indicatif::{ProgressBar, ProgressStyle};
use serde::Serialize;
use std::path::PathBuf;
use std::time::Duration;
use tempfile::TempDir;
use tracing::warn;

/// Result of sync operation
#[derive(Debug, Serialize)]
pub struct SyncResult {
    pub success: bool,
    pub operation: String,
    pub directory: PathBuf,
    pub files_processed: Option<usize>,
    pub changes: Option<SyncChanges>,
    pub backup_path: Option<PathBuf>,
    pub errors: Vec<String>,
}

/// Summary of changes for sync operation
#[derive(Debug, Clone, Serialize)]
pub struct SyncChanges {
    pub added: usize,
    pub removed: usize,
    pub modified: usize,
}

impl From<&DiffResult> for SyncChanges {
    fn from(diff: &DiffResult) -> Self {
        Self {
            added: diff.added.len(),
            removed: diff.removed.len(),
            modified: diff.modified.len(),
        }
    }
}

/// Run sync command
pub async fn run(
    cmd: &SyncCommands,
    config: &Config,
    profile_name: Option<&str>,
    output_format: OutputFormat,
    verbose: bool,
) -> Result<SyncResult> {
    match cmd {
        SyncCommands::Pull(args) => {
            run_pull(args, config, profile_name, output_format, verbose).await
        }
        SyncCommands::Push(args) => {
            run_push(args, config, profile_name, output_format, verbose).await
        }
    }
}

/// Run sync pull (export from remote to local)
async fn run_pull(
    args: &SyncPullArgs,
    config: &Config,
    profile_name: Option<&str>,
    output_format: OutputFormat,
    _verbose: bool,
) -> Result<SyncResult> {
    // Load profile
    let manager = ProfileManager::new()?;
    let profile_name = profile_name.unwrap_or(&manager.store().default_profile);
    let profile = manager.get_resolved(profile_name)?;

    if !profile.has_auth() {
        return Err(VqxError::ProfileInvalid {
            message: format!(
                "Profile '{}' has no authentication configured",
                profile_name
            ),
        });
    }

    let output_dir = &args.directory;

    // Display sync pull info
    if !matches!(output_format, OutputFormat::Json) {
        println!();
        println!("{}", style("Sync Pull").bold().cyan());
        println!("{}", style("─".repeat(50)).dim());
        println!("  Profile:   {}", style(profile_name).green());
        println!("  Server:    {}", profile.url);
        println!("  Directory: {}", output_dir.display());
        println!();
    }

    // Check if directory exists and has content
    let dir_exists = output_dir.exists() && output_dir.is_dir();
    let has_content = dir_exists
        && std::fs::read_dir(output_dir)
            .map(|d| d.count() > 0)
            .unwrap_or(false);

    // Warn about overwriting if directory has content
    if has_content && !args.force {
        if !matches!(output_format, OutputFormat::Json) {
            println!(
                "{}",
                style("⚠  Directory already contains files. They may be overwritten.").yellow()
            );
            println!();
        }

        let confirmed = Confirm::new()
            .with_prompt("Continue with sync pull?")
            .default(false)
            .interact()
            .map_err(|e| VqxError::Other(e.to_string()))?;

        if !confirmed {
            return Ok(SyncResult {
                success: false,
                operation: "pull".to_string(),
                directory: output_dir.clone(),
                files_processed: None,
                changes: None,
                backup_path: None,
                errors: vec!["Cancelled by user".to_string()],
            });
        }
    }

    // Create output directory if it doesn't exist
    if !dir_exists {
        std::fs::create_dir_all(output_dir).map_err(|_e| VqxError::FileWriteFailed {
            path: output_dir.display().to_string(),
        })?;
    }

    // Progress bar
    let progress = if !matches!(output_format, OutputFormat::Json) {
        let pb = ProgressBar::new_spinner();
        pb.set_style(
            ProgressStyle::default_spinner()
                .template("{spinner:.green} {msg}")
                .unwrap(),
        );
        pb.set_message("Pulling from Vantiq...");
        pb.enable_steady_tick(Duration::from_millis(100));
        Some(pb)
    } else {
        None
    };

    // Build CLI and export
    let cli = UnderlyingCli::new(config.cli_path.clone())
        .with_timeout(config.timeout())
        .with_retries(config.max_retries, config.retry_delay_ms);

    let options = CliOptions::from_profile(&profile);

    let result = cli
        .export(
            &options,
            Some("metadata"),
            Some(output_dir.to_str().unwrap()),
            Some(config.default_chunk_size),
            None,
            None,
            None,
            false,
        )
        .await?;

    if !result.success() {
        if let Some(ref pb) = progress {
            pb.finish_and_clear();
        }

        if !matches!(output_format, OutputFormat::Json) {
            println!(
                "{} Sync pull failed with exit code {}",
                style("✗").red(),
                result.code()
            );
        }

        return Ok(SyncResult {
            success: false,
            operation: "pull".to_string(),
            directory: output_dir.clone(),
            files_processed: None,
            changes: None,
            backup_path: None,
            errors: vec![result.stderr],
        });
    }

    // Normalize exported files
    if let Some(ref pb) = progress {
        pb.set_message("Normalizing JSON files...");
    }

    let normalizer = ResourceNormalizer::new(config.normalization.clone());
    let stats = normalizer.normalize_export_directory(output_dir)?;

    if let Some(ref pb) = progress {
        pb.finish_and_clear();
    }

    // Output success
    if !matches!(output_format, OutputFormat::Json) {
        println!();
        println!("{}", style("─".repeat(50)).dim());
        println!("{} Sync pull complete", style("✓").green().bold());
        println!("  Files: {}", stats.files_processed);
        println!("  Directory: {}", output_dir.display());
        println!();
    }

    // JSON output
    if matches!(output_format, OutputFormat::Json) {
        let json_result = SyncResult {
            success: true,
            operation: "pull".to_string(),
            directory: output_dir.clone(),
            files_processed: Some(stats.files_processed),
            changes: None,
            backup_path: None,
            errors: vec![],
        };
        println!("{}", serde_json::to_string_pretty(&json_result)?);
    }

    Ok(SyncResult {
        success: true,
        operation: "pull".to_string(),
        directory: output_dir.clone(),
        files_processed: Some(stats.files_processed),
        changes: None,
        backup_path: None,
        errors: vec![],
    })
}

/// Run sync push (import from local to remote with diff + confirm)
async fn run_push(
    args: &SyncPushArgs,
    config: &Config,
    profile_name: Option<&str>,
    output_format: OutputFormat,
    _verbose: bool,
) -> Result<SyncResult> {
    // Load profile
    let manager = ProfileManager::new()?;
    let profile_name = profile_name.unwrap_or(&manager.store().default_profile);
    let profile = manager.get_resolved(profile_name)?;

    if !profile.has_auth() {
        return Err(VqxError::ProfileInvalid {
            message: format!(
                "Profile '{}' has no authentication configured",
                profile_name
            ),
        });
    }

    let input_dir = &args.directory;

    // Verify directory exists
    if !input_dir.exists() {
        return Err(VqxError::FileReadFailed {
            path: input_dir.display().to_string(),
        });
    }

    if !input_dir.is_dir() {
        return Err(VqxError::Other(format!(
            "Not a directory: {}",
            input_dir.display()
        )));
    }

    // Display sync push info
    if !matches!(output_format, OutputFormat::Json) {
        println!();
        println!("{}", style("Sync Push").bold().cyan());
        println!("{}", style("─".repeat(50)).dim());
        println!("  Profile:   {}", style(profile_name).green());
        println!("  Server:    {}", profile.url);
        println!("  Directory: {}", input_dir.display());
        println!();
    }

    // Progress bar
    let progress = if !matches!(output_format, OutputFormat::Json) {
        let pb = ProgressBar::new_spinner();
        pb.set_style(
            ProgressStyle::default_spinner()
                .template("{spinner:.green} {msg}")
                .unwrap(),
        );
        pb.enable_steady_tick(Duration::from_millis(100));
        Some(pb)
    } else {
        None
    };

    // First, export current state from server to temp dir for diff
    if let Some(ref pb) = progress {
        pb.set_message("Fetching current server state for comparison...");
    }

    let temp_dir = TempDir::new().map_err(|e| VqxError::Other(e.to_string()))?;
    let temp_path = temp_dir.path().to_path_buf();

    let cli = UnderlyingCli::new(config.cli_path.clone())
        .with_timeout(config.timeout())
        .with_retries(config.max_retries, config.retry_delay_ms);

    let options = CliOptions::from_profile(&profile);

    // Export current server state
    let export_result = cli
        .export(
            &options,
            Some("metadata"),
            Some(temp_path.to_str().unwrap()),
            Some(config.default_chunk_size),
            None,
            None,
            None,
            false,
        )
        .await?;

    if !export_result.success() {
        if let Some(ref pb) = progress {
            pb.finish_and_clear();
        }

        // If export fails (e.g., empty namespace), continue without diff
        warn!("Could not export current server state for diff comparison");
    } else {
        // Normalize exported files
        let normalizer = ResourceNormalizer::new(config.normalization.clone());
        let _ = normalizer.normalize_export_directory(&temp_path);
    }

    // Perform diff
    if let Some(ref pb) = progress {
        pb.set_message("Comparing changes...");
    }

    let diff_result = diff::run(
        &crate::cli::DiffArgs {
            source: temp_path.to_str().unwrap().to_string(),
            target: input_dir.to_str().unwrap().to_string(),
            resource: vec![],
            full: false,
        },
        config,
        OutputFormat::Text, // Don't output diff as JSON here
        false,
    )
    .await;

    if let Some(ref pb) = progress {
        pb.finish_and_clear();
    }

    // Show diff summary
    let changes = if let Ok(ref diff) = diff_result {
        if !matches!(output_format, OutputFormat::Json) && diff.has_changes() {
            println!();
            println!("{}", style("Changes to push:").bold());
            println!(
                "  {} added, {} removed, {} modified",
                style(format!("+{}", diff.added.len())).green(),
                style(format!("-{}", diff.removed.len())).red(),
                style(format!("~{}", diff.modified.len())).yellow()
            );
            println!();
        }
        Some(SyncChanges::from(diff))
    } else {
        None
    };

    // Dry run mode
    if args.dry_run {
        if !matches!(output_format, OutputFormat::Json) {
            println!("{}", style("Dry run - no changes made").dim());
            println!();
        }

        return Ok(SyncResult {
            success: true,
            operation: "push (dry-run)".to_string(),
            directory: input_dir.clone(),
            files_processed: None,
            changes,
            backup_path: None,
            errors: vec![],
        });
    }

    // Confirmation
    if !args.yes && !matches!(output_format, OutputFormat::Json) {
        println!(
            "{}",
            style("⚠  Warning: This will modify resources on the server!").yellow()
        );
        println!();

        let confirmed = Confirm::new()
            .with_prompt(format!(
                "Push changes to {} ({})?",
                profile.url, profile_name
            ))
            .default(false)
            .interact()
            .map_err(|e| VqxError::Other(e.to_string()))?;

        if !confirmed {
            return Ok(SyncResult {
                success: false,
                operation: "push".to_string(),
                directory: input_dir.clone(),
                files_processed: None,
                changes,
                backup_path: None,
                errors: vec!["Cancelled by user".to_string()],
            });
        }
    }

    // Progress for import
    let progress = if !matches!(output_format, OutputFormat::Json) {
        let pb = ProgressBar::new_spinner();
        pb.set_style(
            ProgressStyle::default_spinner()
                .template("{spinner:.green} {msg}")
                .unwrap(),
        );
        pb.set_message("Pushing to Vantiq...");
        pb.enable_steady_tick(Duration::from_millis(100));
        Some(pb)
    } else {
        None
    };

    // Execute import
    let import_result = cli
        .import(
            &options,
            Some("metadata"),
            Some(input_dir.to_str().unwrap()),
            Some(config.default_chunk_size),
            None,
            None,
            None,
        )
        .await?;

    if let Some(ref pb) = progress {
        pb.finish_and_clear();
    }

    if !import_result.success() {
        if !matches!(output_format, OutputFormat::Json) {
            println!(
                "{} Sync push failed with exit code {}",
                style("✗").red(),
                import_result.code()
            );
            if !import_result.stderr.is_empty() {
                println!("{}", style(&import_result.stderr).red());
            }
        }

        return Ok(SyncResult {
            success: false,
            operation: "push".to_string(),
            directory: input_dir.clone(),
            files_processed: None,
            changes,
            backup_path: None,
            errors: vec![import_result.stderr],
        });
    }

    // Count files
    let files_count = count_files(input_dir);

    // Output success
    if !matches!(output_format, OutputFormat::Json) {
        println!();
        println!("{}", style("─".repeat(50)).dim());
        println!("{} Sync push complete", style("✓").green().bold());
        println!("  Files: {}", files_count);
        println!("  Server: {}", profile.url);
        println!();
    }

    // JSON output
    if matches!(output_format, OutputFormat::Json) {
        let json_result = SyncResult {
            success: true,
            operation: "push".to_string(),
            directory: input_dir.clone(),
            files_processed: Some(files_count),
            changes: changes.clone(),
            backup_path: None,
            errors: vec![],
        };
        println!("{}", serde_json::to_string_pretty(&json_result)?);
    }

    Ok(SyncResult {
        success: true,
        operation: "push".to_string(),
        directory: input_dir.clone(),
        files_processed: Some(files_count),
        changes,
        backup_path: None,
        errors: vec![],
    })
}

/// Count files in directory recursively
fn count_files(dir: &PathBuf) -> usize {
    let mut count = 0;

    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                count += count_files(&path);
            } else if path.is_file() {
                let ext = path.extension().and_then(|e| e.to_str());
                if matches!(ext, Some("json") | Some("vail")) {
                    count += 1;
                }
            }
        }
    }

    count
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sync_changes_from_diff() {
        let diff_result = DiffResult {
            success: true,
            source: "source".to_string(),
            target: "target".to_string(),
            added: vec![],
            removed: vec![],
            modified: vec![],
            errors: vec![],
        };

        let changes = SyncChanges::from(&diff_result);
        assert_eq!(changes.added, 0);
        assert_eq!(changes.removed, 0);
        assert_eq!(changes.modified, 0);
    }
}
