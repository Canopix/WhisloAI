use base64::Engine as _;
use keyring::Entry;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Mutex;
use std::thread;
use std::time::{Instant, SystemTime, UNIX_EPOCH};
use tauri::path::BaseDirectory;
use tauri::{Emitter, LogicalSize, Manager, PhysicalPosition, Position, Size};
use tauri_plugin_clipboard_manager::ClipboardExt;
use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut, ShortcutState};

const KEYRING_SERVICE: &str = "whisloai";
const QUICK_WINDOW_WIDTH: f64 = 252.0;
const QUICK_WINDOW_HEIGHT_COMPACT: f64 = 64.0;
const QUICK_WINDOW_HEIGHT_EXPANDED: f64 = 96.0;
const TRAY_ICON_ID: &str = "whisloai-tray";
const TRAY_MENU_OPEN_APP: &str = "tray-open-app";
const TRAY_MENU_OPEN_SETTINGS: &str = "tray-open-settings";
const TRAY_MENU_QUIT: &str = "tray-quit";
const SUPPORTED_STYLE_MODES: [&str; 5] = ["simple", "professional", "friendly", "casual", "formal"];
const INPUT_TARGET_TTL_MS: u128 = 90_000;
const REFOCUS_CLICK_STABILIZE_MS: u64 = 45;
const REFOCUS_POST_RESTORE_MS: u64 = 35;

#[derive(Default)]
struct PendingLaunchText(Mutex<Option<String>>);

#[derive(Default)]
struct PendingQuickAction(Mutex<Option<String>>);

#[derive(Default)]
struct LastExternalAppBundle(Mutex<Option<String>>);

#[derive(Default)]
struct LastAnchorPosition(Mutex<Option<AnchorPosition>>);

#[derive(Default)]
struct LastAnchorTimestamp(Mutex<Option<u128>>);

#[derive(Default)]
struct LastInputFocusTarget(Mutex<Option<InputFocusTarget>>);

static ANCHOR_MONITOR_STARTED: AtomicBool = AtomicBool::new(false);
static SETTINGS_WINDOW_OPEN: AtomicBool = AtomicBool::new(false);
static TRAY_READY: AtomicBool = AtomicBool::new(false);
static APP_QUIT_REQUESTED: AtomicBool = AtomicBool::new(false);
static QUICK_OPEN_REQUEST_COUNTER: AtomicU64 = AtomicU64::new(1);

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
    #[serde(default = "default_source_language")]
    source_language: String,
    #[serde(default = "default_target_language")]
    target_language: String,
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
    #[serde(default)]
    source_language: String,
    #[serde(default)]
    target_language: String,
    mode_instructions: HashMap<String, String>,
    quick_mode: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
struct UiSettings {
    #[serde(default = "default_ui_language_preference")]
    ui_language_preference: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct UiSettingsInput {
    #[serde(default = "default_ui_language_preference")]
    ui_language_preference: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
struct TranscriptionConfig {
    #[serde(default)]
    mode: String,
    #[serde(default)]
    local_model_path: Option<String>,
}

impl Default for TranscriptionConfig {
    fn default() -> Self {
        Self {
            mode: "api".to_string(),
            local_model_path: None,
        }
    }
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
    #[serde(default)]
    transcription: TranscriptionConfig,
    #[serde(default = "default_ui_language_preference")]
    ui_language_preference: String,
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

#[derive(Debug, Clone)]
struct InputFocusTarget {
    bundle_id: String,
    x: i32,
    y: i32,
    captured_at_ms: u128,
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
            transcription: TranscriptionConfig::default(),
            ui_language_preference: default_ui_language_preference(),
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
    "You are a translation assistant. Convert text from {source} into clear, concise, natural {target} for workplace chat. Preserve names and technical terms. Return only final text.".to_string()
}

fn default_source_language() -> String {
    "Spanish".to_string()
}

fn default_target_language() -> String {
    "English".to_string()
}

fn default_quick_mode() -> String {
    "simple".to_string()
}

fn default_ui_language_preference() -> String {
    "system".to_string()
}

fn default_mode_instruction_for(mode: &str) -> Option<&'static str> {
    match mode {
        "simple" => Some("Use clear, concise wording with everyday vocabulary."),
        "professional" => Some("Use a polished workplace tone with direct and confident wording."),
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
        source_language: default_source_language(),
        target_language: default_target_language(),
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

fn normalize_ui_language_preference(value: &str) -> String {
    match value.trim().to_lowercase().as_str() {
        "system" | "en" | "es" => value.trim().to_lowercase(),
        _ => default_ui_language_preference(),
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
        // Migrate old prompts that hardcode languages to use {source} and {target} placeholders
        if !settings.translate_system_prompt.contains("{source}")
            || !settings.translate_system_prompt.contains("{target}")
        {
            let migrated = settings
                .translate_system_prompt
                .replace("Spanish", "{source}")
                .replace("English", "{target}");
            if migrated != settings.translate_system_prompt {
                settings.translate_system_prompt = migrated;
                changed = true;
            }
        }
    }

    if settings.source_language.trim().is_empty() {
        settings.source_language = default_source_language();
        changed = true;
    } else {
        let clean = settings.source_language.trim().to_string();
        if clean != settings.source_language {
            settings.source_language = clean;
            changed = true;
        }
    }

    if settings.target_language.trim().is_empty() {
        settings.target_language = default_target_language();
        changed = true;
    } else {
        let clean = settings.target_language.trim().to_string();
        if clean != settings.target_language {
            settings.target_language = clean;
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

fn models_dir(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    let dir = app
        .path()
        .app_config_dir()
        .map_err(|e| format!("Could not resolve app config directory: {e}"))?
        .join("whisper-models");
    fs::create_dir_all(&dir).map_err(|e| format!("Could not create models directory: {e}"))?;
    Ok(dir)
}

const WHISPER_MODELS: &[(&str, &str, &str)] = &[
    ("tiny", "ggml-tiny.bin", "~75 MB"),
    ("tiny.en", "ggml-tiny.en.bin", "~75 MB"),
    ("base", "ggml-base.bin", "~142 MB"),
    ("base.en", "ggml-base.en.bin", "~142 MB"),
    ("small", "ggml-small.bin", "~466 MB"),
    ("small.en", "ggml-small.en.bin", "~466 MB"),
];

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

    let normalized_ui_language_preference =
        normalize_ui_language_preference(&config.ui_language_preference);
    if normalized_ui_language_preference != config.ui_language_preference {
        config.ui_language_preference = normalized_ui_language_preference;
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
    value.is_empty() || value.contains("whisloai") || value.contains("com.whisloai.app")
}

fn save_last_external_app_bundle(app: &tauri::AppHandle, bundle_id: &str) {
    let clean = bundle_id.trim();
    if clean.is_empty() || is_internal_app_bundle_id(clean) {
        return;
    }
    if let Some(state) = app.try_state::<LastExternalAppBundle>() {
        if let Ok(mut guard) = state.0.lock() {
            *guard = Some(clean.to_string());
        }
    }
}

fn last_external_app(app: &tauri::AppHandle) -> Option<String> {
    let state = app.try_state::<LastExternalAppBundle>()?;
    let guard = state.0.lock().ok()?;
    guard.clone()
}

fn save_last_anchor_position(app: &tauri::AppHandle, position: AnchorPosition) {
    if let Some(state) = app.try_state::<LastAnchorPosition>() {
        if let Ok(mut guard) = state.0.lock() {
            *guard = Some(position);
        }
    }
}

fn clear_last_anchor_position(app: &tauri::AppHandle) {
    if let Some(state) = app.try_state::<LastAnchorPosition>() {
        if let Ok(mut guard) = state.0.lock() {
            *guard = None;
        }
    }
}

fn last_anchor_position(app: &tauri::AppHandle) -> Option<AnchorPosition> {
    let state = app.try_state::<LastAnchorPosition>()?;
    let guard = state.0.lock().ok()?;
    *guard
}

fn save_last_anchor_timestamp(app: &tauri::AppHandle, timestamp: u128) {
    if let Some(state) = app.try_state::<LastAnchorTimestamp>() {
        if let Ok(mut guard) = state.0.lock() {
            *guard = Some(timestamp);
        }
    }
}

fn clear_last_anchor_timestamp(app: &tauri::AppHandle) {
    if let Some(state) = app.try_state::<LastAnchorTimestamp>() {
        if let Ok(mut guard) = state.0.lock() {
            *guard = None;
        }
    }
}

fn last_anchor_timestamp(app: &tauri::AppHandle) -> Option<u128> {
    let state = app.try_state::<LastAnchorTimestamp>()?;
    let guard = state.0.lock().ok()?;
    *guard
}

fn last_anchor_age_ms(app: &tauri::AppHandle) -> Option<u128> {
    let timestamp = last_anchor_timestamp(app)?;
    let now = now_millis();
    Some(now.saturating_sub(timestamp))
}

fn save_last_input_focus_target(app: &tauri::AppHandle, target: InputFocusTarget) {
    if target.bundle_id.trim().is_empty() || is_internal_app_bundle_id(&target.bundle_id) {
        return;
    }
    if let Some(state) = app.try_state::<LastInputFocusTarget>() {
        if let Ok(mut guard) = state.0.lock() {
            *guard = Some(target);
        }
    }
}

fn clear_last_input_focus_target(app: &tauri::AppHandle) {
    if let Some(state) = app.try_state::<LastInputFocusTarget>() {
        if let Ok(mut guard) = state.0.lock() {
            *guard = None;
        }
    }
}

fn last_input_focus_target(app: &tauri::AppHandle) -> Option<InputFocusTarget> {
    let state = app.try_state::<LastInputFocusTarget>()?;
    let guard = state.0.lock().ok()?;
    guard.clone()
}

#[derive(Debug, Clone, Copy)]
struct RefocusAttempt {
    attempted: bool,
    ok: bool,
    target_age_ms: Option<u128>,
}

#[cfg(any(target_os = "macos", target_os = "windows"))]
fn refocus_last_input_target(app: &tauri::AppHandle) -> Result<RefocusAttempt, String> {
    use enigo::Direction::Click;
    use enigo::{Button, Coordinate, Enigo, Mouse, Settings};

    let Some(target) = last_input_focus_target(app) else {
        return Ok(RefocusAttempt {
            attempted: false,
            ok: false,
            target_age_ms: None,
        });
    };

    let target_age_ms = now_millis().saturating_sub(target.captured_at_ms);
    if target_age_ms > INPUT_TARGET_TTL_MS || is_internal_app_bundle_id(&target.bundle_id) {
        return Ok(RefocusAttempt {
            attempted: false,
            ok: false,
            target_age_ms: Some(target_age_ms),
        });
    }

    let mut enigo = Enigo::new(&Settings::default())
        .map_err(|e| format!("Could not initialize input automation for refocus: {e}"))?;

    // Best effort validation: avoid out-of-bounds points on main display.
    if let Ok((display_w, display_h)) = enigo.main_display() {
        let max_x = display_w.saturating_sub(1);
        let max_y = display_h.saturating_sub(1);
        if target.x < 0 || target.y < 0 || target.x > max_x || target.y > max_y {
            return Ok(RefocusAttempt {
                attempted: false,
                ok: false,
                target_age_ms: Some(target_age_ms),
            });
        }
    }

    let original_cursor = enigo
        .location()
        .map_err(|e| format!("Could not read current cursor position: {e}"))?;

    enigo
        .move_mouse(target.x, target.y, Coordinate::Abs)
        .map_err(|e| format!("Could not move cursor to input target: {e}"))?;
    enigo
        .button(Button::Left, Click)
        .map_err(|e| format!("Could not click input target: {e}"))?;
    thread::sleep(std::time::Duration::from_millis(REFOCUS_CLICK_STABILIZE_MS));

    let _ = enigo.move_mouse(original_cursor.0, original_cursor.1, Coordinate::Abs);
    thread::sleep(std::time::Duration::from_millis(REFOCUS_POST_RESTORE_MS));

    Ok(RefocusAttempt {
        attempted: true,
        ok: true,
        target_age_ms: Some(target_age_ms),
    })
}

#[cfg(not(any(target_os = "macos", target_os = "windows")))]
fn refocus_last_input_target(_app: &tauri::AppHandle) -> Result<RefocusAttempt, String> {
    Err("Input refocus is not supported on this platform.".to_string())
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

/// Maps common language names to ISO-639-1 codes for the Whisper transcription API.
fn language_to_iso639(language: &str) -> Option<&'static str> {
    let normalized = language.trim().to_lowercase();
    match normalized.as_str() {
        "spanish" | "español" => Some("es"),
        "english" | "inglés" => Some("en"),
        "portuguese" | "português" => Some("pt"),
        "french" | "français" => Some("fr"),
        "german" | "deutsch" => Some("de"),
        "italian" | "italiano" => Some("it"),
        "japanese" | "日本語" => Some("ja"),
        "chinese" | "中文" => Some("zh"),
        "korean" | "한국어" => Some("ko"),
        "russian" => Some("ru"),
        "dutch" => Some("nl"),
        "arabic" => Some("ar"),
        "hindi" => Some("hi"),
        "turkish" => Some("tr"),
        "polish" => Some("pl"),
        "swedish" => Some("sv"),
        "catalan" => Some("ca"),
        _ => None,
    }
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

#[derive(Debug, Clone)]
struct FocusedAnchorSnapshot {
    position: AnchorPosition,
    bundle_id: Option<String>,
    input_focus_point: Option<(i32, i32)>,
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
        .title("WhisloAI Anchor")
        .inner_size(36.0, 36.0)
        .min_inner_size(36.0, 36.0)
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
        .title("WhisloAI Quick")
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

fn to_u64_saturating(value: u128) -> u64 {
    value.min(u64::MAX as u128) as u64
}

fn clamp_quick_window_position(
    anchor: &tauri::WebviewWindow,
    default_x: i32,
    default_y: i32,
    quick_width: i32,
    quick_height: i32,
) -> (i32, i32) {
    if let Ok(Some(monitor)) = anchor.monitor_from_point(default_x as f64, default_y as f64) {
        let work = monitor.work_area();
        let min_x = work.position.x + 8;
        let min_y = work.position.y + 8;
        let max_x = (work.position.x + work.size.width as i32 - quick_width - 8).max(min_x);
        let max_y = (work.position.y + work.size.height as i32 - quick_height - 8).max(min_y);
        (default_x.clamp(min_x, max_x), default_y.clamp(min_y, max_y))
    } else {
        (default_x.max(8), default_y.max(8))
    }
}

fn position_quick_window_near_anchor(app: &tauri::AppHandle) -> (&'static str, Option<u128>) {
    let Some(quick) = app.get_webview_window("quick") else {
        return ("quick-window-missing", None);
    };
    let quick_size = quick.outer_size().unwrap_or_default();
    let quick_width = quick_size.width as i32;
    let quick_height = quick_size.height as i32;

    if let Some(position) = last_anchor_position(app) {
        if let Some(anchor) = app.get_webview_window("anchor") {
            let default_x = position.x + 34;
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
        if let Ok(pos) = anchor.outer_position() {
            let default_x = pos.x + 34;
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

fn log_quick_open_trace(
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

fn log_auto_insert_trace(
    outcome: &str,
    error: Option<&str>,
    target_age_ms: Option<u128>,
    refocus_attempted: bool,
    refocus_ok: bool,
    paste_ok: bool,
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

fn open_quick_window_with_action(
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

#[cfg(target_os = "macos")]
fn focused_text_anchor_snapshot() -> Option<FocusedAnchorSnapshot> {
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
      if processName contains "whisloai" then return "NONE"
      if processBundleId contains "whisloai" then return "NONE"
      if processBundleId contains "com.whisloai.app" then return "NONE"
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
      return processBundleId & tab & (px as string) & "," & (py as string) & "," & (pw as string) & "," & (ph as string)
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

    let (bundle_raw, geometry_raw) = raw.split_once('\t')?;
    let mut parts = geometry_raw.split(',').map(str::trim);
    let x = parts.next()?.parse::<i32>().ok()?;
    let y = parts.next()?.parse::<i32>().ok()?;
    let w = parts.next()?.parse::<i32>().ok()?;
    let h = parts.next()?.parse::<i32>().ok()?;

    let bundle_id = {
        let clean = bundle_raw.trim();
        if clean.is_empty() {
            None
        } else {
            Some(clean.to_string())
        }
    };

    let input_focus_point = if w > 2 && h > 2 {
        Some((x + (w / 2), y + (h / 2)))
    } else {
        None
    };

    Some(FocusedAnchorSnapshot {
        position: AnchorPosition {
            x: (x + w - 10).max(8),
            y: (y - 44).max(8), // Above the input row, not among inline icons (emoji, mic, etc.)
        },
        bundle_id,
        input_focus_point,
    })
}

#[cfg(not(target_os = "macos"))]
fn focused_text_anchor_snapshot() -> Option<FocusedAnchorSnapshot> {
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
                clear_last_anchor_position(&app);
                clear_last_anchor_timestamp(&app);
                clear_last_input_focus_target(&app);
                thread::sleep(std::time::Duration::from_millis(180));
                continue;
            }

            if let Some(quick) = app.get_webview_window("quick") {
                if quick.is_visible().unwrap_or(false) {
                    let _ = anchor.hide();
                    last = None;
                    thread::sleep(std::time::Duration::from_millis(180));
                    continue;
                }
            }

            let snapshot = focused_text_anchor_snapshot();
            let next = snapshot.as_ref().map(|entry| entry.position);

            if let Some(entry) = snapshot.as_ref() {
                save_last_anchor_position(&app, entry.position);
                save_last_anchor_timestamp(&app, now_millis());
                if let Some(bundle_id) = entry.bundle_id.as_deref() {
                    save_last_external_app_bundle(&app, bundle_id);
                    if let Some((focus_x, focus_y)) = entry.input_focus_point {
                        save_last_input_focus_target(
                            &app,
                            InputFocusTarget {
                                bundle_id: bundle_id.to_string(),
                                x: focus_x.max(1),
                                y: focus_y.max(1),
                                captured_at_ms: now_millis(),
                            },
                        );
                    } else {
                        clear_last_input_focus_target(&app);
                    }
                } else {
                    clear_last_input_focus_target(&app);
                }
            } else {
                clear_last_anchor_position(&app);
                clear_last_anchor_timestamp(&app);
                clear_last_input_focus_target(&app);
            }

            if next != last {
                match next {
                    Some(position) => {
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

            thread::sleep(std::time::Duration::from_millis(180));
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
        .text(TRAY_MENU_OPEN_APP, "Open WhisloAI")
        .text(TRAY_MENU_OPEN_SETTINGS, "Settings")
        .separator()
        .text(TRAY_MENU_QUIT, "Quit")
        .build()
        .map_err(|e| format!("Could not build tray menu: {e}"))?;

    let mut tray_builder = tauri::tray::TrayIconBuilder::with_id(TRAY_ICON_ID)
        .menu(&tray_menu)
        .tooltip("WhisloAI")
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
fn get_ui_settings(app: tauri::AppHandle) -> Result<UiSettings, String> {
    let config = load_config(&app)?;
    Ok(UiSettings {
        ui_language_preference: normalize_ui_language_preference(&config.ui_language_preference),
    })
}

#[tauri::command]
fn get_transcription_config(app: tauri::AppHandle) -> Result<TranscriptionConfig, String> {
    let config = load_config(&app)?;
    Ok(config.transcription)
}

#[tauri::command]
fn save_transcription_config(
    app: tauri::AppHandle,
    transcription: TranscriptionConfig,
) -> Result<TranscriptionConfig, String> {
    let mut config = load_config(&app)?;
    let mode = transcription.mode.trim().to_lowercase();
    let valid_mode = matches!(mode.as_str(), "api" | "local");
    config.transcription = TranscriptionConfig {
        mode: if valid_mode { mode } else { "api".to_string() },
        local_model_path: transcription
            .local_model_path
            .filter(|p| !p.trim().is_empty())
            .map(|p| p.trim().to_string()),
    };
    save_config(&app, &config)?;
    Ok(config.transcription)
}

#[tauri::command]
fn list_whisper_models() -> Vec<serde_json::Value> {
    WHISPER_MODELS
        .iter()
        .map(|(id, filename, size)| {
            serde_json::json!({
                "id": id,
                "filename": filename,
                "size": size,
            })
        })
        .collect()
}

#[tauri::command]
async fn download_whisper_model(app: tauri::AppHandle, model_id: String) -> Result<String, String> {
    let (_, filename, _) = WHISPER_MODELS
        .iter()
        .find(|(id, _, _)| *id == model_id)
        .ok_or_else(|| format!("Unknown model: {model_id}"))?;

    let models_dir = models_dir(&app)?;
    let dest_path = models_dir.join(*filename);

    if dest_path.exists() {
        return Ok(dest_path.to_string_lossy().to_string());
    }

    let url = format!("https://huggingface.co/ggerganov/whisper.cpp/resolve/main/{filename}");

    let client = reqwest::Client::new();
    let response = client
        .get(&url)
        .send()
        .await
        .map_err(|e| format!("Download failed: {e}"))?;

    if !response.status().is_success() {
        return Err(format!(
            "Download failed with status {}",
            response.status().as_u16()
        ));
    }

    let bytes = response
        .bytes()
        .await
        .map_err(|e| format!("Download failed: {e}"))?;

    fs::write(&dest_path, &bytes).map_err(|e| format!("Could not save model: {e}"))?;

    Ok(dest_path.to_string_lossy().to_string())
}

#[tauri::command]
async fn pick_local_whisper_model(app: tauri::AppHandle) -> Result<Option<String>, String> {
    use tauri_plugin_dialog::DialogExt;
    let path = tauri::async_runtime::spawn_blocking(move || {
        app.dialog()
            .file()
            .add_filter("Whisper model", &["bin"])
            .set_title("Select Whisper model (.bin)")
            .blocking_pick_file()
    })
    .await
    .map_err(|e| format!("Dialog failed: {e}"))?;
    Ok(path.map(|p| p.to_string()))
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
        .title("WhisloAI Settings")
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
    let source = prompt_settings.source_language.trim().to_string();
    let target = prompt_settings.target_language.trim().to_string();
    let mut next = PromptSettings {
        improve_system_prompt: prompt_settings.improve_system_prompt.trim().to_string(),
        translate_system_prompt: prompt_settings.translate_system_prompt.trim().to_string(),
        source_language: if source.is_empty() {
            default_source_language()
        } else {
            source
        },
        target_language: if target.is_empty() {
            default_target_language()
        } else {
            target
        },
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
fn save_ui_settings(
    app: tauri::AppHandle,
    ui_settings: UiSettingsInput,
) -> Result<UiSettings, String> {
    let mut config = load_config(&app)?;
    let normalized = normalize_ui_language_preference(&ui_settings.ui_language_preference);
    config.ui_language_preference = normalized.clone();
    save_config(&app, &config)?;

    let payload = UiSettings {
        ui_language_preference: normalized,
    };
    app.emit("ui-language-changed", &payload)
        .map_err(|e| format!("Could not emit ui-language-changed: {e}"))?;
    Ok(payload)
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

#[cfg(feature = "local-transcription")]
fn transcribe_with_local_whisper(
    model_path: &str,
    audio_bytes: &[u8],
    _mime_type: Option<&str>,
    source_language: &str,
) -> Result<String, String> {
    use symphonia::core::audio::Signal;
    use symphonia::core::io::MediaSourceStream;
    use symphonia::core::probe::Hint;
    use whisper_rs::{FullParams, SamplingStrategy, WhisperContext, WhisperContextParameters};

    let audio_copy = audio_bytes.to_vec();
    let cursor = std::io::Cursor::new(audio_copy);
    let mss = MediaSourceStream::new(Box::new(cursor), Default::default());
    let hint = Hint::new();
    let mut format = symphonia::default::get_probe()
        .format(&hint, mss, &Default::default(), &Default::default())
        .map_err(|e| format!("Could not detect audio format: {e}"))?;
    let track = format
        .format
        .tracks()
        .iter()
        .find(|t| t.codec_params.codec != symphonia::core::codecs::CODEC_TYPE_NULL)
        .ok_or("No audio track found")?;
    let mut decoder = symphonia::default::get_codecs()
        .make(&track.codec_params, &Default::default())
        .map_err(|e| format!("Could not create decoder: {e}"))?;
    let sample_rate = track.codec_params.sample_rate.unwrap_or(16000) as u32;
    use symphonia::core::audio::AudioBufferRef;
    let mut samples: Vec<f32> = Vec::new();
    while let Ok(packet) = format.format.next_packet() {
        if let Ok(decoded) = decoder.decode(&packet) {
            match decoded {
                AudioBufferRef::F32(buf) => {
                    for frame in buf.chan(0) {
                        samples.push(*frame);
                    }
                }
                AudioBufferRef::S16(buf) => {
                    let s16_samples = buf.chan(0);
                    let mut floats = vec![0.0f32; s16_samples.len()];
                    let _ = whisper_rs::convert_integer_to_float_audio(s16_samples, &mut floats);
                    samples.extend(floats);
                }
                _ => {}
            }
        }
    }
    if samples.is_empty() {
        return Err("No audio samples decoded.".to_string());
    }
    let resampled = if sample_rate != 16000 {
        let new_len = (samples.len() as u64 * 16000 / sample_rate as u64) as usize;
        (0..new_len)
            .map(|i| {
                let src_idx = (i as f64 * sample_rate as f64 / 16000.0) as usize;
                samples.get(src_idx).copied().unwrap_or(0.0)
            })
            .collect::<Vec<_>>()
    } else {
        samples
    };
    let ctx = WhisperContext::new_with_params(model_path, WhisperContextParameters::default())
        .map_err(|e| format!("Could not load Whisper model: {e}"))?;
    let mut state = ctx
        .create_state()
        .map_err(|e| format!("Could not create state: {e}"))?;
    let mut params = FullParams::new(SamplingStrategy::BeamSearch {
        beam_size: 5,
        patience: -1.0,
    });
    if let Some(iso639) = language_to_iso639(source_language) {
        params.set_language(Some(iso639));
    }
    params.set_print_progress(false);
    state
        .full(params, &resampled)
        .map_err(|e| format!("Transcription failed: {e}"))?;
    let text: String = state
        .as_iter()
        .map(|s| s.to_string())
        .collect::<Vec<_>>()
        .join(" ")
        .trim()
        .to_string();
    Ok(if text.is_empty() {
        "".to_string()
    } else {
        text
    })
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

    if config.transcription.mode == "local" {
        if let Some(ref path) = config.transcription.local_model_path {
            if std::path::Path::new(path).exists() {
                #[cfg(feature = "local-transcription")]
                {
                    let source = config.prompt_settings.source_language.trim();
                    return transcribe_with_local_whisper(
                        path,
                        &audio_bytes,
                        mime_type.as_deref(),
                        source,
                    );
                }
                #[cfg(not(feature = "local-transcription"))]
                {
                    return Err("Local transcription requires building with the 'local-transcription' feature (cmake needed). Use API mode or rebuild with: cargo build --features local-transcription".to_string());
                }
            }
        }
        return Err(
            "Local model path not set or file not found. Configure in Settings.".to_string(),
        );
    }

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

    let mut form = reqwest::multipart::Form::new()
        .text("model", provider.transcribe_model.clone())
        .part("file", file_part);

    // Use source language from settings to improve transcription accuracy and latency
    let source = config.prompt_settings.source_language.trim();
    if let Some(iso639) = language_to_iso639(source) {
        form = form.text("language", iso639.to_string());
    }

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
    let total_started = Instant::now();
    let value = text.trim();
    if value.is_empty() {
        return Err("Nothing to insert.".to_string());
    }

    let previous_clipboard = app.clipboard().read_text().ok();

    app.clipboard()
        .write_text(value.to_string())
        .map_err(|e| format!("Could not copy text to clipboard: {e}"))?;

    hide_main_window(&app);
    if let Some(window) = app.get_webview_window("quick") {
        let _ = window.hide();
    }

    restore_last_external_app(&app);
    thread::sleep(std::time::Duration::from_millis(180));

    let (target_age_ms, refocus_attempted, refocus_ok) = match refocus_last_input_target(&app) {
        Ok(attempt) => (attempt.target_age_ms, attempt.attempted, attempt.ok),
        Err(error) => {
            let result = InsertTextResult {
                copied: true,
                pasted: false,
                message: format!("Automatic paste skipped: could not restore input focus: {error}"),
            };
            log_auto_insert_trace(
                "refocus-error",
                Some(error.as_str()),
                None,
                true,
                false,
                false,
                total_started.elapsed().as_millis(),
            );
            return Ok(result);
        }
    };

    if !refocus_ok {
        let result = InsertTextResult {
            copied: true,
            pasted: false,
            message:
                "Automatic paste skipped: input focus target not restored. Click target input and try again."
                    .to_string(),
        };
        log_auto_insert_trace(
            "focus-target-not-restored",
            None,
            target_age_ms,
            refocus_attempted,
            false,
            false,
            total_started.elapsed().as_millis(),
        );
        return Ok(result);
    }

    let mut paste_error_message: Option<String> = None;
    let result = match simulate_paste_shortcut() {
        Ok(()) => InsertTextResult {
            copied: true,
            pasted: true,
            message: "Text copied and pasted in the active app.".to_string(),
        },
        Err(error) => InsertTextResult {
            copied: true,
            pasted: false,
            message: {
                paste_error_message = Some(error.clone());
                format!("Automatic paste failed: {error}")
            },
        },
    };

    if result.pasted {
        thread::sleep(std::time::Duration::from_millis(100));
        if let Some(prev) = previous_clipboard {
            let _ = app.clipboard().write_text(prev);
        }
    }

    log_auto_insert_trace(
        if result.pasted { "ok" } else { "paste-error" },
        paste_error_message.as_deref(),
        target_age_ms,
        refocus_attempted,
        refocus_ok,
        result.pasted,
        total_started.elapsed().as_millis(),
    );

    Ok(result)
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
    let source = config.prompt_settings.source_language.trim();
    let target = config.prompt_settings.target_language.trim();
    let system_prompt = config
        .prompt_settings
        .translate_system_prompt
        .trim()
        .replace("{source}", source)
        .replace("{target}", target);
    let user_prompt = format!(
        "Mode: {mode_name}\nMode instruction: {mode_instruction}\n\n{source} text:\n{}",
        input.trim(),
        source = source
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
        .plugin(tauri_plugin_dialog::init())
        .manage(PendingLaunchText::default())
        .manage(PendingQuickAction::default())
        .manage(LastExternalAppBundle::default())
        .manage(LastAnchorPosition::default())
        .manage(LastAnchorTimestamp::default())
        .manage(LastInputFocusTarget::default())
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
            get_ui_settings,
            get_transcription_config,
            save_transcription_config,
            list_whisper_models,
            download_whisper_model,
            pick_local_whisper_model,
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
            save_ui_settings,
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
