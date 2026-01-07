//! CLI command definitions using clap
//!
//! This module defines the vqx CLI structure.
//! All subcommands are designed to wrap the underlying Vantiq CLI
//! as documented in the CLI Reference Guide PDF.

use clap::{Args, Parser, Subcommand, ValueEnum};
use std::path::PathBuf;

/// vqx - A safe, feature-rich Rust wrapper for the Vantiq CLI
///
/// Provides workflow automation, safety guards for destructive operations,
/// profile management, and developer-friendly features.
#[derive(Parser, Debug)]
#[command(name = "vqx")]
#[command(author, version, about, long_about = None)]
#[command(propagate_version = true)]
pub struct Cli {
    /// Path to the underlying Vantiq CLI executable
    /// PDF: Default is "vantiq" (Mac/Linux) or "vantiq.bat" (Windows)
    #[arg(long, global = true, env = "VQX_CLI_PATH")]
    pub cli: Option<String>,

    /// Profile name to use for connection
    /// Maps to PDF's "-s <profileName>" option
    #[arg(short = 's', long, global = true, env = "VQX_PROFILE")]
    pub profile: Option<String>,

    /// Path to vqx config file
    #[arg(long, global = true, env = "VQX_CONFIG")]
    pub config: Option<PathBuf>,

    /// Enable verbose output
    #[arg(short, long, global = true)]
    pub verbose: bool,

    /// Suppress non-essential output
    #[arg(short, long, global = true)]
    pub quiet: bool,

    /// Output format
    #[arg(long, global = true, value_enum, default_value = "text")]
    pub output: OutputFormat,

    #[command(subcommand)]
    pub command: Commands,
}

/// Output format for command results
#[derive(Debug, Clone, Copy, ValueEnum, Default)]
pub enum OutputFormat {
    /// Human-readable text output
    #[default]
    Text,
    /// JSON output
    Json,
    /// CSV output (where applicable)
    Csv,
}

/// Available subcommands
#[derive(Subcommand, Debug)]
pub enum Commands {
    // =========================================================================
    // Phase 1: Core utilities
    // =========================================================================
    /// Check environment and CLI prerequisites
    ///
    /// Verifies:
    /// - Java 11 is installed (PDF: "Prerequisites" section)
    /// - Vantiq CLI is available in PATH
    /// - CLI can execute basic commands
    Doctor(DoctorArgs),

    /// Manage connection profiles
    ///
    /// vqx uses TOML-based profiles that map to the underlying CLI's
    /// connection options (PDF: "Profile" and "Command Line Options" sections)
    #[command(subcommand)]
    Profile(ProfileCommands),

    /// Pass commands directly to the underlying CLI
    ///
    /// Use this for debugging or accessing CLI features not yet wrapped by vqx.
    /// All arguments after "--" are passed directly to the CLI.
    Passthrough(PassthroughArgs),

    // =========================================================================
    // Phase 2: Export/Import (to be implemented)
    // =========================================================================
    /// Export resources from Vantiq
    ///
    /// Wraps PDF's "export" command with JSON normalization
    Export(ExportArgs),

    /// Import resources to Vantiq
    ///
    /// Wraps PDF's "import" command with safety checks
    Import(ImportArgs),

    // =========================================================================
    // Phase 3: Diff/Sync (to be implemented)
    // =========================================================================
    /// Compare resources between environments or files
    Diff(DiffArgs),

    /// Synchronize resources
    #[command(subcommand)]
    Sync(SyncCommands),

    // =========================================================================
    // Phase 4: Safe operations (to be implemented)
    // =========================================================================
    /// Safely delete resources with confirmation and backup
    ///
    /// Wraps PDF's "delete" and "deleteMatching" commands with:
    /// - Dry-run mode
    /// - Confirmation prompts
    /// - Automatic backup
    SafeDelete(SafeDeleteArgs),

    /// Promote resources between environments
    ///
    /// Workflow: export -> diff -> confirm -> import -> test
    Promote(PromoteArgs),

    /// Run smoke tests
    ///
    /// Wraps PDF's "run testsuite" and "run procedure" commands
    #[command(subcommand)]
    Run(RunCommands),
}

// =============================================================================
// Phase 1: Doctor
// =============================================================================

/// Arguments for the doctor command
#[derive(Args, Debug)]
pub struct DoctorArgs {
    /// Only check Java installation
    #[arg(long)]
    pub java_only: bool,

    /// Only check CLI installation
    #[arg(long)]
    pub cli_only: bool,

    /// Test connection to the server using a profile
    #[arg(long)]
    pub test_connection: bool,
}

// =============================================================================
// Phase 1: Profile
// =============================================================================

/// Profile management subcommands
#[derive(Subcommand, Debug)]
pub enum ProfileCommands {
    /// List all configured profiles
    List,

    /// Show details of a profile
    Show(ProfileShowArgs),

    /// Create or update a profile
    Set(ProfileSetArgs),

    /// Delete a profile
    Delete(ProfileDeleteArgs),

    /// Set the default profile
    Default(ProfileDefaultArgs),

    /// Import profiles from a file
    Import(ProfileImportArgs),

    /// Export profiles to a file
    Export(ProfileExportArgs),

    /// Interactively create a new profile
    Init(ProfileInitArgs),
}

#[derive(Args, Debug)]
pub struct ProfileShowArgs {
    /// Profile name to show
    pub name: String,

    /// Show sensitive values (passwords/tokens)
    #[arg(long)]
    pub show_secrets: bool,
}

#[derive(Args, Debug)]
pub struct ProfileSetArgs {
    /// Profile name
    pub name: String,

    /// Vantiq server URL
    /// PDF: "url = '...'" in profile, maps to "-b <baseURL>"
    #[arg(short = 'b', long)]
    pub url: Option<String>,

    /// Username for authentication
    /// PDF: "username = '...'" in profile, maps to "-u <username>"
    /// Note: "username/password can only be used for Edge servers"
    #[arg(short, long)]
    pub username: Option<String>,

    /// Password for authentication
    /// PDF: "password = '...'" in profile, maps to "-p <password>"
    #[arg(short, long)]
    pub password: Option<String>,

    /// Access token for authentication
    /// PDF: "token = '...'" in profile, maps to "-t <token>"
    /// Note: "public clouds and any server using keycloak access require use of the token option"
    #[arg(short, long)]
    pub token: Option<String>,

    /// Target namespace
    /// PDF: "namespace = '...'" in profile, maps to "-n <namespace>"
    /// Note: "the namespace option can only be used with username/password"
    #[arg(short, long)]
    pub namespace: Option<String>,

    /// Trust SSL certificates
    /// PDF: "-trust" flag
    #[arg(long)]
    pub trust_ssl: bool,

    /// Store credentials in secure storage (keyring)
    #[arg(long)]
    pub secure: bool,

    /// Description for this profile
    #[arg(long)]
    pub description: Option<String>,
}

#[derive(Args, Debug)]
pub struct ProfileDeleteArgs {
    /// Profile name to delete
    pub name: String,

    /// Skip confirmation
    #[arg(short, long)]
    pub force: bool,
}

#[derive(Args, Debug)]
pub struct ProfileDefaultArgs {
    /// Profile name to set as default
    pub name: String,
}

#[derive(Args, Debug)]
pub struct ProfileImportArgs {
    /// File to import from
    pub file: PathBuf,

    /// Overwrite existing profiles
    #[arg(long)]
    pub overwrite: bool,
}

#[derive(Args, Debug)]
pub struct ProfileExportArgs {
    /// File to export to
    pub file: PathBuf,

    /// Include sensitive values
    #[arg(long)]
    pub include_secrets: bool,
}

#[derive(Args, Debug)]
pub struct ProfileInitArgs {
    /// Profile name to create
    pub name: Option<String>,
}

// =============================================================================
// Phase 1: Passthrough
// =============================================================================

/// Arguments for passthrough command
#[derive(Args, Debug)]
pub struct PassthroughArgs {
    /// Arguments to pass to the underlying CLI
    #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
    pub args: Vec<String>,
}

// =============================================================================
// Phase 2: Export/Import (placeholders)
// =============================================================================

/// Arguments for export command
/// Based on PDF "Export" section
#[derive(Args, Debug)]
pub struct ExportArgs {
    /// What to export: metadata, data, project, projectdata, hidden
    /// PDF: "export [data | metadata | project <projectName> | projectdata <projectName> | hidden]"
    #[arg(value_enum, default_value = "metadata")]
    pub export_type: ExportType,

    /// Project name (required for project/projectdata types)
    #[arg(long)]
    pub project: Option<String>,

    /// Output directory
    /// PDF: "-d <directoryName>"
    #[arg(short = 'd', long)]
    pub directory: Option<PathBuf>,

    /// Chunk size for large exports
    /// PDF: "-chunk <integer>"
    #[arg(long)]
    pub chunk: Option<u32>,

    /// Types to include
    /// PDF: "-include <typeName(s)>"
    #[arg(long)]
    pub include: Vec<String>,

    /// Types to exclude
    /// PDF: "-exclude <typeName(s)>"
    #[arg(long)]
    pub exclude: Vec<String>,

    /// Export data until this timestamp
    /// PDF: "-until <DateTime>" (ISO format or "NOW")
    #[arg(long)]
    pub until: Option<String>,

    /// Ignore errors during export
    /// PDF: "-ignoreErrors"
    #[arg(long)]
    pub ignore_errors: bool,

    /// Normalize JSON output for git-friendly diffs (vqx extension)
    /// Use --no-normalize to disable
    #[arg(long, default_value_t = true, action = clap::ArgAction::Set)]
    pub normalize: bool,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum ExportType {
    Metadata,
    Data,
    Project,
    ProjectData,
    Hidden,
}

/// Arguments for import command
/// Based on PDF "Import" section
#[derive(Args, Debug)]
pub struct ImportArgs {
    /// What to import: metadata or data
    /// PDF: "import [data | metadata]"
    #[arg(value_enum, default_value = "metadata")]
    pub import_type: ImportType,

    /// Input directory
    /// PDF: "-d <directoryName>"
    #[arg(short = 'd', long)]
    pub directory: Option<PathBuf>,

    /// Chunk size for large imports
    /// PDF: "-chunk <integer>"
    #[arg(long)]
    pub chunk: Option<u32>,

    /// Types to include
    /// PDF: "-include <typeName>"
    #[arg(long)]
    pub include: Vec<String>,

    /// Types to exclude
    /// PDF: "-exclude <typeName>"
    #[arg(long)]
    pub exclude: Vec<String>,

    /// Resource types to ignore
    /// PDF: "-ignore <resourceType>"
    #[arg(long)]
    pub ignore: Vec<String>,

    /// Skip confirmation prompt
    #[arg(short, long)]
    pub yes: bool,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum ImportType {
    Metadata,
    Data,
}

// =============================================================================
// Phase 3: Diff/Sync (placeholders)
// =============================================================================

/// Arguments for diff command
#[derive(Args, Debug)]
pub struct DiffArgs {
    /// Source: profile name or directory path
    pub source: String,

    /// Target: profile name or directory path
    pub target: String,

    /// Only diff specific resource types
    #[arg(long)]
    pub resource: Vec<String>,

    /// Show full diff output
    #[arg(long)]
    pub full: bool,
}

/// Sync subcommands
#[derive(Subcommand, Debug)]
pub enum SyncCommands {
    /// Pull from remote to local (export)
    Pull(SyncPullArgs),

    /// Push from local to remote (import with diff + confirm)
    Push(SyncPushArgs),
}

#[derive(Args, Debug)]
pub struct SyncPullArgs {
    /// Local directory
    #[arg(short = 'd', long)]
    pub directory: PathBuf,

    /// Force overwrite local changes
    #[arg(short, long)]
    pub force: bool,
}

#[derive(Args, Debug)]
pub struct SyncPushArgs {
    /// Local directory
    #[arg(short = 'd', long)]
    pub directory: PathBuf,

    /// Skip confirmation
    #[arg(short, long)]
    pub yes: bool,

    /// Dry run - show what would be pushed
    #[arg(long)]
    pub dry_run: bool,
}

// =============================================================================
// Phase 4: Safe operations (placeholders)
// =============================================================================

/// Arguments for safe-delete command
/// Wraps PDF's "delete" and "deleteMatching" with safety guards
#[derive(Args, Debug)]
pub struct SafeDeleteArgs {
    /// Resource type
    /// PDF: "delete <resource> <resourceId>"
    pub resource: String,

    /// Resource ID or query
    /// If starts with '{', treated as deleteMatching query
    /// PDF: "deleteMatching <resource> <query>"
    pub target: String,

    /// Dry run - only show what would be deleted
    #[arg(long)]
    pub dry_run: bool,

    /// Skip backup
    #[arg(long)]
    pub no_backup: bool,

    /// Skip confirmation
    #[arg(short, long)]
    pub yes: bool,

    /// Force delete even if over limit
    #[arg(long)]
    pub force: bool,
}

/// Arguments for promote command
#[derive(Args, Debug)]
pub struct PromoteArgs {
    /// Source profile
    #[arg(long)]
    pub from: String,

    /// Target profile
    #[arg(long)]
    pub to: String,

    /// Skip diff display
    #[arg(long)]
    pub no_diff: bool,

    /// Skip smoke tests after promotion
    #[arg(long)]
    pub no_test: bool,

    /// Test suite to run after promotion
    /// PDF: "run testsuite <testSuiteName>"
    #[arg(long)]
    pub testsuite: Option<String>,

    /// Procedure to run after promotion
    /// PDF: "run procedure <procedureName>"
    #[arg(long)]
    pub procedure: Option<String>,

    /// Skip confirmation
    #[arg(short, long)]
    pub yes: bool,
}

/// Run subcommands
/// Based on PDF "Run" section
#[derive(Subcommand, Debug)]
pub enum RunCommands {
    /// Run a test
    /// PDF: "run test <testName>"
    Test(RunTestArgs),

    /// Run a test suite
    /// PDF: "run testsuite <testSuiteName>"
    TestSuite(RunTestSuiteArgs),

    /// Run a procedure
    /// PDF: "run procedure <procedureName>"
    Procedure(RunProcedureArgs),
}

#[derive(Args, Debug)]
pub struct RunTestArgs {
    /// Test name
    pub name: String,
}

#[derive(Args, Debug)]
pub struct RunTestSuiteArgs {
    /// Test suite name
    pub name: String,

    /// Start from specific test
    #[arg(long)]
    pub start_from: Option<String>,
}

#[derive(Args, Debug)]
pub struct RunProcedureArgs {
    /// Procedure name
    pub name: String,

    /// Parameters as name:value pairs
    /// PDF: "<p1Name>:<p1Value> ... <pNName>:<pNValue>"
    #[arg(trailing_var_arg = true)]
    pub params: Vec<String>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::CommandFactory;

    #[test]
    fn test_cli_parses() {
        Cli::command().debug_assert();
    }

    #[test]
    fn test_doctor_command() {
        let cli = Cli::parse_from(["vqx", "doctor"]);
        assert!(matches!(cli.command, Commands::Doctor(_)));
    }

    #[test]
    fn test_profile_list() {
        let cli = Cli::parse_from(["vqx", "profile", "list"]);
        assert!(matches!(
            cli.command,
            Commands::Profile(ProfileCommands::List)
        ));
    }

    #[test]
    fn test_passthrough() {
        let cli = Cli::parse_from(["vqx", "passthrough", "list", "types"]);
        if let Commands::Passthrough(args) = cli.command {
            assert_eq!(args.args, vec!["list", "types"]);
        } else {
            panic!("Expected Passthrough command");
        }
    }
}
