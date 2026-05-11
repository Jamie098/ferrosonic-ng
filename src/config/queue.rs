//! Queue persistence - save and restore play queue

use serde::{Deserialize, Serialize};
use std::path::Path;
use tracing::{debug, info};

use crate::app::state::RepeatMode;
use crate::subsonic::models::Child;

/// Persisted queue state
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct QueuePersist {
    /// Play queue (songs)
    #[serde(default)]
    pub queue: Vec<Child>,
    /// Current position in queue
    #[serde(default)]
    pub queue_position: Option<usize>,
    /// Repeat mode
    #[serde(default)]
    pub repeat_mode: RepeatMode,
    /// Volume (0-100)
    #[serde(default = "default_volume")]
    pub volume: u8,
}

fn default_volume() -> u8 {
    100
}

impl QueuePersist {
    /// Load queue from the default location
    pub fn load_default() -> Option<Self> {
        let path = crate::config::paths::queue_file()?;
        Self::load_from_file(&path).ok()
    }

    /// Load queue from a specific file
    pub fn load_from_file(path: &Path) -> std::io::Result<Self> {
        debug!("Loading queue from {}", path.display());

        if !path.exists() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                "Queue file not found",
            ));
        }

        let contents = std::fs::read_to_string(path)?;
        let queue: QueuePersist = serde_json::from_str(&contents).map_err(|e| {
            std::io::Error::new(std::io::ErrorKind::InvalidData, e)
        })?;

        debug!("Queue loaded with {} songs", queue.queue.len());
        Ok(queue)
    }

    /// Save queue to the default location
    pub fn save_default(&self) -> std::io::Result<()> {
        let path = crate::config::paths::queue_file().ok_or_else(|| {
            std::io::Error::new(
                std::io::ErrorKind::NotFound,
                "Could not determine queue file location",
            )
        })?;

        self.save_to_file(&path)
    }

    /// Save queue to a specific file
    pub fn save_to_file(&self, path: &Path) -> std::io::Result<()> {
        debug!("Saving queue to {}", path.display());

        // Ensure parent directory exists
        if let Some(parent) = path.parent() {
            if !parent.exists() {
                std::fs::create_dir_all(parent)?;
            }
        }

        let contents = serde_json::to_string_pretty(self).map_err(|e| {
            std::io::Error::new(std::io::ErrorKind::InvalidData, e)
        })?;
        std::fs::write(path, contents)?;

        info!("Queue saved with {} songs to {}", self.queue.len(), path.display());
        Ok(())
    }

    /// Clear the saved queue file
    pub fn clear_default() -> std::io::Result<()> {
        let path = crate::config::paths::queue_file().ok_or_else(|| {
            std::io::Error::new(
                std::io::ErrorKind::NotFound,
                "Could not determine queue file location",
            )
        })?;

        if path.exists() {
            std::fs::remove_file(path)?;
            debug!("Queue file removed");
        }
        Ok(())
    }
}
