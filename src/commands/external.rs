//! External CLI command execution
//!
//! Passes unrecognized commands directly to the underlying Vantiq CLI.
//! This allows users to run any CLI command through vqx:
//!   vqx list types
//!   vqx find procedures MyProc
//!   vqx --profile dev select types

use crate::config::Config;
use crate::error::Result;
use crate::profile::ProfileManager;
use crate::underlying::{CliOptions, UnderlyingCli};
use console::style;
use tracing::info;

/// Run an external CLI command
pub async fn run(
    args: &[String],
    config: &Config,
    profile_name: Option<&str>,
    verbose: bool,
) -> Result<i32> {
    info!(
        args = ?args,
        profile = ?profile_name,
        "Running external CLI command"
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
        let _options = CliOptions::from_profile(&profile);

        // Add connection options first
        full_args.push("-b".to_string());
        full_args.push(profile.url.clone());

        if let Some(ref username) = profile.username {
            full_args.push("-u".to_string());
            full_args.push(username.clone());
        }
        if let Some(ref password) = profile.password {
            full_args.push("-p".to_string());
            full_args.push(password.clone());
        }

        // Token only if no password
        if profile.password.is_none() {
            if let Some(ref token) = profile.token {
                full_args.push("-t".to_string());
                full_args.push(token.clone());
            }
        }

        if let Some(ref ns) = profile.namespace {
            full_args.push("-n".to_string());
            full_args.push(ns.clone());
        }

        if profile.trust_ssl {
            full_args.push("-trust".to_string());
        }
    }

    // Add user-provided arguments
    full_args.extend_from_slice(args);

    if verbose {
        println!();
        println!("{}", style("External CLI Command").bold().yellow());
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

    for arg in args.iter() {
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
            let prefix = &arg[..3];
            masked.push(format!("{}********", prefix));
            continue;
        }

        masked.push(arg.clone());
    }

    masked
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
