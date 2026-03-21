use super::{PermissionTarget, PlatformBackend, PlatformCapabilities};
use std::process::Command;

pub(super) struct WindowsBackend;

impl PlatformBackend for WindowsBackend {
    fn capabilities(&self) -> PlatformCapabilities {
        PlatformCapabilities {
            platform: "windows",
            needs_accessibility: false,
            needs_automation: false,
            supports_permission_settings: true,
            supports_contextual_anchor: false,
        }
    }

    fn open_permission_settings(&self, target: PermissionTarget) -> Result<(), String> {
        let location = match target {
            PermissionTarget::Microphone => "ms-settings:privacy-microphone",
            PermissionTarget::Accessibility | PermissionTarget::Automation => {
                "ms-settings:easeofaccess-keyboard"
            }
        };

        Command::new("cmd")
            .args(["/C", "start", "", location])
            .spawn()
            .map_err(|e| format!("Could not open Windows settings: {e}"))?;

        Ok(())
    }

    fn open_external_url(&self, url: &str) -> Result<(), String> {
        Command::new("cmd")
            .args(["/C", "start", "", url])
            .spawn()
            .map_err(|e| format!("Could not open URL on Windows: {e}"))?;
        Ok(())
    }

    fn ensure_accessibility_permission(&self) -> Result<(), String> {
        use enigo::Direction::{Press, Release};
        use enigo::{Enigo, Key, Keyboard, Settings};

        let mut enigo = Enigo::new(&Settings::default())
            .map_err(|e| format!("Could not initialize keyboard automation: {e}"))?;

        enigo
            .key(Key::Control, Press)
            .map_err(|e| format!("Accessibility permission denied: {e}"))?;
        enigo
            .key(Key::Control, Release)
            .map_err(|e| format!("Accessibility permission denied: {e}"))?;

        Ok(())
    }

    fn ensure_system_events_permission(&self) -> Result<(), String> {
        Ok(())
    }

    fn simulate_modifier_shortcut(&self, character: char) -> Result<(), String> {
        use enigo::Direction::{Click, Press, Release};
        use enigo::{Enigo, Key, Keyboard, Settings};

        let mut enigo = Enigo::new(&Settings::default())
            .map_err(|e| format!("Could not initialize keyboard automation: {e}"))?;

        enigo
            .key(Key::Control, Press)
            .map_err(|e| format!("Could not press shortcut modifier key: {e}"))?;

        let shortcut_result = enigo
            .key(Key::Unicode(character), Click)
            .map_err(|e| format!("Could not send shortcut key: {e}"));

        let _ = enigo.key(Key::Control, Release);
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
