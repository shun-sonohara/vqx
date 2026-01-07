//! Global configuration for vqx
//!
//! This module manages vqx-specific configuration that extends beyond
//! the underlying CLI's profile system.

use crate::error::{Result, VqxError};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;
use tracing::{debug, info};

const CONFIG_DIR_NAME: &str = "vqx";
const CONFIG_FILE: &str = "config.toml";

/// Global vqx configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    /// Path to the underlying Vantiq CLI executable
    /// PDF: Default is "vantiq" (Mac/Linux) or "vantiq.bat" (Windows)
    #[serde(default = "default_cli_path")]
    pub cli_path: String,

    /// Default timeout for CLI operations in seconds
    #[serde(default = "default_timeout")]
    pub timeout_seconds: u64,

    /// Maximum number of retries for transient failures
    #[serde(default = "default_max_retries")]
    pub max_retries: u32,

    /// Base delay for exponential backoff (milliseconds)
    #[serde(default = "default_retry_delay")]
    pub retry_delay_ms: u64,

    /// Default chunk size for export/import operations
    /// PDF: "-chunk <integer>" option
    #[serde(default = "default_chunk_size")]
    pub default_chunk_size: u32,

    /// Logging configuration
    #[serde(default)]
    pub logging: LoggingConfig,

    /// Output format preferences
    #[serde(default)]
    pub output: OutputConfig,

    /// Safe delete configuration
    #[serde(default)]
    pub safe_delete: SafeDeleteConfig,

    /// Normalization settings for JSON output
    #[serde(default)]
    pub normalization: NormalizationConfig,
}

fn default_cli_path() -> String {
    if cfg!(windows) {
        "vantiq.bat".to_string()
    } else {
        "vantiq".to_string()
    }
}

fn default_timeout() -> u64 {
    120
}

fn default_max_retries() -> u32 {
    3
}

fn default_retry_delay() -> u64 {
    1000
}

fn default_chunk_size() -> u32 {
    5000
}

impl Default for Config {
    fn default() -> Self {
        Self {
            cli_path: default_cli_path(),
            timeout_seconds: default_timeout(),
            max_retries: default_max_retries(),
            retry_delay_ms: default_retry_delay(),
            default_chunk_size: default_chunk_size(),
            logging: LoggingConfig::default(),
            output: OutputConfig::default(),
            safe_delete: SafeDeleteConfig::default(),
            normalization: NormalizationConfig::default(),
        }
    }
}

impl Config {
    /// Get the config directory path
    /// Uses ~/.config/vqx on Unix (macOS/Linux) for consistency with documentation
    /// Uses %APPDATA%\vqx on Windows
    pub fn config_dir() -> Result<PathBuf> {
        // On Unix systems (macOS/Linux), use ~/.config/vqx for XDG-style config
        // This matches the documentation and is more familiar to CLI users
        #[cfg(unix)]
        {
            let home = dirs::home_dir()
                .ok_or_else(|| VqxError::Other("Could not determine home directory".to_string()))?;
            Ok(home.join(".config").join(CONFIG_DIR_NAME))
        }

        // On Windows, use the standard AppData location
        #[cfg(windows)]
        {
            if let Some(proj_dirs) = ProjectDirs::from("", "", CONFIG_DIR_NAME) {
                Ok(proj_dirs.config_dir().to_path_buf())
            } else {
                let home = dirs::home_dir().ok_or_else(|| {
                    VqxError::Other("Could not determine home directory".to_string())
                })?;
                Ok(home.join(format!(".{}", CONFIG_DIR_NAME)))
            }
        }
    }

    /// Get the config file path
    pub fn config_file_path() -> Result<PathBuf> {
        Ok(Self::config_dir()?.join(CONFIG_FILE))
    }

    /// Load config from the default location
    pub fn load() -> Result<Self> {
        let path = Self::config_file_path()?;
        Self::load_from(&path)
    }

    /// Load config from a specific file
    pub fn load_from(path: &Path) -> Result<Self> {
        if !path.exists() {
            debug!(path = %path.display(), "Config file not found, using defaults");
            return Ok(Self::default());
        }

        let content = fs::read_to_string(path).map_err(|_| VqxError::FileReadFailed {
            path: path.display().to_string(),
        })?;

        let config: Self = toml::from_str(&content)?;
        info!(path = %path.display(), "Loaded configuration");
        Ok(config)
    }

    /// Save config to the default location
    pub fn save(&self) -> Result<()> {
        let path = Self::config_file_path()?;
        self.save_to(&path)
    }

    /// Save config to a specific file
    pub fn save_to(&self, path: &Path) -> Result<()> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|_| VqxError::FileWriteFailed {
                path: parent.display().to_string(),
            })?;
        }

        let content = toml::to_string_pretty(self).map_err(|e| VqxError::InvalidToml {
            message: e.to_string(),
        })?;

        fs::write(path, content).map_err(|_| VqxError::FileWriteFailed {
            path: path.display().to_string(),
        })?;

        info!(path = %path.display(), "Saved configuration");
        Ok(())
    }

    /// Get timeout as Duration
    pub fn timeout(&self) -> Duration {
        Duration::from_secs(self.timeout_seconds)
    }

    /// Get retry delay as Duration
    pub fn retry_delay(&self) -> Duration {
        Duration::from_millis(self.retry_delay_ms)
    }
}

/// Logging configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoggingConfig {
    /// Log level: trace, debug, info, warn, error
    #[serde(default = "default_log_level")]
    pub level: String,

    /// Log format: text, json
    #[serde(default = "default_log_format")]
    pub format: String,

    /// Include timestamps in logs
    #[serde(default = "default_true")]
    pub timestamps: bool,

    /// Log file path (optional)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub file: Option<String>,
}

fn default_log_level() -> String {
    "info".to_string()
}

fn default_log_format() -> String {
    "text".to_string()
}

fn default_true() -> bool {
    true
}

impl Default for LoggingConfig {
    fn default() -> Self {
        Self {
            level: default_log_level(),
            format: default_log_format(),
            timestamps: true,
            file: None,
        }
    }
}

/// Output format configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OutputConfig {
    /// Default output format: json, table, csv
    #[serde(default = "default_output_format")]
    pub default_format: String,

    /// Pretty print JSON output
    #[serde(default = "default_true")]
    pub pretty_json: bool,

    /// Use colors in output
    #[serde(default = "default_true")]
    pub colors: bool,

    /// Show progress bars for long operations
    #[serde(default = "default_true")]
    pub progress: bool,
}

fn default_output_format() -> String {
    "table".to_string()
}

impl Default for OutputConfig {
    fn default() -> Self {
        Self {
            default_format: default_output_format(),
            pretty_json: true,
            colors: true,
            progress: true,
        }
    }
}

/// Safe delete configuration
/// Extension: Wraps PDF's delete/deleteMatching with safety measures
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SafeDeleteConfig {
    /// Always require confirmation for destructive operations
    #[serde(default = "default_true")]
    pub require_confirm: bool,

    /// Always create backup before delete
    #[serde(default = "default_true")]
    pub require_backup: bool,

    /// Maximum items to delete without explicit --force
    #[serde(default = "default_max_delete")]
    pub max_items_without_force: u32,

    /// Directory for backups
    #[serde(skip_serializing_if = "Option::is_none")]
    pub backup_dir: Option<String>,

    /// Allowlist of resource prefixes that can be deleted
    #[serde(default)]
    pub allowed_prefixes: Vec<String>,

    /// Blocklist of resource prefixes that cannot be deleted
    #[serde(default)]
    pub blocked_prefixes: Vec<String>,
}

fn default_max_delete() -> u32 {
    10
}

impl Default for SafeDeleteConfig {
    fn default() -> Self {
        Self {
            require_confirm: true,
            require_backup: true,
            max_items_without_force: default_max_delete(),
            backup_dir: None,
            allowed_prefixes: vec![],
            blocked_prefixes: vec!["System".to_string(), "ARS".to_string()], // Common system prefixes
        }
    }
}

/// JSON normalization settings for diff operations
/// Extension: Normalizes CLI output for git-friendly diffs
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NormalizationConfig {
    /// Sort object keys alphabetically
    #[serde(default = "default_true")]
    pub sort_keys: bool,

    /// Sort arrays by name/id field
    #[serde(default = "default_true")]
    pub sort_arrays: bool,

    /// Fields to exclude from normalized output (timestamps, etc.)
    #[serde(default = "default_excluded_fields")]
    pub excluded_fields: Vec<String>,

    /// Field to use for array sorting (fallback order: name, id, _id)
    #[serde(default = "default_sort_fields")]
    pub array_sort_fields: Vec<String>,
}

fn default_excluded_fields() -> Vec<String> {
    vec![
        "ars_modifiedAt".to_string(),
        "ars_createdAt".to_string(),
        "ars_modifiedBy".to_string(),
        "ars_createdBy".to_string(),
        "_id".to_string(),
        "ars_version".to_string(),
    ]
}

fn default_sort_fields() -> Vec<String> {
    vec!["name".to_string(), "id".to_string(), "_id".to_string()]
}

impl Default for NormalizationConfig {
    fn default() -> Self {
        Self {
            sort_keys: true,
            sort_arrays: true,
            excluded_fields: default_excluded_fields(),
            array_sort_fields: default_sort_fields(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_default() {
        let config = Config::default();
        assert_eq!(config.timeout_seconds, 120);
        assert_eq!(config.max_retries, 3);
        assert!(config.safe_delete.require_confirm);
    }

    #[test]
    fn test_config_roundtrip() {
        let config = Config::default();
        let toml = toml::to_string_pretty(&config).unwrap();
        let loaded: Config = toml::from_str(&toml).unwrap();

        assert_eq!(config.cli_path, loaded.cli_path);
        assert_eq!(config.timeout_seconds, loaded.timeout_seconds);
    }

    #[test]
    fn test_normalization_config() {
        let config = NormalizationConfig::default();
        assert!(config.sort_keys);
        assert!(config
            .excluded_fields
            .contains(&"ars_modifiedAt".to_string()));
    }
}
