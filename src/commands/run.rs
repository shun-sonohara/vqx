//! Run command implementation
//!
//! Provides commands to run tests, test suites, and procedures on Vantiq.
//! Based on CLI Reference Guide "Run" section.

use crate::cli::{OutputFormat, RunCommands, RunProcedureArgs, RunTestArgs, RunTestSuiteArgs};
use crate::config::Config;
use crate::error::Result;
use crate::profile::ProfileManager;
use crate::underlying::{CliOptions, UnderlyingCli};
use console::style;
use serde::Serialize;
use tracing::info;

/// Result of a run operation
#[derive(Debug, Serialize)]
pub struct RunResult {
    pub success: bool,
    pub command_type: String,
    pub name: String,
    pub output: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// Run a test, test suite, or procedure
pub async fn run(
    cmd: &RunCommands,
    config: &Config,
    profile_name: Option<&str>,
    output_format: OutputFormat,
    verbose: bool,
) -> Result<RunResult> {
    match cmd {
        RunCommands::Test(args) => {
            run_test(args, config, profile_name, output_format, verbose).await
        }
        RunCommands::TestSuite(args) => {
            run_testsuite(args, config, profile_name, output_format, verbose).await
        }
        RunCommands::Procedure(args) => {
            run_procedure(args, config, profile_name, output_format, verbose).await
        }
    }
}

/// Run a single test
async fn run_test(
    args: &RunTestArgs,
    config: &Config,
    profile_name: Option<&str>,
    output_format: OutputFormat,
    verbose: bool,
) -> Result<RunResult> {
    info!(test = %args.name, "Running test");

    let cli = UnderlyingCli::new(config.cli_path.clone())
        .with_timeout(config.timeout())
        .with_retries(config.max_retries, config.retry_delay_ms);

    let options = build_cli_options(profile_name)?;

    if verbose {
        println!();
        println!("{}", style("Running Test").bold().cyan());
        println!("{}", style("─".repeat(40)).dim());
        println!("Test: {}", style(&args.name).yellow());
        println!();
    }

    let exec_result = cli.run_test(&options, &args.name).await?;

    let result = RunResult {
        success: exec_result.success(),
        command_type: "test".to_string(),
        name: args.name.clone(),
        output: exec_result.stdout.clone(),
        error: if exec_result.success() {
            None
        } else {
            Some(exec_result.stderr.clone())
        },
    };

    display_result(&result, output_format, verbose);
    Ok(result)
}

/// Run a test suite
async fn run_testsuite(
    args: &RunTestSuiteArgs,
    config: &Config,
    profile_name: Option<&str>,
    output_format: OutputFormat,
    verbose: bool,
) -> Result<RunResult> {
    info!(
        testsuite = %args.name,
        start_from = ?args.start_from,
        "Running test suite"
    );

    let cli = UnderlyingCli::new(config.cli_path.clone())
        .with_timeout(config.timeout())
        .with_retries(config.max_retries, config.retry_delay_ms);

    let options = build_cli_options(profile_name)?;

    if verbose {
        println!();
        println!("{}", style("Running Test Suite").bold().cyan());
        println!("{}", style("─".repeat(40)).dim());
        println!("Test Suite: {}", style(&args.name).yellow());
        if let Some(ref start) = args.start_from {
            println!("Start from: {}", style(start).dim());
        }
        println!();
    }

    let exec_result = cli
        .run_testsuite(&options, &args.name, args.start_from.as_deref())
        .await?;

    let result = RunResult {
        success: exec_result.success(),
        command_type: "testsuite".to_string(),
        name: args.name.clone(),
        output: exec_result.stdout.clone(),
        error: if exec_result.success() {
            None
        } else {
            Some(exec_result.stderr.clone())
        },
    };

    display_result(&result, output_format, verbose);
    Ok(result)
}

/// Run a procedure
async fn run_procedure(
    args: &RunProcedureArgs,
    config: &Config,
    profile_name: Option<&str>,
    output_format: OutputFormat,
    verbose: bool,
) -> Result<RunResult> {
    info!(
        procedure = %args.name,
        params = ?args.params,
        "Running procedure"
    );

    let cli = UnderlyingCli::new(config.cli_path.clone())
        .with_timeout(config.timeout())
        .with_retries(config.max_retries, config.retry_delay_ms);

    let options = build_cli_options(profile_name)?;

    // Parse parameters from "name:value" format
    let params: Vec<(&str, &str)> = args
        .params
        .iter()
        .filter_map(|p| {
            let parts: Vec<&str> = p.splitn(2, ':').collect();
            if parts.len() == 2 {
                Some((parts[0], parts[1]))
            } else {
                None
            }
        })
        .collect();

    if verbose {
        println!();
        println!("{}", style("Running Procedure").bold().cyan());
        println!("{}", style("─".repeat(40)).dim());
        println!("Procedure: {}", style(&args.name).yellow());
        if !params.is_empty() {
            println!("Parameters:");
            for (name, value) in &params {
                println!("  {}: {}", style(name).dim(), value);
            }
        }
        println!();
    }

    let exec_result = cli.run_procedure(&options, &args.name, &params).await?;

    let result = RunResult {
        success: exec_result.success(),
        command_type: "procedure".to_string(),
        name: args.name.clone(),
        output: exec_result.stdout.clone(),
        error: if exec_result.success() {
            None
        } else {
            Some(exec_result.stderr.clone())
        },
    };

    display_result(&result, output_format, verbose);
    Ok(result)
}

/// Build CLI options from profile
fn build_cli_options(profile_name: Option<&str>) -> Result<CliOptions> {
    if let Some(name) = profile_name {
        let manager = ProfileManager::new()?;
        let profile = manager.get_resolved(name)?;
        Ok(CliOptions::from_profile(&profile))
    } else {
        Ok(CliOptions::default())
    }
}

/// Display the run result
fn display_result(result: &RunResult, output_format: OutputFormat, verbose: bool) {
    match output_format {
        OutputFormat::Json => {
            if let Ok(json) = serde_json::to_string_pretty(result) {
                println!("{}", json);
            }
        }
        OutputFormat::Text | OutputFormat::Csv => {
            // Print output from the command
            if !result.output.is_empty() {
                print!("{}", result.output);
            }

            if verbose {
                println!();
                println!("{}", style("─".repeat(40)).dim());
            }

            // Print status
            if result.success {
                println!(
                    "{} {} '{}' completed successfully",
                    style("✓").green().bold(),
                    result.command_type,
                    result.name
                );
            } else {
                println!(
                    "{} {} '{}' failed",
                    style("✗").red().bold(),
                    result.command_type,
                    result.name
                );
                if let Some(ref err) = result.error {
                    if !err.is_empty() {
                        eprintln!("{}", style(err).red());
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_params() {
        let params = vec!["name:value".to_string(), "foo:bar".to_string()];
        let parsed: Vec<(&str, &str)> = params
            .iter()
            .filter_map(|p| {
                let parts: Vec<&str> = p.splitn(2, ':').collect();
                if parts.len() == 2 {
                    Some((parts[0], parts[1]))
                } else {
                    None
                }
            })
            .collect();

        assert_eq!(parsed.len(), 2);
        assert_eq!(parsed[0], ("name", "value"));
        assert_eq!(parsed[1], ("foo", "bar"));
    }

    #[test]
    fn test_parse_params_with_colon_in_value() {
        let params = vec!["url:http://example.com:8080".to_string()];
        let parsed: Vec<(&str, &str)> = params
            .iter()
            .filter_map(|p| {
                let parts: Vec<&str> = p.splitn(2, ':').collect();
                if parts.len() == 2 {
                    Some((parts[0], parts[1]))
                } else {
                    None
                }
            })
            .collect();

        assert_eq!(parsed.len(), 1);
        assert_eq!(parsed[0], ("url", "http://example.com:8080"));
    }
}
