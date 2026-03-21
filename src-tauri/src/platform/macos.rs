use super::{PermissionTarget, PlatformBackend, PlatformCapabilities};
use std::process::Command;

pub(super) struct MacosBackend;

impl PlatformBackend for MacosBackend {
    fn capabilities(&self) -> PlatformCapabilities {
        PlatformCapabilities {
            platform: "macos",
            needs_accessibility: true,
            needs_automation: true,
            supports_permission_settings: true,
            supports_contextual_anchor: true,
        }
    }

    fn open_permission_settings(&self, target: PermissionTarget) -> Result<(), String> {
        let location = match target {
            PermissionTarget::Microphone => {
                "x-apple.systempreferences:com.apple.preference.security?Privacy_Microphone"
            }
            PermissionTarget::Accessibility => {
                "x-apple.systempreferences:com.apple.preference.security?Privacy_Accessibility"
            }
            PermissionTarget::Automation => {
                "x-apple.systempreferences:com.apple.preference.security?Privacy_Automation"
            }
        };

        Command::new("open")
            .arg(location)
            .spawn()
            .map_err(|e| format!("Could not open macOS settings: {e}"))?;

        Ok(())
    }

    fn open_external_url(&self, url: &str) -> Result<(), String> {
        Command::new("open")
            .arg(url)
            .spawn()
            .map_err(|e| format!("Could not open URL on macOS: {e}"))?;
        Ok(())
    }

    fn ensure_accessibility_permission(&self) -> Result<(), String> {
        use enigo::Direction::{Press, Release};
        use enigo::{Enigo, Key, Keyboard, Settings};

        let mut enigo = Enigo::new(&Settings::default())
            .map_err(|e| format!("Could not initialize keyboard automation: {e}"))?;

        enigo
            .key(Key::Meta, Press)
            .map_err(|e| format!("Accessibility permission denied: {e}"))?;
        enigo
            .key(Key::Meta, Release)
            .map_err(|e| format!("Accessibility permission denied: {e}"))?;

        Ok(())
    }

    fn ensure_system_events_permission(&self) -> Result<(), String> {
        let probe_script = r#"
try
  tell application "System Events"
    set _front to first application process whose frontmost is true
    set _name to name of _front
  end tell
  return "ok"
on error errMsg
  return "ERROR:" & errMsg
end try
"#;

        let output = Command::new("osascript")
            .arg("-e")
            .arg(probe_script)
            .output()
            .map_err(|e| format!("Could not run macOS automation probe: {e}"))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
            let detail = if stderr.is_empty() {
                "System Events check failed.".to_string()
            } else {
                stderr
            };
            return Err(format!(
                "Automation permission denied: {detail}. Enable WhisloAI in Privacy & Security > Accessibility and Automation (System Events), then restart WhisloAI."
            ));
        }

        let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if stdout.starts_with("ERROR:") {
            let detail = stdout.trim_start_matches("ERROR:").trim();
            return Err(format!(
                "Automation permission denied: {}. Enable WhisloAI in Privacy & Security > Accessibility and Automation (System Events), then restart WhisloAI.",
                if detail.is_empty() {
                    "System Events access is blocked."
                } else {
                    detail
                }
            ));
        }

        Ok(())
    }

    fn simulate_modifier_shortcut(&self, character: char) -> Result<(), String> {
        use enigo::Direction::{Click, Press, Release};
        use enigo::{Enigo, Key, Keyboard, Settings};

        let mut enigo = Enigo::new(&Settings::default())
            .map_err(|e| format!("Could not initialize keyboard automation: {e}"))?;

        enigo
            .key(Key::Meta, Press)
            .map_err(|e| format!("Could not press shortcut modifier key: {e}"))?;

        let shortcut_result = enigo
            .key(Key::Unicode(character), Click)
            .map_err(|e| format!("Could not send shortcut key: {e}"));

        let _ = enigo.key(Key::Meta, Release);
        shortcut_result
    }

    fn refocus_point(
        &self,
        x: i32,
        y: i32,
        click_stabilize_ms: u64,
        post_restore_ms: u64,
    ) -> Result<(), String> {
        use enigo::Direction::Click;
        use enigo::{Button, Coordinate, Enigo, Mouse, Settings};

        let mut enigo = Enigo::new(&Settings::default())
            .map_err(|e| format!("Could not initialize input automation for refocus: {e}"))?;

        let original_cursor = enigo
            .location()
            .map_err(|e| format!("Could not read current cursor position: {e}"))?;

        enigo
            .move_mouse(x, y, Coordinate::Abs)
            .map_err(|e| format!("Could not move cursor to input target: {e}"))?;
        enigo
            .button(Button::Left, Click)
            .map_err(|e| format!("Could not click input target: {e}"))?;

        std::thread::sleep(std::time::Duration::from_millis(click_stabilize_ms));

        let _ = enigo.move_mouse(original_cursor.0, original_cursor.1, Coordinate::Abs);
        std::thread::sleep(std::time::Duration::from_millis(post_restore_ms));

        Ok(())
    }
}
