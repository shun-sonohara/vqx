//! Error types for vqx
//!
//! Based on: CLI Reference Guide - Installation section (Java 11 requirement)
//! and Command Line Options section (connection errors)

use thiserror::Error;

/// Main error type for vqx operations
#[derive(Error, Debug)]
pub enum VqxError {
    // ===========================================
    // Environment / Prerequisites errors
    // Based on: PDF "Prerequisites" section - Java 11 requirement
    // ===========================================
    #[error("Java is not installed or not found in PATH. The Vantiq CLI requires Java 11.")]
    JavaNotFound,

    #[error("Java version {found} is not supported. The Vantiq CLI requires Java 11 or later.")]
    JavaVersionUnsupported { found: String },

    #[error("Vantiq CLI executable not found at: {path}")]
    CliNotFound { path: String },

    #[error("Vantiq CLI is not executable: {path}")]
    CliNotExecutable { path: String },

    // ===========================================
    // Profile errors
    // Based on: PDF "Profile" section
    // ===========================================
    #[error("Profile '{name}' not found")]
    ProfileNotFound { name: String },

    #[error("Profile file not found: {path}")]
    ProfileFileNotFound { path: String },

    #[error("Invalid profile configuration: {message}")]
    ProfileInvalid { message: String },

    #[error("Cannot use namespace option with access token. Use username/password instead. (PDF: Profile section notes)")]
    NamespaceWithToken,

    // ===========================================
    // CLI execution errors
    // ===========================================
    #[error("CLI command failed with exit code {code}: {message}")]
    CliExecutionFailed { code: i32, message: String },

    #[error("CLI command timed out after {seconds} seconds")]
    CliTimeout { seconds: u64 },

    #[error("Failed to spawn CLI process: {message}")]
    CliSpawnFailed { message: String },

    // ===========================================
    // Destructive operation safeguards
    // Based on: PDF "Delete" and "DeleteMatching" sections
    // ===========================================
    #[error("Destructive operation '{operation}' requires explicit confirmation")]
    DestructiveOperationNotConfirmed { operation: String },

    #[error("Backup required before destructive operation but failed: {message}")]
    BackupFailed { message: String },

    // ===========================================
    // I/O and configuration errors
    // ===========================================
    #[error("Failed to read file: {path}")]
    FileReadFailed { path: String },

    #[error("Failed to write file: {path}")]
    FileWriteFailed { path: String },

    #[error("Invalid JSON: {message}")]
    InvalidJson { message: String },

    #[error("Invalid TOML configuration: {message}")]
    InvalidToml { message: String },

    // ===========================================
    // Secret storage errors
    // ===========================================
    #[error("Failed to access secure storage: {message}")]
    SecretStorageFailed { message: String },

    #[error("Failed to encrypt/decrypt credentials: {message}")]
    EncryptionFailed { message: String },

    // ===========================================
    // Generic errors
    // ===========================================
    #[error("{0}")]
    Other(String),
}

impl From<std::io::Error> for VqxError {
    fn from(err: std::io::Error) -> Self {
        VqxError::Other(err.to_string())
    }
}

impl From<serde_json::Error> for VqxError {
    fn from(err: serde_json::Error) -> Self {
        VqxError::InvalidJson {
            message: err.to_string(),
        }
    }
}

impl From<toml::de::Error> for VqxError {
    fn from(err: toml::de::Error) -> Self {
        VqxError::InvalidToml {
            message: err.to_string(),
        }
    }
}

pub type Result<T> = std::result::Result<T, VqxError>;
