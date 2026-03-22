use std::sync::atomic::Ordering;

use tauri::Manager;
use tauri::path::BaseDirectory;
use tauri_plugin_dialog::{DialogExt, MessageDialogButtons, MessageDialogKind};
use tauri_plugin_updater::UpdaterExt;

use crate::commands;
use crate::overlay::refocus::{APP_QUIT_REQUESTED, TRAY_READY, UPDATE_CHECK_IN_FLIGHT};
use crate::overlay::windows::open_quick_window_with_action;

const TRAY_ICON_ID: &str = "whisloai-tray";
const TRAY_MENU_VERSION: &str = "tray-version";
const TRAY_MENU_OPEN_APP: &str = "tray-open-app";
const TRAY_MENU_OPEN_SETTINGS: &str = "tray-open-settings";
const TRAY_MENU_CHECK_UPDATES: &str = "tray-check-updates";
const TRAY_MENU_QUIT: &str = "tray-quit";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum UpdateCheckTrigger {
    Startup,
    TrayMenu,
}

impl UpdateCheckTrigger {
    pub(crate) fn is_user_initiated(self) -> bool {
        matches!(self, Self::TrayMenu)
    }
}

pub(crate) async fn check_for_update_with_dialog(
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

pub(crate) fn start_background_update_check(app: tauri::AppHandle, trigger: UpdateCheckTrigger) {
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

pub(crate) fn setup_tray_icon(app: &tauri::AppHandle) -> Result<(), String> {
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
