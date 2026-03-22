use super::{
    PermissionTarget, PlatformBackend, PlatformCapabilities, PlatformFocusedAnchorProbe,
    PlatformFocusedAnchorSnapshot,
};
use std::path::Path;
use std::process::Command;
use windows_sys::Win32::Foundation::{CloseHandle, HWND, POINT, RECT};
use windows_sys::Win32::Graphics::Gdi::ClientToScreen;
use windows_sys::Win32::System::Threading::{
    GetCurrentProcessId, OpenProcess, QueryFullProcessImageNameW, PROCESS_QUERY_LIMITED_INFORMATION,
};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    GetClassNameW, GetForegroundWindow, GetGUIThreadInfo, GetWindowLongW, GetWindowRect,
    GetWindowThreadProcessId, IsWindowVisible, ES_PASSWORD, GUITHREADINFO, GWL_STYLE,
};

pub(super) struct WindowsBackend;

impl PlatformBackend for WindowsBackend {
    fn capabilities(&self) -> PlatformCapabilities {
        PlatformCapabilities {
            platform: "windows",
            needs_accessibility: false,
            needs_automation: false,
            supports_permission_settings: true,
            supports_contextual_anchor: true,
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

pub(super) fn focused_anchor_probe() -> PlatformFocusedAnchorProbe {
    const ANCHOR_X_OFFSET_FROM_CARET: i32 = 14;
    const ANCHOR_Y_OFFSET_FROM_CARET: i32 = 44;
    const ANCHOR_X_OFFSET_FROM_RECT: i32 = 10;
    const ANCHOR_Y_OFFSET_FROM_RECT: i32 = 44;
    const MIN_FOCUS_DIMENSION_PX: i32 = 2;

    // SAFETY: Win32 API calls are used read-only for focused-window/caret inspection.
    unsafe {
        let foreground = GetForegroundWindow();
        if foreground.is_null() {
            return skip_probe("missing_foreground_window", None, None, None);
        }

        let mut foreground_pid = 0u32;
        let _ = GetWindowThreadProcessId(foreground, &mut foreground_pid);
        let pid_i32 = i32::try_from(foreground_pid).ok();

        if foreground_pid != 0 && foreground_pid == GetCurrentProcessId() {
            return skip_probe(
                "internal_app",
                pid_i32,
                process_bundle_id(foreground_pid),
                None,
            );
        }

        let mut gui = GUITHREADINFO {
            cbSize: std::mem::size_of::<GUITHREADINFO>() as u32,
            ..Default::default()
        };
        if GetGUIThreadInfo(0, &mut gui) == 0 {
            return skip_probe(
                "gui_thread_info_failed",
                pid_i32,
                process_bundle_id(foreground_pid),
                None,
            );
        }

        let focus = if !gui.hwndFocus.is_null() {
            gui.hwndFocus
        } else {
            gui.hwndCaret
        };

        if focus.is_null() {
            return skip_probe(
                "missing_focus_window",
                pid_i32,
                process_bundle_id(foreground_pid),
                None,
            );
        }

        let role = window_class_name(focus);
        let role_ref = role.as_deref().unwrap_or_default();

        let mut focus_pid = foreground_pid;
        let _ = GetWindowThreadProcessId(focus, &mut focus_pid);
        let effective_pid = if focus_pid != 0 {
            focus_pid
        } else {
            foreground_pid
        };
        let effective_pid_i32 = i32::try_from(effective_pid).ok();
        let bundle_id = process_bundle_id(effective_pid);

        if effective_pid != 0 && effective_pid == GetCurrentProcessId() {
            return skip_probe("internal_app", effective_pid_i32, bundle_id, role);
        }

        let style = GetWindowLongW(focus, GWL_STYLE);
        let is_known_text_class = is_known_text_input_class(role_ref);
        if is_known_text_class && (style & ES_PASSWORD) != 0 {
            return skip_probe("blocked_password_field", effective_pid_i32, bundle_id, role);
        }

        let caret_rect = caret_screen_rect(gui.hwndCaret, gui.rcCaret);
        let focus_rect = window_screen_rect(focus);

        let is_text_like = caret_rect.is_some() || is_known_text_class;

        if !is_text_like {
            return skip_probe("focus_not_text_like", effective_pid_i32, bundle_id, role);
        }

        if IsWindowVisible(focus) == 0 && caret_rect.is_none() {
            return skip_probe("focus_not_visible", effective_pid_i32, bundle_id, role);
        }

        let snapshot = if let Some(rect) = caret_rect {
            let width = (rect.right - rect.left).max(MIN_FOCUS_DIMENSION_PX);
            let height = (rect.bottom - rect.top).max(MIN_FOCUS_DIMENSION_PX);
            let focus_x = rect.left + (width / 2);
            let focus_y = rect.top + (height / 2);

            PlatformFocusedAnchorSnapshot {
                anchor_x: focus_x + ANCHOR_X_OFFSET_FROM_CARET,
                anchor_y: focus_y - ANCHOR_Y_OFFSET_FROM_CARET,
                focus_x,
                focus_y,
            }
        } else if let Some(rect) = focus_rect {
            let width = rect.right - rect.left;
            let height = rect.bottom - rect.top;
            if width < MIN_FOCUS_DIMENSION_PX || height < MIN_FOCUS_DIMENSION_PX {
                return skip_probe("tiny_focus_rect", effective_pid_i32, bundle_id, role);
            }

            let focus_x = rect.left + (width / 2);
            let focus_y = rect.top + (height / 2);
            PlatformFocusedAnchorSnapshot {
                anchor_x: rect.right - ANCHOR_X_OFFSET_FROM_RECT,
                anchor_y: rect.top - ANCHOR_Y_OFFSET_FROM_RECT,
                focus_x,
                focus_y,
            }
        } else {
            return skip_probe("missing_focus_geometry", effective_pid_i32, bundle_id, role);
        };

        PlatformFocusedAnchorProbe {
            snapshot: Some(snapshot),
            reason: "focused_input_detected".to_string(),
            bundle_id,
            source: "win32",
            pid: effective_pid_i32,
            role,
        }
    }
}

fn skip_probe(
    reason: &str,
    pid: Option<i32>,
    bundle_id: Option<String>,
    role: Option<String>,
) -> PlatformFocusedAnchorProbe {
    PlatformFocusedAnchorProbe {
        snapshot: None,
        reason: reason.to_string(),
        bundle_id,
        source: "win32",
        pid,
        role,
    }
}

fn window_class_name(hwnd: HWND) -> Option<String> {
    let mut buffer = [0u16; 256];

    // SAFETY: Buffer is valid and sized, hwnd comes from OS APIs.
    let count = unsafe { GetClassNameW(hwnd, buffer.as_mut_ptr(), buffer.len() as i32) };
    if count <= 0 {
        return None;
    }

    let count_usize = usize::try_from(count).ok()?;
    let value = String::from_utf16_lossy(&buffer[..count_usize]);
    let trimmed = value.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

fn is_known_text_input_class(class_name: &str) -> bool {
    let value = class_name.trim().to_lowercase();
    if value.is_empty() {
        return false;
    }

    value == "edit"
        || value.contains("richedit")
        || value.contains("textbox")
        || value.contains("text")
}

fn window_screen_rect(hwnd: HWND) -> Option<RECT> {
    let mut rect = RECT::default();

    // SAFETY: rect points to valid memory and hwnd is provided by OS APIs.
    let ok = unsafe { GetWindowRect(hwnd, &mut rect) };
    if ok == 0 {
        return None;
    }
    Some(rect)
}

fn caret_screen_rect(hwnd: HWND, mut rect: RECT) -> Option<RECT> {
    if hwnd.is_null() {
        return None;
    }

    let mut top_left = POINT {
        x: rect.left,
        y: rect.top,
    };
    let mut bottom_right = POINT {
        x: rect.right,
        y: rect.bottom,
    };

    // SAFETY: points are valid and hwnd was provided by Win32 focus info.
    let top_left_ok = unsafe { ClientToScreen(hwnd, &mut top_left) };
    // SAFETY: points are valid and hwnd was provided by Win32 focus info.
    let bottom_right_ok = unsafe { ClientToScreen(hwnd, &mut bottom_right) };
    if top_left_ok == 0 || bottom_right_ok == 0 {
        return None;
    }

    rect.left = top_left.x;
    rect.top = top_left.y;
    rect.right = bottom_right.x.max(top_left.x + 2);
    rect.bottom = bottom_right.y.max(top_left.y + 2);

    Some(rect)
}

fn process_bundle_id(pid: u32) -> Option<String> {
    if pid == 0 {
        return None;
    }

    // SAFETY: OpenProcess is called read-only and handle is always closed.
    let process = unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid) };
    if process.is_null() {
        return Some(format!("pid:{pid}"));
    }

    let mut buffer = [0u16; 1024];
    let mut size = buffer.len() as u32;
    // SAFETY: process handle is valid, buffer is writable, size is initialized.
    let ok = unsafe { QueryFullProcessImageNameW(process, 0, buffer.as_mut_ptr(), &mut size) };
    // SAFETY: process was returned by OpenProcess and must be closed once.
    let _ = unsafe { CloseHandle(process) };

    if ok == 0 || size == 0 {
        return Some(format!("pid:{pid}"));
    }

    let text = String::from_utf16_lossy(&buffer[..size as usize]);
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return Some(format!("pid:{pid}"));
    }

    let path = Path::new(trimmed);
    if let Some(file_name) = path.file_stem().and_then(|value| value.to_str()) {
        let clean = file_name.trim();
        if !clean.is_empty() {
            return Some(clean.to_string());
        }
    }

    Some(trimmed.to_string())
}

#[cfg(test)]
mod tests {
    use super::is_known_text_input_class;

    #[test]
    fn known_text_input_classes_are_detected() {
        assert!(is_known_text_input_class("Edit"));
        assert!(is_known_text_input_class("RICHEDIT50W"));
        assert!(is_known_text_input_class("TextboxControl"));
        assert!(!is_known_text_input_class("Chrome_WidgetWin_1"));
    }
}
