use crate::platform::{self, PermissionTarget};
use crate::*;

#[tauri::command]
pub(crate) fn get_onboarding_status(app: tauri::AppHandle) -> Result<OnboardingStatus, String> {
    let config = load_config(&app)?;
    let capabilities = platform::capabilities();

    Ok(OnboardingStatus {
        completed: config.onboarding_completed,
        platform: capabilities.platform.to_string(),
        needs_accessibility: capabilities.needs_accessibility,
        needs_automation: capabilities.needs_automation,
        supports_contextual_anchor: capabilities.supports_contextual_anchor,
    })
}

#[tauri::command]
pub(crate) fn complete_onboarding(app: tauri::AppHandle) -> Result<(), String> {
    let mut config = load_config(&app)?;
    config.onboarding_completed = true;
    save_config(&app, &config)?;
    activate_overlay_mode(&app)?;
    open_quick_window_with_action(&app, Some("open-app".to_string()))?;
    Ok(())
}

#[tauri::command]
pub(crate) fn open_permission_settings(permission: String) -> Result<(), String> {
    let target = PermissionTarget::parse(&permission)
        .ok_or_else(|| "Unknown permission target.".to_string())?;
    platform::backend().open_permission_settings(target)
}

#[tauri::command]
pub(crate) fn probe_auto_insert_permission() -> Result<bool, String> {
    probe_input_automation_permission().map(|_| true)
}

#[tauri::command]
pub(crate) fn probe_accessibility_permission() -> Result<bool, String> {
    ensure_accessibility_permission().map(|_| true)
}

#[tauri::command]
pub(crate) fn probe_system_events_permission() -> Result<bool, String> {
    ensure_system_events_permission().map(|_| true)
}
