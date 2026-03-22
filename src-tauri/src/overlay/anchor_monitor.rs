use std::sync::atomic::Ordering;
use std::thread;

use tauri::{Manager, PhysicalPosition, Position};

use crate::domain::anchor::{
    anchor_monitor_poll_interval_ms, contextual_anchor_tracking_supported,
    focused_text_anchor_probe, log_contextual_anchor_decision, should_hide_contextual_anchor,
    AnchorPosition, TimedContextualSnapshot, ANCHOR_HIDE_DEBOUNCE_MS,
    ANCHOR_LAST_VALID_SNAPSHOT_TTL_MS, ANCHOR_MONITOR_ACTIVE_POLL_MS,
};
use crate::domain::config::InputFocusTarget;
use crate::overlay::refocus::*;
use crate::overlay::windows::clamp_anchor_window_position;

pub(crate) fn start_anchor_monitor_once(app: tauri::AppHandle) {
    if ANCHOR_MONITOR_STARTED
        .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
        .is_err()
    {
        return;
    }

    thread::spawn(move || {
        let mut last: Option<AnchorPosition> = None;
        let mut last_contextual_decision: Option<String> = None;
        let mut last_contextual_reason: Option<String> = None;
        let mut hide_candidate_since_ms: Option<u128> = None;
        let mut last_valid_contextual_snapshot: Option<TimedContextualSnapshot> = None;
        let contextual_tracking_supported = contextual_anchor_tracking_supported();

        loop {
            let Some(anchor) = app.get_webview_window("anchor") else {
                thread::sleep(std::time::Duration::from_millis(
                    ANCHOR_MONITOR_ACTIVE_POLL_MS,
                ));
                continue;
            };
            let floating_mode = is_anchor_floating_mode(&app);
            let poll_interval_ms =
                anchor_monitor_poll_interval_ms(floating_mode, contextual_tracking_supported);

            if SETTINGS_WINDOW_OPEN.load(Ordering::SeqCst) {
                let _ = anchor.hide();
                last = None;
                last_contextual_decision = None;
                last_contextual_reason = None;
                hide_candidate_since_ms = None;
                last_valid_contextual_snapshot = None;
                if !floating_mode {
                    clear_last_anchor_position(&app);
                    clear_last_anchor_timestamp(&app);
                }
                clear_last_input_focus_target(&app);
                thread::sleep(std::time::Duration::from_millis(poll_interval_ms));
                continue;
            }

            if let Some(quick) = app.get_webview_window("quick") {
                if quick.is_visible().unwrap_or(false) {
                    let _ = anchor.hide();
                    last = None;
                    last_contextual_decision = None;
                    last_contextual_reason = None;
                    hide_candidate_since_ms = None;
                    last_valid_contextual_snapshot = None;
                    thread::sleep(std::time::Duration::from_millis(poll_interval_ms));
                    continue;
                }
            }

            if !floating_mode && !contextual_tracking_supported {
                let _ = anchor.hide();
                last = None;
                last_contextual_decision = None;
                last_contextual_reason = None;
                hide_candidate_since_ms = None;
                last_valid_contextual_snapshot = None;
                clear_last_anchor_position(&app);
                clear_last_anchor_timestamp(&app);
                clear_last_input_focus_target(&app);
                thread::sleep(std::time::Duration::from_millis(poll_interval_ms));
                continue;
            }

            if floating_mode {
                clear_last_input_focus_target(&app);
                last_contextual_decision = None;
                last_contextual_reason = None;
                hide_candidate_since_ms = None;
                last_valid_contextual_snapshot = None;

                let next = last_anchor_position(&app)
                    .or_else(|| {
                        anchor
                            .outer_position()
                            .ok()
                            .map(|pos| AnchorPosition { x: pos.x, y: pos.y })
                    })
                    .or_else(|| {
                        anchor.cursor_position().ok().map(|cursor| AnchorPosition {
                            x: cursor.x as i32 + 12,
                            y: cursor.y as i32 + 12,
                        })
                    });

                if let Some(position) = next {
                    let (x, y) = clamp_anchor_window_position(&anchor, position.x, position.y);
                    save_last_anchor_position(&app, AnchorPosition { x, y });
                    save_last_anchor_timestamp(&app, now_millis());
                    if last != Some(AnchorPosition { x, y }) {
                        let _ =
                            anchor.set_position(Position::Physical(PhysicalPosition::new(x, y)));
                    }
                    let _ = anchor.show();
                    last = Some(AnchorPosition { x, y });
                } else {
                    let _ = anchor.show();
                    last = None;
                }

                thread::sleep(std::time::Duration::from_millis(poll_interval_ms));
                continue;
            }

            let probe = focused_text_anchor_probe(&app);
            let now_ms = now_millis();

            if let Some(entry) = probe.snapshot.as_ref() {
                hide_candidate_since_ms = None;
                last_valid_contextual_snapshot = Some(TimedContextualSnapshot {
                    snapshot: entry.clone(),
                    captured_at_ms: now_ms,
                });
                save_last_anchor_position(&app, entry.position);
                save_last_anchor_timestamp(&app, now_ms);
                if let Some(bundle_id) = entry.bundle_id.as_deref() {
                    save_last_external_app_bundle(&app, bundle_id);
                    if let Some((focus_x, focus_y)) = entry.input_focus_point {
                        save_last_input_focus_target(
                            &app,
                            InputFocusTarget {
                                bundle_id: bundle_id.to_string(),
                                x: focus_x.max(1),
                                y: focus_y.max(1),
                                captured_at_ms: now_ms,
                            },
                        );
                    } else {
                        clear_last_input_focus_target(&app);
                    }
                } else {
                    clear_last_input_focus_target(&app);
                }
            } else if hide_candidate_since_ms.is_none() {
                hide_candidate_since_ms = Some(now_ms);
            }

            let should_hide = should_hide_contextual_anchor(
                now_ms,
                hide_candidate_since_ms,
                last_valid_contextual_snapshot
                    .as_ref()
                    .map(|snapshot| snapshot.captured_at_ms),
                ANCHOR_HIDE_DEBOUNCE_MS,
                ANCHOR_LAST_VALID_SNAPSHOT_TTL_MS,
            );

            let effective_snapshot = if let Some(entry) = probe.snapshot.clone() {
                Some(entry)
            } else if should_hide {
                last_valid_contextual_snapshot = None;
                None
            } else {
                last_valid_contextual_snapshot
                    .as_ref()
                    .map(|snapshot| snapshot.snapshot.clone())
            };

            if probe.snapshot.is_none() && should_hide {
                clear_last_anchor_position(&app);
                clear_last_anchor_timestamp(&app);
                clear_last_input_focus_target(&app);
            }

            let next = effective_snapshot.as_ref().map(|entry| entry.position);
            let decision = if next.is_some() { "show" } else { "hide" };
            let should_log = last_contextual_decision.as_deref() != Some(decision)
                || last_contextual_reason.as_deref() != Some(probe.reason.as_str());
            if should_log {
                let bundle_for_log = probe
                    .bundle_id
                    .as_deref()
                    .or_else(|| {
                        probe
                            .snapshot
                            .as_ref()
                            .and_then(|entry| entry.bundle_id.as_deref())
                    })
                    .or_else(|| {
                        effective_snapshot
                            .as_ref()
                            .and_then(|entry| entry.bundle_id.as_deref())
                    });
                log_contextual_anchor_decision(
                    decision,
                    &probe.reason,
                    bundle_for_log,
                    next,
                    probe.source,
                    probe.pid,
                    probe.role.as_deref(),
                    probe.subrole.as_deref(),
                    probe.dom_input_type.as_deref(),
                );
                last_contextual_decision = Some(decision.to_string());
                last_contextual_reason = Some(probe.reason.clone());
            }

            if next != last {
                match next {
                    Some(position) => {
                        let (x, y) = clamp_anchor_window_position(&anchor, position.x, position.y);
                        save_last_anchor_position(&app, AnchorPosition { x, y });
                        let _ =
                            anchor.set_position(Position::Physical(PhysicalPosition::new(x, y)));
                        let _ = anchor.show();
                        last = Some(AnchorPosition { x, y });
                    }
                    None => {
                        let _ = anchor.hide();
                        last = None;
                    }
                }
            }

            thread::sleep(std::time::Duration::from_millis(poll_interval_ms));
        }
    });
}
