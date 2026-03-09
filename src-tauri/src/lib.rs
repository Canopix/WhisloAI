use base64::Engine as _;
use keyring::Entry;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
use std::thread;
use std::time::{SystemTime, UNIX_EPOCH};
use tauri::{Emitter, LogicalSize, Manager, PhysicalPosition, Position, Size};
use tauri_plugin_clipboard_manager::ClipboardExt;
use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut, ShortcutState};

const KEYRING_SERVICE: &str = "best-text";
const QUICK_WINDOW_WIDTH: f64 = 252.0;
const QUICK_WINDOW_HEIGHT_COMPACT: f64 = 64.0;
const QUICK_WINDOW_HEIGHT_EXPANDED: f64 = 96.0;
const TRAY_ICON_ID: &str = "besttext-tray";
const TRAY_MENU_OPEN_APP: &str = "tray-open-app";
const TRAY_MENU_OPEN_SETTINGS: &str = "tray-open-settings";
const TRAY_MENU_QUIT: &str = "tray-quit";
const SUPPORTED_STYLE_MODES: [&str; 5] =
    ["simple", "professional", "friendly", "casual", "formal"];

#[derive(Default)]
struct PendingLaunchText(Mutex<Option<String>>);

#[derive(Default)]
struct PendingQuickAction(Mutex<Option<String>>);

#[derive(Default)]
struct LastExternalApp(Mutex<Option<String>>);

static ANCHOR_MONITOR_STARTED: AtomicBool = AtomicBool::new(false);
static SETTINGS_WINDOW_OPEN: AtomicBool = AtomicBool::new(false);
static TRAY_READY: AtomicBool = AtomicBool::new(false);
static APP_QUIT_REQUESTED: AtomicBool = AtomicBool::new(false);

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
struct HotkeyConfig {
    open_app: String,
    open_improve: String,
    open_dictate_translate: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
struct ProviderConfig {
    id: String,
    name: String,
    provider_type: String,
    base_url: String,
    improve_model: String,
    translate_model: String,
    #[serde(default = "default_transcribe_model")]
    transcribe_model: String,
    #[serde(default)]
    api_key_fallback_b64: Option<String>,
    is_active: bool,
}

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct ProviderView {
    id: String,
    name: String,
    provider_type: String,
    base_url: String,
    improve_model: String,
    translate_model: String,
    transcribe_model: String,
    is_active: bool,
    has_api_key: bool,
    api_key: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ProviderInput {
    id: Option<String>,
    name: String,
    provider_type: String,
    base_url: String,
    improve_model: String,
    translate_model: String,
    #[serde(default)]
    transcribe_model: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
struct PromptSettings {
    #[serde(default = "default_improve_system_prompt")]
    improve_system_prompt: String,
    #[serde(default = "default_translate_system_prompt")]
    translate_system_prompt: String,
    #[serde(default = "default_mode_instructions")]
    mode_instructions: HashMap<String, String>,
    #[serde(default = "default_quick_mode")]
    quick_mode: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PromptSettingsInput {
    improve_system_prompt: String,
    translate_system_prompt: String,
    mode_instructions: HashMap<String, String>,
    quick_mode: String,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct AppConfig {
    #[serde(default = "default_hotkeys")]
    hotkeys: HotkeyConfig,
    #[serde(default)]
    onboarding_completed: bool,
    #[serde(default)]
    providers: Vec<ProviderConfig>,
    #[serde(default = "default_prompt_settings")]
    prompt_settings: PromptSettings,
}

#[derive(Debug, Deserialize)]
struct OpenAiModelsResponse {
    data: Vec<serde_json::Value>,
}

#[derive(Debug, Serialize)]
struct ChatRequest<'a> {
    model: &'a str,
    messages: Vec<ChatMessage<'a>>,
    temperature: f32,
}

#[derive(Debug, Serialize)]
struct ChatMessage<'a> {
    role: &'a str,
    content: &'a str,
}

#[derive(Debug, Deserialize)]
struct ChatResponse {
    choices: Vec<ChatChoice>,
}

#[derive(Debug, Deserialize)]
struct ChatChoice {
    message: ChatOutput,
}

#[derive(Debug, Deserialize)]
struct ChatOutput {
    content: serde_json::Value,
}

#[derive(Debug, Deserialize)]
struct AudioTranscriptionResponse {
    text: Option<String>,
}

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct ExternalImproveEvent {
    text: String,
    source: String,
}

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct HotkeyTriggerEvent {
    action: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct InsertTextResult {
    copied: bool,
    pasted: bool,
    message: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct OnboardingStatus {
    completed: bool,
    platform: String,
    needs_accessibility: bool,
}

impl Default for PromptSettings {
    fn default() -> Self {
        default_prompt_settings()
    }
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            hotkeys: default_hotkeys(),
            onboarding_completed: false,
            providers: vec![default_provider()],
            prompt_settings: default_prompt_settings(),
        }
    }
}

fn default_hotkeys() -> HotkeyConfig {
    HotkeyConfig {
        open_app: "CommandOrControl+Shift+Space".to_string(),
        open_improve: "CommandOrControl+Shift+I".to_string(),
        open_dictate_translate: "CommandOrControl+Shift+D".to_string(),
    }
}

fn default_provider() -> ProviderConfig {
    ProviderConfig {
        id: "openai-default".to_string(),
        name: "OpenAI".to_string(),
        provider_type: "openai".to_string(),
        base_url: "https://api.openai.com/v1".to_string(),
        improve_model: "gpt-4.1-mini".to_string(),
        translate_model: "gpt-4.1-mini".to_string(),
        transcribe_model: default_transcribe_model(),
        api_key_fallback_b64: None,
        is_active: true,
    }
}

fn default_transcribe_model() -> String {
    "gpt-4o-mini-transcribe".to_string()
}

fn default_improve_system_prompt() -> String {
    "You are a writing assistant. Rewrite text in clear, concise, natural English. Keep intent and facts unchanged. Return only final text.".to_string()
}

fn default_translate_system_prompt() -> String {
    "You are a translation assistant. Convert Spanish text into clear, concise, natural English for workplace chat. Preserve names and technical terms. Return only final text.".to_string()
}

fn default_quick_mode() -> String {
    "simple".to_string()
}

fn default_mode_instruction_for(mode: &str) -> Option<&'static str> {
    match mode {
        "simple" => Some("Use clear, concise wording with everyday vocabulary."),
        "professional" => {
            Some("Use a polished workplace tone with direct and confident wording.")
        }
        "friendly" => Some("Use a warm, approachable tone while staying concise."),
        "casual" => Some("Use a relaxed conversational tone with natural phrasing."),
        "formal" => Some("Use a formal, respectful tone with complete sentences."),
        _ => None,
    }
}

fn default_mode_instructions() -> HashMap<String, String> {
    let mut map = HashMap::new();
    for mode in SUPPORTED_STYLE_MODES {
        if let Some(value) = default_mode_instruction_for(mode) {
            map.insert(mode.to_string(), value.to_string());
        }
    }
    map
}

fn default_prompt_settings() -> PromptSettings {
    PromptSettings {
        improve_system_prompt: default_improve_system_prompt(),
        translate_system_prompt: default_translate_system_prompt(),
        mode_instructions: default_mode_instructions(),
        quick_mode: default_quick_mode(),
    }
}

fn normalize_mode_name(mode: &str) -> String {
    let normalized = mode.trim().to_lowercase();
    if normalized.is_empty() {
        return default_quick_mode();
    }
    match normalized.as_str() {
        "simple" | "professional" | "friendly" | "casual" | "formal" => normalized,
        "pro" => "professional".to_string(),
        "informal" => "casual".to_string(),
        _ => default_quick_mode(),
    }
}

fn normalize_prompt_settings(settings: &mut PromptSettings) -> bool {
    let mut changed = false;

    if settings.improve_system_prompt.trim().is_empty() {
        settings.improve_system_prompt = default_improve_system_prompt();
        changed = true;
    } else {
        let clean = settings.improve_system_prompt.trim().to_string();
        if clean != settings.improve_system_prompt {
            settings.improve_system_prompt = clean;
            changed = true;
        }
    }

    if settings.translate_system_prompt.trim().is_empty() {
        settings.translate_system_prompt = default_translate_system_prompt();
        changed = true;
    } else {
        let clean = settings.translate_system_prompt.trim().to_string();
        if clean != settings.translate_system_prompt {
            settings.translate_system_prompt = clean;
            changed = true;
        }
    }

    let mut normalized_modes = HashMap::new();
    for mode in SUPPORTED_STYLE_MODES {
        let clean = settings
            .mode_instructions
            .get(mode)
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
            .or_else(|| default_mode_instruction_for(mode).map(|value| value.to_string()))
            .unwrap_or_default();
        normalized_modes.insert(mode.to_string(), clean);
    }

    if settings
        .mode_instructions
        .iter()
        .any(|(mode, value)| normalized_modes.get(mode) != Some(value))
        || settings.mode_instructions.len() != normalized_modes.len()
    {
        changed = true;
    }
    settings.mode_instructions = normalized_modes;

    let normalized_quick_mode = normalize_mode_name(&settings.quick_mode);
    if normalized_quick_mode != settings.quick_mode {
        settings.quick_mode = normalized_quick_mode;
        changed = true;
    }

    changed
}

fn mode_instruction_for(settings: &PromptSettings, style: &str) -> (String, String) {
    let mode = normalize_mode_name(style);
    let instruction = settings
        .mode_instructions
        .get(&mode)
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .or_else(|| default_mode_instruction_for(&mode).map(|value| value.to_string()))
        .unwrap_or_else(|| "Use clear, concise wording.".to_string());
    (mode, instruction)
}

fn normalize_base_url(base_url: &str) -> String {
    base_url.trim().trim_end_matches('/').to_string()
}

fn config_path(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    let config_dir = app
        .path()
        .app_config_dir()
        .map_err(|e| format!("Could not resolve app config directory: {e}"))?;

    fs::create_dir_all(&config_dir)
        .map_err(|e| format!("Could not create app config directory: {e}"))?;

    Ok(config_dir.join("providers.json"))
}

fn load_config(app: &tauri::AppHandle) -> Result<AppConfig, String> {
    let path = config_path(app)?;

    if !path.exists() {
        let config = AppConfig::default();
        save_config(app, &config)?;
        return Ok(config);
    }

    let raw = fs::read_to_string(&path)
        .map_err(|e| format!("Could not read settings file {}: {e}", path.display()))?;

    let mut config: AppConfig = serde_json::from_str(&raw)
        .map_err(|e| format!("Could not parse settings file {}: {e}", path.display()))?;

    let mut needs_save = false;
    if dedupe_providers(&mut config) {
        needs_save = true;
    }

    if !config.providers.is_empty() && !config.providers.iter().any(|provider| provider.is_active) {
        if let Some(first) = config.providers.first_mut() {
            first.is_active = true;
        }
        needs_save = true;
    }

    if normalize_prompt_settings(&mut config.prompt_settings) {
        needs_save = true;
    }

    if needs_save {
        save_config(app, &config)?;
    }

    Ok(config)
}

fn save_config(app: &tauri::AppHandle, config: &AppConfig) -> Result<(), String> {
    let path = config_path(app)?;
    let payload = serde_json::to_string_pretty(config)
        .map_err(|e| format!("Could not serialize settings: {e}"))?;
    fs::write(&path, payload)
        .map_err(|e| format!("Could not write settings file {}: {e}", path.display()))
}

fn keyring_entry(provider_id: &str) -> Result<Entry, String> {
    Entry::new(KEYRING_SERVICE, provider_id)
        .map_err(|e| format!("Could not create keyring entry for provider {provider_id}: {e}"))
}

fn read_keyring_secret(provider_id: &str) -> Option<String> {
    let entry = keyring_entry(provider_id).ok()?;
    let secret = entry.get_password().ok()?;
    let clean = secret.trim().to_string();
    if clean.is_empty() {
        None
    } else {
        Some(clean)
    }
}

fn encode_api_key_fallback(secret: &str) -> Option<String> {
    let clean = secret.trim();
    if clean.is_empty() {
        return None;
    }
    Some(base64::engine::general_purpose::STANDARD.encode(clean.as_bytes()))
}

fn decode_api_key_fallback(encoded: Option<&String>) -> Option<String> {
    let value = encoded?.trim();
    if value.is_empty() {
        return None;
    }

    let decoded = base64::engine::general_purpose::STANDARD
        .decode(value)
        .ok()?;
    let text = String::from_utf8(decoded).ok()?;
    let clean = text.trim().to_string();
    if clean.is_empty() {
        None
    } else {
        Some(clean)
    }
}

fn provider_api_key_from_config(provider: &ProviderConfig) -> Option<String> {
    decode_api_key_fallback(provider.api_key_fallback_b64.as_ref())
        .or_else(|| read_keyring_secret(&provider.id))
}

fn provider_dedupe_signature(provider: &ProviderConfig) -> String {
    format!(
        "{}|{}|{}",
        provider.name.trim().to_lowercase(),
        provider.provider_type.trim().to_lowercase(),
        normalize_base_url(&provider.base_url).to_lowercase(),
    )
}

fn dedupe_providers(config: &mut AppConfig) -> bool {
    let mut changed = false;
    let mut seen: HashMap<String, usize> = HashMap::new();
    let mut deduped: Vec<ProviderConfig> = Vec::with_capacity(config.providers.len());

    for provider in config.providers.drain(..) {
        let signature = provider_dedupe_signature(&provider);
        if let Some(existing_index) = seen.get(&signature).copied() {
            changed = true;
            let existing = &mut deduped[existing_index];
            if provider.is_active {
                existing.is_active = true;
            }

            if provider_api_key_from_config(existing).is_none() {
                if let Some(secret) = provider_api_key_from_config(&provider) {
                    existing.api_key_fallback_b64 = encode_api_key_fallback(&secret);
                }
            }
            continue;
        }

        seen.insert(signature, deduped.len());
        deduped.push(provider);
    }

    config.providers = deduped;

    changed
}

fn has_provider_api_key(provider: &ProviderConfig) -> bool {
    provider_api_key_from_config(provider).is_some()
}

fn provider_to_view(provider: &ProviderConfig) -> ProviderView {
    let api_key = provider_api_key_from_config(provider);
    ProviderView {
        id: provider.id.clone(),
        name: provider.name.clone(),
        provider_type: provider.provider_type.clone(),
        base_url: provider.base_url.clone(),
        improve_model: provider.improve_model.clone(),
        translate_model: provider.translate_model.clone(),
        transcribe_model: provider.transcribe_model.clone(),
        is_active: provider.is_active,
        has_api_key: has_provider_api_key(provider),
        api_key,
    }
}

fn provider_api_key(provider: &ProviderConfig) -> Result<String, String> {
    provider_api_key_from_config(provider).ok_or_else(|| {
        "Missing API key. Configure a valid API key in Settings > Providers.".to_string()
    })
}

fn provider_api_key_for_input(
    config: &AppConfig,
    provider: &ProviderInput,
    api_key: Option<String>,
) -> Result<String, String> {
    if let Some(secret) = api_key {
        let clean = secret.trim().to_string();
        if !clean.is_empty() {
            return Ok(clean);
        }
    }

    if let Some(id) = provider
        .id
        .as_deref()
        .map(str::trim)
        .filter(|id| !id.is_empty())
    {
        if let Some(existing) = config.providers.iter().find(|item| item.id == id) {
            if let Ok(secret) = provider_api_key(existing) {
                return Ok(secret);
            }
        } else if let Some(secret) = read_keyring_secret(id) {
            return Ok(secret);
        }
    }

    let normalized_name = provider.name.trim();
    let normalized_type = provider.provider_type.trim();
    let normalized_base_url = normalize_base_url(&provider.base_url);

    if let Some(existing) = config.providers.iter().find(|existing| {
        existing.name.trim().eq_ignore_ascii_case(normalized_name)
            && existing
                .provider_type
                .trim()
                .eq_ignore_ascii_case(normalized_type)
            && normalize_base_url(&existing.base_url).eq_ignore_ascii_case(&normalized_base_url)
    }) {
        if let Ok(secret) = provider_api_key(existing) {
            return Ok(secret);
        }
    }

    Err("Missing API key. Type one in the API key field or save provider first.".to_string())
}

fn active_provider(config: &AppConfig) -> Result<ProviderConfig, String> {
    config
        .providers
        .iter()
        .find(|provider| provider.is_active)
        .cloned()
        .or_else(|| config.providers.first().cloned())
        .ok_or_else(|| "No provider configured. Add one in Settings > Providers.".to_string())
}

fn now_millis() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .unwrap_or(0)
}

fn is_internal_app_bundle_id(bundle_id: &str) -> bool {
    let value = bundle_id.trim().to_lowercase();
    value.is_empty() || value.contains("besttext") || value.contains("com.besttext.app")
}

#[cfg(target_os = "macos")]
fn current_frontmost_bundle_id() -> Option<String> {
    let script = r#"
try
  tell application "System Events"
    set frontProcess to first application process whose frontmost is true
    try
      return bundle identifier of frontProcess as string
    on error
      return ""
    end try
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

    let value = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if value.is_empty() {
        None
    } else {
        Some(value)
    }
}

#[cfg(not(target_os = "macos"))]
fn current_frontmost_bundle_id() -> Option<String> {
    None
}

fn remember_last_external_app(app: &tauri::AppHandle) {
    let Some(bundle_id) = current_frontmost_bundle_id() else {
        return;
    };
    if is_internal_app_bundle_id(&bundle_id) {
        return;
    }

    if let Some(state) = app.try_state::<LastExternalApp>() {
        if let Ok(mut guard) = state.0.lock() {
            *guard = Some(bundle_id);
        }
    }
}

fn last_external_app(app: &tauri::AppHandle) -> Option<String> {
    let state = app.try_state::<LastExternalApp>()?;
    let guard = state.0.lock().ok()?;
    guard.clone()
}

fn escape_applescript_string(value: &str) -> String {
    value.replace('\\', "\\\\").replace('"', "\\\"")
}

#[cfg(target_os = "macos")]
fn activate_bundle_id(bundle_id: &str) -> Result<(), String> {
    let escaped = escape_applescript_string(bundle_id);
    let script = format!(
        r#"
try
  tell application id "{escaped}" to activate
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
fn activate_bundle_id(_bundle_id: &str) -> Result<(), String> {
    Ok(())
}

fn restore_last_external_app(app: &tauri::AppHandle) {
    let Some(bundle_id) = last_external_app(app) else {
        return;
    };
    if is_internal_app_bundle_id(&bundle_id) {
        return;
    }

    if let Err(error) = activate_bundle_id(&bundle_id) {
        log::warn!("Could not restore focus to previous app '{bundle_id}': {error}");
    } else {
        thread::sleep(std::time::Duration::from_millis(120));
    }
}

fn read_external_text_file(path: &str) -> Option<String> {
    let content = fs::read_to_string(path).ok()?;
    let text = content.trim().to_string();
    if text.is_empty() {
        None
    } else {
        Some(text)
    }
}

fn parse_improve_text_from_args(args: &[String]) -> Option<String> {
    let mut index = 0;

    while index < args.len() {
        match args[index].as_str() {
            "--improve-text" => {
                let value = args.get(index + 1)?.trim().to_string();
                if !value.is_empty() {
                    return Some(value);
                }
            }
            "--improve-text-file" => {
                if let Some(path) = args.get(index + 1) {
                    if let Some(text) = read_external_text_file(path) {
                        return Some(text);
                    }
                }
            }
            _ => {}
        }
        index += 1;
    }

    None
}

fn normalize_hotkeys(hotkeys: &HotkeyConfig) -> HotkeyConfig {
    HotkeyConfig {
        open_app: hotkeys.open_app.trim().to_string(),
        open_improve: hotkeys.open_improve.trim().to_string(),
        open_dictate_translate: hotkeys.open_dictate_translate.trim().to_string(),
    }
}

fn validate_hotkeys(hotkeys: &HotkeyConfig) -> Result<Vec<(String, String)>, String> {
    let bindings = vec![
        ("open-app".to_string(), hotkeys.open_app.trim().to_string()),
        (
            "open-improve".to_string(),
            hotkeys.open_improve.trim().to_string(),
        ),
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

fn save_pending_quick_action(app: &tauri::AppHandle, action: String) {
    if let Some(state) = app.try_state::<PendingQuickAction>() {
        if let Ok(mut pending) = state.0.lock() {
            *pending = Some(action);
        }
    }
}

fn emit_quick_action(app: &tauri::AppHandle, action: &str) {
    save_pending_quick_action(app, action.to_string());
    let _ = app.emit(
        "quick-action",
        HotkeyTriggerEvent {
            action: action.to_string(),
        },
    );
}

fn handle_hotkey_trigger(app: &tauri::AppHandle, action: &str) {
    let quick_action = match action {
        "open-app" => "open-app",
        "open-improve" => "open-improve",
        "open-dictate-translate" => "open-dictate-translate",
        _ => "open-app",
    };

    if let Err(error) = open_quick_window_with_action(app, Some(quick_action.to_string())) {
        log::warn!("Hotkey action '{action}' failed: {error}");
    }
}

fn register_hotkeys(app: &tauri::AppHandle, hotkeys: &HotkeyConfig) -> Result<(), String> {
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

fn save_pending_text(app: &tauri::AppHandle, text: String) {
    if let Some(state) = app.try_state::<PendingLaunchText>() {
        if let Ok(mut pending) = state.0.lock() {
            *pending = Some(text);
        }
    }
}

fn emit_external_text_event(app: &tauri::AppHandle, text: String, source: &str) {
    let payload = ExternalImproveEvent {
        text: text.clone(),
        source: source.to_string(),
    };

    if app.emit("external-improve-text", payload).is_err() {
        save_pending_text(app, text);
    }
}

fn extract_content(content: &serde_json::Value) -> Option<String> {
    if let Some(text) = content.as_str() {
        return Some(text.trim().to_string());
    }

    let items = content.as_array()?;
    let mut fragments = Vec::new();

    for item in items {
        if let Some(text) = item.get("text").and_then(serde_json::Value::as_str) {
            fragments.push(text.trim());
        }
    }

    if fragments.is_empty() {
        None
    } else {
        Some(fragments.join(" "))
    }
}

async fn run_chat_completion(
    provider: &ProviderConfig,
    api_key: &str,
    model: &str,
    system_prompt: &str,
    user_prompt: &str,
) -> Result<String, String> {
    let base_url = normalize_base_url(&provider.base_url);
    let endpoint = format!("{base_url}/chat/completions");

    let request = ChatRequest {
        model,
        messages: vec![
            ChatMessage {
                role: "system",
                content: system_prompt,
            },
            ChatMessage {
                role: "user",
                content: user_prompt,
            },
        ],
        temperature: 0.2,
    };

    let client = reqwest::Client::new();
    let response = client
        .post(endpoint)
        .bearer_auth(api_key)
        .json(&request)
        .send()
        .await
        .map_err(|e| format!("Provider request failed: {e}"))?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response
            .text()
            .await
            .unwrap_or_else(|_| "<empty body>".to_string());
        return Err(format!(
            "Provider returned HTTP {} while generating text: {}",
            status.as_u16(),
            body
        ));
    }

    let payload: ChatResponse = response
        .json()
        .await
        .map_err(|e| format!("Could not parse provider response: {e}"))?;

    let content = payload
        .choices
        .first()
        .and_then(|choice| extract_content(&choice.message.content))
        .ok_or_else(|| "Provider response did not include generated text.".to_string())?;

    Ok(content)
}

fn parse_transcription_text(raw_body: &str) -> Option<String> {
    let trimmed = raw_body.trim();
    if trimmed.is_empty() {
        return None;
    }

    if let Ok(payload) = serde_json::from_str::<AudioTranscriptionResponse>(trimmed) {
        if let Some(text) = payload.text {
            let clean = text.trim().to_string();
            if !clean.is_empty() {
                return Some(clean);
            }
        }
    }

    Some(trimmed.to_string())
}

fn audio_file_name(mime_type: Option<&str>) -> String {
    let extension = match mime_type.unwrap_or_default() {
        "audio/webm" | "audio/webm;codecs=opus" => "webm",
        "audio/ogg" | "audio/ogg;codecs=opus" => "ogg",
        "audio/mp4" | "audio/m4a" => "m4a",
        "audio/wav" | "audio/x-wav" | "audio/wave" => "wav",
        _ => "bin",
    };

    format!("recording.{extension}")
}

#[cfg(any(target_os = "macos", target_os = "windows"))]
fn simulate_modifier_shortcut(character: char) -> Result<(), String> {
    use enigo::Direction::{Click, Press, Release};
    use enigo::{Enigo, Key, Keyboard, Settings};

    let mut enigo = Enigo::new(&Settings::default())
        .map_err(|e| format!("Could not initialize keyboard automation: {e}"))?;

    #[cfg(target_os = "macos")]
    let modifier_key = Key::Meta;
    #[cfg(target_os = "windows")]
    let modifier_key = Key::Control;

    enigo
        .key(modifier_key, Press)
        .map_err(|e| format!("Could not press shortcut modifier key: {e}"))?;

    let shortcut_result = enigo
        .key(Key::Unicode(character), Click)
        .map_err(|e| format!("Could not send shortcut key: {e}"));

    let _ = enigo.key(modifier_key, Release);
    shortcut_result
}

#[cfg(any(target_os = "macos", target_os = "windows"))]
fn simulate_paste_shortcut() -> Result<(), String> {
    simulate_modifier_shortcut('v')
}

#[cfg(any(target_os = "macos", target_os = "windows"))]
fn simulate_copy_shortcut() -> Result<(), String> {
    simulate_modifier_shortcut('c')
}

#[cfg(not(any(target_os = "macos", target_os = "windows")))]
fn simulate_paste_shortcut() -> Result<(), String> {
    Err("Automatic paste is not supported on this platform in the MVP.".to_string())
}

#[cfg(not(any(target_os = "macos", target_os = "windows")))]
fn simulate_copy_shortcut() -> Result<(), String> {
    Err("Automatic copy is not supported on this platform in the MVP.".to_string())
}

#[cfg(any(target_os = "macos", target_os = "windows"))]
fn probe_input_automation_permission() -> Result<(), String> {
    use enigo::Direction::{Press, Release};
    use enigo::{Enigo, Key, Keyboard, Settings};

    let mut enigo = Enigo::new(&Settings::default())
        .map_err(|e| format!("Could not initialize keyboard automation: {e}"))?;

    #[cfg(target_os = "macos")]
    let modifier_key = Key::Meta;
    #[cfg(target_os = "windows")]
    let modifier_key = Key::Control;

    enigo
        .key(modifier_key, Press)
        .map_err(|e| format!("Automation permission denied: {e}"))?;
    enigo
        .key(modifier_key, Release)
        .map_err(|e| format!("Automation permission denied: {e}"))?;
    Ok(())
}

#[cfg(not(any(target_os = "macos", target_os = "windows")))]
fn probe_input_automation_permission() -> Result<(), String> {
    Err("Input automation permission check is not supported on this platform.".to_string())
}

fn platform_name() -> String {
    if cfg!(target_os = "macos") {
        "macos".to_string()
    } else if cfg!(target_os = "windows") {
        "windows".to_string()
    } else if cfg!(target_os = "linux") {
        "linux".to_string()
    } else {
        "unknown".to_string()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct AnchorPosition {
    x: i32,
    y: i32,
}

fn show_main_window_for_onboarding(app: &tauri::AppHandle) {
    if let Some(main) = app.get_webview_window("main") {
        let _ = main.show();
        let _ = main.set_focus();
    }
}

fn hide_main_window(app: &tauri::AppHandle) {
    if let Some(main) = app.get_webview_window("main") {
        let _ = main.hide();
    }
}

fn ensure_anchor_window(app: &tauri::AppHandle) -> Result<(), String> {
    if app.get_webview_window("anchor").is_some() {
        return Ok(());
    }

    tauri::WebviewWindowBuilder::new(app, "anchor", tauri::WebviewUrl::App("anchor.html".into()))
        .title("BestText Anchor")
        .inner_size(30.0, 30.0)
        .min_inner_size(30.0, 30.0)
        .resizable(false)
        .transparent(true)
        .always_on_top(true)
        .decorations(false)
        .accept_first_mouse(true)
        .skip_taskbar(true)
        .visible(false)
        .build()
        .map_err(|e| format!("Could not create anchor window: {e}"))?;

    Ok(())
}

fn ensure_quick_window(app: &tauri::AppHandle) -> Result<(), String> {
    if app.get_webview_window("quick").is_some() {
        return Ok(());
    }

    tauri::WebviewWindowBuilder::new(app, "quick", tauri::WebviewUrl::App("widget.html".into()))
        .title("BestText Quick")
        .inner_size(QUICK_WINDOW_WIDTH, QUICK_WINDOW_HEIGHT_COMPACT)
        .min_inner_size(QUICK_WINDOW_WIDTH, QUICK_WINDOW_HEIGHT_COMPACT)
        .resizable(false)
        .transparent(true)
        .always_on_top(true)
        .decorations(false)
        .accept_first_mouse(true)
        .skip_taskbar(true)
        .visible(false)
        .build()
        .map_err(|e| format!("Could not create quick window: {e}"))?;

    Ok(())
}

fn position_quick_window_near_anchor(app: &tauri::AppHandle) {
    let Some(quick) = app.get_webview_window("quick") else {
        return;
    };

    #[cfg(target_os = "macos")]
    {
        if let Some(position) = focused_text_anchor_position() {
            let quick_size = quick.outer_size().unwrap_or_default();
            let default_x = position.x + 34;
            let default_y = position.y;

            let mut x = default_x;
            let mut y = default_y;

            if let Some(anchor) = app.get_webview_window("anchor") {
                if let Ok(Some(monitor)) =
                    anchor.monitor_from_point(position.x as f64, position.y as f64)
                {
                    let work = monitor.work_area();
                    let min_x = work.position.x + 8;
                    let min_y = work.position.y + 8;
                    let max_x =
                        (work.position.x + work.size.width as i32 - quick_size.width as i32 - 8)
                            .max(min_x);
                    let max_y =
                        (work.position.y + work.size.height as i32 - quick_size.height as i32 - 8)
                            .max(min_y);
                    x = x.clamp(min_x, max_x);
                    y = y.clamp(min_y, max_y);
                } else {
                    x = x.max(8);
                    y = y.max(8);
                }
            } else {
                x = x.max(8);
                y = y.max(8);
            }

            let _ = quick.set_position(Position::Physical(PhysicalPosition::new(x, y)));
            return;
        }
    }

    if let Some(anchor) = app.get_webview_window("anchor") {
        if let Ok(pos) = anchor.outer_position() {
            let quick_size = quick.outer_size().unwrap_or_default();
            let default_x = pos.x + 34;
            let default_y = pos.y;

            let mut x = default_x;
            let mut y = default_y;

            if let Ok(Some(monitor)) = anchor.monitor_from_point(pos.x as f64, pos.y as f64) {
                let work = monitor.work_area();
                let min_x = work.position.x + 8;
                let min_y = work.position.y + 8;
                let max_x =
                    (work.position.x + work.size.width as i32 - quick_size.width as i32 - 8)
                        .max(min_x);
                let max_y =
                    (work.position.y + work.size.height as i32 - quick_size.height as i32 - 8)
                        .max(min_y);
                x = x.clamp(min_x, max_x);
                y = y.clamp(min_y, max_y);
            } else {
                x = x.max(8);
                y = y.max(8);
            }

            let _ = quick.set_position(Position::Physical(PhysicalPosition::new(x, y)));
        }
    }
}

fn open_quick_window_with_action(
    app: &tauri::AppHandle,
    action: Option<String>,
) -> Result<(), String> {
    remember_last_external_app(app);
    ensure_quick_window(app)?;
    position_quick_window_near_anchor(app);

    let window = app
        .get_webview_window("quick")
        .ok_or_else(|| "Quick window not found.".to_string())?;

    if window.is_minimized().unwrap_or(false) {
        let _ = window.unminimize();
    }

    window
        .show()
        .map_err(|e| format!("Could not show quick window: {e}"))?;
    if let Err(error) = window.set_focus() {
        log::warn!("Could not focus quick window: {error}");
    }

    if let Some(value) = action {
        emit_quick_action(app, &value);
    }

    Ok(())
}

#[cfg(target_os = "macos")]
fn focused_text_anchor_position() -> Option<AnchorPosition> {
    let script = r#"
set textRoles to {"AXTextField", "AXTextArea", "AXTextView"}
set blockedTerms to {"address", "url", "navigation", "omnibox", "search", "buscar", "password", "contraseña", "contrasena", "email", "correo"}
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
      if processName contains "besttext" then return "NONE"
      if processBundleId contains "besttext" then return "NONE"
      if processBundleId contains "com.besttext.app" then return "NONE"
    end ignoring

    tell frontProcess
      set focusedElement to value of attribute "AXFocusedUIElement"
      if focusedElement is missing value then return "NONE"
      set roleName to value of attribute "AXRole" of focusedElement
      if textRoles does not contain roleName then return "NONE"

      set subroleName to ""
      try
        set subroleName to value of attribute "AXSubrole" of focusedElement as string
      end try
      if subroleName is "AXSearchField" then return "NONE"

      set domInputType to ""
      try
        set domInputType to value of attribute "AXDOMInputType" of focusedElement as string
      end try
      ignoring case
        if domInputType is "search" or domInputType is "password" or domInputType is "email" then return "NONE"
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

      ignoring case
        repeat with blocked in blockedTerms
          if metadataText contains (blocked as string) then return "NONE"
        end repeat
      end ignoring

      set p to value of attribute "AXPosition" of focusedElement
      set s to value of attribute "AXSize" of focusedElement
      set px to item 1 of p as integer
      set py to item 2 of p as integer
      set pw to item 1 of s as integer
      set ph to item 2 of s as integer
      return (px as string) & "," & (py as string) & "," & (pw as string) & "," & (ph as string)
    end tell
  end tell
on error
  return "NONE"
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
    if raw.is_empty() || raw == "NONE" {
        return None;
    }

    let mut parts = raw.split(',').map(str::trim);
    let x = parts.next()?.parse::<i32>().ok()?;
    let y = parts.next()?.parse::<i32>().ok()?;
    let w = parts.next()?.parse::<i32>().ok()?;
    let _h = parts.next()?.parse::<i32>().ok()?;

    Some(AnchorPosition {
        x: (x + w - 10).max(8),
        y: (y + 2).max(8),
    })
}

#[cfg(not(target_os = "macos"))]
fn focused_text_anchor_position() -> Option<AnchorPosition> {
    None
}

fn start_anchor_monitor_once(app: tauri::AppHandle) {
    if !cfg!(target_os = "macos") {
        return;
    }

    if ANCHOR_MONITOR_STARTED
        .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
        .is_err()
    {
        return;
    }

    thread::spawn(move || {
        let mut last: Option<AnchorPosition> = None;

        loop {
            let Some(anchor) = app.get_webview_window("anchor") else {
                break;
            };

            if SETTINGS_WINDOW_OPEN.load(Ordering::SeqCst) {
                let _ = anchor.hide();
                last = None;
                thread::sleep(std::time::Duration::from_millis(420));
                continue;
            }

            let next = focused_text_anchor_position();
            if next != last {
                match next {
                    Some(position) => {
                        remember_last_external_app(&app);
                        let _ = anchor.set_position(Position::Physical(PhysicalPosition::new(
                            position.x, position.y,
                        )));
                        let _ = anchor.show();
                    }
                    None => {
                        let _ = anchor.hide();
                    }
                }
                last = next;
            }

            thread::sleep(std::time::Duration::from_millis(420));
        }
    });
}

fn activate_overlay_mode(app: &tauri::AppHandle) -> Result<(), String> {
    ensure_anchor_window(app)?;
    ensure_quick_window(app)?;
    hide_main_window(app);
    start_anchor_monitor_once(app.clone());

    Ok(())
}

fn settings_external_url(cache_bust: bool) -> Result<tauri::Url, String> {
    let mut url = "http://127.0.0.1:4173/settings.html".to_string();
    if cache_bust {
        url.push_str(&format!("?v={}", now_millis()));
    }
    tauri::Url::parse(&url).map_err(|e| format!("Invalid settings URL '{url}': {e}"))
}

fn settings_webview_url(cache_bust: bool) -> Result<tauri::WebviewUrl, String> {
    if cfg!(debug_assertions) {
        return settings_external_url(cache_bust).map(tauri::WebviewUrl::External);
    }
    Ok(tauri::WebviewUrl::App("settings.html".into()))
}

fn setup_tray_icon(app: &tauri::AppHandle) -> Result<(), String> {
    let tray_menu = tauri::menu::MenuBuilder::new(app)
        .text(TRAY_MENU_OPEN_APP, "Open BestText")
        .text(TRAY_MENU_OPEN_SETTINGS, "Settings")
        .separator()
        .text(TRAY_MENU_QUIT, "Quit")
        .build()
        .map_err(|e| format!("Could not build tray menu: {e}"))?;

    let mut tray_builder = tauri::tray::TrayIconBuilder::with_id(TRAY_ICON_ID)
        .menu(&tray_menu)
        .tooltip("BestText")
        .icon_as_template(true)
        .on_menu_event(|app, event| match event.id().as_ref() {
            TRAY_MENU_OPEN_APP => {
                if let Err(error) = open_quick_window_with_action(app, Some("open-app".to_string()))
                {
                    log::warn!("Tray action 'open app' failed: {error}");
                }
            }
            TRAY_MENU_OPEN_SETTINGS => {
                if let Err(error) = open_settings_window(app.clone()) {
                    log::warn!("Tray action 'open settings' failed: {error}");
                }
            }
            TRAY_MENU_QUIT => {
                APP_QUIT_REQUESTED.store(true, Ordering::SeqCst);
                app.exit(0);
            }
            _ => {}
        });

    if let Some(icon) = app.default_window_icon().cloned() {
        tray_builder = tray_builder.icon(icon);
    }

    tray_builder
        .build(app)
        .map_err(|e| format!("Could not create tray icon: {e}"))?;

    TRAY_READY.store(true, Ordering::SeqCst);
    Ok(())
}

#[tauri::command]
fn list_providers(app: tauri::AppHandle) -> Result<Vec<ProviderView>, String> {
    let config = load_config(&app)?;
    Ok(config.providers.iter().map(provider_to_view).collect())
}

#[tauri::command]
fn get_hotkeys(app: tauri::AppHandle) -> Result<HotkeyConfig, String> {
    let config = load_config(&app)?;
    Ok(config.hotkeys)
}

#[tauri::command]
fn get_prompt_settings(app: tauri::AppHandle) -> Result<PromptSettings, String> {
    let config = load_config(&app)?;
    Ok(config.prompt_settings)
}

#[tauri::command]
fn get_onboarding_status(app: tauri::AppHandle) -> Result<OnboardingStatus, String> {
    let config = load_config(&app)?;
    Ok(OnboardingStatus {
        completed: config.onboarding_completed,
        platform: platform_name(),
        needs_accessibility: cfg!(target_os = "macos"),
    })
}

#[tauri::command]
fn complete_onboarding(app: tauri::AppHandle) -> Result<(), String> {
    let mut config = load_config(&app)?;
    config.onboarding_completed = true;
    save_config(&app, &config)?;
    activate_overlay_mode(&app)?;
    open_quick_window_with_action(&app, Some("open-app".to_string()))?;
    Ok(())
}

#[tauri::command]
fn open_permission_settings(permission: String) -> Result<(), String> {
    let permission = permission.trim().to_lowercase();

    #[cfg(target_os = "macos")]
    {
        let target = match permission.as_str() {
            "microphone" => {
                "x-apple.systempreferences:com.apple.preference.security?Privacy_Microphone"
            }
            "accessibility" => {
                "x-apple.systempreferences:com.apple.preference.security?Privacy_Accessibility"
            }
            _ => return Err("Unknown permission target.".to_string()),
        };

        Command::new("open")
            .arg(target)
            .spawn()
            .map_err(|e| format!("Could not open macOS settings: {e}"))?;
        return Ok(());
    }

    #[cfg(target_os = "windows")]
    {
        let target = match permission.as_str() {
            "microphone" => "ms-settings:privacy-microphone",
            "accessibility" => "ms-settings:easeofaccess-keyboard",
            _ => return Err("Unknown permission target.".to_string()),
        };

        Command::new("cmd")
            .args(["/C", "start", "", target])
            .spawn()
            .map_err(|e| format!("Could not open Windows settings: {e}"))?;
        return Ok(());
    }

    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        let _ = permission;
        Err("Opening system permission settings is not implemented for this platform.".to_string())
    }
}

#[tauri::command]
fn probe_auto_insert_permission() -> Result<bool, String> {
    probe_input_automation_permission().map(|_| true)
}

#[tauri::command]
fn open_settings_window(app: tauri::AppHandle) -> Result<(), String> {
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
        .title("BestText Settings")
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
fn open_widget_window(app: tauri::AppHandle) -> Result<(), String> {
    open_quick_window_with_action(&app, Some("open-app".to_string()))
}

#[tauri::command]
fn open_quick_window(app: tauri::AppHandle) -> Result<(), String> {
    open_quick_window_with_action(&app, Some("open-app".to_string()))
}

#[tauri::command]
fn set_quick_window_expanded(app: tauri::AppHandle, expanded: bool) -> Result<(), String> {
    if let Some(window) = app.get_webview_window("quick") {
        let target_height = if expanded {
            QUICK_WINDOW_HEIGHT_EXPANDED
        } else {
            QUICK_WINDOW_HEIGHT_COMPACT
        };

        window
            .set_size(Size::Logical(LogicalSize::new(
                QUICK_WINDOW_WIDTH,
                target_height,
            )))
            .map_err(|e| format!("Could not resize quick window: {e}"))?;

        position_quick_window_near_anchor(&app);
    }

    Ok(())
}

#[tauri::command]
fn close_quick_window(app: tauri::AppHandle) -> Result<(), String> {
    let window = app
        .get_webview_window("quick")
        .ok_or_else(|| "Quick window not found.".to_string())?;
    window
        .hide()
        .map_err(|e| format!("Could not hide quick window: {e}"))
}

#[tauri::command]
fn open_main_mode(app: tauri::AppHandle, mode: String) -> Result<(), String> {
    let normalized = mode.trim().to_lowercase();
    let action = match normalized.as_str() {
        "improve" => "open-improve",
        "translate" => "open-dictate-translate",
        "dictate" => "open-dictate-translate-record",
        "app" => "open-app",
        _ => return Err("Unknown panel mode.".to_string()),
    };
    open_quick_window_with_action(&app, Some(action.to_string()))
}

#[tauri::command]
fn capture_selected_text(app: tauri::AppHandle) -> Result<String, String> {
    if let Some(window) = app.get_webview_window("quick") {
        let _ = window.hide();
    }

    restore_last_external_app(&app);
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
fn consume_pending_quick_action(state: tauri::State<'_, PendingQuickAction>) -> Option<String> {
    let mut pending = state.0.lock().ok()?;
    pending.take()
}

#[tauri::command]
fn save_hotkeys(app: tauri::AppHandle, hotkeys: HotkeyConfig) -> Result<HotkeyConfig, String> {
    let mut config = load_config(&app)?;
    let previous_hotkeys = config.hotkeys.clone();
    let next_hotkeys = normalize_hotkeys(&hotkeys);

    if let Err(error) = register_hotkeys(&app, &next_hotkeys) {
        if let Err(restore_error) = register_hotkeys(&app, &previous_hotkeys) {
            log::error!("Could not restore previous hotkeys after save failure: {restore_error}");
        }
        return Err(error);
    }

    config.hotkeys = next_hotkeys.clone();
    save_config(&app, &config)?;
    Ok(next_hotkeys)
}

#[tauri::command]
fn save_prompt_settings(
    app: tauri::AppHandle,
    prompt_settings: PromptSettingsInput,
) -> Result<PromptSettings, String> {
    let mut config = load_config(&app)?;
    let mut next = PromptSettings {
        improve_system_prompt: prompt_settings.improve_system_prompt.trim().to_string(),
        translate_system_prompt: prompt_settings.translate_system_prompt.trim().to_string(),
        mode_instructions: HashMap::new(),
        quick_mode: normalize_mode_name(&prompt_settings.quick_mode),
    };

    if next.improve_system_prompt.is_empty() {
        return Err("Improve system prompt cannot be empty.".to_string());
    }
    if next.translate_system_prompt.is_empty() {
        return Err("Translate system prompt cannot be empty.".to_string());
    }

    for mode in SUPPORTED_STYLE_MODES {
        let clean = prompt_settings
            .mode_instructions
            .get(mode)
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
            .or_else(|| default_mode_instruction_for(mode).map(|value| value.to_string()))
            .ok_or_else(|| format!("Mode instruction for '{mode}' cannot be empty."))?;
        next.mode_instructions.insert(mode.to_string(), clean);
    }

    normalize_prompt_settings(&mut next);
    config.prompt_settings = next.clone();
    save_config(&app, &config)?;
    Ok(next)
}

#[tauri::command]
fn save_provider(
    app: tauri::AppHandle,
    provider: ProviderInput,
    api_key: Option<String>,
) -> Result<ProviderView, String> {
    let mut config = load_config(&app)?;
    let normalized_name = provider.name.trim().to_string();
    let normalized_type = provider.provider_type.trim().to_string();
    let normalized_base_url = normalize_base_url(&provider.base_url);
    let normalized_improve_model = provider.improve_model.trim().to_string();
    let normalized_translate_model = provider.translate_model.trim().to_string();

    if normalized_name.is_empty()
        || normalized_type.is_empty()
        || normalized_base_url.is_empty()
        || normalized_improve_model.is_empty()
        || normalized_translate_model.is_empty()
    {
        return Err("Complete provider name, type, base URL and models before saving.".to_string());
    }
    let default_transcribe = default_transcribe_model();
    let transcribe_model = provider
        .transcribe_model
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or(default_transcribe.as_str())
        .to_string();
    let incoming_api_key = api_key
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string);

    let provider_id = provider
        .id
        .clone()
        .and_then(|id| {
            let clean = id.trim().to_string();
            if clean.is_empty() {
                None
            } else {
                Some(clean)
            }
        })
        .or_else(|| {
            config
                .providers
                .iter()
                .find(|existing| {
                    existing
                        .name
                        .trim()
                        .eq_ignore_ascii_case(normalized_name.as_str())
                        && existing
                            .provider_type
                            .trim()
                            .eq_ignore_ascii_case(normalized_type.as_str())
                        && normalize_base_url(&existing.base_url)
                            .eq_ignore_ascii_case(normalized_base_url.as_str())
                })
                .map(|existing| existing.id.clone())
        })
        .unwrap_or_else(|| format!("provider-{}", now_millis()));
    let had_active_provider = config.providers.iter().any(|item| item.is_active);

    let mut updated = false;
    for existing in &mut config.providers {
        if existing.id == provider_id {
            existing.name = normalized_name.clone();
            existing.provider_type = normalized_type.clone();
            existing.base_url = normalized_base_url.clone();
            existing.improve_model = normalized_improve_model.clone();
            existing.translate_model = normalized_translate_model.clone();
            existing.transcribe_model = transcribe_model.clone();
            updated = true;
            break;
        }
    }

    if !updated {
        if incoming_api_key.is_none() {
            return Err("API key is required for new providers. Add it before saving.".to_string());
        }

        config.providers.push(ProviderConfig {
            id: provider_id.clone(),
            name: normalized_name,
            provider_type: normalized_type,
            base_url: normalized_base_url,
            improve_model: normalized_improve_model,
            translate_model: normalized_translate_model,
            transcribe_model,
            api_key_fallback_b64: incoming_api_key
                .as_deref()
                .and_then(encode_api_key_fallback),
            is_active: !had_active_provider,
        });
    }

    if !config.providers.is_empty() && !config.providers.iter().any(|item| item.is_active) {
        if let Some(first) = config.providers.first_mut() {
            first.is_active = true;
        }
    }

    if let Some(saved_provider) = config
        .providers
        .iter_mut()
        .find(|item| item.id == provider_id)
    {
        if let Some(secret) = incoming_api_key.as_deref() {
            saved_provider.api_key_fallback_b64 = encode_api_key_fallback(secret);
        } else if let Some(existing_secret) = provider_api_key_from_config(saved_provider) {
            if saved_provider.api_key_fallback_b64.is_none() {
                saved_provider.api_key_fallback_b64 = encode_api_key_fallback(&existing_secret);
            }
        } else {
            return Err("API key is required for this provider. Add it before saving.".to_string());
        }
    }

    save_config(&app, &config)?;

    let saved = config
        .providers
        .iter()
        .find(|provider| provider.id == provider_id)
        .cloned()
        .ok_or_else(|| "Provider was saved but could not be reloaded.".to_string())?;

    Ok(provider_to_view(&saved))
}

#[tauri::command]
fn set_active_provider(app: tauri::AppHandle, provider_id: String) -> Result<(), String> {
    let mut config = load_config(&app)?;

    if !config
        .providers
        .iter()
        .any(|provider| provider.id == provider_id)
    {
        return Err("Provider not found.".to_string());
    }

    for provider in &mut config.providers {
        provider.is_active = provider.id == provider_id;
    }

    save_config(&app, &config)
}

#[tauri::command]
fn delete_provider(app: tauri::AppHandle, provider_id: String) -> Result<(), String> {
    let mut config = load_config(&app)?;
    let clean_id = provider_id.trim();
    let previous_len = config.providers.len();

    config.providers.retain(|provider| provider.id != clean_id);

    if config.providers.len() == previous_len {
        return Err("Provider not found.".to_string());
    }

    if !config.providers.is_empty() && !config.providers.iter().any(|provider| provider.is_active) {
        if let Some(first) = config.providers.first_mut() {
            first.is_active = true;
        }
    }

    if let Ok(entry) = keyring_entry(clean_id) {
        let _ = entry.delete_credential();
    }

    save_config(&app, &config)
}

#[tauri::command]
async fn test_provider_connection(
    app: tauri::AppHandle,
    provider_id: String,
) -> Result<String, String> {
    let config = load_config(&app)?;
    let provider = config
        .providers
        .iter()
        .find(|item| item.id == provider_id)
        .cloned()
        .ok_or_else(|| "Provider not found.".to_string())?;

    let api_key = provider_api_key(&provider)?;
    let endpoint = format!("{}/models", normalize_base_url(&provider.base_url));

    let client = reqwest::Client::new();
    let response = client
        .get(endpoint)
        .bearer_auth(api_key)
        .send()
        .await
        .map_err(|e| format!("Provider connection failed: {e}"))?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response
            .text()
            .await
            .unwrap_or_else(|_| "<empty body>".to_string());
        return Err(format!(
            "Connection failed with HTTP {}: {}",
            status.as_u16(),
            body
        ));
    }

    let payload: OpenAiModelsResponse = response
        .json()
        .await
        .map_err(|e| format!("Could not parse models response: {e}"))?;

    Ok(format!(
        "Connected successfully. Provider returned {} model entries from /models.",
        payload.data.len()
    ))
}

#[tauri::command]
async fn test_provider_connection_input(
    app: tauri::AppHandle,
    provider: ProviderInput,
    api_key: Option<String>,
) -> Result<String, String> {
    let name = provider.name.trim().to_string();
    let provider_type = provider.provider_type.trim().to_string();
    let base_url = normalize_base_url(&provider.base_url);

    if name.is_empty() || provider_type.is_empty() || base_url.is_empty() {
        return Err("Complete provider name, type and base URL before testing.".to_string());
    }

    let config = load_config(&app)?;
    let resolved_api_key = provider_api_key_for_input(&config, &provider, api_key)?;

    let endpoint = format!("{}/models", base_url);
    let client = reqwest::Client::new();
    let response = client
        .get(endpoint)
        .bearer_auth(resolved_api_key)
        .send()
        .await
        .map_err(|e| format!("Provider connection failed: {e}"))?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response
            .text()
            .await
            .unwrap_or_else(|_| "<empty body>".to_string());
        return Err(format!(
            "Connection failed with HTTP {}: {}",
            status.as_u16(),
            body
        ));
    }

    let payload: OpenAiModelsResponse = response
        .json()
        .await
        .map_err(|e| format!("Could not parse models response: {e}"))?;

    Ok(format!(
        "Connected successfully. Provider returned {} model entries from /models.",
        payload.data.len()
    ))
}

#[tauri::command]
async fn transcribe_audio(
    app: tauri::AppHandle,
    audio_base64: String,
    mime_type: Option<String>,
) -> Result<String, String> {
    let base64_payload = audio_base64.trim();
    if base64_payload.is_empty() {
        return Err("Audio payload is empty.".to_string());
    }

    let audio_bytes = base64::engine::general_purpose::STANDARD
        .decode(base64_payload)
        .map_err(|e| format!("Could not decode audio payload: {e}"))?;

    if audio_bytes.is_empty() {
        return Err("Audio payload is empty.".to_string());
    }

    let config = load_config(&app)?;
    let provider = active_provider(&config)?;
    let api_key = provider_api_key(&provider)?;
    let endpoint = format!(
        "{}/audio/transcriptions",
        normalize_base_url(&provider.base_url)
    );

    let mime = mime_type
        .as_ref()
        .map(|value| value.trim())
        .filter(|value| !value.is_empty());
    let mime_for_part = mime
        .map(|value| value.split(';').next().unwrap_or(value).trim())
        .filter(|value| !value.is_empty())
        .unwrap_or("application/octet-stream");

    let file_part = reqwest::multipart::Part::bytes(audio_bytes)
        .file_name(audio_file_name(mime))
        .mime_str(mime_for_part)
        .map_err(|e| format!("Invalid audio mime type: {e}"))?;

    let form = reqwest::multipart::Form::new()
        .text("model", provider.transcribe_model.clone())
        .part("file", file_part);

    let client = reqwest::Client::new();
    let response = client
        .post(endpoint)
        .bearer_auth(api_key)
        .multipart(form)
        .send()
        .await
        .map_err(|e| format!("Transcription request failed: {e}"))?;

    let status = response.status();
    let body = response
        .text()
        .await
        .unwrap_or_else(|_| "<empty body>".to_string());

    if !status.is_success() {
        return Err(format!(
            "Transcription failed with HTTP {}: {}",
            status.as_u16(),
            body
        ));
    }

    parse_transcription_text(&body)
        .ok_or_else(|| "Transcription response was empty. Try recording again.".to_string())
}

#[tauri::command]
fn auto_insert_text(app: tauri::AppHandle, text: String) -> Result<InsertTextResult, String> {
    let value = text.trim();
    if value.is_empty() {
        return Err("Nothing to insert.".to_string());
    }

    app.clipboard()
        .write_text(value.to_string())
        .map_err(|e| format!("Could not copy text to clipboard: {e}"))?;

    hide_main_window(&app);
    if let Some(window) = app.get_webview_window("quick") {
        let _ = window.hide();
    }

    restore_last_external_app(&app);
    thread::sleep(std::time::Duration::from_millis(180));

    match simulate_paste_shortcut() {
        Ok(()) => Ok(InsertTextResult {
            copied: true,
            pasted: true,
            message: "Text copied and pasted in the active app.".to_string(),
        }),
        Err(error) => Ok(InsertTextResult {
            copied: true,
            pasted: false,
            message: format!("Automatic paste failed: {error}"),
        }),
    }
}

#[tauri::command]
async fn improve_text(
    app: tauri::AppHandle,
    input: String,
    style: String,
) -> Result<String, String> {
    if input.trim().is_empty() {
        return Err("Input text is empty.".to_string());
    }

    let config = load_config(&app)?;
    let provider = active_provider(&config)?;
    let api_key = provider_api_key(&provider)?;
    let (mode_name, mode_instruction) = mode_instruction_for(&config.prompt_settings, &style);
    let system_prompt = config
        .prompt_settings
        .improve_system_prompt
        .trim()
        .to_string();
    let user_prompt = format!(
        "Mode: {mode_name}\nMode instruction: {mode_instruction}\nAudience: coworker.\n\nText:\n{}",
        input.trim()
    );

    run_chat_completion(
        &provider,
        &api_key,
        &provider.improve_model,
        &system_prompt,
        &user_prompt,
    )
    .await
}

#[tauri::command]
async fn translate_text(
    app: tauri::AppHandle,
    input: String,
    style: String,
) -> Result<String, String> {
    if input.trim().is_empty() {
        return Err("Input text is empty.".to_string());
    }

    let config = load_config(&app)?;
    let provider = active_provider(&config)?;
    let api_key = provider_api_key(&provider)?;
    let (mode_name, mode_instruction) = mode_instruction_for(&config.prompt_settings, &style);
    let system_prompt = config
        .prompt_settings
        .translate_system_prompt
        .trim()
        .to_string();
    let user_prompt = format!(
        "Mode: {mode_name}\nMode instruction: {mode_instruction}\n\nSpanish text:\n{}",
        input.trim()
    );

    run_chat_completion(
        &provider,
        &api_key,
        &provider.translate_model,
        &system_prompt,
        &user_prompt,
    )
    .await
}

#[tauri::command]
fn consume_pending_improve_text(state: tauri::State<'_, PendingLaunchText>) -> Option<String> {
    let mut pending = state.0.lock().ok()?;
    pending.take()
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .manage(PendingLaunchText::default())
        .manage(PendingQuickAction::default())
        .manage(LastExternalApp::default())
        .plugin(tauri_plugin_clipboard_manager::init())
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .plugin(tauri_plugin_single_instance::init(|app, args, _cwd| {
            if let Some(text) = parse_improve_text_from_args(&args) {
                emit_external_text_event(app, text, "single-instance");
            }
        }))
        .on_window_event(|window, event| {
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
            if cfg!(debug_assertions) {
                app.handle().plugin(
                    tauri_plugin_log::Builder::default()
                        .level(log::LevelFilter::Info)
                        .build(),
                )?;
            }

            if let Err(error) = setup_tray_icon(app.handle()) {
                log::warn!("Tray icon is unavailable: {error}");
            }

            let launch_args = std::env::args().collect::<Vec<_>>();
            if let Some(text) = parse_improve_text_from_args(&launch_args) {
                save_pending_text(app.handle(), text);
            }

            let config = load_config(app.handle())?;
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

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            list_providers,
            get_hotkeys,
            get_prompt_settings,
            get_onboarding_status,
            complete_onboarding,
            open_permission_settings,
            probe_auto_insert_permission,
            open_settings_window,
            open_widget_window,
            open_quick_window,
            set_quick_window_expanded,
            close_quick_window,
            open_main_mode,
            capture_selected_text,
            save_hotkeys,
            save_prompt_settings,
            save_provider,
            set_active_provider,
            delete_provider,
            test_provider_connection,
            test_provider_connection_input,
            transcribe_audio,
            auto_insert_text,
            improve_text,
            translate_text,
            consume_pending_improve_text,
            consume_pending_quick_action,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
