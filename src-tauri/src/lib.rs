use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::process::Command;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Mutex, OnceLock};
use std::thread;
use std::time::{Instant, SystemTime, UNIX_EPOCH};
use tauri::path::BaseDirectory;
use tauri::{Emitter, LogicalSize, Manager, PhysicalPosition, Position, Size};
use tauri_plugin_dialog::{DialogExt, MessageDialogButtons, MessageDialogKind};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut, ShortcutState};
use tauri_plugin_updater::UpdaterExt;

mod app;
mod commands;
mod domain;
mod overlay;
mod platform;

use domain::anchor::*;
use domain::ai::*;
use domain::config::*;
use domain::geometry::*;
use domain::providers::*;
use overlay::*;

#[cfg(target_os = "macos")]
mod macos_ax;

const QUICK_WINDOW_WIDTH_COMPACT: f64 = 252.0;
const QUICK_WINDOW_WIDTH_EXPANDED: f64 = 252.0;
const QUICK_WINDOW_HEIGHT_COMPACT: f64 = 64.0;
const QUICK_WINDOW_HEIGHT_EXPANDED: f64 = 96.0;
const TRAY_ICON_ID: &str = "whisloai-tray";
const TRAY_MENU_VERSION: &str = "tray-version";
const TRAY_MENU_OPEN_APP: &str = "tray-open-app";
const TRAY_MENU_OPEN_SETTINGS: &str = "tray-open-settings";
const TRAY_MENU_CHECK_UPDATES: &str = "tray-check-updates";
const TRAY_MENU_QUIT: &str = "tray-quit";
const INPUT_TARGET_TTL_MS: u128 = 90_000;
const REFOCUS_CLICK_STABILIZE_MS: u64 = 45;
const REFOCUS_POST_RESTORE_MS: u64 = 35;
const ANCHOR_HIDE_DEBOUNCE_MS: u128 = 420;
const ANCHOR_LAST_VALID_SNAPSHOT_TTL_MS: u128 = 1_200;
#[cfg(target_os = "macos")]
const AX_NATIVE_FALLBACK_FAILURE_THRESHOLD: u32 = 3;
#[cfg(target_os = "macos")]
const AX_NATIVE_FALLBACK_COOLDOWN_MS: u128 = 1_800;

#[cfg(target_os = "macos")]
static AX_NATIVE_FALLBACK_STATE: OnceLock<Mutex<HybridFallbackState>> = OnceLock::new();




#[cfg(target_os = "macos")]
fn native_fallback_state() -> &'static Mutex<HybridFallbackState> {
    AX_NATIVE_FALLBACK_STATE.get_or_init(|| Mutex::new(HybridFallbackState::default()))
}


#[cfg(target_os = "macos")]
fn focused_text_anchor_probe(app: &tauri::AppHandle) -> FocusedAnchorProbe {
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
fn focused_text_anchor_probe_native(app: &tauri::AppHandle) -> (FocusedAnchorProbe, bool) {
    let macos_ax::AxProbeOutput {
        decision,
        fallback_eligible,
        diagnostics,
    } = macos_ax::probe_focused_anchor_snapshot();

    let pid = diagnostics.pid;
    let role = diagnostics.role.clone();
    let subrole = diagnostics.subrole.clone();
    let dom_input_type = diagnostics.dom_input_type.clone();
    let diagnostics_bundle_id = diagnostics.bundle_id.clone();

    let probe = match decision {
        macos_ax::AxProbeDecision::Show(snapshot) => {
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
        macos_ax::AxProbeDecision::Hide(skip_reason) => FocusedAnchorProbe {
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
fn focused_text_anchor_probe_apple_script(app: &tauri::AppHandle) -> FocusedAnchorProbe {
    let script = r#"
set textRoles to {"AXTextField", "AXTextArea", "AXTextView"}
set blockedTerms to {"address", "url", "navigation", "omnibox", "search", "buscar", "password", "contraseña", "contrasena", "email", "correo"}
set browserBundles to {"com.apple.Safari", "com.google.Chrome", "com.brave.Browser", "com.microsoft.edgemac", "org.mozilla.firefox", "company.thebrowser.Browser"}
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
      if textRoles does not contain roleName and isEditable is not true then return skipPrefix & "role_not_text_or_editable:" & roleName & tab & processBundleId

      set subroleName to ""
      try
        set subroleName to value of attribute "AXSubrole" of focusedElement as string
      end try
      if subroleName is "AXSearchField" then return skipPrefix & "blocked_search_subrole" & tab & processBundleId

      set domInputType to ""
      try
        set domInputType to value of attribute "AXDOMInputType" of focusedElement as string
      end try
      ignoring case
        if domInputType is "search" or domInputType is "password" or domInputType is "email" then return skipPrefix & "blocked_dom_input_type:" & domInputType & tab & processBundleId
      end ignoring

      set metadataText to ""
      repeat with attrName in {"AXTitle", "AXDescription", "AXHelp", "AXPlaceholderValue", "AXIdentifier", "AXRoleDescription"}
        try
          set attrValue to value of attribute attrName of focusedElement
          if attrValue is not missing value then
            set metadataText to metadataText & " " & (attrValue as string)
          end if
        end try
      end repeat

      set shouldApplyBlockedTerms to false
      repeat with browserBundle in browserBundles
        if processBundleId is (browserBundle as string) then
          set shouldApplyBlockedTerms to true
          exit repeat
        end if
      end repeat
      if shouldApplyBlockedTerms then
        ignoring case
          repeat with blocked in blockedTerms
            if metadataText contains (blocked as string) then return skipPrefix & "blocked_browser_metadata:" & (blocked as string) & tab & processBundleId
          end repeat
        end ignoring
      end if

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
        if domInputType is "password" then return skipPrefix & "blocked_password_dom_type" & tab & processBundleId
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
fn focused_text_anchor_snapshot(app: &tauri::AppHandle) -> Option<FocusedAnchorSnapshot> {
    focused_text_anchor_probe(app).snapshot
}

#[cfg(target_os = "windows")]
fn focused_text_anchor_probe(_app: &tauri::AppHandle) -> FocusedAnchorProbe {
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
fn focused_text_anchor_snapshot(app: &tauri::AppHandle) -> Option<FocusedAnchorSnapshot> {
    focused_text_anchor_probe(app).snapshot
}

#[cfg(not(any(target_os = "macos", target_os = "windows")))]
fn focused_text_anchor_probe(_app: &tauri::AppHandle) -> FocusedAnchorProbe {
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
fn focused_text_anchor_snapshot(_app: &tauri::AppHandle) -> Option<FocusedAnchorSnapshot> {
    None
}

fn contextual_anchor_tracking_supported() -> bool {
    platform::capabilities().supports_contextual_anchor
}

fn start_anchor_monitor_once(app: tauri::AppHandle) {
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
                    domain::anchor::ANCHOR_MONITOR_ACTIVE_POLL_MS,
                ));
                continue;
            };
            let floating_mode = is_anchor_floating_mode(&app);
            let poll_interval_ms = domain::anchor::anchor_monitor_poll_interval_ms(
                floating_mode,
                contextual_tracking_supported,
            );

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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum UpdateCheckTrigger {
    Startup,
    TrayMenu,
}

impl UpdateCheckTrigger {
    fn is_user_initiated(self) -> bool {
        matches!(self, Self::TrayMenu)
    }
}

async fn check_for_update_with_dialog(
    app: tauri::AppHandle,
    trigger: UpdateCheckTrigger,
) -> Result<(), String> {
    let updater = app
        .updater()
        .map_err(|error| format!("Could not initialize updater: {error}"))?;

    let update = updater
        .check()
        .await
        .map_err(|error| format!("Could not check for updates: {error}"))?;

    let Some(update) = update else {
        if trigger.is_user_initiated() {
            app.dialog()
                .message("WhisloAI is up to date.")
                .title("WhisloAI Updates")
                .kind(MessageDialogKind::Info)
                .buttons(MessageDialogButtons::Ok)
                .show(|_| {});
        }
        return Ok(());
    };

    let should_install = app
        .dialog()
        .message(format!(
            "A new version is available.\n\nCurrent: {}\nLatest: {}\n\nInstall now?",
            update.current_version, update.version
        ))
        .title("Update available")
        .kind(MessageDialogKind::Info)
        .buttons(MessageDialogButtons::OkCancelCustom(
            "Install now".to_string(),
            "Later".to_string(),
        ))
        .blocking_show();

    if !should_install {
        return Ok(());
    }

    update
        .download_and_install(
            |chunk_len, content_len| {
                if let Some(total) = content_len {
                    log::info!(
                        "Updater download progress: {chunk_len} bytes downloaded (total: {total})"
                    );
                } else {
                    log::info!("Updater downloaded chunk: {chunk_len} bytes");
                }
            },
            || {
                log::info!("Updater download completed. Installing package.");
            },
        )
        .await
        .map_err(|error| format!("Could not install update: {error}"))?;

    app.restart();
}

fn start_background_update_check(app: tauri::AppHandle, trigger: UpdateCheckTrigger) {
    if UPDATE_CHECK_IN_FLIGHT.swap(true, Ordering::SeqCst) {
        if trigger.is_user_initiated() {
            app.dialog()
                .message("An update check is already in progress.")
                .title("WhisloAI Updates")
                .kind(MessageDialogKind::Info)
                .buttons(MessageDialogButtons::Ok)
                .show(|_| {});
        }
        return;
    }

    tauri::async_runtime::spawn(async move {
        if let Err(error) = check_for_update_with_dialog(app.clone(), trigger).await {
            log::warn!("Update check failed: {error}");
            if trigger.is_user_initiated() {
                app.dialog()
                    .message(format!("Could not check for updates.\n\n{}", error.trim()))
                    .title("WhisloAI Updates")
                    .kind(MessageDialogKind::Error)
                    .buttons(MessageDialogButtons::Ok)
                    .show(|_| {});
            }
        }

        UPDATE_CHECK_IN_FLIGHT.store(false, Ordering::SeqCst);
    });
}

fn setup_tray_icon(app: &tauri::AppHandle) -> Result<(), String> {
    let version_label = format!("Version {}", app.package_info().version);
    let tray_menu = tauri::menu::MenuBuilder::new(app)
        .text(TRAY_MENU_VERSION, version_label)
        .separator()
        .text(TRAY_MENU_OPEN_APP, "Open WhisloAI")
        .text(TRAY_MENU_OPEN_SETTINGS, "Settings")
        .text(TRAY_MENU_CHECK_UPDATES, "Check for updates")
        .separator()
        .text(TRAY_MENU_QUIT, "Quit")
        .build()
        .map_err(|e| format!("Could not build tray menu: {e}"))?;

    let mut tray_builder = tauri::tray::TrayIconBuilder::with_id(TRAY_ICON_ID)
        .menu(&tray_menu)
        .tooltip(format!("WhisloAI v{}", app.package_info().version))
        .icon_as_template(true)
        .on_menu_event(|app, event| match event.id().as_ref() {
            TRAY_MENU_VERSION => {}
            TRAY_MENU_OPEN_APP => {
                if let Err(error) = open_quick_window_with_action(app, Some("open-app".to_string()))
                {
                    log::warn!("Tray action 'open app' failed: {error}");
                }
            }
            TRAY_MENU_OPEN_SETTINGS => {
                if let Err(error) = commands::open_settings_window(app.clone()) {
                    log::warn!("Tray action 'open settings' failed: {error}");
                }
            }
            TRAY_MENU_CHECK_UPDATES => {
                start_background_update_check(app.clone(), UpdateCheckTrigger::TrayMenu);
            }
            TRAY_MENU_QUIT => {
                APP_QUIT_REQUESTED.store(true, Ordering::SeqCst);
                app.exit(0);
            }
            _ => {}
        });

    let icon = app
        .path()
        .resolve("icons/tray-icon.png", BaseDirectory::Resource)
        .ok()
        .and_then(|path| tauri::image::Image::from_path(&path).ok());
    if let Some(icon) = icon.or_else(|| app.default_window_icon().cloned()) {
        tray_builder = tray_builder.icon(icon);
    }

    tray_builder
        .build(app)
        .map_err(|e| format!("Could not create tray icon: {e}"))?;

    TRAY_READY.store(true, Ordering::SeqCst);
    Ok(())
}

#[cfg(feature = "local-transcription")]

#[cfg(target_os = "macos")]
fn is_running_under_rosetta() -> bool {
    *RUNNING_UNDER_ROSETTA.get_or_init(|| {
        let Ok(output) = Command::new("sysctl")
            .args(["-in", "sysctl.proc_translated"])
            .output()
        else {
            return false;
        };
        if !output.status.success() {
            return false;
        }
        String::from_utf8_lossy(&output.stdout).trim() == "1"
    })
}

#[cfg(not(target_os = "macos"))]
fn is_running_under_rosetta() -> bool {
    false
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    app::run();
}

#[cfg(test)]
mod tests {
    use super::{
        audio_file_name, download_progress_percent, external_target_restore_reason,
        local_prefers_openai_chat_endpoint, local_transcription_block_reason, logical_to_physical,
        non_empty_trimmed, normalize_anchor_behavior, normalize_local_transcription_output,
        normalize_provider_base_url, parse_anchor_snapshot_probe_output, point_in_rect,
        provider_endpoint, sanitize_scale_factor, scale_for_logical_point_in_rects,
        should_clear_external_cache_on_restore_error,
        should_clear_external_cache_on_restore_reason, should_hide_contextual_anchor,
        transcribe_error, update_hybrid_fallback_state, AnchorSnapshotRawParse, ExternalAppTarget,
        HybridFallbackState, INPUT_TARGET_TTL_MS,
    };

    #[test]
    fn point_in_rect_is_inclusive_of_min_and_exclusive_of_max() {
        assert!(point_in_rect(10, 10, 10, 10, 20, 20));
        assert!(point_in_rect(29, 29, 10, 10, 20, 20));
        assert!(!point_in_rect(30, 30, 10, 10, 20, 20));
        assert!(!point_in_rect(9, 10, 10, 10, 20, 20));
    }

    #[test]
    fn normalize_anchor_behavior_maps_unknown_to_contextual() {
        assert_eq!(normalize_anchor_behavior("floating"), "floating");
        assert_eq!(normalize_anchor_behavior("FLOATING"), "floating");
        assert_eq!(normalize_anchor_behavior("contextual"), "contextual");
        assert_eq!(normalize_anchor_behavior("anything-else"), "contextual");
    }

    #[test]
    fn sanitize_scale_factor_falls_back_to_one_for_invalid_values() {
        assert_eq!(sanitize_scale_factor(0.0), 1.0);
        assert_eq!(sanitize_scale_factor(-1.5), 1.0);
        assert_eq!(sanitize_scale_factor(f64::NAN), 1.0);
        assert_eq!(sanitize_scale_factor(1.5), 1.5);
    }

    #[test]
    fn logical_to_physical_rounds_to_nearest_pixel() {
        assert_eq!(logical_to_physical(11, 1.5), 17);
        assert_eq!(logical_to_physical(10, 2.0), 20);
        assert_eq!(logical_to_physical(-3, 2.0), -6);
    }

    #[test]
    fn scale_for_logical_point_selects_candidate_that_contains_converted_point() {
        let candidates = vec![(0, 0, 1000, 800, 2.0), (1000, 0, 1200, 900, 1.0)];
        assert_eq!(
            scale_for_logical_point_in_rects(1500, 200, &candidates),
            Some(1.0)
        );
        assert_eq!(
            scale_for_logical_point_in_rects(300, 100, &candidates),
            Some(2.0)
        );
        assert_eq!(
            scale_for_logical_point_in_rects(2600, 100, &candidates),
            None
        );
    }

    #[test]
    fn download_progress_percent_handles_known_totals() {
        assert_eq!(download_progress_percent(0, Some(100)), Some(0));
        assert_eq!(download_progress_percent(55, Some(100)), Some(55));
        assert_eq!(download_progress_percent(101, Some(100)), Some(100));
    }

    #[test]
    fn download_progress_percent_returns_none_for_unknown_or_invalid_total() {
        assert_eq!(download_progress_percent(50, None), None);
        assert_eq!(download_progress_percent(50, Some(0)), None);
    }

    #[test]
    fn non_empty_trimmed_returns_none_for_blank_values() {
        assert_eq!(non_empty_trimmed(""), None);
        assert_eq!(non_empty_trimmed("   "), None);
        assert_eq!(non_empty_trimmed("  hola "), Some("hola"));
    }

    #[test]
    fn normalize_provider_base_url_strips_terminal_endpoint_paths() {
        assert_eq!(
            normalize_provider_base_url("http://localhost:1234/api/v1/chat"),
            "http://localhost:1234/api/v1"
        );
        assert_eq!(
            normalize_provider_base_url("http://localhost:1234/api/v1/chat/completions"),
            "http://localhost:1234/api/v1"
        );
        assert_eq!(
            normalize_provider_base_url("http://localhost:1234/api/v1/models"),
            "http://localhost:1234/api/v1"
        );
        assert_eq!(
            normalize_provider_base_url("http://localhost:1234/api/v1/audio/transcriptions"),
            "http://localhost:1234/api/v1"
        );
    }

    #[test]
    fn provider_endpoint_avoids_endpoint_duplication() {
        assert_eq!(
            provider_endpoint("http://localhost:1234/api/v1/chat", "chat"),
            "http://localhost:1234/api/v1/chat"
        );
        assert_eq!(
            provider_endpoint(
                "http://localhost:1234/api/v1/chat/completions",
                "chat/completions"
            ),
            "http://localhost:1234/api/v1/chat/completions"
        );
        assert_eq!(
            provider_endpoint("http://localhost:1234/api/v1", "chat"),
            "http://localhost:1234/api/v1/chat"
        );
    }

    #[test]
    fn local_prefers_openai_chat_endpoint_detects_v1_roots() {
        assert!(local_prefers_openai_chat_endpoint(
            "http://localhost:1234/api/v1"
        ));
        assert!(local_prefers_openai_chat_endpoint(
            "http://localhost:1234/api/v1/chat"
        ));
        assert!(local_prefers_openai_chat_endpoint(
            "http://localhost:1234/v1/chat/completions"
        ));
        assert!(!local_prefers_openai_chat_endpoint("http://localhost:1234"));
        assert!(!local_prefers_openai_chat_endpoint(
            "http://localhost:1234/api"
        ));
    }

    #[test]
    fn local_transcription_guard_does_not_block_rosetta() {
        assert!(local_transcription_block_reason(true, true).is_none());
        assert!(local_transcription_block_reason(true, false).is_none());
        assert!(local_transcription_block_reason(false, true).is_none());
    }

    #[test]
    fn transcribe_error_passthrough_returns_original_message() {
        let result = transcribe_error("boom");
        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), "boom".to_string());
    }

    #[test]
    fn audio_file_name_handles_mime_variants() {
        assert_eq!(
            audio_file_name(Some("audio/webm;codecs=opus")),
            "recording.webm"
        );
        assert_eq!(
            audio_file_name(Some("audio/webm; codecs=opus")),
            "recording.webm"
        );
        assert_eq!(
            audio_file_name(Some("Audio/WEBM; codecs=opus")),
            "recording.webm"
        );
        assert_eq!(
            audio_file_name(Some("audio/mp4; codecs=mp4a.40.2")),
            "recording.m4a"
        );
        assert_eq!(
            audio_file_name(Some("audio/wav; charset=binary")),
            "recording.wav"
        );
    }

    #[test]
    fn normalize_local_transcription_output_rejects_empty_text() {
        let result = normalize_local_transcription_output("   ".to_string());
        assert!(result.is_err());
    }

    #[test]
    fn contextual_hide_requires_ttl_expiry_and_debounce_elapsed() {
        assert!(!should_hide_contextual_anchor(
            1_400,
            Some(1_000),
            Some(1_000),
            420,
            1_200
        ));
        assert!(!should_hide_contextual_anchor(
            2_050,
            Some(1_900),
            Some(500),
            420,
            1_200
        ));
        assert!(should_hide_contextual_anchor(
            2_500,
            Some(1_900),
            Some(500),
            420,
            1_200
        ));
        assert!(!should_hide_contextual_anchor(
            2_500,
            None,
            Some(500),
            420,
            1_200
        ));
    }

    #[test]
    fn hybrid_fallback_activates_after_threshold_and_respects_cooldown() {
        let mut state = HybridFallbackState::default();

        assert!(!update_hybrid_fallback_state(
            &mut state, true, 100, 3, 1_000
        ));
        assert!(!update_hybrid_fallback_state(
            &mut state, true, 200, 3, 1_000
        ));
        assert!(update_hybrid_fallback_state(
            &mut state, true, 300, 3, 1_000
        ));
        assert_eq!(state.fallback_cooldown_until_ms, 1_300);
        assert_eq!(state.consecutive_native_failures, 0);

        assert!(!update_hybrid_fallback_state(
            &mut state, true, 500, 3, 1_000
        ));
        assert!(!update_hybrid_fallback_state(
            &mut state, true, 600, 3, 1_000
        ));
        assert!(!update_hybrid_fallback_state(
            &mut state, true, 700, 3, 1_000
        ));
        assert!(update_hybrid_fallback_state(
            &mut state, true, 1_300, 3, 1_000
        ));

        assert!(!update_hybrid_fallback_state(
            &mut state, false, 1_301, 3, 1_000
        ));
        assert_eq!(state.consecutive_native_failures, 0);
    }

    #[test]
    fn restore_error_classification_clears_cache_for_not_running_targets() {
        assert!(should_clear_external_cache_on_restore_error("NOT_RUNNING"));
        assert!(should_clear_external_cache_on_restore_error("not running"));
        assert!(!should_clear_external_cache_on_restore_error(
            "Automation denied"
        ));
    }

    #[test]
    fn external_target_validation_detects_missing_invalid_stale_and_ready_targets() {
        let now_ms = 1_000_000_u128;
        assert_eq!(
            external_target_restore_reason(None, now_ms),
            ("missing_target", None)
        );

        let invalid = ExternalAppTarget {
            bundle_id: "com.whisloai.desktop".to_string(),
            captured_at_ms: now_ms,
        };
        assert_eq!(
            external_target_restore_reason(Some(&invalid), now_ms),
            ("invalid_target", None)
        );

        let stale = ExternalAppTarget {
            bundle_id: "com.tinyspeck.slackmacgap".to_string(),
            captured_at_ms: now_ms.saturating_sub(INPUT_TARGET_TTL_MS + 1),
        };
        let (stale_reason, stale_age_ms) = external_target_restore_reason(Some(&stale), now_ms);
        assert_eq!(stale_reason, "stale_target");
        assert!(stale_age_ms.unwrap_or(0) > INPUT_TARGET_TTL_MS);

        let fresh = ExternalAppTarget {
            bundle_id: "com.tinyspeck.slackmacgap".to_string(),
            captured_at_ms: now_ms.saturating_sub(250),
        };
        assert_eq!(
            external_target_restore_reason(Some(&fresh), now_ms),
            ("ready", Some(250))
        );
    }

    #[test]
    fn cleanup_policy_clears_cache_for_invalid_stale_and_not_running_reasons() {
        assert!(should_clear_external_cache_on_restore_reason(
            "invalid_target"
        ));
        assert!(should_clear_external_cache_on_restore_reason(
            "stale_target"
        ));
        assert!(should_clear_external_cache_on_restore_reason("not_running"));
        assert!(!should_clear_external_cache_on_restore_reason(
            "missing_target"
        ));
        assert!(!should_clear_external_cache_on_restore_reason(
            "restore_error"
        ));
    }

    #[test]
    fn parse_anchor_snapshot_probe_output_reads_ok_payload() {
        let parsed =
            parse_anchor_snapshot_probe_output("OK\tcom.tinyspeck.slackmacgap\t100,200,300,48");
        assert_eq!(
            parsed,
            Some(AnchorSnapshotRawParse::Found {
                bundle_id: Some("com.tinyspeck.slackmacgap".to_string()),
                x: 100,
                y: 200,
                w: 300,
                h: 48,
            })
        );
    }

    #[test]
    fn parse_anchor_snapshot_probe_output_reads_skip_reason_and_bundle() {
        let parsed = parse_anchor_snapshot_probe_output(
            "SKIP\tblocked_dom_input_type:search\tcom.google.Chrome",
        );
        assert_eq!(
            parsed,
            Some(AnchorSnapshotRawParse::Skip {
                reason: "blocked_dom_input_type:search".to_string(),
                bundle_id: Some("com.google.Chrome".to_string()),
            })
        );
    }
}
