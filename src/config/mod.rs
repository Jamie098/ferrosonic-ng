//! Configuration loading and management

pub mod paths;

use serde::{Deserialize, Serialize};
use std::path::Path;
use tracing::{debug, info, warn};

use crate::error::ConfigError;

/// Main application configuration
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Config {
    /// Subsonic server base URL
    #[serde(rename = "BaseURL", default)]
    pub base_url: String,

    /// Username for authentication
    #[serde(rename = "Username", default)]
    pub username: String,

    /// Password for authentication
    #[serde(rename = "Password", default)]
    pub password: String,

    /// UI Theme name
    #[serde(rename = "Theme", default)]
    pub theme: String,

    /// Enable cava audio visualizer
    #[serde(rename = "Cava", default)]
    pub cava: bool,

    /// Cava visualizer height percentage (10-80, step 5)
    #[serde(rename = "CavaSize", default = "Config::default_cava_size")]
    pub cava_size: u8,

    /// Notifications
    #[serde(rename = "Notifications", default)]
    pub notifications: bool,

    /// Number of random songs to fetch
    #[serde(
        rename = "RandomSongsCount",
        default = "Config::default_random_songs_count"
    )]
    pub random_songs_count: usize,
}

impl Config {
    fn default_cava_size() -> u8 {
        40
    }

    fn default_random_songs_count() -> usize {
        250
    }

    /// Create a new empty config
    pub fn new() -> Self {
        Self::default()
    }

    /// Load config from the default location
    pub fn load_default() -> Result<Self, ConfigError> {
        let path = paths::config_file().ok_or_else(|| ConfigError::NotFound {
            path: "default config location".to_string(),
        })?;

        if path.exists() {
            Self::load_from_file(&path)
        } else {
            info!("No config file found at {}, using defaults", path.display());
            Ok(Self::new())
        }
    }

    /// Load config from a specific file
    pub fn load_from_file(path: &Path) -> Result<Self, ConfigError> {
        debug!("Loading config from {}", path.display());

        if !path.exists() {
            return Err(ConfigError::NotFound {
                path: path.display().to_string(),
            });
        }

        let contents = std::fs::read_to_string(path)?;
        let config: Config = toml::from_str(&contents)?;

        debug!("Config loaded successfully");
        Ok(config)
    }

    /// Save config to the default location
    pub fn save_default(&self) -> Result<(), ConfigError> {
        let path = paths::config_file().ok_or_else(|| ConfigError::NotFound {
            path: "default config location".to_string(),
        })?;

        self.save_to_file(&path)
    }

    /// Save config to a specific file
    pub fn save_to_file(&self, path: &Path) -> Result<(), ConfigError> {
        debug!("Saving config to {}", path.display());

        // Ensure parent directory exists
        if let Some(parent) = path.parent() {
            if !parent.exists() {
                std::fs::create_dir_all(parent)?;
            }
        }

        let contents = toml::to_string_pretty(self)?;
        std::fs::write(path, contents)?;

        info!("Config saved to {}", path.display());
        Ok(())
    }

    /// Check if the config has valid server settings
    pub fn is_configured(&self) -> bool {
        !self.base_url.is_empty() && !self.username.is_empty() && !self.password.is_empty()
    }

    /// Validate the config
    #[allow(dead_code)]
    pub fn validate(&self) -> Result<(), ConfigError> {
        if self.base_url.is_empty() {
            return Err(ConfigError::MissingField {
                field: "BaseURL".to_string(),
            });
        }

        // Validate URL format
        if url::Url::parse(&self.base_url).is_err() {
            return Err(ConfigError::InvalidUrl {
                url: self.base_url.clone(),
            });
        }

        if self.username.is_empty() {
            warn!("Username is empty");
        }

        if self.password.is_empty() {
            warn!("Password is empty");
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    #[test]
    fn test_config_parse() {
        let toml_content = r#"
BaseURL = "https://example.com"
Username = "testuser"
Password = "testpass"
"#;

        let mut file = NamedTempFile::new().unwrap();
        file.write_all(toml_content.as_bytes()).unwrap();

        let config = Config::load_from_file(file.path()).unwrap();
        assert_eq!(config.base_url, "https://example.com");
        assert_eq!(config.username, "testuser");
        assert_eq!(config.password, "testpass");
    }

    #[test]
    fn test_is_configured() {
        let mut config = Config::new();
        assert!(!config.is_configured());

        config.base_url = "https://example.com".to_string();
        config.username = "user".to_string();
        config.password = "pass".to_string();
        assert!(config.is_configured());
    }
}
