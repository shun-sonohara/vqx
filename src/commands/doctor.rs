//! Doctor command implementation
//!
//! Checks environment prerequisites for running the Vantiq CLI.
//!
//! Based on: CLI Reference Guide PDF
//! - "Prerequisites" section: "The Vantiq CLI is a Java (Groovy) application
//!    and requires an installation of Java 11."
//! - "Installation" section: CLI binary location

use crate::cli::DoctorArgs;
use crate::config::Config;
use crate::error::Result;
use crate::profile::ProfileManager;
use crate::underlying::{CliOptions, UnderlyingCli};
use console::{style, Emoji};
use regex::Regex;
use std::process::Command;
use tracing::{debug, info};

// Emojis for status display
static CHECK: Emoji<'_, '_> = Emoji("✅ ", "[OK] ");
static CROSS: Emoji<'_, '_> = Emoji("❌ ", "[FAIL] ");
static WARN: Emoji<'_, '_> = Emoji("⚠️  ", "[WARN] ");
static INFO: Emoji<'_, '_> = Emoji("ℹ️  ", "[INFO] ");

/// Result of a single check
#[derive(Debug)]
pub struct CheckResult {
    pub name: String,
    pub passed: bool,
    pub message: String,
    pub details: Option<String>,
}

impl CheckResult {
    fn ok(name: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            passed: true,
            message: message.into(),
            details: None,
        }
    }

    fn fail(name: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            passed: false,
            message: message.into(),
            details: None,
        }
    }

    fn with_details(mut self, details: impl Into<String>) -> Self {
        self.details = Some(details.into());
        self
    }
}

/// Run the doctor command
pub async fn run(args: &DoctorArgs, config: &Config) -> Result<Vec<CheckResult>> {
    let mut results = Vec::new();

    if !args.cli_only {
        // Check Java installation
        // PDF: "The Vantiq CLI is a Java (Groovy) application and requires an installation of Java 11."
        results.push(check_java().await);
    }

    if !args.java_only {
        // Check CLI installation
        results.push(check_cli(&config.cli_path).await);

        // Check CLI help command works
        results.push(check_cli_help(&config.cli_path).await);
    }

    if args.test_connection {
        // Test connection using profile
        results.push(check_connection(&config.cli_path).await);
    }

    Ok(results)
}

/// Check Java installation
/// PDF: "Prerequisites" - "requires an installation of Java 11"
async fn check_java() -> CheckResult {
    info!("Checking Java installation...");

    // Try to run java -version
    let output = Command::new("java").arg("-version").output();

    match output {
        Ok(output) => {
            // Java version is typically printed to stderr
            let version_output = String::from_utf8_lossy(&output.stderr);
            debug!(output = %version_output, "Java version output");

            // Parse version number
            // Common formats:
            // - openjdk version "11.0.12" ...
            // - java version "1.8.0_301"
            // - openjdk version "17.0.1" ...
            let version_regex = Regex::new(r#"version "([^"]+)""#).unwrap();

            if let Some(captures) = version_regex.captures(&version_output) {
                let version_str = captures.get(1).map(|m| m.as_str()).unwrap_or("unknown");

                // Parse major version
                let major_version = parse_java_major_version(version_str);

                if let Some(major) = major_version {
                    if major >= 11 {
                        CheckResult::ok(
                            "Java",
                            format!("Java {} found (>= 11 required)", version_str),
                        )
                        .with_details(format!(
                            "PDF Reference: Prerequisites section states 'requires an installation of Java 11'"
                        ))
                    } else {
                        CheckResult::fail(
                            "Java",
                            format!(
                                "Java {} found, but Java 11 or later is required",
                                version_str
                            ),
                        )
                        .with_details(format!(
                            "PDF Reference: Prerequisites section - 'The Vantiq CLI is a Java (Groovy) application and requires an installation of Java 11.'\n\
                             Please install Java 11 or later from https://adoptium.net/"
                        ))
                    }
                } else {
                    CheckResult::fail(
                        "Java",
                        format!("Could not parse Java version: {}", version_str),
                    )
                }
            } else {
                CheckResult::fail("Java", "Could not determine Java version")
                    .with_details(version_output.to_string())
            }
        }
        Err(e) => CheckResult::fail("Java", format!("Java not found: {}", e)).with_details(
            "PDF Reference: Prerequisites section - 'The Vantiq CLI is a Java (Groovy) application and requires an installation of Java 11.'\n\
             Please install Java 11 from https://adoptium.net/ and ensure it's in your PATH."
        ),
    }
}

/// Parse Java major version from version string
/// Handles both old format (1.8) and new format (11, 17, etc.)
fn parse_java_major_version(version: &str) -> Option<u32> {
    let parts: Vec<&str> = version.split('.').collect();

    if parts.is_empty() {
        return None;
    }

    // Handle 1.x format (Java 8 and earlier)
    if parts[0] == "1" && parts.len() > 1 {
        parts[1].parse().ok()
    } else {
        // Handle modern format (9+)
        parts[0].parse().ok()
    }
}

/// Check CLI installation
/// PDF: "Installation" - CLI should be in PATH
async fn check_cli(cli_path: &str) -> CheckResult {
    info!(cli = cli_path, "Checking CLI installation...");

    let cli = UnderlyingCli::new(cli_path.to_string());

    match cli.check_cli_exists() {
        Ok(path) => CheckResult::ok("Vantiq CLI", format!("Found at: {}", path)).with_details(
            "PDF Reference: Installation section - 'It is recommended that the directory ./vantiq-x.x.x/bin be added to your path.'"
        ),
        Err(_) => CheckResult::fail(
            "Vantiq CLI",
            format!("CLI not found: {}", cli_path),
        )
        .with_details(format!(
            "PDF Reference: Installation section\n\
             - Download from: Help -> Developer Resources in Vantiq UI\n\
             - On Mac/Linux: use 'vantiq' command\n\
             - On Windows: use 'vantiq.bat' command\n\
             - Ensure {}/bin is in your PATH",
            cli_path
        )),
    }
}

/// Check CLI help command works
/// PDF: "Help" - "The help command displays a short summary of the commands available"
async fn check_cli_help(cli_path: &str) -> CheckResult {
    info!("Checking CLI help command...");

    let cli = UnderlyingCli::new(cli_path.to_string());

    match cli.help().await {
        Ok(result) => {
            if result.success() {
                // Check that output looks like Vantiq CLI help
                if result.stdout.contains("vantiq") || result.stdout.contains("Vantiq") {
                    CheckResult::ok("CLI Help", "CLI responds to help command").with_details(
                        "PDF Reference: 'The help command displays a short summary of the commands available in the CLI.'"
                    )
                } else {
                    CheckResult::fail("CLI Help", "Unexpected help output")
                        .with_details(result.stdout)
                }
            } else {
                CheckResult::fail(
                    "CLI Help",
                    format!("Help command failed with code {}", result.code()),
                )
                .with_details(result.stderr)
            }
        }
        Err(e) => CheckResult::fail("CLI Help", format!("Failed to run help command: {}", e)),
    }
}

/// Check connection to Vantiq server
/// Uses the default profile or prompts for credentials
async fn check_connection(cli_path: &str) -> CheckResult {
    info!("Checking connection to Vantiq server...");

    // Try to load profile manager
    let profile_manager = match ProfileManager::new() {
        Ok(pm) => pm,
        Err(e) => {
            return CheckResult::fail("Connection", format!("Could not load profiles: {}", e));
        }
    };

    // Get default profile
    let profile = match profile_manager.get_default_resolved() {
        Ok(p) => p,
        Err(_) => {
            return CheckResult::fail(
                "Connection",
                "No default profile configured. Run 'vqx profile init' to create one.",
            );
        }
    };

    if !profile.has_auth() {
        return CheckResult::fail(
            "Connection",
            "Default profile has no authentication configured.",
        )
        .with_details(
            "PDF Reference: Profile section - Use either:\n\
             - token option for public clouds (recommended)\n\
             - username/password for Edge servers only",
        );
    }

    let cli = UnderlyingCli::new(cli_path.to_string());
    let options = CliOptions::from_profile(&profile);

    // Try to run a simple command that requires authentication
    // PDF: Example shows "vantiq -s personal execute Utils.getNamespaceAndProfiles"
    match cli
        .run_procedure(&options, "Utils.getNamespaceAndProfiles", &[])
        .await
    {
        Ok(result) => {
            if result.success() {
                CheckResult::ok(
                    "Connection",
                    format!("Connected to {} as authenticated user", profile.url),
                )
                .with_details(format!(
                    "Auth type: {}\nResponse: {}",
                    profile.auth_type(),
                    &result.stdout[..result.stdout.len().min(200)]
                ))
            } else {
                CheckResult::fail(
                    "Connection",
                    format!("Authentication failed: {}", result.stderr),
                )
                .with_details(format!(
                    "URL: {}\nAuth type: {}\n\nPDF Reference: Profile section notes:\n\
                     - public clouds require use of the token option\n\
                     - username/password can only be used for Edge servers",
                    profile.url,
                    profile.auth_type()
                ))
            }
        }
        Err(e) => CheckResult::fail("Connection", format!("Connection test failed: {}", e)),
    }
}

/// Display check results to the user
pub fn display_results(results: &[CheckResult], verbose: bool) {
    println!();
    println!("{}", style("vqx Doctor").bold().cyan());
    println!("{}", style("═".repeat(40)).dim());
    println!();

    let mut all_passed = true;

    for result in results {
        let emoji = if result.passed { CHECK } else { CROSS };
        let status_style = if result.passed {
            style(&result.message).green()
        } else {
            all_passed = false;
            style(&result.message).red()
        };

        println!("{} {}: {}", emoji, style(&result.name).bold(), status_style);

        if verbose || !result.passed {
            if let Some(ref details) = result.details {
                for line in details.lines() {
                    println!("    {}", style(line).dim());
                }
            }
        }
        println!();
    }

    println!("{}", style("═".repeat(40)).dim());

    if all_passed {
        println!("{} {}", CHECK, style("All checks passed!").green().bold());
    } else {
        println!(
            "{} {}",
            CROSS,
            style("Some checks failed. See details above.").red().bold()
        );
    }
    println!();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_java_version_modern() {
        assert_eq!(parse_java_major_version("11.0.12"), Some(11));
        assert_eq!(parse_java_major_version("17.0.1"), Some(17));
        assert_eq!(parse_java_major_version("21"), Some(21));
    }

    #[test]
    fn test_parse_java_version_legacy() {
        assert_eq!(parse_java_major_version("1.8.0_301"), Some(8));
        assert_eq!(parse_java_major_version("1.7.0"), Some(7));
    }

    #[test]
    fn test_check_result_ok() {
        let result = CheckResult::ok("Test", "All good");
        assert!(result.passed);
        assert_eq!(result.name, "Test");
    }

    #[test]
    fn test_check_result_fail() {
        let result = CheckResult::fail("Test", "Something wrong");
        assert!(!result.passed);
    }
}
