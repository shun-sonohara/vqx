//! Diff command implementation
//!
//! Compares resources between two sources (profiles or directories).
//!
//! This command supports comparing:
//! - Two directories (local-to-local)
//! - A profile and a directory (remote-to-local)
//! - Two profiles (remote-to-remote)
//!
//! The diff output shows:
//! - Added resources (exist in target but not source)
//! - Removed resources (exist in source but not target)
//! - Modified resources (exist in both but differ)

use crate::cli::{DiffArgs, OutputFormat};
use crate::config::Config;
use crate::error::{Result, VqxError};
use crate::normalizer::ResourceNormalizer;
use crate::profile::ProfileManager;
use crate::underlying::{CliOptions, UnderlyingCli};
use console::style;
use indicatif::{ProgressBar, ProgressStyle};
use serde::{Deserialize, Serialize};
use similar::{ChangeTag, TextDiff};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::time::Duration;
use tempfile::TempDir;
use tracing::{debug, info};

/// Represents a difference between two resources
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceDiff {
    /// Resource type (e.g., "types", "procedures")
    pub resource_type: String,
    /// Resource name
    pub name: String,
    /// Kind of change
    pub change: ChangeKind,
    /// Unified diff output (for modified resources)
    pub diff_text: Option<String>,
}

/// Kind of change detected
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ChangeKind {
    Added,
    Removed,
    Modified,
}

impl std::fmt::Display for ChangeKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ChangeKind::Added => write!(f, "added"),
            ChangeKind::Removed => write!(f, "removed"),
            ChangeKind::Modified => write!(f, "modified"),
        }
    }
}

/// Result of diff operation
#[derive(Debug, Serialize)]
pub struct DiffResult {
    pub success: bool,
    pub source: String,
    pub target: String,
    pub added: Vec<ResourceDiff>,
    pub removed: Vec<ResourceDiff>,
    pub modified: Vec<ResourceDiff>,
    pub errors: Vec<String>,
}

impl DiffResult {
    pub fn total_changes(&self) -> usize {
        self.added.len() + self.removed.len() + self.modified.len()
    }

    pub fn has_changes(&self) -> bool {
        self.total_changes() > 0
    }
}

/// Source type for diff comparison
enum DiffSource {
    Directory(PathBuf),
    Profile(String),
}

impl DiffSource {
    fn parse(s: &str) -> Self {
        let path = PathBuf::from(s);
        if path.exists() && path.is_dir() {
            DiffSource::Directory(path)
        } else {
            DiffSource::Profile(s.to_string())
        }
    }

    fn description(&self) -> String {
        match self {
            DiffSource::Directory(p) => format!("directory: {}", p.display()),
            DiffSource::Profile(name) => format!("profile: {}", name),
        }
    }
}

/// Run diff command
pub async fn run(
    args: &DiffArgs,
    config: &Config,
    output_format: OutputFormat,
    verbose: bool,
) -> Result<DiffResult> {
    let source = DiffSource::parse(&args.source);
    let target = DiffSource::parse(&args.target);

    // Display diff info
    if !matches!(output_format, OutputFormat::Json) {
        println!();
        println!("{}", style("Diff").bold().cyan());
        println!("{}", style("─".repeat(50)).dim());
        println!("  Source: {}", source.description());
        println!("  Target: {}", target.description());
        if !args.resource.is_empty() {
            println!("  Filter: {}", args.resource.join(", "));
        }
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

    // Get source directory (export if profile)
    let (source_dir, _source_temp) =
        get_directory_for_source(&source, config, progress.as_ref()).await?;

    // Get target directory (export if profile)
    let (target_dir, _target_temp) =
        get_directory_for_source(&target, config, progress.as_ref()).await?;

    if let Some(ref pb) = progress {
        pb.set_message("Comparing resources...");
    }

    // Perform diff
    let result = compare_directories(
        &source_dir,
        &target_dir,
        &args.resource,
        args.full,
        &args.source,
        &args.target,
    )?;

    if let Some(ref pb) = progress {
        pb.finish_and_clear();
    }

    // Display results
    if !matches!(output_format, OutputFormat::Json) {
        display_diff_results(&result, args.full);
    } else {
        println!("{}", serde_json::to_string_pretty(&result)?);
    }

    Ok(result)
}

/// Get a directory for a diff source, exporting if necessary
async fn get_directory_for_source(
    source: &DiffSource,
    config: &Config,
    progress: Option<&ProgressBar>,
) -> Result<(PathBuf, Option<TempDir>)> {
    match source {
        DiffSource::Directory(path) => Ok((path.clone(), None)),
        DiffSource::Profile(name) => {
            if let Some(pb) = progress {
                pb.set_message(format!("Exporting from profile '{}'...", name));
            }

            // Create temp directory
            let temp_dir = TempDir::new().map_err(|e| VqxError::Other(e.to_string()))?;
            let export_path = temp_dir.path().to_path_buf();

            // Load profile
            let manager = ProfileManager::new()?;
            let profile = manager.get_resolved(name)?;

            if !profile.has_auth() {
                return Err(VqxError::ProfileInvalid {
                    message: format!("Profile '{}' has no authentication configured", name),
                });
            }

            // Export to temp directory
            let cli = UnderlyingCli::new(config.cli_path.clone())
                .with_timeout(config.timeout())
                .with_retries(config.max_retries, config.retry_delay_ms);

            let options = CliOptions::from_profile(&profile);

            let result = cli
                .export(
                    &options,
                    Some("metadata"),
                    Some(export_path.to_str().unwrap()),
                    Some(config.default_chunk_size),
                    None,
                    None,
                    None,
                    false,
                )
                .await?;

            if !result.success() {
                return Err(VqxError::CliExecutionFailed {
                    code: result.code(),
                    message: result.stderr,
                });
            }

            // Normalize exported files
            let normalizer = ResourceNormalizer::new(config.normalization.clone());
            normalizer.normalize_export_directory(&export_path)?;

            Ok((export_path, Some(temp_dir)))
        }
    }
}

/// Compare two directories
fn compare_directories(
    source_dir: &Path,
    target_dir: &Path,
    filter_types: &[String],
    full_diff: bool,
    source_name: &str,
    target_name: &str,
) -> Result<DiffResult> {
    let mut added = Vec::new();
    let mut removed = Vec::new();
    let mut modified = Vec::new();
    let mut errors = Vec::new();

    // Get resource types to compare
    let resource_types = get_resource_types(source_dir, target_dir, filter_types);

    for resource_type in resource_types {
        let source_type_dir = source_dir.join(&resource_type);
        let target_type_dir = target_dir.join(&resource_type);

        // Get files in each directory
        let source_files = get_json_files(&source_type_dir);
        let target_files = get_json_files(&target_type_dir);

        let source_names: HashSet<_> = source_files.keys().collect();
        let target_names: HashSet<_> = target_files.keys().collect();

        // Find added (in target but not source)
        for name in target_names.difference(&source_names) {
            added.push(ResourceDiff {
                resource_type: resource_type.clone(),
                name: (*name).clone(),
                change: ChangeKind::Added,
                diff_text: None,
            });
        }

        // Find removed (in source but not target)
        for name in source_names.difference(&target_names) {
            removed.push(ResourceDiff {
                resource_type: resource_type.clone(),
                name: (*name).clone(),
                change: ChangeKind::Removed,
                diff_text: None,
            });
        }

        // Find modified (in both but different)
        for name in source_names.intersection(&target_names) {
            let source_path = &source_files[*name];
            let target_path = &target_files[*name];

            match compare_files(source_path, target_path, full_diff) {
                Ok(Some(diff_text)) => {
                    modified.push(ResourceDiff {
                        resource_type: resource_type.clone(),
                        name: (*name).clone(),
                        change: ChangeKind::Modified,
                        diff_text: Some(diff_text),
                    });
                }
                Ok(None) => {
                    // Files are identical
                }
                Err(e) => {
                    errors.push(format!("{}/{}: {}", resource_type, name, e));
                }
            }
        }
    }

    // Sort results for consistent output
    added.sort_by(|a, b| (&a.resource_type, &a.name).cmp(&(&b.resource_type, &b.name)));
    removed.sort_by(|a, b| (&a.resource_type, &a.name).cmp(&(&b.resource_type, &b.name)));
    modified.sort_by(|a, b| (&a.resource_type, &a.name).cmp(&(&b.resource_type, &b.name)));

    Ok(DiffResult {
        success: errors.is_empty(),
        source: source_name.to_string(),
        target: target_name.to_string(),
        added,
        removed,
        modified,
        errors,
    })
}

/// Get resource types from both directories
fn get_resource_types(source_dir: &Path, target_dir: &Path, filter: &[String]) -> Vec<String> {
    let mut types = HashSet::new();

    // Known resource type directories
    let known_types = [
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
    ];

    for dir in [source_dir, target_dir] {
        if let Ok(entries) = std::fs::read_dir(dir) {
            for entry in entries.flatten() {
                if entry.path().is_dir() {
                    if let Some(name) = entry.file_name().to_str() {
                        if known_types.contains(&name) {
                            types.insert(name.to_string());
                        }
                    }
                }
            }
        }
    }

    // Filter if specified
    let mut result: Vec<_> = if filter.is_empty() {
        types.into_iter().collect()
    } else {
        types
            .into_iter()
            .filter(|t| filter.iter().any(|f| t.contains(f)))
            .collect()
    };

    result.sort();
    result
}

/// Get JSON files in a directory
fn get_json_files(dir: &Path) -> HashMap<String, PathBuf> {
    let mut files = HashMap::new();

    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_file() && path.extension().map(|e| e == "json").unwrap_or(false) {
                if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
                    files.insert(stem.to_string(), path);
                }
            }
        }
    }

    files
}

/// Compare two JSON files
fn compare_files(source: &Path, target: &Path, full_diff: bool) -> Result<Option<String>> {
    let source_content = std::fs::read_to_string(source).map_err(|_| VqxError::FileReadFailed {
        path: source.display().to_string(),
    })?;
    let target_content = std::fs::read_to_string(target).map_err(|_| VqxError::FileReadFailed {
        path: target.display().to_string(),
    })?;

    if source_content == target_content {
        return Ok(None);
    }

    // Generate unified diff
    let diff = TextDiff::from_lines(&source_content, &target_content);

    let mut diff_text = String::new();

    if full_diff {
        // Full unified diff
        for change in diff.iter_all_changes() {
            let sign = match change.tag() {
                ChangeTag::Delete => "-",
                ChangeTag::Insert => "+",
                ChangeTag::Equal => " ",
            };
            diff_text.push_str(&format!("{}{}", sign, change));
        }
    } else {
        // Summary only - count changes
        let mut additions = 0;
        let mut deletions = 0;

        for change in diff.iter_all_changes() {
            match change.tag() {
                ChangeTag::Insert => additions += 1,
                ChangeTag::Delete => deletions += 1,
                ChangeTag::Equal => {}
            }
        }

        diff_text = format!("+{} -{}", additions, deletions);
    }

    Ok(Some(diff_text))
}

/// Display diff results to terminal
fn display_diff_results(result: &DiffResult, full_diff: bool) {
    println!();
    println!("{}", style("─".repeat(50)).dim());

    if !result.has_changes() {
        println!("{} No differences found", style("✓").green().bold());
        println!();
        return;
    }

    println!(
        "{} Found {} change(s)",
        style("!").yellow().bold(),
        result.total_changes()
    );
    println!();

    // Added
    if !result.added.is_empty() {
        println!(
            "{} {} added:",
            style("+").green().bold(),
            result.added.len()
        );
        for diff in &result.added {
            println!(
                "    {} {}/{}",
                style("+").green(),
                diff.resource_type,
                diff.name
            );
        }
        println!();
    }

    // Removed
    if !result.removed.is_empty() {
        println!(
            "{} {} removed:",
            style("-").red().bold(),
            result.removed.len()
        );
        for diff in &result.removed {
            println!(
                "    {} {}/{}",
                style("-").red(),
                diff.resource_type,
                diff.name
            );
        }
        println!();
    }

    // Modified
    if !result.modified.is_empty() {
        println!(
            "{} {} modified:",
            style("~").yellow().bold(),
            result.modified.len()
        );
        for diff in &result.modified {
            println!(
                "    {} {}/{}",
                style("~").yellow(),
                diff.resource_type,
                diff.name
            );
            if full_diff {
                if let Some(ref text) = diff.diff_text {
                    for line in text.lines() {
                        let colored_line = if line.starts_with('+') {
                            style(line).green().to_string()
                        } else if line.starts_with('-') {
                            style(line).red().to_string()
                        } else {
                            line.to_string()
                        };
                        println!("        {}", colored_line);
                    }
                }
            } else if let Some(ref text) = diff.diff_text {
                println!("        {}", style(text).dim());
            }
        }
        println!();
    }

    // Errors
    if !result.errors.is_empty() {
        println!(
            "{} {} error(s):",
            style("⚠").red().bold(),
            result.errors.len()
        );
        for error in &result.errors {
            println!("    {}", style(error).red());
        }
        println!();
    }

    println!("{}", style("─".repeat(50)).dim());
    println!();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_diff_source_parse_directory() {
        // Current directory should be detected as directory
        let source = DiffSource::parse(".");
        assert!(matches!(source, DiffSource::Directory(_)));
    }

    #[test]
    fn test_diff_source_parse_profile() {
        // Non-existent path should be treated as profile name
        let source = DiffSource::parse("my-profile");
        assert!(matches!(source, DiffSource::Profile(_)));
    }

    #[test]
    fn test_change_kind_display() {
        assert_eq!(format!("{}", ChangeKind::Added), "added");
        assert_eq!(format!("{}", ChangeKind::Removed), "removed");
        assert_eq!(format!("{}", ChangeKind::Modified), "modified");
    }
}
