use std::sync::atomic::Ordering;
use std::time::Instant;

use tauri::{LogicalSize, Manager, PhysicalPosition, Position, Size};

use crate::domain::geometry::{
    point_in_rect, sanitize_scale_factor, scale_for_logical_point_in_rects, to_u64_saturating,
};
use crate::overlay::anchor_monitor::start_anchor_monitor_once;
use crate::overlay::refocus::*;

pub(crate) const QUICK_WINDOW_WIDTH_COMPACT: f64 = 252.0;
pub(crate) const QUICK_WINDOW_WIDTH_EXPANDED: f64 = 252.0;
pub(crate) const QUICK_WINDOW_HEIGHT_COMPACT: f64 = 64.0;
pub(crate) const QUICK_WINDOW_HEIGHT_EXPANDED: f64 = 96.0;

pub(crate) fn show_main_window_for_onboarding(app: &tauri::AppHandle) {
    if let Some(main) = app.get_webview_window("main") {
        let _ = main.show();
        let _ = main.set_focus();
    }
}

pub(crate) fn hide_main_window(app: &tauri::AppHandle) {
    if let Some(main) = app.get_webview_window("main") {
        let _ = main.hide();
    }
}

pub(crate) fn ensure_anchor_window(app: &tauri::AppHandle) -> Result<(), String> {
    if let Some(anchor) = app.get_webview_window("anchor") {
        let _ = anchor.set_size(Size::Logical(LogicalSize::new(40.0, 40.0)));
        let _ = anchor.set_min_size(Some(Size::Logical(LogicalSize::new(40.0, 40.0))));
        let _ = anchor.set_shadow(false);
        return Ok(());
    }

    tauri::WebviewWindowBuilder::new(app, "anchor", tauri::WebviewUrl::App("anchor.html".into()))
        .title("WhisloAI Anchor")
        .inner_size(40.0, 40.0)
        .min_inner_size(40.0, 40.0)
        .resizable(false)
        .transparent(true)
        .always_on_top(true)
        .decorations(false)
        .shadow(false)
        .accept_first_mouse(true)
        .skip_taskbar(true)
        .visible(false)
        .build()
        .map_err(|e| format!("Could not create anchor window: {e}"))?;

    Ok(())
}

pub(crate) fn ensure_quick_window(app: &tauri::AppHandle) -> Result<(), String> {
    if let Some(quick) = app.get_webview_window("quick") {
        let _ = quick.set_shadow(false);
        return Ok(());
    }

    tauri::WebviewWindowBuilder::new(app, "quick", tauri::WebviewUrl::App("widget.html".into()))
        .title("WhisloAI Quick")
        .inner_size(QUICK_WINDOW_WIDTH_COMPACT, QUICK_WINDOW_HEIGHT_COMPACT)
        .min_inner_size(QUICK_WINDOW_WIDTH_COMPACT, QUICK_WINDOW_HEIGHT_COMPACT)
        .resizable(false)
        .transparent(true)
        .always_on_top(true)
        .decorations(false)
        .shadow(false)
        .accept_first_mouse(true)
        .skip_taskbar(true)
        .visible(false)
        .build()
        .map_err(|e| format!("Could not create quick window: {e}"))?;

    Ok(())
}

pub(crate) fn monitor_from_cursor(window: &tauri::WebviewWindow) -> Option<tauri::Monitor> {
    let cursor = window.cursor_position().ok()?;
    window.monitor_from_point(cursor.x, cursor.y).ok().flatten()
}

pub(crate) fn resolve_monitor_for_position(
    window: &tauri::WebviewWindow,
    x: i32,
    y: i32,
) -> Option<tauri::Monitor> {
    window
        .monitor_from_point(x as f64, y as f64)
        .ok()
        .flatten()
        .or_else(|| monitor_from_cursor(window))
        .or_else(|| window.current_monitor().ok().flatten())
}

#[cfg(target_os = "macos")]
pub(crate) fn monitor_scale_factor_for_logical_point(
    app: &tauri::AppHandle,
    logical_x: i32,
    logical_y: i32,
) -> f64 {
    let Some(anchor) = app.get_webview_window("anchor") else {
        return 1.0;
    };

    if let Ok(monitors) = anchor.available_monitors() {
        let candidates = monitors
            .iter()
            .map(|monitor| {
                let work = monitor.work_area();
                (
                    work.position.x,
                    work.position.y,
                    work.size.width as i32,
                    work.size.height as i32,
                    monitor.scale_factor(),
                )
            })
            .collect::<Vec<(i32, i32, i32, i32, f64)>>();
        if let Some(scale) = scale_for_logical_point_in_rects(logical_x, logical_y, &candidates) {
            return scale;
        }
    }

    if let Some(cursor_monitor) = monitor_from_cursor(&anchor) {
        return sanitize_scale_factor(cursor_monitor.scale_factor());
    }

    anchor
        .current_monitor()
        .ok()
        .flatten()
        .map(|monitor| sanitize_scale_factor(monitor.scale_factor()))
        .unwrap_or(1.0)
}

pub(crate) fn app_monitor_probe_window(app: &tauri::AppHandle) -> Option<tauri::WebviewWindow> {
    app.get_webview_window("anchor")
        .or_else(|| app.get_webview_window("quick"))
        .or_else(|| app.get_webview_window("main"))
        .or_else(|| app.get_webview_window("settings"))
}

pub(crate) fn point_maps_to_any_monitor(app: &tauri::AppHandle, x: i32, y: i32) -> bool {
    let Some(window) = app_monitor_probe_window(app) else {
        return true;
    };

    if window
        .monitor_from_point(x as f64, y as f64)
        .ok()
        .flatten()
        .is_some()
    {
        return true;
    }

    if let Ok(monitors) = window.available_monitors() {
        return monitors.iter().any(|monitor| {
            let work = monitor.work_area();
            point_in_rect(
                x,
                y,
                work.position.x,
                work.position.y,
                work.size.width as i32,
                work.size.height as i32,
            )
        });
    }

    true
}

pub(crate) fn clamp_quick_window_position(
    anchor: &tauri::WebviewWindow,
    default_x: i32,
    default_y: i32,
    quick_width: i32,
    quick_height: i32,
) -> (i32, i32) {
    let monitor = resolve_monitor_for_position(anchor, default_x, default_y);

    if let Some(monitor) = monitor {
        let work = monitor.work_area();
        let min_x = work.position.x + 8;
        let min_y = work.position.y + 8;
        let max_x = (work.position.x + work.size.width as i32 - quick_width - 8).max(min_x);
        let max_y = (work.position.y + work.size.height as i32 - quick_height - 8).max(min_y);
        (default_x.clamp(min_x, max_x), default_y.clamp(min_y, max_y))
    } else {
        (default_x, default_y)
    }
}

pub(crate) fn clamp_anchor_window_position(
    anchor: &tauri::WebviewWindow,
    default_x: i32,
    default_y: i32,
) -> (i32, i32) {
    let anchor_size = anchor.outer_size().unwrap_or_default();
    let anchor_width = anchor_size.width as i32;
    let anchor_height = anchor_size.height as i32;
    let monitor = resolve_monitor_for_position(anchor, default_x, default_y);

    if let Some(monitor) = monitor {
        let work = monitor.work_area();
        let min_x = work.position.x + 8;
        let min_y = work.position.y + 8;
        let max_x = (work.position.x + work.size.width as i32 - anchor_width - 8).max(min_x);
        let max_y = (work.position.y + work.size.height as i32 - anchor_height - 8).max(min_y);
        (default_x.clamp(min_x, max_x), default_y.clamp(min_y, max_y))
    } else {
        (default_x, default_y)
    }
}

pub(crate) fn position_quick_window_near_anchor(
    app: &tauri::AppHandle,
) -> (&'static str, Option<u128>) {
    let Some(quick) = app.get_webview_window("quick") else {
        return ("quick-window-missing", None);
    };
    let quick_size = quick.outer_size().unwrap_or_default();
    let quick_width = quick_size.width as i32;
    let quick_height = quick_size.height as i32;

    if let Some(position) = last_anchor_position(app) {
        if let Some(anchor) = app.get_webview_window("anchor") {
            let default_x = position.x;
            let default_y = position.y;
            let (x, y) = clamp_quick_window_position(
                &anchor,
                default_x,
                default_y,
                quick_width,
                quick_height,
            );
            let _ = quick.set_position(Position::Physical(PhysicalPosition::new(x, y)));
            return ("anchor-cache", last_anchor_age_ms(app));
        }
    }

    if let Some(anchor) = app.get_webview_window("anchor") {
        if anchor.is_visible().unwrap_or(false) {
            if let Ok(pos) = anchor.outer_position() {
                let default_x = pos.x;
                let default_y = pos.y;
                let (x, y) = clamp_quick_window_position(
                    &anchor,
                    default_x,
                    default_y,
                    quick_width,
                    quick_height,
                );
                let _ = quick.set_position(Position::Physical(PhysicalPosition::new(x, y)));
                return ("anchor-window", None);
            }
        }

        if let Ok(cursor) = anchor.cursor_position() {
            let default_x = cursor.x.round() as i32;
            let default_y = cursor.y.round() as i32;
            let (x, y) = clamp_quick_window_position(
                &anchor,
                default_x,
                default_y,
                quick_width,
                quick_height,
            );
            let _ = quick.set_position(Position::Physical(PhysicalPosition::new(x, y)));
            return ("cursor-fallback", None);
        }

        if let Ok(pos) = anchor.outer_position() {
            let default_x = pos.x;
            let default_y = pos.y;
            let (x, y) = clamp_quick_window_position(
                &anchor,
                default_x,
                default_y,
                quick_width,
                quick_height,
            );
            let _ = quick.set_position(Position::Physical(PhysicalPosition::new(x, y)));
            return ("anchor-window", None);
        }
    }

    ("unpositioned", None)
}

pub(crate) fn log_quick_open_trace(
    request_id: u64,
    action: Option<&str>,
    outcome: &str,
    error: Option<&str>,
    position_source: &str,
    cache_age_ms: Option<u128>,
    external_cache_hit: bool,
    phases: &[(String, u128)],
    total_ms: u128,
) {
    let phase_payload = phases
        .iter()
        .map(|(name, elapsed)| {
            (
                name.clone(),
                serde_json::Value::from(to_u64_saturating(*elapsed)),
            )
        })
        .collect::<serde_json::Map<String, serde_json::Value>>();

    let payload = serde_json::json!({
        "event": "quick_open_trace",
        "request_id": request_id,
        "action": action.unwrap_or("open-app"),
        "outcome": outcome,
        "error": error.unwrap_or(""),
        "position_source": position_source,
        "cache_age_ms": cache_age_ms.map(to_u64_saturating),
        "external_cache_hit": external_cache_hit,
        "phases_ms": phase_payload,
        "total_ms": to_u64_saturating(total_ms),
    });

    if cfg!(debug_assertions) {
        log::info!("{payload}");
        return;
    }

    if outcome != "ok" || total_ms >= 180 {
        log::warn!("{payload}");
    }
}

pub(crate) fn log_auto_insert_trace(
    outcome: &str,
    error: Option<&str>,
    target_age_ms: Option<u128>,
    refocus_attempted: bool,
    refocus_ok: bool,
    paste_ok: bool,
    external_restore_reason: &str,
    total_ms: u128,
) {
    let payload = serde_json::json!({
        "event": "auto_insert_trace",
        "outcome": outcome,
        "error": error.unwrap_or(""),
        "target_age_ms": target_age_ms.map(to_u64_saturating),
        "refocus_attempted": refocus_attempted,
        "refocus_ok": refocus_ok,
        "paste_ok": paste_ok,
        "external_restore_reason": external_restore_reason,
        "total_ms": to_u64_saturating(total_ms),
    });

    if cfg!(debug_assertions) {
        log::info!("{payload}");
        return;
    }

    if outcome != "ok" || !paste_ok {
        log::warn!("{payload}");
    }
}

pub(crate) fn open_quick_window_with_action(
    app: &tauri::AppHandle,
    action: Option<String>,
) -> Result<(), String> {
    let request_id = QUICK_OPEN_REQUEST_COUNTER.fetch_add(1, Ordering::Relaxed);
    let total_started = Instant::now();
    let mut phases: Vec<(String, u128)> = Vec::with_capacity(6);
    let mut position_source = "unpositioned";
    let mut cache_age_ms = None;
    let mut external_cache_hit = false;

    let result = (|| -> Result<(), String> {
        let remember_started = Instant::now();
        refresh_last_external_app_bundle(app);
        external_cache_hit = last_external_app(app).is_some();
        phases.push((
            "remember_last_external_app".to_string(),
            remember_started.elapsed().as_millis(),
        ));

        let ensure_started = Instant::now();
        ensure_quick_window(app)?;
        phases.push((
            "ensure_quick_window".to_string(),
            ensure_started.elapsed().as_millis(),
        ));

        let position_started = Instant::now();
        let (source, cache_age) = position_quick_window_near_anchor(app);
        position_source = source;
        cache_age_ms = cache_age;
        phases.push((
            "position_quick_window".to_string(),
            position_started.elapsed().as_millis(),
        ));

        let window = app
            .get_webview_window("quick")
            .ok_or_else(|| "Quick window not found.".to_string())?;

        let hide_anchor_started = Instant::now();
        if let Some(anchor) = app.get_webview_window("anchor") {
            let _ = anchor.hide();
        }
        phases.push((
            "hide_anchor_window".to_string(),
            hide_anchor_started.elapsed().as_millis(),
        ));

        if window.is_minimized().unwrap_or(false) {
            let unminimize_started = Instant::now();
            let _ = window.unminimize();
            phases.push((
                "unminimize_window".to_string(),
                unminimize_started.elapsed().as_millis(),
            ));
        }

        let show_started = Instant::now();
        window
            .show()
            .map_err(|e| format!("Could not show quick window: {e}"))?;
        phases.push((
            "show_window".to_string(),
            show_started.elapsed().as_millis(),
        ));

        let focus_started = Instant::now();
        if let Err(error) = window.set_focus() {
            log::warn!("Could not focus quick window: {error}");
        }
        phases.push((
            "focus_window".to_string(),
            focus_started.elapsed().as_millis(),
        ));

        if let Some(value) = action.as_deref() {
            let emit_started = Instant::now();
            emit_quick_action(app, value);
            phases.push((
                "emit_quick_action".to_string(),
                emit_started.elapsed().as_millis(),
            ));
        }

        Ok(())
    })();

    let total_ms = total_started.elapsed().as_millis();
    let (outcome, error) = match &result {
        Ok(_) => ("ok", None),
        Err(error) => ("error", Some(error.as_str())),
    };
    log_quick_open_trace(
        request_id,
        action.as_deref(),
        outcome,
        error,
        position_source,
        cache_age_ms,
        external_cache_hit,
        &phases,
        total_ms,
    );

    result
}

pub(crate) fn activate_overlay_mode(app: &tauri::AppHandle) -> Result<(), String> {
    ensure_anchor_window(app)?;
    ensure_quick_window(app)?;
    hide_main_window(app);
    start_anchor_monitor_once(app.clone());

    Ok(())
}

pub(crate) fn settings_external_url(cache_bust: bool) -> Result<tauri::Url, String> {
    let mut url = "http://127.0.0.1:4173/settings.html".to_string();
    if cache_bust {
        url.push_str(&format!("?v={}", now_millis()));
    }
    tauri::Url::parse(&url).map_err(|e| format!("Invalid settings URL '{url}': {e}"))
}

pub(crate) fn settings_webview_url(cache_bust: bool) -> Result<tauri::WebviewUrl, String> {
    if cfg!(debug_assertions) {
        return settings_external_url(cache_bust).map(tauri::WebviewUrl::External);
    }
    Ok(tauri::WebviewUrl::App("settings.html".into()))
}
