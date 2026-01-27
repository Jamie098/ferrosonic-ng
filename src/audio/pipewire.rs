//! PipeWire sample rate control

use std::process::Command;
use tracing::{debug, error, info};

use crate::error::AudioError;

/// Default audio device ID for PipeWire
const DEFAULT_DEVICE_ID: u32 = 0;

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
        let original_rate = Self::get_current_rate_internal().ok();
        debug!("Original PipeWire sample rate: {:?}", original_rate);

        Self {
            original_rate,
            current_rate: None,
        }
    }

    /// Get current sample rate from PipeWire
    fn get_current_rate_internal() -> Result<u32, AudioError> {
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

    /// Set the sample rate
    pub fn set_rate(&mut self, rate: u32) -> Result<(), AudioError> {
        if self.current_rate == Some(rate) {
            debug!("Sample rate already set to {}", rate);
            return Ok(());
        }

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

    /// Clear the forced sample rate (let PipeWire use default)
    pub fn clear_forced_rate(&mut self) -> Result<(), AudioError> {
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

        self.current_rate = None;
        Ok(())
    }

    /// Check if PipeWire is available
    pub fn is_available() -> bool {
        Command::new("pw-metadata")
            .arg("--version")
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
    }

    /// Get the effective sample rate (from pw-metadata or system default)
    pub fn get_effective_rate() -> Option<u32> {
        // Try to get from PipeWire
        let output = Command::new("pw-metadata")
            .arg("-n")
            .arg("settings")
            .output()
            .ok()?;

        let stdout = String::from_utf8_lossy(&output.stdout);

        // Look for clock.rate or clock.force-rate
        for line in stdout.lines() {
            if (line.contains("clock.rate") || line.contains("clock.force-rate"))
                && line.contains("value:")
            {
                if let Some(start) = line.find("value:'") {
                    let rest = &line[start + 7..];
                    if let Some(end) = rest.find('\'') {
                        let rate_str = &rest[..end];
                        if let Ok(rate) = rate_str.parse::<u32>() {
                            if rate > 0 {
                                return Some(rate);
                            }
                        }
                    }
                }
            }
        }

        None
    }
}

impl Default for PipeWireController {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for PipeWireController {
    fn drop(&mut self) {
        if let Err(e) = self.restore_original() {
            error!("Failed to restore sample rate: {}", e);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_available() {
        // This test just checks the function doesn't panic
        let _ = PipeWireController::is_available();
    }
}
