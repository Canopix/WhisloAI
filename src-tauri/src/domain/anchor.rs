pub(crate) const ANCHOR_MONITOR_ACTIVE_POLL_MS: u64 = 180;
pub(crate) const ANCHOR_MONITOR_IDLE_UNSUPPORTED_POLL_MS: u64 = 700;

pub(crate) fn anchor_monitor_poll_interval_ms(
    floating_mode: bool,
    contextual_tracking_supported: bool,
) -> u64 {
    if !floating_mode && !contextual_tracking_supported {
        return ANCHOR_MONITOR_IDLE_UNSUPPORTED_POLL_MS;
    }
    ANCHOR_MONITOR_ACTIVE_POLL_MS
}

#[cfg(test)]
mod tests {
    use super::{
        anchor_monitor_poll_interval_ms, ANCHOR_MONITOR_ACTIVE_POLL_MS,
        ANCHOR_MONITOR_IDLE_UNSUPPORTED_POLL_MS,
    };

    #[test]
    fn anchor_monitor_poll_interval_stays_fast_for_active_tracking() {
        assert_eq!(
            anchor_monitor_poll_interval_ms(true, false),
            ANCHOR_MONITOR_ACTIVE_POLL_MS
        );
        assert_eq!(
            anchor_monitor_poll_interval_ms(false, true),
            ANCHOR_MONITOR_ACTIVE_POLL_MS
        );
    }

    #[test]
    fn anchor_monitor_poll_interval_slows_down_when_contextual_tracking_is_unavailable() {
        assert_eq!(
            anchor_monitor_poll_interval_ms(false, false),
            ANCHOR_MONITOR_IDLE_UNSUPPORTED_POLL_MS
        );
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct AnchorPosition {
    pub(crate) x: i32,
    pub(crate) y: i32,
}

#[derive(Debug, Clone)]
pub(crate) struct FocusedAnchorSnapshot {
    pub(crate) position: AnchorPosition,
    pub(crate) bundle_id: Option<String>,
    pub(crate) input_focus_point: Option<(i32, i32)>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum AnchorSnapshotRawParse {
    Found {
        bundle_id: Option<String>,
        x: i32,
        y: i32,
        w: i32,
        h: i32,
    },
    Skip {
        reason: String,
        bundle_id: Option<String>,
    },
}

#[derive(Debug, Clone)]
pub(crate) struct FocusedAnchorProbe {
    pub(crate) snapshot: Option<FocusedAnchorSnapshot>,
    pub(crate) reason: String,
    pub(crate) bundle_id: Option<String>,
    pub(crate) source: &'static str,
    pub(crate) pid: Option<i32>,
    pub(crate) role: Option<String>,
    pub(crate) subrole: Option<String>,
    pub(crate) dom_input_type: Option<String>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct HybridFallbackState {
    pub(crate) consecutive_native_failures: u32,
    pub(crate) fallback_cooldown_until_ms: u128,
}

#[derive(Debug, Clone)]
pub(crate) struct TimedContextualSnapshot {
    pub(crate) snapshot: FocusedAnchorSnapshot,
    pub(crate) captured_at_ms: u128,
}

pub(crate) fn should_hide_contextual_anchor(
    now_ms: u128,
    hide_candidate_since_ms: Option<u128>,
    last_valid_snapshot_at_ms: Option<u128>,
    hide_debounce_ms: u128,
    snapshot_ttl_ms: u128,
) -> bool {
    if let Some(last_valid_snapshot_at_ms) = last_valid_snapshot_at_ms {
        if now_ms.saturating_sub(last_valid_snapshot_at_ms) <= snapshot_ttl_ms {
            return false;
        }
    }

    let Some(hide_candidate_since_ms) = hide_candidate_since_ms else {
        return false;
    };

    now_ms.saturating_sub(hide_candidate_since_ms) >= hide_debounce_ms
}

pub(crate) fn update_hybrid_fallback_state(
    state: &mut HybridFallbackState,
    native_fallback_eligible: bool,
    now_ms: u128,
    failure_threshold: u32,
    cooldown_ms: u128,
) -> bool {
    if !native_fallback_eligible {
        state.consecutive_native_failures = 0;
        return false;
    }

    state.consecutive_native_failures = state.consecutive_native_failures.saturating_add(1);
    if state.consecutive_native_failures < failure_threshold {
        return false;
    }
    if now_ms < state.fallback_cooldown_until_ms {
        return false;
    }

    state.fallback_cooldown_until_ms = now_ms.saturating_add(cooldown_ms);
    state.consecutive_native_failures = 0;
    true
}

pub(crate) fn non_empty_optional(value: &str) -> Option<String> {
    let clean = value.trim();
    if clean.is_empty() {
        None
    } else {
        Some(clean.to_string())
    }
}

pub(crate) fn parse_anchor_snapshot_probe_output(raw: &str) -> Option<AnchorSnapshotRawParse> {
    let clean = raw.trim();
    if clean.is_empty() {
        return None;
    }

    if clean == "NONE" {
        return Some(AnchorSnapshotRawParse::Skip {
            reason: "none".to_string(),
            bundle_id: None,
        });
    }

    let mut parts = clean.split('\t');
    match parts.next()?.trim() {
        "OK" => {
            let bundle_id = non_empty_optional(parts.next().unwrap_or_default());
            let geometry_raw = parts.next()?.trim();
            let mut geometry = geometry_raw.split(',').map(str::trim);
            let x = geometry.next()?.parse::<i32>().ok()?;
            let y = geometry.next()?.parse::<i32>().ok()?;
            let w = geometry.next()?.parse::<i32>().ok()?;
            let h = geometry.next()?.parse::<i32>().ok()?;
            Some(AnchorSnapshotRawParse::Found {
                bundle_id,
                x,
                y,
                w,
                h,
            })
        }
        "SKIP" => {
            let reason = parts.next().unwrap_or("unknown_skip").trim();
            let bundle_id = non_empty_optional(parts.next().unwrap_or_default());
            Some(AnchorSnapshotRawParse::Skip {
                reason: if reason.is_empty() {
                    "unknown_skip".to_string()
                } else {
                    reason.to_string()
                },
                bundle_id,
            })
        }
        "ERROR" => {
            let reason = parts.next().unwrap_or("script_error").trim();
            Some(AnchorSnapshotRawParse::Skip {
                reason: if reason.is_empty() {
                    "script_error".to_string()
                } else {
                    format!("script_error:{reason}")
                },
                bundle_id: None,
            })
        }
        _ => {
            // Backward-compatible parsing for older output format: "<bundle>\t<x,y,w,h>".
            if let Some((bundle_raw, geometry_raw)) = clean.split_once('\t') {
                let mut geometry = geometry_raw.split(',').map(str::trim);
                let x = geometry.next()?.parse::<i32>().ok()?;
                let y = geometry.next()?.parse::<i32>().ok()?;
                let w = geometry.next()?.parse::<i32>().ok()?;
                let h = geometry.next()?.parse::<i32>().ok()?;
                return Some(AnchorSnapshotRawParse::Found {
                    bundle_id: non_empty_optional(bundle_raw),
                    x,
                    y,
                    w,
                    h,
                });
            }
            None
        }
    }
}

pub(crate) fn log_contextual_anchor_decision(
    decision: &str,
    reason: &str,
    bundle_id: Option<&str>,
    position: Option<AnchorPosition>,
    source: &str,
    pid: Option<i32>,
    role: Option<&str>,
    subrole: Option<&str>,
    dom_input_type: Option<&str>,
) {
    let payload = serde_json::json!({
        "event": "anchor_contextual_decision",
        "decision": decision,
        "reason": reason,
        "bundle_id": bundle_id,
        "source": source,
        "pid": pid,
        "role": role,
        "subrole": subrole,
        "dom_input_type": dom_input_type,
        "position": position.map(|point| serde_json::json!({ "x": point.x, "y": point.y })),
    });
    log::info!("{payload}");
}
