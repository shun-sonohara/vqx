//! Profile management for vqx
//!
//! This module provides TOML-based profile management that maps to the underlying CLI's
//! connection options as defined in the PDF "Profile" and "Command Line Options" sections.
//!
//! ## Extension from PDF
//! The underlying CLI uses a Groovy-format profile file (~/.vantiq/profile).
//! vqx extends this with:
//! - TOML-based configuration for better tooling support
//! - Secure credential storage via keyring or encrypted files
//! - Environment variable integration
//! - Interactive profile creation

use crate::error::{Result, VqxError};
use directories::ProjectDirs;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use tracing::{debug, info, warn};

/// Default profile name (matches PDF: "Default: base")
pub const DEFAULT_PROFILE_NAME: &str = "base";

/// Default Vantiq URL (PDF: "Default: https://dev.vantiq.com")
pub const DEFAULT_VANTIQ_URL: &str = "https://dev.vantiq.com";

/// vqx config directory name
const CONFIG_DIR_NAME: &str = "vqx";

/// Profile file name
const PROFILES_FILE: &str = "profiles.toml";

/// A single profile configuration
///
/// Maps to the profile entries in PDF "Profile" section:
/// ```text
/// base {
///     url = 'https://dev.vantiq.com'
///     username = 'myUsername'
///     password = 'myPassword'
/// }
/// oauth1 {
///     url = 'https://dev.vantiq.com'
///     token = 'rTTbtHd8Z7gFPEQPE32137HfYNDg8YA84zmOWtVbdYg='
/// }
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Profile {
    /// Base URL for Vantiq server
    /// PDF: "url = '...'" (optional, defaults to https://dev.vantiq.com)
    #[serde(default = "default_url")]
    pub url: String,

    /// Username for authentication
    /// PDF: "username = '...'"
    /// PDF Note: "username/password can only be used for Edge servers"
    #[serde(skip_serializing_if = "Option::is_none")]
    pub username: Option<String>,

    /// Password for authentication
    /// PDF: "password = '...'"
    /// Note: Stored securely when possible, not in plain text
    #[serde(skip_serializing_if = "Option::is_none")]
    pub password: Option<String>,

    /// Access token
    /// PDF: "token = '...'"
    /// PDF Note: "public clouds and any server using keycloak access require use of the token option"
    #[serde(skip_serializing_if = "Option::is_none")]
    pub token: Option<String>,

    /// Target namespace
    /// PDF: "namespace = '...'"
    /// PDF Note: "the namespace option can only be used with username/password;
    ///           it cannot be used with long-lived access tokens"
    #[serde(skip_serializing_if = "Option::is_none")]
    pub namespace: Option<String>,

    /// Trust SSL certificates
    /// PDF: "-trust" flag
    /// PDF: "Force SSL options to trust remote server certificate and host name"
    #[serde(default)]
    pub trust_ssl: bool,

    /// HTTP client options (extension)
    /// Based on PDF "HttpClient options" section
    #[serde(skip_serializing_if = "Option::is_none")]
    pub client_options: Option<ClientOptions>,

    /// Description for this profile (vqx extension)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,

    /// Indicates if credentials are stored in secure storage
    /// If true, password/token should be retrieved from keyring
    #[serde(default)]
    pub use_secure_storage: bool,
}

fn default_url() -> String {
    DEFAULT_VANTIQ_URL.to_string()
}

impl Default for Profile {
    fn default() -> Self {
        Self {
            url: DEFAULT_VANTIQ_URL.to_string(),
            username: None,
            password: None,
            token: None,
            namespace: None,
            trust_ssl: false,
            client_options: None,
            description: None,
            use_secure_storage: false,
        }
    }
}

impl Profile {
    /// Create a new profile with URL only
    pub fn new(url: impl Into<String>) -> Self {
        Self {
            url: url.into(),
            ..Default::default()
        }
    }

    /// Set username/password authentication
    /// PDF Note: "username/password can only be used for Edge servers"
    pub fn with_credentials(
        mut self,
        username: impl Into<String>,
        password: impl Into<String>,
    ) -> Self {
        self.username = Some(username.into());
        self.password = Some(password.into());
        self
    }

    /// Set token authentication
    /// PDF Note: "public clouds and any server using keycloak access require use of the token option"
    pub fn with_token(mut self, token: impl Into<String>) -> Self {
        self.token = Some(token.into());
        self
    }

    /// Set target namespace
    /// PDF Note: "the namespace option can only be used with username/password"
    pub fn with_namespace(mut self, namespace: impl Into<String>) -> Self {
        self.namespace = Some(namespace.into());
        self
    }

    /// Enable SSL trust
    pub fn with_trust_ssl(mut self) -> Self {
        self.trust_ssl = true;
        self
    }

    /// Check if profile has valid authentication
    pub fn has_auth(&self) -> bool {
        self.token.is_some() || (self.username.is_some() && self.password.is_some())
    }

    /// Get authentication type description
    pub fn auth_type(&self) -> &'static str {
        if self.password.is_some() && self.username.is_some() {
            "username/password"
        } else if self.token.is_some() {
            "access token"
        } else {
            "none"
        }
    }

    /// Validate profile configuration based on PDF constraints
    pub fn validate(&self) -> Result<()> {
        // PDF: "the namespace option can only be used with username/password;
        //       it cannot be used with long-lived access tokens."
        if self.namespace.is_some() && self.token.is_some() && self.password.is_none() {
            return Err(VqxError::NamespaceWithToken);
        }
        Ok(())
    }

    /// Mask sensitive fields for display
    pub fn masked(&self) -> Self {
        Self {
            password: self.password.as_ref().map(|_| "********".to_string()),
            token: self.token.as_ref().map(|_| "********".to_string()),
            ..self.clone()
        }
    }
}

/// HTTP client options
/// Based on PDF "HttpClient options" section:
/// ```text
/// clientOptions {
///     trustAll = true
///     verifyHost = false
///     forceSni = true
///     proxyOptions { ... }
/// }
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClientOptions {
    #[serde(default)]
    pub trust_all: bool,

    #[serde(default = "default_verify_host")]
    pub verify_host: bool,

    #[serde(default)]
    pub force_sni: bool,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub proxy: Option<ProxyOptions>,
}

fn default_verify_host() -> bool {
    true
}

impl Default for ClientOptions {
    fn default() -> Self {
        Self {
            trust_all: false,
            verify_host: true,
            force_sni: false,
            proxy: None,
        }
    }
}

/// Proxy configuration
/// Based on PDF "proxyOptions" section
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProxyOptions {
    pub host: String,
    pub port: u16,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub username: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub password: Option<String>,
}

/// Collection of profiles
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ProfileStore {
    /// Default profile name to use
    #[serde(default = "default_profile_name")]
    pub default_profile: String,

    /// All profiles
    #[serde(default)]
    pub profiles: HashMap<String, Profile>,
}

fn default_profile_name() -> String {
    DEFAULT_PROFILE_NAME.to_string()
}

impl ProfileStore {
    /// Create a new empty profile store
    pub fn new() -> Self {
        Self::default()
    }

    /// Get the config directory path
    /// Uses ~/.config/vqx on Unix (macOS/Linux) for consistency with documentation
    /// Uses %APPDATA%\vqx on Windows
    pub fn config_dir() -> Result<PathBuf> {
        // On Unix systems (macOS/Linux), use ~/.config/vqx for XDG-style config
        // This matches the documentation and is more familiar to CLI users
        #[cfg(unix)]
        {
            let home = dirs_home()?;
            Ok(home.join(".config").join(CONFIG_DIR_NAME))
        }

        // On Windows, use the standard AppData location
        #[cfg(windows)]
        {
            if let Some(proj_dirs) = ProjectDirs::from("", "", CONFIG_DIR_NAME) {
                Ok(proj_dirs.config_dir().to_path_buf())
            } else {
                // Fallback to home directory
                let home = dirs_home()?;
                Ok(home.join(format!(".{}", CONFIG_DIR_NAME)))
            }
        }
    }

    /// Get the profiles file path
    pub fn profiles_file_path() -> Result<PathBuf> {
        Ok(Self::config_dir()?.join(PROFILES_FILE))
    }

    /// Load profiles from the default location
    pub fn load() -> Result<Self> {
        let path = Self::profiles_file_path()?;
        Self::load_from(&path)
    }

    /// Load profiles from a specific file
    pub fn load_from(path: &Path) -> Result<Self> {
        if !path.exists() {
            debug!(path = %path.display(), "Profile file not found, using defaults");
            return Ok(Self::default());
        }

        let content = fs::read_to_string(path).map_err(|e| VqxError::ProfileFileNotFound {
            path: path.display().to_string(),
        })?;

        let store: Self = toml::from_str(&content)?;
        info!(
            path = %path.display(),
            profiles = store.profiles.len(),
            "Loaded profiles"
        );
        Ok(store)
    }

    /// Save profiles to the default location
    pub fn save(&self) -> Result<()> {
        let path = Self::profiles_file_path()?;
        self.save_to(&path)
    }

    /// Save profiles to a specific file
    pub fn save_to(&self, path: &Path) -> Result<()> {
        // Ensure parent directory exists
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

        info!(path = %path.display(), "Saved profiles");
        Ok(())
    }

    /// Get a profile by name
    pub fn get(&self, name: &str) -> Result<&Profile> {
        self.profiles.get(name).ok_or(VqxError::ProfileNotFound {
            name: name.to_string(),
        })
    }

    /// Get the default profile
    pub fn get_default(&self) -> Result<&Profile> {
        self.get(&self.default_profile)
    }

    /// Get a profile, falling back to default if name is None
    pub fn get_or_default(&self, name: Option<&str>) -> Result<&Profile> {
        match name {
            Some(n) => self.get(n),
            None => self.get_default(),
        }
    }

    /// Add or update a profile
    pub fn set(&mut self, name: impl Into<String>, profile: Profile) {
        let name = name.into();
        info!(name = %name, "Setting profile");
        self.profiles.insert(name, profile);
    }

    /// Remove a profile
    pub fn remove(&mut self, name: &str) -> Option<Profile> {
        info!(name = %name, "Removing profile");
        self.profiles.remove(name)
    }

    /// Set the default profile
    pub fn set_default(&mut self, name: impl Into<String>) -> Result<()> {
        let name = name.into();
        if !self.profiles.contains_key(&name) {
            return Err(VqxError::ProfileNotFound { name });
        }
        self.default_profile = name;
        Ok(())
    }

    /// List all profile names
    pub fn list_names(&self) -> Vec<&str> {
        self.profiles.keys().map(|s| s.as_str()).collect()
    }

    /// Check if a profile exists
    pub fn exists(&self, name: &str) -> bool {
        self.profiles.contains_key(name)
    }

    /// Export to TOML string
    pub fn to_toml(&self) -> Result<String> {
        toml::to_string_pretty(self).map_err(|e| VqxError::InvalidToml {
            message: e.to_string(),
        })
    }

    /// Import from TOML string
    pub fn from_toml(content: &str) -> Result<Self> {
        toml::from_str(content).map_err(|e| VqxError::InvalidToml {
            message: e.to_string(),
        })
    }
}

/// Get home directory
fn dirs_home() -> Result<PathBuf> {
    dirs::home_dir()
        .ok_or_else(|| VqxError::Other("Could not determine home directory".to_string()))
}

/// Profile manager with secure storage support
pub struct ProfileManager {
    store: ProfileStore,
    store_path: PathBuf,
}

impl ProfileManager {
    /// Create a new profile manager
    pub fn new() -> Result<Self> {
        let store_path = ProfileStore::profiles_file_path()?;
        let store = ProfileStore::load()?;
        Ok(Self { store, store_path })
    }

    /// Create with a specific path
    pub fn with_path(path: PathBuf) -> Result<Self> {
        let store = ProfileStore::load_from(&path)?;
        Ok(Self {
            store,
            store_path: path,
        })
    }

    /// Get the underlying store
    pub fn store(&self) -> &ProfileStore {
        &self.store
    }

    /// Get mutable store
    pub fn store_mut(&mut self) -> &mut ProfileStore {
        &mut self.store
    }

    /// Save changes
    pub fn save(&self) -> Result<()> {
        self.store.save_to(&self.store_path)
    }

    /// Get a profile with credentials resolved (from secure storage if needed)
    pub fn get_resolved(&self, name: &str) -> Result<Profile> {
        let profile = self.store.get(name)?;
        self.resolve_credentials(name, profile.clone())
    }

    /// Get default profile with credentials resolved
    pub fn get_default_resolved(&self) -> Result<Profile> {
        let name = self.store.default_profile.clone();
        self.get_resolved(&name)
    }

    /// Resolve credentials from secure storage if needed
    fn resolve_credentials(&self, name: &str, mut profile: Profile) -> Result<Profile> {
        if profile.use_secure_storage {
            // Try to get credentials from keyring
            #[cfg(feature = "keyring-storage")]
            {
                if let Some(password) = self.get_secret(name, "password")? {
                    profile.password = Some(password);
                }
                if let Some(token) = self.get_secret(name, "token")? {
                    profile.token = Some(token);
                }
            }
        }
        Ok(profile)
    }

    /// Store a secret in secure storage
    #[cfg(feature = "keyring-storage")]
    pub fn set_secret(&self, profile_name: &str, key: &str, value: &str) -> Result<()> {
        let service = format!("vqx-{}", profile_name);
        let entry =
            keyring::Entry::new(&service, key).map_err(|e| VqxError::SecretStorageFailed {
                message: e.to_string(),
            })?;

        entry
            .set_password(value)
            .map_err(|e| VqxError::SecretStorageFailed {
                message: e.to_string(),
            })?;

        debug!(
            profile = profile_name,
            key = key,
            "Stored secret in keyring"
        );
        Ok(())
    }

    /// Get a secret from secure storage
    #[cfg(feature = "keyring-storage")]
    pub fn get_secret(&self, profile_name: &str, key: &str) -> Result<Option<String>> {
        let service = format!("vqx-{}", profile_name);
        let entry =
            keyring::Entry::new(&service, key).map_err(|e| VqxError::SecretStorageFailed {
                message: e.to_string(),
            })?;

        match entry.get_password() {
            Ok(value) => Ok(Some(value)),
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(e) => {
                warn!(error = %e, "Failed to get secret from keyring");
                Err(VqxError::SecretStorageFailed {
                    message: e.to_string(),
                })
            }
        }
    }

    /// Delete a secret from secure storage
    #[cfg(feature = "keyring-storage")]
    pub fn delete_secret(&self, profile_name: &str, key: &str) -> Result<()> {
        let service = format!("vqx-{}", profile_name);
        let entry =
            keyring::Entry::new(&service, key).map_err(|e| VqxError::SecretStorageFailed {
                message: e.to_string(),
            })?;

        match entry.delete_credential() {
            Ok(()) => Ok(()),
            Err(keyring::Error::NoEntry) => Ok(()), // Already deleted
            Err(e) => Err(VqxError::SecretStorageFailed {
                message: e.to_string(),
            }),
        }
    }

    // Fallback implementations when keyring is not available
    #[cfg(not(feature = "keyring-storage"))]
    pub fn set_secret(&self, _profile_name: &str, _key: &str, _value: &str) -> Result<()> {
        warn!("Secure storage not available, credentials will be stored in config file");
        Ok(())
    }

    #[cfg(not(feature = "keyring-storage"))]
    pub fn get_secret(&self, _profile_name: &str, _key: &str) -> Result<Option<String>> {
        Ok(None)
    }

    #[cfg(not(feature = "keyring-storage"))]
    pub fn delete_secret(&self, _profile_name: &str, _key: &str) -> Result<()> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_profile_default() {
        let profile = Profile::default();
        assert_eq!(profile.url, DEFAULT_VANTIQ_URL);
        assert!(!profile.has_auth());
    }

    #[test]
    fn test_profile_with_credentials() {
        let profile = Profile::new("https://test.vantiq.com")
            .with_credentials("user", "pass")
            .with_namespace("ns");

        assert!(profile.has_auth());
        assert_eq!(profile.auth_type(), "username/password");
        assert!(profile.validate().is_ok());
    }

    #[test]
    fn test_profile_with_token() {
        let profile = Profile::new("https://test.vantiq.com").with_token("my-token");

        assert!(profile.has_auth());
        assert_eq!(profile.auth_type(), "access token");
    }

    #[test]
    fn test_namespace_with_token_error() {
        let profile = Profile::new("https://test.vantiq.com")
            .with_token("my-token")
            .with_namespace("ns");

        assert!(matches!(
            profile.validate(),
            Err(VqxError::NamespaceWithToken)
        ));
    }

    #[test]
    fn test_profile_store_roundtrip() {
        let mut store = ProfileStore::new();
        store.set(
            "test",
            Profile::new("https://test.vantiq.com").with_token("token"),
        );

        let toml = store.to_toml().unwrap();
        let loaded = ProfileStore::from_toml(&toml).unwrap();

        assert!(loaded.exists("test"));
        assert_eq!(loaded.get("test").unwrap().url, "https://test.vantiq.com");
    }

    #[test]
    fn test_profile_masked() {
        let profile = Profile::new("https://test.vantiq.com")
            .with_credentials("user", "secret_password")
            .with_token("secret_token");

        let masked = profile.masked();
        assert_eq!(masked.username, Some("user".to_string()));
        assert_eq!(masked.password, Some("********".to_string()));
        assert_eq!(masked.token, Some("********".to_string()));
    }
}
