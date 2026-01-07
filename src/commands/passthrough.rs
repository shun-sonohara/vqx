//! Passthrough command implementation
//!
//! Passes commands directly to the underlying Vantiq CLI.
//! This is useful for:
//! - Debugging
//! - Accessing CLI features not yet wrapped by vqx
//! - Testing CLI behavior directly
//!
//! All arguments after the subcommand are passed verbatim to the CLI.

use crate::cli::PassthroughArgs;
use crate::config::Config;
use crate::error::Result;
use crate::profile::ProfileManager;
use crate::underlying::{CliOptions, UnderlyingCli};
use console::style;
use tracing::{debug, info, warn};

/// Run passthrough command
pub async fn run(
    args: &PassthroughArgs,
    config: &Config,
    profile_name: Option<&str>,
    verbose: bool,
) -> Result<i32> {
    info!(
        args = ?args.args,
        profile = ?profile_name,
        "Running passthrough command"
    );

    let cli = UnderlyingCli::new(config.cli_path.clone())
        .with_timeout(config.timeout())
        .with_retries(config.max_retries, config.retry_delay_ms);

    // Build arguments with profile credentials if specified
    let mut full_args: Vec<String> = Vec::new();

    if let Some(profile_name) = profile_name {
        // Load profile and add connection options
        let manager = ProfileManager::new()?;
        let profile = manager.get_resolved(profile_name)?;
        let options = CliOptions::from_profile(&profile);

        // Add connection options first
        // PDF: "-b <baseURL>"
        full_args.push("-b".to_string());
        full_args.push(profile.url.clone());

        // PDF: "-u <username>" and "-p <password>"
        if let Some(ref username) = profile.username {
            full_args.push("-u".to_string());
            full_args.push(username.clone());
        }
        if let Some(ref password) = profile.password {
            full_args.push("-p".to_string());
            full_args.push(password.clone());
        }

        // PDF: "-t <token>" (only if no password)
        if profile.password.is_none() {
            if let Some(ref token) = profile.token {
                full_args.push("-t".to_string());
                full_args.push(token.clone());
            }
        }

        // PDF: "-n <namespace>"
        if let Some(ref ns) = profile.namespace {
            full_args.push("-n".to_string());
            full_args.push(ns.clone());
        }

        // PDF: "-trust"
        if profile.trust_ssl {
            full_args.push("-trust".to_string());
        }
    }

    // Add user-provided arguments
    full_args.extend(args.args.clone());

    if verbose {
        println!();
        println!("{}", style("Passthrough Mode").bold().yellow());
        println!("{}", style("─".repeat(40)).dim());
        println!("CLI: {}", style(&config.cli_path).cyan());

        // Show masked arguments
        let masked_args = mask_sensitive_args(&full_args);
        println!("Args: {}", style(masked_args.join(" ")).dim());
        println!();
    }

    // Execute
    let result = cli.execute_raw(&full_args).await?;

    // Print output
    if !result.stdout.is_empty() {
        print!("{}", result.stdout);
    }
    if !result.stderr.is_empty() {
        eprint!("{}", result.stderr);
    }

    if verbose {
        println!();
        println!("{}", style("─".repeat(40)).dim());
        let status_style = if result.success() {
            style(format!("Exit code: {}", result.code())).green()
        } else {
            style(format!("Exit code: {}", result.code())).red()
        };
        println!("{}", status_style);
    }

    Ok(result.code())
}

/// Mask sensitive arguments for display
fn mask_sensitive_args(args: &[String]) -> Vec<String> {
    let mut masked = Vec::new();
    let mut skip_next = false;

    for (i, arg) in args.iter().enumerate() {
        if skip_next {
            skip_next = false;
            masked.push("********".to_string());
            continue;
        }

        // Flags that have sensitive values following
        if arg == "-p" || arg == "-t" {
            masked.push(arg.clone());
            skip_next = true;
            continue;
        }

        // Combined form like -p=password
        if arg.starts_with("-p=") || arg.starts_with("-t=") {
            let prefix = &arg[..3]; // "-p=" or "-t="
            masked.push(format!("{}********", prefix));
            continue;
        }

        masked.push(arg.clone());
    }

    masked
}

/// Display help for passthrough mode
pub fn display_help() {
    println!();
    println!("{}", style("Passthrough Mode").bold().cyan());
    println!("{}", style("─".repeat(60)).dim());
    println!();
    println!("Pass commands directly to the underlying Vantiq CLI.");
    println!();
    println!("{}", style("Usage:").bold());
    println!("  vqx passthrough [OPTIONS] <CLI_ARGS>...");
    println!("  vqx --profile <name> passthrough <CLI_ARGS>...");
    println!();
    println!("{}", style("Examples:").bold());
    println!();
    println!("  # List all types (PDF: 'list <resource>')");
    println!("  {} vqx passthrough list types", style("$").dim());
    println!();
    println!("  # Find a specific procedure (PDF: 'find <resource> <resourceId>')");
    println!(
        "  {} vqx passthrough find procedures MyProcedure",
        style("$").dim()
    );
    println!();
    println!("  # Export metadata (PDF: 'export' command)");
    println!(
        "  {} vqx passthrough export metadata -d ./export",
        style("$").dim()
    );
    println!();
    println!("  # Run a procedure (PDF: 'run procedure <name>')");
    println!(
        "  {} vqx passthrough run procedure Utils.getNamespaceAndProfiles",
        style("$").dim()
    );
    println!();
    println!("  # Use with a specific profile");
    println!(
        "  {} vqx --profile production passthrough list sources",
        style("$").dim()
    );
    println!();
    println!("{}", style("PDF Reference - Supported Commands:").bold());
    println!("  help, list, find, dump, load, delete, deleteMatching,");
    println!("  select, insert, upsert, checkedInsert, checkedUpsert,");
    println!("  execute (deprecated), run, stop, recommend,");
    println!("  deploy, undeploy, pull, export, import");
    println!();
    println!(
        "{}",
        style("Note: Connection options (-b, -u, -p, -t, -n, -trust) are").dim()
    );
    println!(
        "{}",
        style("automatically added from the selected profile.").dim()
    );
    println!();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mask_sensitive_args() {
        let args = vec![
            "-b".to_string(),
            "https://dev.vantiq.com".to_string(),
            "-u".to_string(),
            "user".to_string(),
            "-p".to_string(),
            "secret_password".to_string(),
            "list".to_string(),
            "types".to_string(),
        ];

        let masked = mask_sensitive_args(&args);

        assert!(masked.contains(&"user".to_string()));
        assert!(masked.contains(&"********".to_string()));
        assert!(!masked.contains(&"secret_password".to_string()));
    }

    #[test]
    fn test_mask_combined_form() {
        let args = vec!["-p=secret".to_string(), "-t=token123".to_string()];

        let masked = mask_sensitive_args(&args);

        assert_eq!(masked[0], "-p=********");
        assert_eq!(masked[1], "-t=********");
    }
}
