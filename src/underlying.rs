//! Underlying CLI execution layer
//!
//! This module provides a single point of access for executing the Vantiq CLI.
//! All CLI invocations go through this layer for consistency, logging, and error handling.
//!
//! Based on: CLI Reference Guide
//! - "Command Line Options" section (page 3)
//! - "Installation" section (page 2)

use crate::error::{Result, VqxError};
use crate::profile::Profile;
use std::ffi::OsStr;
use std::process::{ExitStatus, Stdio};
use std::time::Duration;
use tokio::process::Command;
use tokio::time::timeout;
use tracing::{debug, info, warn};

/// Default timeout for CLI operations (2 minutes)
const DEFAULT_TIMEOUT_SECS: u64 = 120;

/// Result of a CLI execution
#[derive(Debug, Clone)]
pub struct ExecResult {
    pub status: ExitStatus,
    pub stdout: String,
    pub stderr: String,
}

impl ExecResult {
    /// Check if the command succeeded
    pub fn success(&self) -> bool {
        self.status.success()
    }

    /// Get exit code (0 if unavailable)
    pub fn code(&self) -> i32 {
        self.status.code().unwrap_or(-1)
    }
}

/// CLI command line options as defined in PDF "Command Line Options" section
///
/// PDF Reference:
/// - `-s <profileName>` : Profile name (default: base)
/// - `-b <baseURL>` : Base URL (default: https://dev.vantiq.com)
/// - `-u <username>` : Username
/// - `-p <password>` : Password
/// - `-t <token>` : Access token (password takes precedence if both specified)
/// - `-n <namespace>` : Target namespace (only works with username/password, not token)
/// - `-trust` : Trust SSL certificates
/// - `-f <profileFile>` : Profile file path
/// - `-v` : Print version
#[derive(Debug, Clone, Default)]
pub struct CliOptions {
    /// -s <profileName> : Profile name from underlying CLI's profile file
    /// PDF: "Specify the name of a profile, stored in: ~/.vantiq/profile"
    pub underlying_profile: Option<String>,

    /// -b <baseURL> : Base URL
    /// PDF: "Default: https://dev.vantiq.com"
    pub base_url: Option<String>,

    /// -u <username> : Username
    pub username: Option<String>,

    /// -p <password> : Password
    /// PDF: "If a password is specified, it is used instead of the token."
    pub password: Option<String>,

    /// -t <token> : Access token
    /// PDF: "public clouds and any server using keycloak access require use of the token option"
    pub token: Option<String>,

    /// -n <namespace> : Target namespace
    /// PDF: "This option will not work when using a long-lived access token.
    ///       It only works with username/password credentials."
    pub namespace: Option<String>,

    /// -trust : Trust SSL certificates
    /// PDF: "Force SSL options to trust remote server certificate and host name"
    pub trust_ssl: bool,

    /// -f <profileFile> : Custom profile file path
    pub profile_file: Option<String>,

    /// -v : Verbose/version flag
    pub verbose: bool,
}

impl CliOptions {
    /// Create CliOptions from a vqx Profile
    pub fn from_profile(profile: &Profile) -> Self {
        Self {
            underlying_profile: None, // We don't use underlying profile when we have credentials
            base_url: Some(profile.url.clone()),
            username: profile.username.clone(),
            password: profile.password.clone(),
            token: profile.token.clone(),
            namespace: profile.namespace.clone(),
            trust_ssl: profile.trust_ssl,
            profile_file: None,
            verbose: false,
        }
    }

    /// Validate options according to PDF constraints
    pub fn validate(&self) -> Result<()> {
        // PDF: "the namespace option can only be used with username/password;
        //       it cannot be used with long-lived access tokens."
        if self.namespace.is_some() && self.token.is_some() && self.password.is_none() {
            return Err(VqxError::NamespaceWithToken);
        }
        Ok(())
    }

    /// Convert to command line arguments
    /// Based on PDF "Command Line Options" section
    fn to_args(&self) -> Vec<String> {
        let mut args = Vec::new();

        // -s <profileName>
        if let Some(ref profile) = self.underlying_profile {
            args.push("-s".to_string());
            args.push(profile.clone());
        }

        // -b <baseURL>
        if let Some(ref url) = self.base_url {
            args.push("-b".to_string());
            args.push(url.clone());
        }

        // -u <username>
        if let Some(ref username) = self.username {
            args.push("-u".to_string());
            args.push(username.clone());
        }

        // -p <password>
        if let Some(ref password) = self.password {
            args.push("-p".to_string());
            args.push(password.clone());
        }

        // -t <token> (only if no password, since password takes precedence per PDF)
        if self.password.is_none() {
            if let Some(ref token) = self.token {
                args.push("-t".to_string());
                args.push(token.clone());
            }
        }

        // -n <namespace>
        if let Some(ref ns) = self.namespace {
            args.push("-n".to_string());
            args.push(ns.clone());
        }

        // -trust
        if self.trust_ssl {
            args.push("-trust".to_string());
        }

        // -f <profileFile>
        if let Some(ref file) = self.profile_file {
            args.push("-f".to_string());
            args.push(file.clone());
        }

        // -v (for version/verbose)
        if self.verbose {
            args.push("-v".to_string());
        }

        args
    }

    /// Create a masked version of args for logging (hide secrets)
    fn to_masked_args(&self) -> Vec<String> {
        let mut args = Vec::new();

        if let Some(ref profile) = self.underlying_profile {
            args.push("-s".to_string());
            args.push(profile.clone());
        }

        if let Some(ref url) = self.base_url {
            args.push("-b".to_string());
            args.push(url.clone());
        }

        if let Some(ref username) = self.username {
            args.push("-u".to_string());
            args.push(username.clone());
        }

        // Mask password
        if self.password.is_some() {
            args.push("-p".to_string());
            args.push("********".to_string());
        }

        // Mask token
        if self.password.is_none() && self.token.is_some() {
            args.push("-t".to_string());
            args.push("********".to_string());
        }

        if let Some(ref ns) = self.namespace {
            args.push("-n".to_string());
            args.push(ns.clone());
        }

        if self.trust_ssl {
            args.push("-trust".to_string());
        }

        if let Some(ref file) = self.profile_file {
            args.push("-f".to_string());
            args.push(file.clone());
        }

        if self.verbose {
            args.push("-v".to_string());
        }

        args
    }
}

/// The underlying Vantiq CLI executor
///
/// This struct encapsulates all interactions with the Vantiq CLI binary.
/// Based on PDF "Installation" section:
/// - Mac/Linux: `vantiq <command>`
/// - Windows: `vantiq.bat <command>`
pub struct UnderlyingCli {
    /// Path to the CLI executable
    /// PDF: "vantiq-x.x.x/bin" should be in PATH
    cli_path: String,

    /// Default timeout for operations
    timeout: Duration,

    /// Retry configuration
    max_retries: u32,

    /// Base delay for exponential backoff (milliseconds)
    retry_base_delay_ms: u64,
}

impl UnderlyingCli {
    /// Create a new CLI executor with the specified path
    pub fn new(cli_path: String) -> Self {
        Self {
            cli_path,
            timeout: Duration::from_secs(DEFAULT_TIMEOUT_SECS),
            max_retries: 3,
            retry_base_delay_ms: 1000,
        }
    }

    /// Create with default CLI name based on platform
    /// PDF: "vantiq" for Mac/Linux, "vantiq.bat" for Windows
    pub fn with_default_path() -> Self {
        let cli_name = if cfg!(windows) {
            "vantiq.bat"
        } else {
            "vantiq"
        };
        Self::new(cli_name.to_string())
    }

    /// Set timeout
    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    /// Set retry configuration
    pub fn with_retries(mut self, max_retries: u32, base_delay_ms: u64) -> Self {
        self.max_retries = max_retries;
        self.retry_base_delay_ms = base_delay_ms;
        self
    }

    /// Get the CLI path
    pub fn cli_path(&self) -> &str {
        &self.cli_path
    }

    /// Check if CLI exists and is executable
    pub fn check_cli_exists(&self) -> Result<String> {
        match which::which(&self.cli_path) {
            Ok(path) => Ok(path.to_string_lossy().to_string()),
            Err(_) => Err(VqxError::CliNotFound {
                path: self.cli_path.clone(),
            }),
        }
    }

    /// Execute a CLI command with options
    ///
    /// This is the main entry point for all CLI operations.
    /// Handles:
    /// - Option validation
    /// - Argument construction (based on PDF "Command Line Options")
    /// - Timeout handling
    /// - Logging with masked secrets
    pub async fn execute<I, S>(
        &self,
        options: &CliOptions,
        command: &str,
        args: I,
    ) -> Result<ExecResult>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<OsStr>,
    {
        // Validate options according to PDF constraints
        options.validate()?;

        let command_args: Vec<String> = args
            .into_iter()
            .map(|s| s.as_ref().to_string_lossy().to_string())
            .collect();

        // Build full argument list: [options] [command] [command_args]
        let mut full_args = options.to_args();
        full_args.push(command.to_string());
        full_args.extend(command_args.clone());

        // Log with masked secrets
        let masked_args = options.to_masked_args();
        info!(
            cli = %self.cli_path,
            command = %command,
            options = ?masked_args,
            args = ?command_args,
            "Executing CLI command"
        );

        self.execute_raw(&full_args).await
    }

    /// Execute CLI with raw arguments (no option processing)
    /// Used for passthrough mode
    pub async fn execute_raw<I, S>(&self, args: I) -> Result<ExecResult>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<OsStr>,
    {
        let args: Vec<String> = args
            .into_iter()
            .map(|s| s.as_ref().to_string_lossy().to_string())
            .collect();

        debug!(cli = %self.cli_path, args = ?args, "Executing raw CLI command");

        let mut cmd = Command::new(&self.cli_path);
        cmd.args(&args)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        let result = timeout(self.timeout, cmd.output()).await;

        match result {
            Ok(Ok(output)) => {
                let stdout = String::from_utf8_lossy(&output.stdout).to_string();
                let stderr = String::from_utf8_lossy(&output.stderr).to_string();

                debug!(
                    status = ?output.status,
                    stdout_len = stdout.len(),
                    stderr_len = stderr.len(),
                    "CLI command completed"
                );

                if !output.status.success() {
                    warn!(
                        code = output.status.code(),
                        stderr = %stderr,
                        "CLI command failed"
                    );
                }

                Ok(ExecResult {
                    status: output.status,
                    stdout,
                    stderr,
                })
            }
            Ok(Err(e)) => {
                warn!(error = %e, "Failed to spawn CLI process");
                Err(VqxError::CliSpawnFailed {
                    message: e.to_string(),
                })
            }
            Err(_) => {
                warn!(
                    timeout_secs = self.timeout.as_secs(),
                    "CLI command timed out"
                );
                Err(VqxError::CliTimeout {
                    seconds: self.timeout.as_secs(),
                })
            }
        }
    }

    /// Execute with retry and exponential backoff
    pub async fn execute_with_retry<I, S>(
        &self,
        options: &CliOptions,
        command: &str,
        args: I,
    ) -> Result<ExecResult>
    where
        I: IntoIterator<Item = S> + Clone,
        S: AsRef<OsStr> + Clone,
    {
        let mut last_error = None;
        let args_vec: Vec<S> = args.into_iter().collect();

        for attempt in 0..=self.max_retries {
            if attempt > 0 {
                let delay = self.retry_base_delay_ms * 2u64.pow(attempt - 1);
                info!(attempt, delay_ms = delay, "Retrying CLI command");
                tokio::time::sleep(Duration::from_millis(delay)).await;
            }

            match self.execute(options, command, args_vec.clone()).await {
                Ok(result) if result.success() => return Ok(result),
                Ok(result) => {
                    // Command executed but returned non-zero
                    // Don't retry for logical errors, only for transient failures
                    if Self::is_transient_error(&result) {
                        last_error = Some(VqxError::CliExecutionFailed {
                            code: result.code(),
                            message: result.stderr.clone(),
                        });
                        continue;
                    }
                    return Ok(result);
                }
                Err(e) => {
                    if Self::is_retryable_error(&e) {
                        last_error = Some(e);
                        continue;
                    }
                    return Err(e);
                }
            }
        }

        Err(last_error.unwrap_or_else(|| VqxError::Other("Max retries exceeded".to_string())))
    }

    /// Check if an error is retryable
    fn is_retryable_error(e: &VqxError) -> bool {
        matches!(e, VqxError::CliTimeout { .. })
    }

    /// Check if a CLI result indicates a transient error
    fn is_transient_error(result: &ExecResult) -> bool {
        // Check for network-related errors in stderr
        let stderr_lower = result.stderr.to_lowercase();
        stderr_lower.contains("connection")
            || stderr_lower.contains("timeout")
            || stderr_lower.contains("network")
    }

    // =========================================================================
    // Convenience methods for specific CLI commands
    // Based on PDF "Supported Commands" section
    // =========================================================================

    /// Execute `help` command
    /// PDF: "The help command displays a short summary of the commands available in the CLI."
    pub async fn help(&self) -> Result<ExecResult> {
        self.execute_raw(["help"]).await
    }

    /// Execute with `-v` flag to get version
    /// PDF: "Prints the CLI version and the URL for the connected Vantiq service."
    pub async fn version(&self, options: &CliOptions) -> Result<ExecResult> {
        let mut opts = options.clone();
        opts.verbose = true;
        // Execute a simple command that will print version info
        self.execute(&opts, "help", Vec::<String>::new()).await
    }

    /// Execute `list` command
    /// PDF: "The list command displays a list of all resources of the type specified"
    pub async fn list(&self, options: &CliOptions, resource: &str) -> Result<ExecResult> {
        self.execute(options, "list", [resource]).await
    }

    /// Execute `find` command
    /// PDF: "The find command finds an individual instance of a resource by name or query"
    pub async fn find(
        &self,
        options: &CliOptions,
        resource: &str,
        resource_id: &str,
    ) -> Result<ExecResult> {
        self.execute(options, "find", [resource, resource_id]).await
    }

    /// Execute `select` command
    /// PDF: "The select command is a convenience to allow you to retrieve data from the Vantiq database"
    pub async fn select(
        &self,
        options: &CliOptions,
        resource: &str,
        resource_id: Option<&str>,
        qual_file: Option<&str>,
        props: Option<&str>,
        chunk_size: Option<u32>,
    ) -> Result<ExecResult> {
        let mut args = vec![resource.to_string()];

        if let Some(id) = resource_id {
            args.push(id.to_string());
        }

        // -qual <fileName>
        if let Some(qual) = qual_file {
            args.push("-qual".to_string());
            args.push(qual.to_string());
        }

        // -props <fileName> | <propertyList>
        if let Some(p) = props {
            args.push("-props".to_string());
            args.push(p.to_string());
        }

        // -chunk <size>
        if let Some(size) = chunk_size {
            args.push("-chunk".to_string());
            args.push(size.to_string());
        }

        self.execute(options, "select", args).await
    }

    /// Execute `export` command
    /// PDF: "The export command writes either the resource meta-data or data stored in user defined types"
    pub async fn export(
        &self,
        options: &CliOptions,
        export_type: Option<&str>, // "data", "metadata", "project <name>", "projectdata <name>", "hidden"
        directory: Option<&str>,
        chunk_size: Option<u32>,
        include: Option<&[&str]>,
        exclude: Option<&[&str]>,
        until: Option<&str>,
        ignore_errors: bool,
    ) -> Result<ExecResult> {
        let mut args: Vec<String> = Vec::new();

        // Export type
        if let Some(t) = export_type {
            args.extend(t.split_whitespace().map(String::from));
        }

        // -d <directory>
        if let Some(dir) = directory {
            args.push("-d".to_string());
            args.push(dir.to_string());
        }

        // -chunk <size>
        if let Some(size) = chunk_size {
            args.push("-chunk".to_string());
            args.push(size.to_string());
        }

        // -include <typeName>
        if let Some(includes) = include {
            for inc in includes {
                args.push("-include".to_string());
                args.push(inc.to_string());
            }
        }

        // -exclude <typeName>
        if let Some(excludes) = exclude {
            for exc in excludes {
                args.push("-exclude".to_string());
                args.push(exc.to_string());
            }
        }

        // -until <DateTime>
        if let Some(u) = until {
            args.push("-until".to_string());
            args.push(u.to_string());
        }

        // -ignoreErrors
        if ignore_errors {
            args.push("-ignoreErrors".to_string());
        }

        self.execute(options, "export", args).await
    }

    /// Execute `import` command
    /// PDF: "The import command reads all artifact definitions stored in a directory"
    pub async fn import(
        &self,
        options: &CliOptions,
        import_type: Option<&str>, // "data" or "metadata"
        directory: Option<&str>,
        chunk_size: Option<u32>,
        include: Option<&[&str]>,
        exclude: Option<&[&str]>,
        ignore: Option<&[&str]>,
    ) -> Result<ExecResult> {
        let mut args: Vec<String> = Vec::new();

        // Import type
        if let Some(t) = import_type {
            args.push(t.to_string());
        }

        // -d <directory>
        if let Some(dir) = directory {
            args.push("-d".to_string());
            args.push(dir.to_string());
        }

        // -chunk <size>
        if let Some(size) = chunk_size {
            args.push("-chunk".to_string());
            args.push(size.to_string());
        }

        // -include <typeName>
        if let Some(includes) = include {
            for inc in includes {
                args.push("-include".to_string());
                args.push(inc.to_string());
            }
        }

        // -exclude <typeName>
        if let Some(excludes) = exclude {
            for exc in excludes {
                args.push("-exclude".to_string());
                args.push(exc.to_string());
            }
        }

        // -ignore <resourceType>
        if let Some(ignores) = ignore {
            for ig in ignores {
                args.push("-ignore".to_string());
                args.push(ig.to_string());
            }
        }

        self.execute(options, "import", args).await
    }

    /// Execute `delete` command
    /// PDF: "The delete command is used to delete a resource instance."
    /// WARNING: This is a destructive operation
    pub async fn delete(
        &self,
        options: &CliOptions,
        resource: &str,
        resource_id: &str,
    ) -> Result<ExecResult> {
        self.execute(options, "delete", [resource, resource_id])
            .await
    }

    /// Execute `deleteMatching` command
    /// PDF: "deleteMatching <resource> <query>"
    /// WARNING: This is a destructive operation
    pub async fn delete_matching(
        &self,
        options: &CliOptions,
        resource: &str,
        query: &str,
    ) -> Result<ExecResult> {
        self.execute(options, "deleteMatching", [resource, query])
            .await
    }

    /// Execute `run testsuite` command
    /// PDF: "When running a test suite you must supply the test suite name"
    pub async fn run_testsuite(
        &self,
        options: &CliOptions,
        testsuite_name: &str,
        start_test: Option<&str>,
    ) -> Result<ExecResult> {
        let mut args = vec!["testsuite".to_string(), testsuite_name.to_string()];
        if let Some(test) = start_test {
            args.push(test.to_string());
        }
        self.execute(options, "run", args).await
    }

    /// Execute `run procedure` command
    /// PDF: "you can run a VAIL procedure by supplying the procedure name and any parameters"
    pub async fn run_procedure(
        &self,
        options: &CliOptions,
        procedure_name: &str,
        params: &[(&str, &str)],
    ) -> Result<ExecResult> {
        let mut args = vec!["procedure".to_string(), procedure_name.to_string()];

        // PDF: "parameters are specified as <name>:<value> pairs"
        for (name, value) in params {
            args.push(format!("{}:{}", name, value));
        }

        self.execute(options, "run", args).await
    }

    /// Execute `deploy` command
    /// PDF: "deploy <configurationName> | <deploymentName>"
    pub async fn deploy(&self, options: &CliOptions, name: &str) -> Result<ExecResult> {
        self.execute(options, "deploy", [name]).await
    }

    /// Execute `undeploy` command
    /// PDF: "undeploy <configurationName> | <deploymentName>"
    /// WARNING: This is a destructive operation
    pub async fn undeploy(&self, options: &CliOptions, name: &str) -> Result<ExecResult> {
        self.execute(options, "undeploy", [name]).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cli_options_to_args() {
        let opts = CliOptions {
            base_url: Some("https://test.vantiq.com".to_string()),
            username: Some("user".to_string()),
            password: Some("pass".to_string()),
            namespace: Some("ns".to_string()),
            trust_ssl: true,
            ..Default::default()
        };

        let args = opts.to_args();
        assert!(args.contains(&"-b".to_string()));
        assert!(args.contains(&"https://test.vantiq.com".to_string()));
        assert!(args.contains(&"-u".to_string()));
        assert!(args.contains(&"-p".to_string()));
        assert!(args.contains(&"-n".to_string()));
        assert!(args.contains(&"-trust".to_string()));
    }

    #[test]
    fn test_password_takes_precedence_over_token() {
        // PDF: "If a password is specified, it is used instead of the token."
        let opts = CliOptions {
            password: Some("pass".to_string()),
            token: Some("token".to_string()),
            ..Default::default()
        };

        let args = opts.to_args();
        assert!(args.contains(&"-p".to_string()));
        assert!(!args.contains(&"-t".to_string()));
    }

    #[test]
    fn test_namespace_with_token_validation() {
        // PDF: "the namespace option can only be used with username/password;
        //       it cannot be used with long-lived access tokens."
        let opts = CliOptions {
            token: Some("token".to_string()),
            namespace: Some("ns".to_string()),
            ..Default::default()
        };

        assert!(matches!(opts.validate(), Err(VqxError::NamespaceWithToken)));
    }

    #[test]
    fn test_masked_args() {
        let opts = CliOptions {
            username: Some("user".to_string()),
            password: Some("secret_password".to_string()),
            token: Some("secret_token".to_string()),
            ..Default::default()
        };

        let masked = opts.to_masked_args();
        assert!(masked.contains(&"user".to_string()));
        assert!(masked.contains(&"********".to_string()));
        assert!(!masked.contains(&"secret_password".to_string()));
        assert!(!masked.contains(&"secret_token".to_string()));
    }
}
