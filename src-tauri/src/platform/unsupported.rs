use super::{PermissionTarget, PlatformBackend, PlatformCapabilities};
use std::process::Command;

pub(super) struct UnsupportedBackend;

impl PlatformBackend for UnsupportedBackend {
    fn capabilities(&self) -> PlatformCapabilities {
        PlatformCapabilities {
            platform: if cfg!(target_os = "linux") {
                "linux"
            } else {
                "unknown"
            },
            needs_accessibility: false,
            needs_automation: false,
            supports_permission_settings: false,
            supports_contextual_anchor: false,
        }
    }

    fn open_permission_settings(&self, _target: PermissionTarget) -> Result<(), String> {
        Err("Opening system permission settings is not implemented for this platform.".to_string())
    }

    fn open_external_url(&self, url: &str) -> Result<(), String> {
        Command::new("xdg-open")
            .arg(url)
            .spawn()
            .map_err(|e| format!("Could not open URL on this platform: {e}"))?;
        Ok(())
    }

    fn ensure_accessibility_permission(&self) -> Result<(), String> {
        Err("Accessibility permission check is not supported on this platform.".to_string())
    }

    fn ensure_system_events_permission(&self) -> Result<(), String> {
        Ok(())
    }

    fn simulate_modifier_shortcut(&self, character: char) -> Result<(), String> {
        if character.eq_ignore_ascii_case(&'v') {
            return Err(
                "Automatic paste is not supported on this platform in the MVP.".to_string(),
            );
        }
        if character.eq_ignore_ascii_case(&'c') {
            return Err("Automatic copy is not supported on this platform in the MVP.".to_string());
        }
        Err("Automatic keyboard shortcut is not supported on this platform in the MVP.".to_string())
    }

    fn refocus_point(
        &self,
        _x: i32,
        _y: i32,
        _click_stabilize_ms: u64,
        _post_restore_ms: u64,
    ) -> Result<(), String> {
        Err("Input refocus is not supported on this platform.".to_string())
    }
}
