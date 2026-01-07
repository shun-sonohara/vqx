//! Export command implementation
//!
//! Wraps the underlying CLI's export command with JSON normalization
//! for git-friendly output.
//!
//! Based on: CLI Reference Guide PDF - "Export" section
//!
//! PDF: "The export command writes either the resource meta-data or data
//! stored in user defined types into files stored in a directory on the
//! local machine."
//!
//! Export types (PDF):
//! - metadata: export the resource definitions (e.g. types, sources, rules, etc.)
//! - data: export the data contained in user defined types and the documents resource
//! - project <projectName>: export the resource definitions within a project
//! - projectdata <projectName>: export the data within a project
//! - hidden: (undocumented in PDF excerpt)
//!
//! Options (PDF):
//! - -d <directoryName>: output directory
//! - -chunk <integer>: chunk size for large exports
//! - -include <typeName(s)>: types to include
//! - -exclude <typeName(s)>: types to exclude
//! - -until <DateTime>: limit to instances before timestamp
//! - -ignoreErrors: continue on errors

use crate::cli::{ExportArgs, ExportType, OutputFormat};
use crate::config::Config;
use crate::error::{Result, VqxError};
use crate::normalizer::ResourceNormalizer;
use crate::profile::ProfileManager;
use crate::underlying::{CliOptions, UnderlyingCli};
use console::style;
use indicatif::{ProgressBar, ProgressStyle};
use std::path::PathBuf;
use std::time::Duration;
use tracing::{debug, info, warn};

/// Export operation result
#[derive(Debug)]
pub struct ExportResult {
    pub success: bool,
    pub directory: PathBuf,
    pub files_exported: Option<usize>,
    pub files_normalized: Option<usize>,
    pub errors: Vec<String>,
}

/// Run export command
pub async fn run(
    args: &ExportArgs,
    config: &Config,
    profile_name: Option<&str>,
    output_format: OutputFormat,
    verbose: bool,
) -> Result<ExportResult> {
    // Load profile
    let manager = ProfileManager::new()?;
    let profile_name = profile_name.unwrap_or(&manager.store().default_profile);
    let profile = manager.get_resolved(profile_name)?;

    if !profile.has_auth() {
        return Err(VqxError::ProfileInvalid {
            message: format!("Profile '{}' has no authentication configured", profile_name),
        });
    }

    // Determine output directory
    let output_dir = args
        .directory
        .clone()
        .unwrap_or_else(|| PathBuf::from("."));

    // Create output directory if it doesn't exist
    if !output_dir.exists() {
        std::fs::create_dir_all(&output_dir).map_err(|e| VqxError::FileWriteFailed {
            path: output_dir.display().to_string(),
        })?;
    }

    // Display export info
    if !matches!(output_format, OutputFormat::Json) {
        println!();
        println!("{}", style("Export").bold().cyan());
        println!("{}", style("─".repeat(50)).dim());
        println!("  Profile:   {}", style(profile_name).green());
        println!("  Server:    {}", profile.url);
        println!("  Type:      {}", format_export_type(&args.export_type, &args.project));
        println!("  Directory: {}", output_dir.display());
        if let Some(chunk) = args.chunk {
            println!("  Chunk:     {}", chunk);
        }
        if args.normalize {
            println!("  Normalize: {}", style("enabled").green());
        }
        println!();
    }

    // Build CLI
    let cli = UnderlyingCli::new(config.cli_path.clone())
        .with_timeout(config.timeout())
        .with_retries(config.max_retries, config.retry_delay_ms);

    let options = CliOptions::from_profile(&profile);

    // Progress bar for export
    let progress = if !matches!(output_format, OutputFormat::Json) {
        let pb = ProgressBar::new_spinner();
        pb.set_style(
            ProgressStyle::default_spinner()
                .template("{spinner:.green} {msg}")
                .unwrap(),
        );
        pb.set_message("Exporting from Vantiq...");
        pb.enable_steady_tick(Duration::from_millis(100));
        Some(pb)
    } else {
        None
    };

    // Build export type string for CLI
    // PDF: "export [data | metadata | project <projectName> | projectdata <projectName> | hidden]"
    let export_type_str = match args.export_type {
        ExportType::Metadata => "metadata".to_string(),
        ExportType::Data => "data".to_string(),
        ExportType::Project => {
            if let Some(ref name) = args.project {
                format!("project {}", name)
            } else {
                return Err(VqxError::Other(
                    "Project name required for project export".to_string(),
                ));
            }
        }
        ExportType::ProjectData => {
            if let Some(ref name) = args.project {
                format!("projectdata {}", name)
            } else {
                return Err(VqxError::Other(
                    "Project name required for projectdata export".to_string(),
                ));
            }
        }
        ExportType::Hidden => "hidden".to_string(),
    };

    // Execute export
    // PDF: "vantiq export [type] [-d <directory>] [-chunk <size>] [-include <type>] [-exclude <type>] [-until <DateTime>] [-ignoreErrors]"
    let include_refs: Vec<&str> = args.include.iter().map(|s| s.as_str()).collect();
    let exclude_refs: Vec<&str> = args.exclude.iter().map(|s| s.as_str()).collect();

    let result = cli
        .export(
            &options,
            Some(&export_type_str),
            Some(output_dir.to_str().unwrap()),
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
            args.until.as_deref(),
            args.ignore_errors,
        )
        .await?;

    if let Some(ref pb) = progress {
        pb.finish_and_clear();
    }

    if !result.success() {
        if !matches!(output_format, OutputFormat::Json) {
            println!(
                "{} Export failed with exit code {}",
                style("✗").red(),
                result.code()
            );
            if !result.stderr.is_empty() {
                println!("{}", style(&result.stderr).red());
            }
        }

        return Ok(ExportResult {
            success: false,
            directory: output_dir,
            files_exported: None,
            files_normalized: None,
            errors: vec![result.stderr],
        });
    }

    // Count exported files
    let files_exported = count_json_files(&output_dir);

    if !matches!(output_format, OutputFormat::Json) {
        println!(
            "{} Exported {} files to {}",
            style("✓").green(),
            files_exported,
            output_dir.display()
        );
    }

    // Normalize if requested
    let files_normalized = if args.normalize {
        if let Some(ref pb) = progress {
            pb.set_message("Normalizing JSON files...");
            pb.enable_steady_tick(Duration::from_millis(100));
        } else if !matches!(output_format, OutputFormat::Json) {
            println!();
            println!("{}", style("Normalizing...").dim());
        }

        let normalizer = ResourceNormalizer::new(config.normalization.clone());
        let stats = normalizer.normalize_export_directory(&output_dir)?;

        if let Some(ref pb) = progress {
            pb.finish_and_clear();
        }

        if !matches!(output_format, OutputFormat::Json) {
            println!(
                "{} Normalized {} files",
                style("✓").green(),
                stats.files_processed
            );

            if stats.errors > 0 {
                println!(
                    "{} {} files had errors during normalization",
                    style("⚠").yellow(),
                    stats.errors
                );
                for (file, err) in &stats.error_files {
                    println!("    {} {}: {}", style("•").dim(), file, err);
                }
            }
        }

        Some(stats.files_processed)
    } else {
        None
    };

    // Output summary
    if !matches!(output_format, OutputFormat::Json) {
        println!();
        println!("{}", style("─".repeat(50)).dim());
        println!(
            "{} Export complete",
            style("✓").green().bold()
        );

        // Show PDF reference for directory structure
        if verbose {
            println!();
            println!("{}", style("PDF Reference: Export creates directories:").dim());
            println!(
                "{}",
                style("  types/, procedures/, rules/, sources/, services/,").dim()
            );
            println!(
                "{}",
                style("  topics/, configurations/, deployconfigs/, etc.").dim()
            );
        }
        println!();
    }

    // JSON output
    if matches!(output_format, OutputFormat::Json) {
        let json_result = serde_json::json!({
            "success": true,
            "directory": output_dir.display().to_string(),
            "files_exported": files_exported,
            "files_normalized": files_normalized,
            "profile": profile_name,
            "server": profile.url,
            "export_type": format_export_type(&args.export_type, &args.project),
        });
        println!("{}", serde_json::to_string_pretty(&json_result)?);
    }

    Ok(ExportResult {
        success: true,
        directory: output_dir,
        files_exported: Some(files_exported),
        files_normalized,
        errors: vec![],
    })
}

/// Format export type for display
fn format_export_type(export_type: &ExportType, project: &Option<String>) -> String {
    match export_type {
        ExportType::Metadata => "metadata".to_string(),
        ExportType::Data => "data".to_string(),
        ExportType::Project => format!("project {}", project.as_deref().unwrap_or("?")),
        ExportType::ProjectData => format!("projectdata {}", project.as_deref().unwrap_or("?")),
        ExportType::Hidden => "hidden".to_string(),
    }
}

/// Count JSON files in a directory recursively
fn count_json_files(dir: &PathBuf) -> usize {
    let mut count = 0;

    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                count += count_json_files(&path);
            } else if path.extension().map(|e| e == "json").unwrap_or(false) {
                count += 1;
            }
        }
    }

    count
}

/// Show help specific to export command
pub fn display_help() {
    println!();
    println!("{}", style("Export Command").bold().cyan());
    println!("{}", style("─".repeat(60)).dim());
    println!();
    println!("Export resources from Vantiq with optional JSON normalization.");
    println!();
    println!("{}", style("PDF Reference: Export section").bold());
    println!();
    println!("{}", style("Export Types:").bold());
    println!("  metadata     Export resource definitions (types, sources, rules, etc.)");
    println!("  data         Export data in user defined types and documents");
    println!("  project      Export resource definitions within a project");
    println!("  projectdata  Export data within a project");
    println!();
    println!("{}", style("Options (from PDF):").bold());
    println!("  -d <dir>           Output directory");
    println!("  --chunk <n>        Chunk size for large exports (PDF: '-chunk <integer>')");
    println!("  --include <type>   Include specific types (PDF: '-include <typeName(s)>')");
    println!("  --exclude <type>   Exclude specific types (PDF: '-exclude <typeName(s)>')");
    println!("  --until <time>     Export data before timestamp (PDF: '-until <DateTime>')");
    println!("  --ignore-errors    Continue on errors (PDF: '-ignoreErrors')");
    println!();
    println!("{}", style("vqx Extensions:").bold());
    println!("  --normalize        Normalize JSON for git-friendly diffs (default: true)");
    println!("  --no-normalize     Disable JSON normalization");
    println!();
    println!("{}", style("Examples:").bold());
    println!();
    println!("  # Export all metadata (PDF: 'vantiq export -d /my/directory')");
    println!("  {} vqx export metadata -d ./export", style("$").dim());
    println!();
    println!("  # Export data with chunking (PDF: '-chunk' option)");
    println!(
        "  {} vqx export data -d ./data --chunk 5000",
        style("$").dim()
    );
    println!();
    println!("  # Export excluding specific types (PDF: '-exclude' option)");
    println!(
        "  {} vqx export data --exclude TypeA --exclude TypeB",
        style("$").dim()
    );
    println!();
    println!("  # Export project resources");
    println!(
        "  {} vqx export project --project MyProject -d ./project",
        style("$").dim()
    );
    println!();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_export_type() {
        assert_eq!(format_export_type(&ExportType::Metadata, &None), "metadata");
        assert_eq!(format_export_type(&ExportType::Data, &None), "data");
        assert_eq!(
            format_export_type(&ExportType::Project, &Some("Test".to_string())),
            "project Test"
        );
    }
}
