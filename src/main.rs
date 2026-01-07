//! vqx - A safe, feature-rich Rust wrapper for the Vantiq CLI
//!
//! This tool provides:
//! - Workflow automation (export → git → import → test)
//! - Safety guards for destructive operations
//! - Profile management with secure credential storage
//! - JSON normalization for git-friendly diffs
//! - Developer-friendly features (progress, retry, logging)
//!
//! Based on: CLI Reference Guide PDF from Vantiq
//!
//! ## Phase 1 Implementation
//! - `doctor`: Check environment prerequisites
//! - `profile`: Manage connection profiles
//! - `passthrough`: Direct CLI access
//!
//! ## Phase 2 Implementation
//! - `export`: Export with JSON normalization
//! - `import`: Import with safety confirmations

mod cli;
mod commands;
mod config;
mod error;
mod normalizer;
mod profile;
mod underlying;

use anyhow::Result;
use clap::Parser;
use cli::{Cli, Commands};
use config::Config;
use console::style;
use tracing::info;
use tracing_subscriber::{fmt, prelude::*, EnvFilter};

#[tokio::main]
async fn main() -> Result<()> {
    // Parse CLI arguments
    let cli = Cli::parse();

    // Initialize logging
    init_logging(&cli)?;

    // Load configuration
    let config = load_config(&cli)?;

    info!(
        cli_path = %config.cli_path,
        profile = ?cli.profile,
        "Starting vqx"
    );

    // Execute command
    let exit_code = match &cli.command {
        // Phase 1: Core utilities
        Commands::Doctor(args) => {
            let results = commands::doctor::run(args, &config).await?;
            commands::doctor::display_results(&results, cli.verbose);

            if results.iter().all(|r| r.passed) {
                0
            } else {
                1
            }
        }

        Commands::Profile(cmd) => {
            commands::profile::run(cmd, cli.output).await?;
            0
        }

        Commands::External(args) => {
            // Direct CLI access: `vqx list types` -> `vantiq list types`
            commands::external::run(&args, &config, cli.profile.as_deref(), cli.verbose)
                .await?
        }

        // Phase 2: Export/Import
        Commands::Export(args) => {
            let result = commands::export::run(
                args,
                &config,
                cli.profile.as_deref(),
                cli.output,
                cli.verbose,
            )
            .await?;

            if result.success {
                0
            } else {
                1
            }
        }

        Commands::Import(args) => {
            let result = commands::import::run(
                args,
                &config,
                cli.profile.as_deref(),
                cli.output,
                cli.verbose,
            )
            .await?;

            if result.success {
                0
            } else {
                1
            }
        }

        // Phase 3: Diff/Sync
        Commands::Diff(args) => {
            let result = commands::diff::run(args, &config, cli.output, cli.verbose).await?;

            if result.success && !result.has_changes() {
                0
            } else if result.success {
                // Changes found, but operation succeeded
                0
            } else {
                1
            }
        }

        Commands::Sync(cmd) => {
            let result = commands::sync::run(
                cmd,
                &config,
                cli.profile.as_deref(),
                cli.output,
                cli.verbose,
            )
            .await?;

            if result.success {
                0
            } else {
                1
            }
        }

        Commands::SafeDelete(_) => {
            println!(
                "{} SafeDelete command is not yet implemented (Phase 4).",
                style("⚠").yellow()
            );
            println!("Use 'vqx passthrough delete ...' with caution for now.");
            1
        }

        Commands::Promote(_) => {
            println!(
                "{} Promote command is not yet implemented (Phase 4).",
                style("⚠").yellow()
            );
            1
        }

        Commands::Run(_) => {
            println!(
                "{} Run command is not yet implemented (Phase 4).",
                style("⚠").yellow()
            );
            println!("Use 'vqx passthrough run ...' for now.");
            1
        }
    };

    std::process::exit(exit_code);
}

/// Initialize logging based on CLI options and config
fn init_logging(cli: &Cli) -> Result<()> {
    let level = if cli.verbose {
        "debug"
    } else if cli.quiet {
        "error"
    } else {
        "info"
    };

    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new(format!("vqx={}", level)));

    tracing_subscriber::registry()
        .with(fmt::layer().with_target(false).without_time())
        .with(filter)
        .init();

    Ok(())
}

/// Load configuration from file or defaults
fn load_config(cli: &Cli) -> Result<Config> {
    let config = if let Some(ref path) = cli.config {
        Config::load_from(path)?
    } else {
        Config::load().unwrap_or_default()
    };

    // Override CLI path if specified on command line
    let config = if let Some(ref cli_path) = cli.cli {
        Config {
            cli_path: cli_path.clone(),
            ..config
        }
    } else {
        config
    };

    Ok(config)
}
