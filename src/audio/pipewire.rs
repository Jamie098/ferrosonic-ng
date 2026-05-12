//! PipeWire sample rate control

use std::process::Command;
use tokio::task;
use tracing::{debug, error, info};

use crate::error::AudioError;

/// PipeWire sample rate controller
pub struct PipeWireController {
    /// Original sample rate before ferrosonic started
    original_rate: Option<u32>,
    /// Current forced sample rate
    current_rate: Option<u32>,
}

impl PipeWireController {
    /// Create a new PipeWire controller
    pub fn new() -> Self {
        let original_rate = task::block_in_place(|| Self::get_current_rate_blocking().ok());
        debug!("Original PipeWire sample rate: {:?}", original_rate);

        Self {
            original_rate,
            current_rate: None,
        }
    }

    /// Blocking helper to get current sample rate from PipeWire
    fn get_current_rate_blocking() -> Result<u32, AudioError> {
        let output = Command::new("pw-metadata")
            .arg("-n")
            .arg("settings")
            .arg("0")
            .arg("clock.force-rate")
            .output()
            .map_err(|e| AudioError::PipeWire(format!("Failed to run pw-metadata: {}", e)))?;

        let stdout = String::from_utf8_lossy(&output.stdout);

        // Parse output like: "update: id:0 key:'clock.force-rate' value:'48000' type:''"
        for line in stdout.lines() {
            if line.contains("clock.force-rate") && line.contains("value:") {
                if let Some(start) = line.find("value:'") {
                    let rest = &line[start + 7..];
                    if let Some(end) = rest.find('\'') {
                        let rate_str = &rest[..end];
                        if let Ok(rate) = rate_str.parse::<u32>() {
                            return Ok(rate);
                        }
                    }
                }
            }
        }

        // No forced rate, return default
        Ok(0)
    }

    /// Get the current forced sample rate
    pub fn get_current_rate(&self) -> Option<u32> {
        self.current_rate
    }

    /// Blocking helper to set the sample rate
    fn set_rate_blocking(rate: u32) -> Result<(), AudioError> {
        info!("Setting PipeWire sample rate to {} Hz", rate);

        let output = Command::new("pw-metadata")
            .arg("-n")
            .arg("settings")
            .arg("0")
            .arg("clock.force-rate")
            .arg(rate.to_string())
            .output()
            .map_err(|e| AudioError::PipeWire(format!("Failed to run pw-metadata: {}", e)))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(AudioError::PipeWire(format!(
                "pw-metadata failed: {}",
                stderr
            )));
        }

        Ok(())
    }

    /// Set the sample rate
    pub fn set_rate(&mut self, rate: u32) -> Result<(), AudioError> {
        if self.current_rate == Some(rate) {
            debug!("Sample rate already set to {}", rate);
            return Ok(());
        }

        task::block_in_place(|| Self::set_rate_blocking(rate))?;
        self.current_rate = Some(rate);
        Ok(())
    }

    /// Restore original sample rate
    pub fn restore_original(&mut self) -> Result<(), AudioError> {
        if let Some(rate) = self.original_rate {
            if rate > 0 {
                info!("Restoring original sample rate: {} Hz", rate);
                self.set_rate(rate)?;
            } else {
                info!("Clearing forced sample rate");
                self.clear_forced_rate()?;
            }
        }
        Ok(())
    }

    /// Blocking helper to clear the forced sample rate
    fn clear_forced_rate_blocking() -> Result<(), AudioError> {
        info!("Clearing PipeWire forced sample rate");

        let output = Command::new("pw-metadata")
            .arg("-n")
            .arg("settings")
            .arg("0")
            .arg("clock.force-rate")
            .arg("0")
            .output()
            .map_err(|e| AudioError::PipeWire(format!("Failed to run pw-metadata: {}", e)))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(AudioError::PipeWire(format!(
                "pw-metadata failed: {}",
                stderr
            )));
        }

        Ok(())
    }

    /// Clear the forced sample rate (let PipeWire use default)
    pub fn clear_forced_rate(&mut self) -> Result<(), AudioError> {
        task::block_in_place(Self::clear_forced_rate_blocking)?;
        self.current_rate = None;
        Ok(())
    }

}

impl Default for PipeWireController {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for PipeWireController {
    fn drop(&mut self) {
        if let Some(rate) = self.original_rate {
            let handle = std::thread::spawn(move || {
                if rate > 0 {
                    if let Err(e) = Self::set_rate_blocking(rate) {
                        error!("Failed to restore sample rate: {}", e);
                    }
                } else if let Err(e) = Self::clear_forced_rate_blocking() {
                    error!("Failed to clear forced sample rate: {}", e);
                }
            });
            let _ = handle.join();
        }
    }
}

