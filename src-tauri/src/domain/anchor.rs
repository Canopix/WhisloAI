use std::process::Command;
use std::sync::{Mutex, OnceLock};

use crate::domain::geometry::logical_to_physical;
use crate::overlay::refocus::now_millis;
use crate::overlay::windows::monitor_scale_factor_for_logical_point;

pub(crate) const ANCHOR_MONITOR_ACTIVE_POLL_MS: u64 = 180;
pub(crate) const ANCHOR_MONITOR_IDLE_UNSUPPORTED_POLL_MS: u64 = 700;
pub(crate) const ANCHOR_HIDE_DEBOUNCE_MS: u128 = 420;
pub(crate) const ANCHOR_LAST_VALID_SNAPSHOT_TTL_MS: u128 = 1_200;

#[cfg(target_os = "macos")]
const AX_NATIVE_FALLBACK_FAILURE_THRESHOLD: u32 = 1;
#[cfg(target_os = "macos")]
const AX_NATIVE_FALLBACK_COOLDOWN_MS: u128 = 900;

#[cfg(target_os = "macos")]
static AX_NATIVE_FALLBACK_STATE: OnceLock<Mutex<HybridFallbackState>> = OnceLock::new();

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

#[cfg(target_os = "macos")]
fn native_fallback_state() -> &'static Mutex<HybridFallbackState> {
    AX_NATIVE_FALLBACK_STATE.get_or_init(|| Mutex::new(HybridFallbackState::default()))
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

pub(crate) fn focused_text_anchor_probe(app: &tauri::AppHandle) -> FocusedAnchorProbe {
    let (native_probe, native_fallback_eligible) = focused_text_anchor_probe_native(app);
    let should_try_fallback = if let Ok(mut state) = native_fallback_state().lock() {
        update_hybrid_fallback_state(
            &mut state,
            native_fallback_eligible,
            now_millis(),
            AX_NATIVE_FALLBACK_FAILURE_THRESHOLD,
            AX_NATIVE_FALLBACK_COOLDOWN_MS,
        )
    } else {
        native_fallback_eligible
    };

    if !should_try_fallback {
        return native_probe;
    }

    focused_text_anchor_probe_apple_script(app)
}

#[cfg(target_os = "macos")]
pub(crate) fn focused_text_anchor_probe_native(
    app: &tauri::AppHandle,
) -> (FocusedAnchorProbe, bool) {
    let crate::macos_ax::AxProbeOutput {
        decision,
        fallback_eligible,
        diagnostics,
    } = crate::macos_ax::probe_focused_anchor_snapshot();

    let pid = diagnostics.pid;
    let role = diagnostics.role.clone();
    let subrole = diagnostics.subrole.clone();
    let dom_input_type = diagnostics.dom_input_type.clone();
    let diagnostics_bundle_id = diagnostics.bundle_id.clone();

    let probe = match decision {
        crate::macos_ax::AxProbeDecision::Show(snapshot) => {
            let scale_factor = monitor_scale_factor_for_logical_point(app, snapshot.x, snapshot.y);
            let px = logical_to_physical(snapshot.x, scale_factor);
            let py = logical_to_physical(snapshot.y, scale_factor);
            let pw = logical_to_physical(snapshot.w, scale_factor);
            let ph = logical_to_physical(snapshot.h, scale_factor);
            let offset_x = logical_to_physical(10, scale_factor);
            let offset_y = logical_to_physical(44, scale_factor);
            let input_focus_point = if pw > 2 && ph > 2 {
                Some((px + (pw / 2), py + (ph / 2)))
            } else {
                None
            };
            let bundle_id = snapshot.bundle_id.or(diagnostics_bundle_id.clone());
            FocusedAnchorProbe {
                snapshot: Some(FocusedAnchorSnapshot {
                    position: AnchorPosition {
                        x: px + pw - offset_x,
                        y: py - offset_y, // Above the input row, not among inline icons (emoji, mic, etc.)
                    },
                    bundle_id: bundle_id.clone(),
                    input_focus_point,
                }),
                reason: "focused_input_detected".to_string(),
                bundle_id,
                source: "native",
                pid,
                role: role.clone(),
                subrole: subrole.clone(),
                dom_input_type: dom_input_type.clone(),
            }
        }
        crate::macos_ax::AxProbeDecision::Hide(skip_reason) => FocusedAnchorProbe {
            snapshot: None,
            reason: skip_reason.as_reason(),
            bundle_id: diagnostics_bundle_id,
            source: "native",
            pid,
            role,
            subrole,
            dom_input_type,
        },
    };

    (probe, fallback_eligible)
}

#[cfg(target_os = "macos")]
pub(crate) fn focused_text_anchor_probe_apple_script(app: &tauri::AppHandle) -> FocusedAnchorProbe {
    let script = r#"
set textRoles to {"AXTextField", "AXTextArea", "AXTextView", "AXComboBox", "AXSearchField"}
set skipPrefix to "SKIP" & tab
try
  tell application "System Events"
    set frontProcess to first application process whose frontmost is true
    set processName to ""
    set processBundleId to ""
    try
      set processName to name of frontProcess as string
    end try
    try
      set processBundleId to bundle identifier of frontProcess as string
    end try

    ignoring case
      if processName contains "whisloai" then return skipPrefix & "internal_app" & tab & processBundleId
      if processBundleId contains "whisloai" then return skipPrefix & "internal_bundle" & tab & processBundleId
      if processBundleId contains "com.whisloai.app" then return skipPrefix & "internal_bundle" & tab & processBundleId
    end ignoring

    tell frontProcess
      set focusedElement to value of attribute "AXFocusedUIElement"
      if focusedElement is missing value then return skipPrefix & "missing_focused_element" & tab & processBundleId
      set roleName to value of attribute "AXRole" of focusedElement
      set isEditable to false
      try
        set isEditable to value of attribute "AXEditable" of focusedElement
      end try

      set domInputType to ""
      try
        set domInputType to value of attribute "AXDOMInputType" of focusedElement as string
      end try
      ignoring case
        if domInputType is "password" then return skipPrefix & "blocked_dom_input_type:password" & tab & processBundleId
      end ignoring
      if textRoles does not contain roleName and isEditable is not true and domInputType is "" then return skipPrefix & "role_not_text_or_editable:" & roleName & tab & processBundleId

      set metadataText to ""
      repeat with attrName in {"AXTitle", "AXDescription", "AXHelp", "AXPlaceholderValue", "AXIdentifier", "AXRoleDescription"}
        try
          set attrValue to value of attribute attrName of focusedElement
          if attrValue is not missing value then
            set metadataText to metadataText & " " & (attrValue as string)
          end if
        end try
      end repeat

      try
        set p to value of attribute "AXPosition" of focusedElement
        set s to value of attribute "AXSize" of focusedElement
      on error
        return skipPrefix & "missing_geometry" & tab & processBundleId
      end try

      set px to item 1 of p as integer
      set py to item 2 of p as integer
      set pw to item 1 of s as integer
      set ph to item 2 of s as integer
      if pw < 2 or ph < 2 then return skipPrefix & "tiny_geometry:" & (pw as string) & "x" & (ph as string) & tab & processBundleId

      ignoring case
        if roleName contains "secure" then return skipPrefix & "blocked_secure_role:" & roleName & tab & processBundleId
        if metadataText contains "password" then return skipPrefix & "blocked_password_metadata" & tab & processBundleId
      end ignoring

      return "OK" & tab & processBundleId & tab & (px as string) & "," & (py as string) & "," & (pw as string) & "," & (ph as string)
    end tell
  end tell
on error errMsg
  return "ERROR" & tab & errMsg
end try
"#;

    let output = Command::new("osascript").arg("-e").arg(script).output();
    let output = match output {
        Ok(value) => value,
        Err(error) => {
            return FocusedAnchorProbe {
                snapshot: None,
                reason: format!("probe_spawn_failed:{error}"),
                bundle_id: None,
                source: "fallback",
                pid: None,
                role: None,
                subrole: None,
                dom_input_type: None,
            };
        }
    };
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        return FocusedAnchorProbe {
            snapshot: None,
            reason: if stderr.is_empty() {
                "probe_non_zero_exit".to_string()
            } else {
                format!("probe_non_zero_exit:{stderr}")
            },
            bundle_id: None,
            source: "fallback",
            pid: None,
            role: None,
            subrole: None,
            dom_input_type: None,
        };
    }

    let raw = String::from_utf8_lossy(&output.stdout).trim().to_string();
    let Some(parsed) = parse_anchor_snapshot_probe_output(&raw) else {
        return FocusedAnchorProbe {
            snapshot: None,
            reason: format!("probe_unparseable:{raw}"),
            bundle_id: None,
            source: "fallback",
            pid: None,
            role: None,
            subrole: None,
            dom_input_type: None,
        };
    };

    match parsed {
        AnchorSnapshotRawParse::Found {
            bundle_id,
            x,
            y,
            w,
            h,
        } => {
            let scale_factor = monitor_scale_factor_for_logical_point(app, x, y);
            let px = logical_to_physical(x, scale_factor);
            let py = logical_to_physical(y, scale_factor);
            let pw = logical_to_physical(w, scale_factor);
            let ph = logical_to_physical(h, scale_factor);
            let offset_x = logical_to_physical(10, scale_factor);
            let offset_y = logical_to_physical(44, scale_factor);
            let input_focus_point = if pw > 2 && ph > 2 {
                Some((px + (pw / 2), py + (ph / 2)))
            } else {
                None
            };
            FocusedAnchorProbe {
                snapshot: Some(FocusedAnchorSnapshot {
                    position: AnchorPosition {
                        x: px + pw - offset_x,
                        y: py - offset_y, // Above the input row, not among inline icons (emoji, mic, etc.)
                    },
                    bundle_id: bundle_id.clone(),
                    input_focus_point,
                }),
                reason: "focused_input_detected".to_string(),
                bundle_id,
                source: "fallback",
                pid: None,
                role: None,
                subrole: None,
                dom_input_type: None,
            }
        }
        AnchorSnapshotRawParse::Skip { reason, bundle_id } => FocusedAnchorProbe {
            snapshot: None,
            reason,
            bundle_id,
            source: "fallback",
            pid: None,
            role: None,
            subrole: None,
            dom_input_type: None,
        },
    }
}

#[cfg(target_os = "macos")]
pub(crate) fn focused_text_anchor_snapshot(
    app: &tauri::AppHandle,
) -> Option<FocusedAnchorSnapshot> {
    focused_text_anchor_probe(app).snapshot
}

#[cfg(target_os = "windows")]
pub(crate) fn focused_text_anchor_probe(_app: &tauri::AppHandle) -> FocusedAnchorProbe {
    let probe = platform::focused_anchor_probe();
    let snapshot = probe.snapshot.map(|entry| FocusedAnchorSnapshot {
        position: AnchorPosition {
            x: entry.anchor_x,
            y: entry.anchor_y,
        },
        bundle_id: probe.bundle_id.clone(),
        input_focus_point: Some((entry.focus_x, entry.focus_y)),
    });

    FocusedAnchorProbe {
        snapshot,
        reason: probe.reason,
        bundle_id: probe.bundle_id,
        source: probe.source,
        pid: probe.pid,
        role: probe.role,
        subrole: None,
        dom_input_type: None,
    }
}

#[cfg(target_os = "windows")]
pub(crate) fn focused_text_anchor_snapshot(
    app: &tauri::AppHandle,
) -> Option<FocusedAnchorSnapshot> {
    focused_text_anchor_probe(app).snapshot
}

#[cfg(not(any(target_os = "macos", target_os = "windows")))]
pub(crate) fn focused_text_anchor_probe(_app: &tauri::AppHandle) -> FocusedAnchorProbe {
    FocusedAnchorProbe {
        snapshot: None,
        reason: "contextual_not_supported".to_string(),
        bundle_id: None,
        source: "unsupported",
        pid: None,
        role: None,
        subrole: None,
        dom_input_type: None,
    }
}

#[cfg(not(any(target_os = "macos", target_os = "windows")))]
pub(crate) fn focused_text_anchor_snapshot(
    _app: &tauri::AppHandle,
) -> Option<FocusedAnchorSnapshot> {
    None
}

pub(crate) fn contextual_anchor_tracking_supported() -> bool {
    crate::platform::capabilities().supports_contextual_anchor
}
