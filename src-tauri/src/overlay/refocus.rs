use std::collections::HashMap;
use std::process::Command;
use std::sync::atomic::{AtomicBool, AtomicU64};
use std::sync::{Mutex, OnceLock};
use std::thread;
use std::time::{SystemTime, UNIX_EPOCH};
use tauri::{Emitter, Manager};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut, ShortcutState};

use crate::domain::anchor::AnchorPosition;
use crate::domain::config::{
    default_anchor_behavior, normalize_anchor_behavior, HotkeyConfig, HotkeyTriggerEvent,
    InputFocusTarget,
};
use crate::domain::geometry::to_u64_saturating;
use crate::platform;
use crate::{
    focused_text_anchor_snapshot, open_quick_window_with_action, point_maps_to_any_monitor,
};

pub(crate) const INPUT_TARGET_TTL_MS: u128 = 90_000;
const REFOCUS_CLICK_STABILIZE_MS: u64 = 45;
const REFOCUS_POST_RESTORE_MS: u64 = 35;

#[derive(Default)]
pub(crate) struct PendingQuickAction(pub(crate) Mutex<Option<String>>);

#[derive(Debug, Clone)]
pub(crate) struct ExternalAppTarget {
    pub(crate) bundle_id: String,
    pub(crate) captured_at_ms: u128,
}

#[derive(Default)]
pub(crate) struct LastExternalAppBundle(pub(crate) Mutex<Option<ExternalAppTarget>>);

#[derive(Default)]
pub(crate) struct LastAnchorPosition(pub(crate) Mutex<Option<AnchorPosition>>);

#[derive(Default)]
pub(crate) struct LastAnchorTimestamp(pub(crate) Mutex<Option<u128>>);

#[derive(Default)]
pub(crate) struct LastInputFocusTarget(pub(crate) Mutex<Option<InputFocusTarget>>);

#[derive(Default)]
pub(crate) struct AnchorBehaviorMode(pub(crate) Mutex<String>);

pub(crate) static ANCHOR_MONITOR_STARTED: AtomicBool = AtomicBool::new(false);
pub(crate) static SETTINGS_WINDOW_OPEN: AtomicBool = AtomicBool::new(false);
pub(crate) static TRAY_READY: AtomicBool = AtomicBool::new(false);
pub(crate) static APP_QUIT_REQUESTED: AtomicBool = AtomicBool::new(false);
pub(crate) static QUICK_OPEN_REQUEST_COUNTER: AtomicU64 = AtomicU64::new(1);
pub(crate) static UPDATE_CHECK_IN_FLIGHT: AtomicBool = AtomicBool::new(false);
#[cfg(target_os = "macos")]
pub(crate) static RUNNING_UNDER_ROSETTA: OnceLock<bool> = OnceLock::new();

pub(crate) fn now_millis() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .unwrap_or(0)
}

pub(crate) fn is_internal_app_bundle_id(bundle_id: &str) -> bool {
    let value = bundle_id.trim().to_lowercase();
    value.is_empty() || value.contains("whisloai") || value.contains("com.whisloai.app")
}

pub(crate) fn save_last_external_app_bundle(app: &tauri::AppHandle, bundle_id: &str) {
    let clean = bundle_id.trim();
    if clean.is_empty() || is_internal_app_bundle_id(clean) {
        return;
    }
    if let Some(state) = app.try_state::<LastExternalAppBundle>() {
        if let Ok(mut guard) = state.0.lock() {
            *guard = Some(ExternalAppTarget {
                bundle_id: clean.to_string(),
                captured_at_ms: now_millis(),
            });
        }
    }
}

pub(crate) fn clear_last_external_app_bundle(app: &tauri::AppHandle) {
    if let Some(state) = app.try_state::<LastExternalAppBundle>() {
        if let Ok(mut guard) = state.0.lock() {
            *guard = None;
        }
    }
}

pub(crate) fn last_external_app(app: &tauri::AppHandle) -> Option<String> {
    let target = last_external_app_target(app)?;
    Some(target.bundle_id)
}

pub(crate) fn last_external_app_target(app: &tauri::AppHandle) -> Option<ExternalAppTarget> {
    let state = app.try_state::<LastExternalAppBundle>()?;
    let guard = state.0.lock().ok()?;
    guard.clone()
}

pub(crate) fn save_last_anchor_position(app: &tauri::AppHandle, position: AnchorPosition) {
    if let Some(state) = app.try_state::<LastAnchorPosition>() {
        if let Ok(mut guard) = state.0.lock() {
            *guard = Some(position);
        }
    }
}

pub(crate) fn clear_last_anchor_position(app: &tauri::AppHandle) {
    if let Some(state) = app.try_state::<LastAnchorPosition>() {
        if let Ok(mut guard) = state.0.lock() {
            *guard = None;
        }
    }
}

pub(crate) fn last_anchor_position(app: &tauri::AppHandle) -> Option<AnchorPosition> {
    let state = app.try_state::<LastAnchorPosition>()?;
    let guard = state.0.lock().ok()?;
    *guard
}

pub(crate) fn save_last_anchor_timestamp(app: &tauri::AppHandle, timestamp: u128) {
    if let Some(state) = app.try_state::<LastAnchorTimestamp>() {
        if let Ok(mut guard) = state.0.lock() {
            *guard = Some(timestamp);
        }
    }
}

pub(crate) fn clear_last_anchor_timestamp(app: &tauri::AppHandle) {
    if let Some(state) = app.try_state::<LastAnchorTimestamp>() {
        if let Ok(mut guard) = state.0.lock() {
            *guard = None;
        }
    }
}

pub(crate) fn last_anchor_timestamp(app: &tauri::AppHandle) -> Option<u128> {
    let state = app.try_state::<LastAnchorTimestamp>()?;
    let guard = state.0.lock().ok()?;
    *guard
}

pub(crate) fn last_anchor_age_ms(app: &tauri::AppHandle) -> Option<u128> {
    let timestamp = last_anchor_timestamp(app)?;
    let now = now_millis();
    Some(now.saturating_sub(timestamp))
}

pub(crate) fn set_anchor_behavior_mode(app: &tauri::AppHandle, mode: &str) {
    if let Some(state) = app.try_state::<AnchorBehaviorMode>() {
        if let Ok(mut guard) = state.0.lock() {
            *guard = normalize_anchor_behavior(mode);
        }
    }
}

pub(crate) fn current_anchor_behavior_mode(app: &tauri::AppHandle) -> String {
    let Some(state) = app.try_state::<AnchorBehaviorMode>() else {
        return default_anchor_behavior();
    };
    let Ok(guard) = state.0.lock() else {
        return default_anchor_behavior();
    };
    if guard.trim().is_empty() {
        default_anchor_behavior()
    } else {
        normalize_anchor_behavior(guard.as_str())
    }
}

pub(crate) fn is_anchor_floating_mode(app: &tauri::AppHandle) -> bool {
    current_anchor_behavior_mode(app) == "floating"
}

pub(crate) fn save_last_input_focus_target(app: &tauri::AppHandle, target: InputFocusTarget) {
    if target.bundle_id.trim().is_empty() || is_internal_app_bundle_id(&target.bundle_id) {
        return;
    }
    if let Some(state) = app.try_state::<LastInputFocusTarget>() {
        if let Ok(mut guard) = state.0.lock() {
            *guard = Some(target);
        }
    }
}

pub(crate) fn clear_last_input_focus_target(app: &tauri::AppHandle) {
    if let Some(state) = app.try_state::<LastInputFocusTarget>() {
        if let Ok(mut guard) = state.0.lock() {
            *guard = None;
        }
    }
}

pub(crate) fn last_input_focus_target(app: &tauri::AppHandle) -> Option<InputFocusTarget> {
    let state = app.try_state::<LastInputFocusTarget>()?;
    let guard = state.0.lock().ok()?;
    guard.clone()
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct RefocusAttempt {
    pub(crate) attempted: bool,
    pub(crate) ok: bool,
    pub(crate) target_age_ms: Option<u128>,
}

pub(crate) fn refocus_last_input_target(app: &tauri::AppHandle) -> Result<RefocusAttempt, String> {
    let Some(target) = last_input_focus_target(app) else {
        return Ok(RefocusAttempt {
            attempted: false,
            ok: false,
            target_age_ms: None,
        });
    };

    let target_age_ms = now_millis().saturating_sub(target.captured_at_ms);
    if target_age_ms > INPUT_TARGET_TTL_MS || is_internal_app_bundle_id(&target.bundle_id) {
        clear_last_input_focus_target(app);
        return Ok(RefocusAttempt {
            attempted: false,
            ok: false,
            target_age_ms: Some(target_age_ms),
        });
    }

    if !point_maps_to_any_monitor(app, target.x, target.y) {
        clear_last_input_focus_target(app);
        return Ok(RefocusAttempt {
            attempted: false,
            ok: false,
            target_age_ms: Some(target_age_ms),
        });
    }

    platform::backend().refocus_point(
        target.x,
        target.y,
        REFOCUS_CLICK_STABILIZE_MS,
        REFOCUS_POST_RESTORE_MS,
    )?;

    Ok(RefocusAttempt {
        attempted: true,
        ok: true,
        target_age_ms: Some(target_age_ms),
    })
}

pub(crate) fn escape_applescript_string(value: &str) -> String {
    value.replace('\\', "\\\\").replace('"', "\\\"")
}

pub(crate) fn external_target_restore_reason(
    target: Option<&ExternalAppTarget>,
    now_ms: u128,
) -> (&'static str, Option<u128>) {
    let Some(target) = target else {
        return ("missing_target", None);
    };
    if is_internal_app_bundle_id(&target.bundle_id) {
        return ("invalid_target", None);
    }

    let target_age_ms = now_ms.saturating_sub(target.captured_at_ms);
    if target_age_ms > INPUT_TARGET_TTL_MS {
        return ("stale_target", Some(target_age_ms));
    }

    ("ready", Some(target_age_ms))
}

pub(crate) fn should_clear_external_cache_on_restore_error(error: &str) -> bool {
    let normalized = error.trim().to_lowercase();
    normalized == "not_running" || normalized.contains("not running")
}

pub(crate) fn should_clear_external_cache_on_restore_reason(reason: &str) -> bool {
    matches!(reason, "invalid_target" | "stale_target" | "not_running")
}

#[cfg(target_os = "macos")]
pub(crate) fn activate_bundle_id(bundle_id: &str) -> Result<(), String> {
    let escaped = escape_applescript_string(bundle_id);
    let script = format!(
        r#"
try
  tell application "System Events"
    if not (exists (application process 1 where bundle identifier is "{escaped}")) then
      return "ERR:NOT_RUNNING"
    end if
    set frontmost of (first application process whose bundle identifier is "{escaped}") to true
  end tell
  return "OK"
on error errMsg
  return "ERR:" & errMsg
end try
"#
    );

    let output = Command::new("osascript")
        .arg("-e")
        .arg(script)
        .output()
        .map_err(|e| format!("Could not reactivate target app: {e}"))?;

    if !output.status.success() {
        return Err("Could not reactivate target app.".to_string());
    }

    let raw = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if let Some(message) = raw.strip_prefix("ERR:") {
        return Err(message.trim().to_string());
    }

    Ok(())
}

#[cfg(not(target_os = "macos"))]
pub(crate) fn activate_bundle_id(_bundle_id: &str) -> Result<(), String> {
    Ok(())
}

#[cfg(target_os = "macos")]
pub(crate) fn frontmost_external_bundle_id() -> Option<String> {
    let script = r#"
try
  tell application "System Events"
    set frontProcess to first application process whose frontmost is true
    set processBundleId to ""
    try
      set processBundleId to bundle identifier of frontProcess as string
    end try
    return processBundleId
  end tell
on error
  return ""
end try
"#;

    let output = Command::new("osascript")
        .arg("-e")
        .arg(script)
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let raw = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if raw.is_empty() || is_internal_app_bundle_id(&raw) {
        return None;
    }
    Some(raw)
}

#[cfg(not(target_os = "macos"))]
pub(crate) fn frontmost_external_bundle_id() -> Option<String> {
    None
}

pub(crate) fn refresh_last_external_app_bundle(app: &tauri::AppHandle) {
    if let Some(bundle_id) = frontmost_external_bundle_id() {
        save_last_external_app_bundle(app, &bundle_id);
    }
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct RestoreExternalAppAttempt {
    pub(crate) attempted: bool,
    pub(crate) ok: bool,
    pub(crate) target_age_ms: Option<u128>,
    pub(crate) reason: &'static str,
}

pub(crate) fn log_external_restore_trace(context: &str, attempt: RestoreExternalAppAttempt) {
    if !cfg!(debug_assertions) {
        return;
    }
    let payload = serde_json::json!({
        "event": "external_restore_trace",
        "context": context,
        "attempted": attempt.attempted,
        "ok": attempt.ok,
        "target_age_ms": attempt.target_age_ms.map(to_u64_saturating),
        "reason": attempt.reason,
    });
    log::info!("{payload}");
}

pub(crate) fn restore_last_external_app(app: &tauri::AppHandle) -> RestoreExternalAppAttempt {
    let now = now_millis();
    let target = last_external_app_target(app);
    let (reason, target_age_ms) = external_target_restore_reason(target.as_ref(), now);

    if reason != "ready" {
        if should_clear_external_cache_on_restore_reason(reason) {
            clear_last_external_app_bundle(app);
            clear_last_input_focus_target(app);
        }
        return RestoreExternalAppAttempt {
            attempted: false,
            ok: false,
            target_age_ms,
            reason,
        };
    }

    let Some(target) = target else {
        return RestoreExternalAppAttempt {
            attempted: false,
            ok: false,
            target_age_ms,
            reason: "missing_target",
        };
    };

    let bundle_id = target.bundle_id;
    if let Err(error) = activate_bundle_id(&bundle_id) {
        let normalized_reason = if should_clear_external_cache_on_restore_error(&error) {
            "not_running"
        } else {
            "restore_error"
        };
        if should_clear_external_cache_on_restore_reason(normalized_reason) {
            clear_last_external_app_bundle(app);
            clear_last_input_focus_target(app);
        }
        if !cfg!(debug_assertions) {
            log::warn!("Could not restore focus to previous app '{bundle_id}': {error}");
        }
        return RestoreExternalAppAttempt {
            attempted: true,
            ok: false,
            target_age_ms,
            reason: normalized_reason,
        };
    }

    thread::sleep(std::time::Duration::from_millis(120));
    RestoreExternalAppAttempt {
        attempted: true,
        ok: true,
        target_age_ms,
        reason: "ok",
    }
}

pub(crate) fn refresh_last_input_focus_target_from_snapshot(app: &tauri::AppHandle) {
    let Some(snapshot) = focused_text_anchor_snapshot(app) else {
        return;
    };

    let Some(bundle_id) = snapshot.bundle_id.as_deref() else {
        return;
    };

    save_last_external_app_bundle(app, bundle_id);

    if let Some((focus_x, focus_y)) = snapshot.input_focus_point {
        save_last_input_focus_target(
            app,
            InputFocusTarget {
                bundle_id: bundle_id.to_string(),
                x: focus_x.max(1),
                y: focus_y.max(1),
                captured_at_ms: now_millis(),
            },
        );
    }
}

pub(crate) fn normalize_hotkeys(hotkeys: &HotkeyConfig) -> HotkeyConfig {
    HotkeyConfig {
        open_app: hotkeys.open_app.trim().to_string(),
        open_dictate_translate: hotkeys.open_dictate_translate.trim().to_string(),
    }
}

pub(crate) fn validate_hotkeys(hotkeys: &HotkeyConfig) -> Result<Vec<(String, String)>, String> {
    let bindings = vec![
        ("open-app".to_string(), hotkeys.open_app.trim().to_string()),
        (
            "open-dictate-translate".to_string(),
            hotkeys.open_dictate_translate.trim().to_string(),
        ),
    ];

    for (action, shortcut) in &bindings {
        if shortcut.is_empty() {
            return Err(format!("Shortcut is required for action '{action}'."));
        }
    }

    let mut seen = HashMap::new();
    for (action, shortcut) in &bindings {
        let parsed = shortcut
            .parse::<Shortcut>()
            .map_err(|e| format!("Invalid shortcut '{shortcut}' for '{action}': {e}"))?;

        if let Some(previous_action) = seen.insert(parsed.id(), action.clone()) {
            return Err(format!(
                "Shortcut conflict: '{shortcut}' is used by '{previous_action}' and '{action}'."
            ));
        }
    }

    Ok(bindings)
}

pub(crate) fn save_pending_quick_action(app: &tauri::AppHandle, action: String) {
    if let Some(state) = app.try_state::<PendingQuickAction>() {
        if let Ok(mut pending) = state.0.lock() {
            *pending = Some(action);
        }
    }
}

pub(crate) fn emit_quick_action(app: &tauri::AppHandle, action: &str) {
    save_pending_quick_action(app, action.to_string());
    let _ = app.emit(
        "quick-action",
        HotkeyTriggerEvent {
            action: action.to_string(),
        },
    );
}

pub(crate) fn handle_hotkey_trigger(app: &tauri::AppHandle, action: &str) {
    let quick_action = match action {
        "open-app" => "open-app",
        "open-dictate-translate" => "open-dictate-translate",
        _ => "open-app",
    };

    if let Err(error) = open_quick_window_with_action(app, Some(quick_action.to_string())) {
        log::warn!("Hotkey action '{action}' failed: {error}");
    }
}

pub(crate) fn register_hotkeys(
    app: &tauri::AppHandle,
    hotkeys: &HotkeyConfig,
) -> Result<(), String> {
    let bindings = validate_hotkeys(hotkeys)?;
    app.global_shortcut()
        .unregister_all()
        .map_err(|e| format!("Could not clear previous shortcuts: {e}"))?;

    for (action, shortcut) in bindings {
        let action_for_handler = action.clone();
        let shortcut_for_error = shortcut.clone();
        let parsed_shortcut = shortcut
            .parse::<Shortcut>()
            .map_err(|e| format!("Invalid shortcut '{shortcut_for_error}': {e}"))?;
        app.global_shortcut()
            .on_shortcut(parsed_shortcut, move |app, _shortcut, event| {
                if event.state == ShortcutState::Pressed {
                    handle_hotkey_trigger(app, &action_for_handler);
                }
            })
            .map_err(|e| {
                format!(
                    "Could not register shortcut '{shortcut_for_error}'. It may already be taken by another app: {e}"
                )
            })?;
    }

    Ok(())
}
