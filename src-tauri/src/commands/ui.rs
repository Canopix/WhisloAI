use crate::platform;
use crate::*;
use std::sync::atomic::Ordering;
use std::thread;
use tauri::{LogicalSize, Manager, PhysicalPosition, Position, Size};
use tauri_plugin_clipboard_manager::ClipboardExt;

#[tauri::command]
pub(crate) fn log_dictation_trace(
    event: String,
    payload: Option<serde_json::Value>,
    level: Option<String>,
) -> Result<(), String> {
    let event = event.trim();
    if event.is_empty() {
        return Err("Event is empty.".to_string());
    }

    let payload_text = payload
        .map(|value| value.to_string())
        .unwrap_or_else(|| "{}".to_string());
    let line = format!("dictation_trace event={} payload={}", event, payload_text);
    match level
        .unwrap_or_else(|| "info".to_string())
        .trim()
        .to_lowercase()
        .as_str()
    {
        "warn" | "error" => log::warn!("{line}"),
        _ => log::info!("{line}"),
    }
    Ok(())
}

#[tauri::command]
pub(crate) fn open_external_url(url: String) -> Result<(), String> {
    let value = url.trim();
    if value.is_empty() {
        return Err("URL is empty.".to_string());
    }
    if !(value.starts_with("https://") || value.starts_with("http://")) {
        return Err("Only http/https URLs are allowed.".to_string());
    }

    platform::backend().open_external_url(value)
}

#[tauri::command]
pub(crate) fn open_settings_window(app: tauri::AppHandle) -> Result<(), String> {
    SETTINGS_WINDOW_OPEN.store(true, Ordering::SeqCst);
    if let Some(quick) = app.get_webview_window("quick") {
        let _ = quick.hide();
    }
    if let Some(anchor) = app.get_webview_window("anchor") {
        let _ = anchor.hide();
    }

    if let Some(existing) = app.get_webview_window("settings") {
        if cfg!(debug_assertions) {
            if let Ok(url) = settings_external_url(true) {
                let _ = existing.navigate(url);
            }
        }
        let _ = existing.unminimize();
        existing
            .show()
            .map_err(|e| format!("Could not show settings window: {e}"))?;
        existing
            .set_focus()
            .map_err(|e| format!("Could not focus settings window: {e}"))?;
        return Ok(());
    }

    let window = tauri::WebviewWindowBuilder::new(&app, "settings", settings_webview_url(true)?)
        .title("WhisloAI Settings")
        .inner_size(980.0, 760.0)
        .min_inner_size(760.0, 600.0)
        .resizable(true)
        .build()
        .map_err(|e| format!("Could not create settings window: {e}"))?;

    window.on_window_event(|event| {
        if matches!(
            event,
            tauri::WindowEvent::CloseRequested { .. } | tauri::WindowEvent::Destroyed
        ) {
            SETTINGS_WINDOW_OPEN.store(false, Ordering::SeqCst);
        }
    });

    window
        .set_focus()
        .map_err(|e| format!("Could not focus settings window: {e}"))?;
    Ok(())
}

#[tauri::command]
pub(crate) fn open_widget_window(app: tauri::AppHandle) -> Result<(), String> {
    open_quick_window_with_action(&app, Some("open-app".to_string()))
}

#[tauri::command]
pub(crate) fn open_quick_window(app: tauri::AppHandle) -> Result<(), String> {
    open_quick_window_with_action(&app, Some("open-app".to_string()))
}

#[tauri::command]
pub(crate) fn start_anchor_window_drag(app: tauri::AppHandle) -> Result<(), String> {
    if !is_anchor_floating_mode(&app) {
        return Ok(());
    }
    let anchor = app
        .get_webview_window("anchor")
        .ok_or_else(|| "Anchor window not found.".to_string())?;
    anchor
        .start_dragging()
        .map_err(|e| format!("Could not start anchor drag: {e}"))
}

#[tauri::command]
pub(crate) fn remember_anchor_window_position(app: tauri::AppHandle) -> Result<(), String> {
    let Some(anchor) = app.get_webview_window("anchor") else {
        return Ok(());
    };
    if let Ok(position) = anchor.outer_position() {
        let (x, y) = clamp_anchor_window_position(&anchor, position.x, position.y);
        let _ = anchor.set_position(Position::Physical(PhysicalPosition::new(x, y)));
        save_last_anchor_position(&app, AnchorPosition { x, y });
        save_last_anchor_timestamp(&app, now_millis());
    }
    Ok(())
}

#[tauri::command]
pub(crate) fn set_quick_window_expanded(
    app: tauri::AppHandle,
    expanded: bool,
) -> Result<(), String> {
    if let Some(window) = app.get_webview_window("quick") {
        let target_width = if expanded {
            QUICK_WINDOW_WIDTH_EXPANDED
        } else {
            QUICK_WINDOW_WIDTH_COMPACT
        };
        let target_height = if expanded {
            QUICK_WINDOW_HEIGHT_EXPANDED
        } else {
            QUICK_WINDOW_HEIGHT_COMPACT
        };

        window
            .set_size(Size::Logical(LogicalSize::new(target_width, target_height)))
            .map_err(|e| format!("Could not resize quick window: {e}"))?;

        position_quick_window_near_anchor(&app);
    }

    Ok(())
}

#[tauri::command]
pub(crate) fn close_quick_window(app: tauri::AppHandle) -> Result<(), String> {
    let window = app
        .get_webview_window("quick")
        .ok_or_else(|| "Quick window not found.".to_string())?;
    window
        .hide()
        .map_err(|e| format!("Could not hide quick window: {e}"))
}

#[tauri::command]
pub(crate) fn open_main_mode(app: tauri::AppHandle, mode: String) -> Result<(), String> {
    let normalized = mode.trim().to_lowercase();
    let action = match normalized.as_str() {
        "translate" => "open-dictate-translate",
        "dictate" => "open-dictate-translate-record",
        "app" => "open-app",
        _ => return Err("Unknown panel mode.".to_string()),
    };
    open_quick_window_with_action(&app, Some(action.to_string()))
}

#[tauri::command]
pub(crate) fn capture_selected_text(app: tauri::AppHandle) -> Result<String, String> {
    let restore_attempt = restore_last_external_app(&app);
    log_external_restore_trace("capture_selected_text", restore_attempt);
    thread::sleep(std::time::Duration::from_millis(150));
    simulate_copy_shortcut()?;
    thread::sleep(std::time::Duration::from_millis(120));
    let selected = app
        .clipboard()
        .read_text()
        .map_err(|e| format!("Could not read selected text from clipboard: {e}"))?;
    let value = selected.trim().to_string();
    if value.is_empty() {
        return Err("No selected text detected. Select text and try again.".to_string());
    }
    Ok(value)
}

#[tauri::command]
pub(crate) fn consume_pending_quick_action(
    state: tauri::State<'_, PendingQuickAction>,
) -> Option<String> {
    let mut pending = state.0.lock().ok()?;
    pending.take()
}
