//! Import command implementation
//!
//! Wraps the underlying CLI's import command with safety confirmations.
//!
//! Based on: CLI Reference Guide PDF - "Import" section
//!
//! PDF: "The import command reads all artifact definitions stored in a
//! directory and loads them into the current namespace."
//!
//! Import types (PDF):
//! - metadata: import the resource definitions (e.g. types, sources, rules, etc.)
//! - data: import the data contained in user defined types and the documents resource
//!
//! Options (PDF):
//! - -d <directoryName>: input directory
//! - -chunk <integer>: chunk size for large imports
//! - -include <typeName>: types to include
//! - -exclude <typeName>: types to exclude
//! - -ignore <resourceType>: resource types to ignore

use crate::cli::{ImportArgs, ImportType, OutputFormat};
use crate::config::Config;
use crate::error::{Result, VqxError};
use crate::profile::ProfileManager;
use crate::underlying::{CliOptions, UnderlyingCli};
use console::style;
use dialoguer::Confirm;
use indicatif::{ProgressBar, ProgressStyle};
use std::path::PathBuf;
use std::time::Duration;
use tracing::{debug, info, warn};

/// Import operation result
#[derive(Debug)]
pub struct ImportResult {
    pub success: bool,
    pub directory: PathBuf,
    pub resources_imported: Option<usize>,
    pub errors: Vec<String>,
}

/// Run import command
pub async fn run(
    args: &ImportArgs,
    config: &Config,
    profile_name: Option<&str>,
    output_format: OutputFormat,
    verbose: bool,
) -> Result<ImportResult> {
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

    // Determine input directory
    let input_dir = args.directory.clone().unwrap_or_else(|| PathBuf::from("."));

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

    // Count files to import
    let file_count = count_import_files(&input_dir);

    // Display import info and warning
    if !matches!(output_format, OutputFormat::Json) {
        println!();
        println!("{}", style("Import").bold().cyan());
        println!("{}", style("─".repeat(50)).dim());
        println!("  Profile:   {}", style(profile_name).green());
        println!("  Server:    {}", profile.url);
        println!("  Type:      {}", format_import_type(&args.import_type));
        println!("  Directory: {}", input_dir.display());
        println!("  Files:     ~{}", file_count);
        if let Some(chunk) = args.chunk {
            println!("  Chunk:     {}", chunk);
        }
        println!();

        // Warning about destructive nature
        println!(
            "{}",
            style("⚠  Warning: Import may overwrite existing resources!").yellow()
        );
        println!(
            "{}",
            style("   PDF: 'The import command reads all artifact definitions stored in a").dim()
        );
        println!(
            "{}",
            style("   directory and loads them into the current namespace.'").dim()
        );
        println!();
    }

    // Confirmation required unless --yes is specified
    if !args.yes && !matches!(output_format, OutputFormat::Json) {
        let confirmed = Confirm::new()
            .with_prompt(format!(
                "Import ~{} files to {} ({})?",
                file_count, profile.url, profile_name
            ))
            .default(false)
            .interact()
            .map_err(|e| VqxError::Other(e.to_string()))?;

        if !confirmed {
            println!("Import cancelled.");
            return Ok(ImportResult {
                success: false,
                directory: input_dir,
                resources_imported: None,
                errors: vec!["Cancelled by user".to_string()],
            });
        }
    }

    // Build CLI
    let cli = UnderlyingCli::new(config.cli_path.clone())
        .with_timeout(config.timeout())
        .with_retries(config.max_retries, config.retry_delay_ms);

    let options = CliOptions::from_profile(&profile);

    // Progress bar
    let progress = if !matches!(output_format, OutputFormat::Json) {
        let pb = ProgressBar::new_spinner();
        pb.set_style(
            ProgressStyle::default_spinner()
                .template("{spinner:.green} {msg}")
                .unwrap(),
        );
        pb.set_message("Importing to Vantiq...");
        pb.enable_steady_tick(Duration::from_millis(100));
        Some(pb)
    } else {
        None
    };

    // Build import type string
    // PDF: "import [data | metadata]"
    let import_type_str = match args.import_type {
        ImportType::Metadata => "metadata",
        ImportType::Data => "data",
    };

    // Execute import
    // PDF: "vantiq import [type] [-d <directory>] [-chunk <size>] [-include <type>] [-exclude <type>] [-ignore <resourceType>]"
    let include_refs: Vec<&str> = args.include.iter().map(|s| s.as_str()).collect();
    let exclude_refs: Vec<&str> = args.exclude.iter().map(|s| s.as_str()).collect();
    let ignore_refs: Vec<&str> = args.ignore.iter().map(|s| s.as_str()).collect();

    let result = cli
        .import(
            &options,
            Some(import_type_str),
            Some(input_dir.to_str().unwrap()),
            args.chunk.or(Some(config.default_chunk_size)),
            if include_refs.is_empty() {
                None
            } else {
                Some(&include_refs)
            },
            if exclude_refs.is_empty() {
                None
            } else {
                Some(&exclude_refs)
            },
            if ignore_refs.is_empty() {
                None
            } else {
                Some(&ignore_refs)
            },
        )
        .await?;

    if let Some(ref pb) = progress {
        pb.finish_and_clear();
    }

    if !result.success() {
        if !matches!(output_format, OutputFormat::Json) {
            println!(
                "{} Import failed with exit code {}",
                style("✗").red(),
                result.code()
            );
            if !result.stderr.is_empty() {
                println!("{}", style(&result.stderr).red());
            }
        }

        return Ok(ImportResult {
            success: false,
            directory: input_dir,
            resources_imported: None,
            errors: vec![result.stderr],
        });
    }

    // Output summary
    if !matches!(output_format, OutputFormat::Json) {
        println!();
        println!("{}", style("─".repeat(50)).dim());
        println!("{} Import complete", style("✓").green().bold());

        if !result.stdout.is_empty() && verbose {
            println!();
            println!("{}", style("CLI Output:").dim());
            for line in result.stdout.lines().take(20) {
                println!("  {}", line);
            }
            if result.stdout.lines().count() > 20 {
                println!("  ... (truncated)");
            }
        }

        // Show PDF reference
        if verbose {
            println!();
            println!(
                "{}",
                style("PDF Reference: Import loads from directories:").dim()
            );
            println!(
                "{}",
                style("  types/, procedures/, rules/, sources/, services/,").dim()
            );
            println!("{}", style("  data/ (for user defined type data)").dim());
        }
        println!();
    }

    // JSON output
    if matches!(output_format, OutputFormat::Json) {
        let json_result = serde_json::json!({
            "success": true,
            "directory": input_dir.display().to_string(),
            "files_in_directory": file_count,
            "profile": profile_name,
            "server": profile.url,
            "import_type": format_import_type(&args.import_type),
        });
        println!("{}", serde_json::to_string_pretty(&json_result)?);
    }

    Ok(ImportResult {
        success: true,
        directory: input_dir,
        resources_imported: Some(file_count),
        errors: vec![],
    })
}

/// Format import type for display
fn format_import_type(import_type: &ImportType) -> String {
    match import_type {
        ImportType::Metadata => "metadata".to_string(),
        ImportType::Data => "data".to_string(),
    }
}

/// Count importable files in directory
fn count_import_files(dir: &PathBuf) -> usize {
    let mut count = 0;

    // Known import directories from PDF
    let import_dirs = [
        "types",
        "procedures",
        "rules",
        "sources",
        "services",
        "topics",
        "collaborationtypes",
        "aicomponents",
        "catalogs",
        "clients",
        "configurations",
        "debugconfigs",
        "deployconfigs",
        "environments",
        "projects",
        "scheduledevents",
        "subscriptions",
        "systemmodels",
        "data",
        "documents",
    ];

    for subdir in &import_dirs {
        let path = dir.join(subdir);
        if path.is_dir() {
            count += count_files_recursive(&path);
        }
    }

    // If no subdirs found, count files in root
    if count == 0 {
        count = count_files_recursive(dir);
    }

    count
}

/// Count files recursively
fn count_files_recursive(dir: &PathBuf) -> usize {
    let mut count = 0;

    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                count += count_files_recursive(&path);
            } else if path.is_file() {
                // Count json, vail files
                let ext = path.extension().and_then(|e| e.to_str());
                if matches!(ext, Some("json") | Some("vail")) {
                    count += 1;
                }
            }
        }
    }

    count
}

/// Show help specific to import command
pub fn display_help() {
    println!();
    println!("{}", style("Import Command").bold().cyan());
    println!("{}", style("─".repeat(60)).dim());
    println!();
    println!("Import resources to Vantiq from a local directory.");
    println!();
    println!(
        "{}",
        style("⚠  Warning: This is a potentially destructive operation!").yellow()
    );
    println!("   Existing resources may be overwritten.");
    println!();
    println!("{}", style("PDF Reference: Import section").bold());
    println!();
    println!("{}", style("Import Types:").bold());
    println!("  metadata     Import resource definitions (types, sources, rules, etc.)");
    println!("  data         Import data into user defined types and documents");
    println!();
    println!("{}", style("Options (from PDF):").bold());
    println!("  -d <dir>           Input directory (PDF: '-d <directoryName>')");
    println!("  --chunk <n>        Chunk size for large imports (PDF: '-chunk <integer>')");
    println!("  --include <type>   Include specific types (PDF: '-include <typeName>')");
    println!("  --exclude <type>   Exclude specific types (PDF: '-exclude <typeName>')");
    println!("  --ignore <res>     Ignore resource types (PDF: '-ignore <resourceType>')");
    println!();
    println!("{}", style("Safety Options:").bold());
    println!("  --yes, -y          Skip confirmation prompt");
    println!();
    println!("{}", style("Examples:").bold());
    println!();
    println!("  # Import metadata (with confirmation)");
    println!("  {} vqx import metadata -d ./export", style("$").dim());
    println!();
    println!("  # Import data with chunking (PDF: '-chunk' option)");
    println!(
        "  {} vqx import data -d ./data --chunk 5000",
        style("$").dim()
    );
    println!();
    println!("  # Import excluding specific types (PDF: '-exclude' option)");
    println!(
        "  {} vqx import metadata --exclude types --exclude rules",
        style("$").dim()
    );
    println!();
    println!("  # Import ignoring specific resource types (PDF: '-ignore' option)");
    println!(
        "  {} vqx import metadata --ignore sources",
        style("$").dim()
    );
    println!();
    println!("  # Import without confirmation (for scripts)");
    println!(
        "  {} vqx import metadata -d ./export --yes",
        style("$").dim()
    );
    println!();
    println!("{}", style("PDF Note:").dim());
    println!(
        "{}",
        style("  'The target directory must be structured as documented for the export command.'")
            .dim()
    );
    println!();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_import_type() {
        assert_eq!(format_import_type(&ImportType::Metadata), "metadata");
        assert_eq!(format_import_type(&ImportType::Data), "data");
    }
}
