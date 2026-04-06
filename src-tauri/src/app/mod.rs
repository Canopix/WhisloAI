use crate::commands;
use crate::*;
use std::sync::atomic::Ordering;

pub(crate) fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .manage(PendingQuickAction::default())
        .manage(LastExternalAppBundle::default())
        .manage(LastAnchorPosition::default())
        .manage(LastAnchorTimestamp::default())
        .manage(LastInputFocusTarget::default())
        .manage(AnchorBehaviorMode::default())
        .manage(BlockedBundleIds::default())
        .plugin(tauri_plugin_clipboard_manager::init())
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .plugin(tauri_plugin_single_instance::init(|_app, _args, _cwd| {}))
        .on_window_event(|window, event| {
            if window.label() == "settings" {
                match event {
                    tauri::WindowEvent::Focused(focused) => {
                        SETTINGS_WINDOW_OPEN.store(*focused, Ordering::SeqCst);
                    }
                    tauri::WindowEvent::Destroyed => {
                        SETTINGS_WINDOW_OPEN.store(false, Ordering::SeqCst);
                    }
                    _ => {}
                }
            }

            let tauri::WindowEvent::CloseRequested { api, .. } = event else {
                return;
            };

            if !TRAY_READY.load(Ordering::SeqCst) || APP_QUIT_REQUESTED.load(Ordering::SeqCst) {
                return;
            }

            if window.label() == "settings" {
                SETTINGS_WINDOW_OPEN.store(false, Ordering::SeqCst);
            }

            api.prevent_close();
            if let Err(error) = window.hide() {
                log::warn!(
                    "Could not hide window '{}' after close request: {error}",
                    window.label()
                );
            }
        })
        .setup(|app| {
            app.handle().plugin(
                tauri_plugin_log::Builder::default()
                    .level(log::LevelFilter::Info)
                    .build(),
            )?;

            if let Err(error) = setup_tray_icon(app.handle()) {
                log::warn!("Tray icon is unavailable: {error}");
            }

            let config = load_config(app.handle())?;
            set_anchor_behavior_mode(app.handle(), &config.anchor_behavior);
            set_blocked_bundle_ids(app.handle(), &config.blocked_bundle_ids);
            if let Err(error) = register_hotkeys(app.handle(), &config.hotkeys) {
                log::warn!("Global hotkeys were not registered: {error}");
            }
            if config.onboarding_completed {
                if let Err(error) = activate_overlay_mode(app.handle()) {
                    log::warn!("Could not initialize overlay mode: {error}");
                }
            } else {
                show_main_window_for_onboarding(app.handle());
            }

            start_background_update_check(app.handle().clone(), UpdateCheckTrigger::Startup);

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::list_providers,
            commands::get_hotkeys,
            commands::get_app_version,
            commands::get_prompt_settings,
            commands::get_ui_settings,
            commands::get_blocked_bundle_ids,
            commands::get_transcription_config,
            commands::save_transcription_config,
            commands::list_whisper_models,
            commands::download_whisper_model,
            commands::pick_whisper_models_dir,
            commands::get_onboarding_status,
            commands::complete_onboarding,
            commands::open_permission_settings,
            commands::probe_auto_insert_permission,
            commands::probe_accessibility_permission,
            commands::probe_system_events_permission,
            commands::log_dictation_trace,
            commands::open_external_url,
            commands::open_settings_window,
            commands::open_widget_window,
            commands::open_quick_window,
            commands::start_anchor_window_drag,
            commands::remember_anchor_window_position,
            commands::set_quick_window_expanded,
            commands::close_quick_window,
            commands::open_main_mode,
            commands::capture_selected_text,
            commands::save_hotkeys,
            commands::save_prompt_settings,
            commands::save_ui_settings,
            commands::blacklist_current_app,
            commands::remove_blocked_bundle_id,
            commands::save_provider,
            commands::set_active_provider,
            commands::delete_provider,
            commands::test_provider_connection,
            commands::test_provider_connection_input,
            commands::transcribe_audio,
            commands::auto_insert_text,
            commands::improve_text,
            commands::translate_text,
            commands::consume_pending_quick_action,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
