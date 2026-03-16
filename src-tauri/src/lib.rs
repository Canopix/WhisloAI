use base64::Engine as _;
use keyring::Entry;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::io::Write;
use std::path::PathBuf;
use std::process::Command;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Mutex;
use std::thread;
use std::time::{Instant, SystemTime, UNIX_EPOCH};
use tauri::path::BaseDirectory;
use tauri::{Emitter, LogicalSize, Manager, PhysicalPosition, Position, Size};
use tauri_plugin_clipboard_manager::ClipboardExt;
use tauri_plugin_dialog::{DialogExt, MessageDialogButtons, MessageDialogKind};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut, ShortcutState};
use tauri_plugin_updater::UpdaterExt;

const KEYRING_SERVICE: &str = "whisloai";
const QUICK_WINDOW_WIDTH_COMPACT: f64 = 252.0;
const QUICK_WINDOW_WIDTH_EXPANDED: f64 = 252.0;
const QUICK_WINDOW_HEIGHT_COMPACT: f64 = 64.0;
const QUICK_WINDOW_HEIGHT_EXPANDED: f64 = 96.0;
const TRAY_ICON_ID: &str = "whisloai-tray";
const TRAY_MENU_OPEN_APP: &str = "tray-open-app";
const TRAY_MENU_OPEN_SETTINGS: &str = "tray-open-settings";
const TRAY_MENU_CHECK_UPDATES: &str = "tray-check-updates";
const TRAY_MENU_QUIT: &str = "tray-quit";
const SUPPORTED_STYLE_MODES: [&str; 5] = ["simple", "professional", "friendly", "casual", "formal"];
const INPUT_TARGET_TTL_MS: u128 = 90_000;
const REFOCUS_CLICK_STABILIZE_MS: u64 = 45;
const REFOCUS_POST_RESTORE_MS: u64 = 35;

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

#[derive(Default)]
struct AnchorBehaviorMode(Mutex<String>);

static ANCHOR_MONITOR_STARTED: AtomicBool = AtomicBool::new(false);
static SETTINGS_WINDOW_OPEN: AtomicBool = AtomicBool::new(false);
static TRAY_READY: AtomicBool = AtomicBool::new(false);
static APP_QUIT_REQUESTED: AtomicBool = AtomicBool::new(false);
static QUICK_OPEN_REQUEST_COUNTER: AtomicU64 = AtomicU64::new(1);
static UPDATE_CHECK_IN_FLIGHT: AtomicBool = AtomicBool::new(false);

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
struct HotkeyConfig {
    open_app: String,
    open_dictate_translate: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
struct ProviderConfig {
    id: String,
    name: String,
    provider_type: String,
    base_url: String,
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
    translate_model: String,
    #[serde(default)]
    transcribe_model: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
struct PromptSettings {
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
    #[serde(default = "default_anchor_behavior")]
    anchor_behavior: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct UiSettingsInput {
    #[serde(default = "default_ui_language_preference")]
    ui_language_preference: String,
    #[serde(default = "default_anchor_behavior")]
    anchor_behavior: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
struct TranscriptionConfig {
    #[serde(default)]
    mode: String,
    #[serde(default)]
    local_model_path: Option<String>,
    #[serde(default)]
    local_models_dir: Option<String>,
}

impl Default for TranscriptionConfig {
    fn default() -> Self {
        Self {
            mode: "api".to_string(),
            local_model_path: None,
            local_models_dir: None,
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
    #[serde(default = "default_anchor_behavior")]
    anchor_behavior: String,
}

#[derive(Debug, Deserialize)]
struct OpenAiModelsResponse {
    data: Vec<serde_json::Value>,
}

#[derive(Debug, Serialize)]
struct ChatRequest<'a> {
    #[serde(skip_serializing_if = "Option::is_none")]
    model: Option<&'a str>,
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

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct WhisperDownloadProgress {
    model_id: String,
    downloaded_bytes: u64,
    total_bytes: Option<u64>,
    percent: Option<u8>,
    done: bool,
    destination: Option<String>,
}

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct WhisperModelItem {
    id: String,
    filename: String,
    size: String,
    downloaded: bool,
    local_path: Option<String>,
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
            anchor_behavior: default_anchor_behavior(),
        }
    }
}

fn default_hotkeys() -> HotkeyConfig {
    HotkeyConfig {
        open_app: "CommandOrControl+Shift+Space".to_string(),
        open_dictate_translate: "CommandOrControl+Shift+D".to_string(),
    }
}

fn default_provider() -> ProviderConfig {
    ProviderConfig {
        id: "openai-default".to_string(),
        name: "OpenAI".to_string(),
        provider_type: "openai".to_string(),
        base_url: "https://api.openai.com/v1".to_string(),
        translate_model: "gpt-4.1-mini".to_string(),
        transcribe_model: default_transcribe_model(),
        api_key_fallback_b64: None,
        is_active: true,
    }
}

fn default_transcribe_model() -> String {
    "gpt-4o-mini-transcribe".to_string()
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

fn default_anchor_behavior() -> String {
    "contextual".to_string()
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

fn normalize_anchor_behavior(value: &str) -> String {
    match value.trim().to_lowercase().as_str() {
        "floating" => "floating".to_string(),
        _ => default_anchor_behavior(),
    }
}

fn normalize_prompt_settings(settings: &mut PromptSettings) -> bool {
    let mut changed = false;

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

fn non_empty_trimmed(value: &str) -> Option<&str> {
    let clean = value.trim();
    if clean.is_empty() {
        None
    } else {
        Some(clean)
    }
}

fn normalize_provider_base_url(base_url: &str) -> String {
    let mut normalized = normalize_base_url(base_url);
    if normalized.is_empty() {
        return normalized;
    }

    let lower = normalized.to_lowercase();
    for suffix in [
        "/chat/completions",
        "/chat",
        "/models",
        "/audio/transcriptions",
    ] {
        if lower.ends_with(suffix) && normalized.len() > suffix.len() {
            let keep_len = normalized.len() - suffix.len();
            normalized.truncate(keep_len);
            normalized = normalize_base_url(&normalized);
            break;
        }
    }
    normalized
}

fn provider_endpoint(base_url: &str, path: &str) -> String {
    let root = normalize_provider_base_url(base_url);
    let suffix = path.trim().trim_start_matches('/');
    if suffix.is_empty() {
        root
    } else {
        format!("{root}/{suffix}")
    }
}

fn local_prefers_openai_chat_endpoint(base_url: &str) -> bool {
    normalize_provider_base_url(base_url)
        .to_lowercase()
        .ends_with("/v1")
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

fn default_models_dir(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    let dir = app
        .path()
        .app_config_dir()
        .map_err(|e| format!("Could not resolve app config directory: {e}"))?
        .join("whisper-models");
    fs::create_dir_all(&dir).map_err(|e| format!("Could not create models directory: {e}"))?;
    Ok(dir)
}

fn resolved_transcription_models_dir(
    app: &tauri::AppHandle,
    configured_dir: Option<&str>,
) -> Result<PathBuf, String> {
    let explicit_dir = configured_dir
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(PathBuf::from);
    let target_dir = if let Some(dir) = explicit_dir {
        dir
    } else {
        default_models_dir(app)?
    };
    fs::create_dir_all(&target_dir).map_err(|e| {
        format!(
            "Could not create models directory {}: {e}",
            target_dir.display()
        )
    })?;
    Ok(target_dir)
}

fn download_progress_percent(downloaded_bytes: u64, total_bytes: Option<u64>) -> Option<u8> {
    let total = total_bytes?;
    if total == 0 {
        return None;
    }
    let pct = ((downloaded_bytes as f64 / total as f64) * 100.0).round() as i64;
    Some(pct.clamp(0, 100) as u8)
}

fn emit_whisper_download_progress(app: &tauri::AppHandle, progress: WhisperDownloadProgress) {
    if let Err(error) = app.emit("whisper-download-progress", progress) {
        log::debug!("Could not emit whisper-download-progress event: {error}");
    }
}

const WHISPER_MODELS: &[(&str, &str, &str)] = &[
    ("tiny", "ggml-tiny.bin", "~75 MB"),
    ("tiny.en", "ggml-tiny.en.bin", "~75 MB"),
    ("base", "ggml-base.bin", "~142 MB"),
    ("base.en", "ggml-base.en.bin", "~142 MB"),
    ("small", "ggml-small.bin", "~466 MB"),
    ("small.en", "ggml-small.en.bin", "~466 MB"),
    ("medium", "ggml-medium.bin", "~1.5 GB"),
    ("medium.en", "ggml-medium.en.bin", "~1.5 GB"),
    ("large-v1", "ggml-large-v1.bin", "~2.9 GB"),
    ("large-v2", "ggml-large-v2.bin", "~2.9 GB"),
    ("large-v3", "ggml-large-v3.bin", "~2.9 GB"),
    ("large-v3-turbo", "ggml-large-v3-turbo.bin", "~1.6 GB"),
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

    for provider in &mut config.providers {
        let normalized_type = normalize_provider_type(&provider.provider_type);
        if normalized_type != provider.provider_type {
            provider.provider_type = normalized_type;
            needs_save = true;
        }
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

    let normalized_anchor_behavior = normalize_anchor_behavior(&config.anchor_behavior);
    if normalized_anchor_behavior != config.anchor_behavior {
        config.anchor_behavior = normalized_anchor_behavior;
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
        normalize_provider_type(&provider.provider_type),
        normalize_provider_base_url(&provider.base_url).to_lowercase(),
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

fn normalize_provider_type(value: &str) -> String {
    let normalized = value.trim().to_lowercase();
    if normalized == "local" {
        "openai-compatible".to_string()
    } else if normalized == "openai-compatible" {
        "openai-compatible".to_string()
    } else if normalized == "openai" {
        "openai".to_string()
    } else {
        normalized
    }
}

fn provider_requires_api_key(provider_type: &str) -> bool {
    normalize_provider_type(provider_type) == "openai"
}

fn with_optional_bearer_auth(
    builder: reqwest::RequestBuilder,
    api_key: Option<&str>,
) -> reqwest::RequestBuilder {
    if let Some(token) = api_key.map(str::trim).filter(|value| !value.is_empty()) {
        builder.bearer_auth(token)
    } else {
        builder
    }
}

fn has_provider_api_key(provider: &ProviderConfig) -> bool {
    provider_api_key_from_config(provider).is_some()
}

fn provider_to_view(provider: &ProviderConfig) -> ProviderView {
    let api_key = provider_api_key_from_config(provider);
    ProviderView {
        id: provider.id.clone(),
        name: provider.name.clone(),
        provider_type: normalize_provider_type(&provider.provider_type),
        base_url: provider.base_url.clone(),
        translate_model: provider.translate_model.clone(),
        transcribe_model: provider.transcribe_model.clone(),
        is_active: provider.is_active,
        has_api_key: has_provider_api_key(provider),
        api_key,
    }
}

fn provider_api_key(provider: &ProviderConfig) -> Result<Option<String>, String> {
    let resolved = provider_api_key_from_config(provider);
    if provider_requires_api_key(&provider.provider_type) {
        resolved.map(Some).ok_or_else(|| {
            "Missing API key. Configure a valid API key in Settings > Providers.".to_string()
        })
    } else {
        Ok(resolved)
    }
}

fn provider_api_key_for_input(
    config: &AppConfig,
    provider: &ProviderInput,
    api_key: Option<String>,
) -> Result<Option<String>, String> {
    if let Some(secret) = api_key {
        let clean = secret.trim().to_string();
        if !clean.is_empty() {
            return Ok(Some(clean));
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
            return Ok(Some(secret));
        }
    }

    let normalized_name = provider.name.trim();
    let normalized_type = normalize_provider_type(&provider.provider_type);
    let normalized_base_url = normalize_provider_base_url(&provider.base_url);

    if let Some(existing) = config.providers.iter().find(|existing| {
        existing.name.trim().eq_ignore_ascii_case(normalized_name)
            && existing
                .provider_type
                .trim()
                .eq_ignore_ascii_case(normalized_type.as_str())
            && normalize_provider_base_url(&existing.base_url)
                .eq_ignore_ascii_case(&normalized_base_url)
    }) {
        if let Ok(secret) = provider_api_key(existing) {
            return Ok(secret);
        }
    }

    if provider_requires_api_key(&provider.provider_type) {
        Err("Missing API key. Type one in the API key field or save provider first.".to_string())
    } else {
        Ok(None)
    }
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

fn set_anchor_behavior_mode(app: &tauri::AppHandle, mode: &str) {
    if let Some(state) = app.try_state::<AnchorBehaviorMode>() {
        if let Ok(mut guard) = state.0.lock() {
            *guard = normalize_anchor_behavior(mode);
        }
    }
}

fn current_anchor_behavior_mode(app: &tauri::AppHandle) -> String {
    let Some(state) = app.try_state::<AnchorBehaviorMode>() else {
        return default_anchor_behavior();
    };
    let Ok(guard) = state.0.lock() else {
        return default_anchor_behavior();
    };
    if guard.trim().is_empty() {
        default_anchor_behavior()
    } else {
        normalize_anchor_behavior(guard.as_str())
    }
}

fn is_anchor_floating_mode(app: &tauri::AppHandle) -> bool {
    current_anchor_behavior_mode(app) == "floating"
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

    if !point_maps_to_any_monitor(app, target.x, target.y) {
        return Ok(RefocusAttempt {
            attempted: false,
            ok: false,
            target_age_ms: Some(target_age_ms),
        });
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

fn refresh_last_input_focus_target_from_snapshot(app: &tauri::AppHandle) {
    let Some(snapshot) = focused_text_anchor_snapshot(app) else {
        return;
    };

    let Some(bundle_id) = snapshot.bundle_id.as_deref() else {
        return;
    };

    save_last_external_app_bundle(app, bundle_id);

    if let Some((focus_x, focus_y)) = snapshot.input_focus_point {
        save_last_input_focus_target(
            app,
            InputFocusTarget {
                bundle_id: bundle_id.to_string(),
                x: focus_x.max(1),
                y: focus_y.max(1),
                captured_at_ms: now_millis(),
            },
        );
    }
}

fn normalize_hotkeys(hotkeys: &HotkeyConfig) -> HotkeyConfig {
    HotkeyConfig {
        open_app: hotkeys.open_app.trim().to_string(),
        open_dictate_translate: hotkeys.open_dictate_translate.trim().to_string(),
    }
}

fn validate_hotkeys(hotkeys: &HotkeyConfig) -> Result<Vec<(String, String)>, String> {
    let bindings = vec![
        ("open-app".to_string(), hotkeys.open_app.trim().to_string()),
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
    api_key: Option<&str>,
    model: &str,
    system_prompt: &str,
    user_prompt: &str,
) -> Result<String, String> {
    let base_url = normalize_provider_base_url(&provider.base_url);
    let provider_type = normalize_provider_type(&provider.provider_type);
    let model_name = non_empty_trimmed(model);
    if provider_type == "openai" {
        let model_name =
            model_name.ok_or_else(|| "Text model is required for cloud providers.".to_string())?;
        return run_openai_chat_completion(
            &base_url,
            api_key,
            Some(model_name),
            system_prompt,
            user_prompt,
        )
        .await;
    }

    // OpenAI-compatible providers may run locally or in the cloud:
    // try /chat/completions first when URL looks OpenAI-like, with /chat fallback.
    if local_prefers_openai_chat_endpoint(&base_url) {
        let openai_attempt =
            run_openai_chat_completion(&base_url, api_key, model_name, system_prompt, user_prompt)
                .await;
        if let Ok(content) = openai_attempt {
            return Ok(content);
        }
        let local_attempt =
            run_local_rest_chat(&base_url, api_key, model_name, system_prompt, user_prompt).await;
        match local_attempt {
            Ok(content) => Ok(content),
            Err(local_error) => Err(format!(
                "{}. /chat fallback also failed: {local_error}",
                openai_attempt
                    .err()
                    .unwrap_or_else(|| "OpenAI-style chat request failed".to_string())
            )),
        }
    } else {
        let local_attempt =
            run_local_rest_chat(&base_url, api_key, model_name, system_prompt, user_prompt).await;
        if let Ok(content) = local_attempt {
            return Ok(content);
        }
        let openai_attempt =
            run_openai_chat_completion(&base_url, api_key, model_name, system_prompt, user_prompt)
                .await;
        match openai_attempt {
            Ok(content) => Ok(content),
            Err(openai_error) => Err(format!(
                "{openai_error}. /chat attempt also failed: {}",
                local_attempt
                    .err()
                    .unwrap_or_else(|| "unknown /chat error".to_string())
            )),
        }
    }
}

async fn run_openai_chat_completion(
    base_url: &str,
    api_key: Option<&str>,
    model: Option<&str>,
    system_prompt: &str,
    user_prompt: &str,
) -> Result<String, String> {
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

    let endpoint = provider_endpoint(base_url, "chat/completions");
    let client = reqwest::Client::new();
    let response = with_optional_bearer_auth(client.post(endpoint), api_key)
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

    payload
        .choices
        .first()
        .and_then(|choice| extract_content(&choice.message.content))
        .ok_or_else(|| "Provider response did not include generated text.".to_string())
}

fn extract_local_rest_chat_content(payload: &serde_json::Value) -> Option<String> {
    if let Some(text) = payload
        .get("output_text")
        .and_then(serde_json::Value::as_str)
    {
        let clean = text.trim().to_string();
        if !clean.is_empty() {
            return Some(clean);
        }
    }

    let output = payload.get("output")?.as_array()?;
    let mut parts: Vec<String> = Vec::new();
    for item in output {
        let item_type = item
            .get("type")
            .and_then(serde_json::Value::as_str)
            .unwrap_or_default();
        if item_type != "message" {
            continue;
        }
        if let Some(content) = item.get("content").and_then(serde_json::Value::as_str) {
            let clean = content.trim();
            if !clean.is_empty() {
                parts.push(clean.to_string());
            }
        }
    }
    if parts.is_empty() {
        None
    } else {
        Some(parts.join("\n\n"))
    }
}

async fn run_local_rest_chat(
    base_url: &str,
    api_key: Option<&str>,
    model: Option<&str>,
    system_prompt: &str,
    input: &str,
) -> Result<String, String> {
    let endpoint = provider_endpoint(base_url, "chat");
    let mut body = serde_json::Map::new();
    if let Some(model_name) = model.and_then(non_empty_trimmed) {
        body.insert(
            "model".to_string(),
            serde_json::Value::String(model_name.to_string()),
        );
    }
    body.insert(
        "system_prompt".to_string(),
        serde_json::Value::String(system_prompt.to_string()),
    );
    body.insert(
        "input".to_string(),
        serde_json::Value::String(input.to_string()),
    );
    body.insert("temperature".to_string(), serde_json::Value::from(0.2_f64));

    let client = reqwest::Client::new();
    let response = with_optional_bearer_auth(client.post(endpoint), api_key)
        .json(&serde_json::Value::Object(body))
        .send()
        .await
        .map_err(|e| format!("/chat request failed: {e}"))?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response
            .text()
            .await
            .unwrap_or_else(|_| "<empty body>".to_string());
        return Err(format!(
            "/chat failed with HTTP {}: {}",
            status.as_u16(),
            body
        ));
    }

    let payload: serde_json::Value = response
        .json()
        .await
        .map_err(|e| format!("Could not parse /chat response: {e}"))?;
    extract_local_rest_chat_content(&payload)
        .ok_or_else(|| "/chat response did not include a text message.".to_string())
}

async fn test_local_provider_connection(
    base_url: &str,
    api_key: Option<&str>,
) -> Result<String, String> {
    let system_prompt = "You are a connection test assistant.";
    let ping_message = "ping";

    if local_prefers_openai_chat_endpoint(base_url) {
        let openai_probe =
            run_openai_chat_completion(base_url, api_key, None, system_prompt, ping_message).await;
        if openai_probe.is_ok() {
            return Ok("Connected successfully via /chat/completions.".to_string());
        }

        let local_probe =
            run_local_rest_chat(base_url, api_key, None, system_prompt, ping_message).await;
        return match local_probe {
            Ok(_) => Ok("Connected successfully via /chat (fallback).".to_string()),
            Err(local_error) => Err(format!(
                "{}. Fallback /chat failed: {local_error}",
                openai_probe
                    .err()
                    .unwrap_or_else(|| "/chat/completions probe failed".to_string())
            )),
        };
    }

    let local_probe =
        run_local_rest_chat(base_url, api_key, None, system_prompt, ping_message).await;
    if local_probe.is_ok() {
        return Ok("Connected successfully via /chat.".to_string());
    }

    let openai_probe =
        run_openai_chat_completion(base_url, api_key, None, system_prompt, ping_message).await;
    match openai_probe {
        Ok(_) => Ok("Connected successfully via /chat/completions (fallback).".to_string()),
        Err(openai_error) => Err(format!(
            "{}. Fallback /chat/completions failed: {openai_error}",
            local_probe
                .err()
                .unwrap_or_else(|| "/chat probe failed".to_string())
        )),
    }
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
    if let Some(anchor) = app.get_webview_window("anchor") {
        let _ = anchor.set_size(Size::Logical(LogicalSize::new(40.0, 40.0)));
        let _ = anchor.set_min_size(Some(Size::Logical(LogicalSize::new(40.0, 40.0))));
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
        .inner_size(QUICK_WINDOW_WIDTH_COMPACT, QUICK_WINDOW_HEIGHT_COMPACT)
        .min_inner_size(QUICK_WINDOW_WIDTH_COMPACT, QUICK_WINDOW_HEIGHT_COMPACT)
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

fn sanitize_scale_factor(scale: f64) -> f64 {
    if scale.is_finite() && scale > 0.0 {
        scale
    } else {
        1.0
    }
}

fn logical_to_physical(value: i32, scale_factor: f64) -> i32 {
    ((value as f64) * scale_factor).round() as i32
}

fn point_in_rect(
    x: i32,
    y: i32,
    rect_x: i32,
    rect_y: i32,
    rect_width: i32,
    rect_height: i32,
) -> bool {
    if rect_width <= 0 || rect_height <= 0 {
        return false;
    }
    let max_x = rect_x.saturating_add(rect_width);
    let max_y = rect_y.saturating_add(rect_height);
    x >= rect_x && y >= rect_y && x < max_x && y < max_y
}

fn scale_for_logical_point_in_rects(
    logical_x: i32,
    logical_y: i32,
    candidates: &[(i32, i32, i32, i32, f64)],
) -> Option<f64> {
    for (rect_x, rect_y, rect_width, rect_height, raw_scale) in candidates {
        let scale = sanitize_scale_factor(*raw_scale);
        let px = logical_to_physical(logical_x, scale);
        let py = logical_to_physical(logical_y, scale);
        if point_in_rect(px, py, *rect_x, *rect_y, *rect_width, *rect_height) {
            return Some(scale);
        }
    }

    None
}

fn monitor_from_cursor(window: &tauri::WebviewWindow) -> Option<tauri::Monitor> {
    let cursor = window.cursor_position().ok()?;
    window.monitor_from_point(cursor.x, cursor.y).ok().flatten()
}

fn resolve_monitor_for_position(
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
fn monitor_scale_factor_for_logical_point(
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

fn app_monitor_probe_window(app: &tauri::AppHandle) -> Option<tauri::WebviewWindow> {
    app.get_webview_window("anchor")
        .or_else(|| app.get_webview_window("quick"))
        .or_else(|| app.get_webview_window("main"))
        .or_else(|| app.get_webview_window("settings"))
}

fn point_maps_to_any_monitor(app: &tauri::AppHandle, x: i32, y: i32) -> bool {
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

fn clamp_quick_window_position(
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

fn clamp_anchor_window_position(
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

fn position_quick_window_near_anchor(app: &tauri::AppHandle) -> (&'static str, Option<u128>) {
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
fn focused_text_anchor_snapshot(app: &tauri::AppHandle) -> Option<FocusedAnchorSnapshot> {
    let script = r#"
set textRoles to {"AXTextField", "AXTextArea", "AXTextView"}
set blockedTerms to {"address", "url", "navigation", "omnibox", "search", "buscar", "password", "contraseña", "contrasena", "email", "correo"}
set browserBundles to {"com.apple.Safari", "com.google.Chrome", "com.brave.Browser", "com.microsoft.edgemac", "org.mozilla.firefox", "company.thebrowser.Browser"}
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
      set isEditable to false
      try
        set isEditable to value of attribute "AXEditable" of focusedElement
      end try
      if textRoles does not contain roleName and isEditable is not true then return "NONE"

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
            if metadataText contains (blocked as string) then return "NONE"
          end repeat
        end ignoring
      end if

      try
        set p to value of attribute "AXPosition" of focusedElement
        set s to value of attribute "AXSize" of focusedElement
      on error
        return "NONE"
      end try

      set px to item 1 of p as integer
      set py to item 2 of p as integer
      set pw to item 1 of s as integer
      set ph to item 2 of s as integer
      if pw < 2 or ph < 2 then return "NONE"

      ignoring case
        if domInputType is "password" then return "NONE"
        if roleName contains "secure" then return "NONE"
        if metadataText contains "password" then return "NONE"
      end ignoring

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
    let scale_factor = monitor_scale_factor_for_logical_point(app, x, y);
    let px = logical_to_physical(x, scale_factor);
    let py = logical_to_physical(y, scale_factor);
    let pw = logical_to_physical(w, scale_factor);
    let ph = logical_to_physical(h, scale_factor);
    let offset_x = logical_to_physical(10, scale_factor);
    let offset_y = logical_to_physical(44, scale_factor);

    let bundle_id = {
        let clean = bundle_raw.trim();
        if clean.is_empty() {
            None
        } else {
            Some(clean.to_string())
        }
    };

    let input_focus_point = if pw > 2 && ph > 2 {
        Some((px + (pw / 2), py + (ph / 2)))
    } else {
        None
    };

    Some(FocusedAnchorSnapshot {
        position: AnchorPosition {
            x: px + pw - offset_x,
            y: py - offset_y, // Above the input row, not among inline icons (emoji, mic, etc.)
        },
        bundle_id,
        input_focus_point,
    })
}

#[cfg(not(target_os = "macos"))]
fn focused_text_anchor_snapshot(_app: &tauri::AppHandle) -> Option<FocusedAnchorSnapshot> {
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
                thread::sleep(std::time::Duration::from_millis(180));
                continue;
            };
            let floating_mode = is_anchor_floating_mode(&app);

            if SETTINGS_WINDOW_OPEN.load(Ordering::SeqCst) {
                let _ = anchor.hide();
                last = None;
                if !floating_mode {
                    clear_last_anchor_position(&app);
                    clear_last_anchor_timestamp(&app);
                }
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

            if floating_mode {
                clear_last_input_focus_target(&app);

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

                thread::sleep(std::time::Duration::from_millis(180));
                continue;
            }

            let snapshot = focused_text_anchor_snapshot(&app);
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
    let tray_menu = tauri::menu::MenuBuilder::new(app)
        .text(TRAY_MENU_OPEN_APP, "Open WhisloAI")
        .text(TRAY_MENU_OPEN_SETTINGS, "Settings")
        .text(TRAY_MENU_CHECK_UPDATES, "Check for updates")
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
        anchor_behavior: normalize_anchor_behavior(&config.anchor_behavior),
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
        local_models_dir: transcription
            .local_models_dir
            .filter(|p| !p.trim().is_empty())
            .map(|p| p.trim().to_string()),
    };
    save_config(&app, &config)?;
    Ok(config.transcription)
}

#[tauri::command]
fn list_whisper_models(app: tauri::AppHandle) -> Vec<WhisperModelItem> {
    let models_dir = load_config(&app)
        .ok()
        .and_then(|config| config.transcription.local_models_dir)
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .map(PathBuf::from);

    WHISPER_MODELS
        .iter()
        .map(|(id, filename, size)| {
            let local_path = models_dir.as_ref().map(|dir| dir.join(*filename));
            let downloaded = local_path
                .as_ref()
                .map(|path| path.exists())
                .unwrap_or(false);
            WhisperModelItem {
                id: (*id).to_string(),
                filename: (*filename).to_string(),
                size: (*size).to_string(),
                downloaded,
                local_path: if downloaded {
                    local_path.map(|path| path.to_string_lossy().to_string())
                } else {
                    None
                },
            }
        })
        .collect()
}

#[tauri::command]
async fn download_whisper_model(app: tauri::AppHandle, model_id: String) -> Result<String, String> {
    let (_, filename, _) = WHISPER_MODELS
        .iter()
        .find(|(id, _, _)| *id == model_id)
        .ok_or_else(|| format!("Unknown model: {model_id}"))?;

    let config = load_config(&app)?;
    let configured_dir = config
        .transcription
        .local_models_dir
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| {
            "Select and save a models folder in Settings before downloading Whisper models."
                .to_string()
        })?;
    let models_dir = resolved_transcription_models_dir(&app, Some(configured_dir))?;
    let dest_path = models_dir.join(*filename);

    if dest_path.exists() {
        emit_whisper_download_progress(
            &app,
            WhisperDownloadProgress {
                model_id: model_id.clone(),
                downloaded_bytes: 0,
                total_bytes: None,
                percent: Some(100),
                done: true,
                destination: Some(dest_path.to_string_lossy().to_string()),
            },
        );
        return Ok(dest_path.to_string_lossy().to_string());
    }

    let url = format!("https://huggingface.co/ggerganov/whisper.cpp/resolve/main/{filename}");

    let client = reqwest::Client::new();
    let mut response = client
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

    let total_bytes = response.content_length();
    let mut downloaded_bytes: u64 = 0;
    let mut last_emitted_percent: Option<u8> = None;
    let temp_path = dest_path.with_extension("part");
    let mut output_file =
        fs::File::create(&temp_path).map_err(|e| format!("Could not save model: {e}"))?;

    emit_whisper_download_progress(
        &app,
        WhisperDownloadProgress {
            model_id: model_id.clone(),
            downloaded_bytes,
            total_bytes,
            percent: download_progress_percent(downloaded_bytes, total_bytes),
            done: false,
            destination: None,
        },
    );

    while let Some(chunk) = response
        .chunk()
        .await
        .map_err(|e| format!("Download failed: {e}"))?
    {
        output_file
            .write_all(&chunk)
            .map_err(|e| format!("Could not save model: {e}"))?;
        downloaded_bytes += chunk.len() as u64;
        let next_percent = download_progress_percent(downloaded_bytes, total_bytes);
        if total_bytes.is_none() || next_percent != last_emitted_percent {
            emit_whisper_download_progress(
                &app,
                WhisperDownloadProgress {
                    model_id: model_id.clone(),
                    downloaded_bytes,
                    total_bytes,
                    percent: next_percent,
                    done: false,
                    destination: None,
                },
            );
            last_emitted_percent = next_percent;
        }
    }

    output_file
        .flush()
        .map_err(|e| format!("Could not finalize model file: {e}"))?;
    fs::rename(&temp_path, &dest_path)
        .map_err(|e| format!("Could not move model file into place: {e}"))?;

    emit_whisper_download_progress(
        &app,
        WhisperDownloadProgress {
            model_id: model_id.clone(),
            downloaded_bytes,
            total_bytes,
            percent: Some(100),
            done: true,
            destination: Some(dest_path.to_string_lossy().to_string()),
        },
    );

    Ok(dest_path.to_string_lossy().to_string())
}

#[tauri::command]
async fn pick_whisper_models_dir(app: tauri::AppHandle) -> Result<Option<String>, String> {
    let initial_dir = load_config(&app)
        .ok()
        .and_then(|config| config.transcription.local_models_dir)
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
        .or_else(|| default_models_dir(&app).ok());

    let path = tauri::async_runtime::spawn_blocking(move || {
        let mut dialog = app
            .dialog()
            .file()
            .set_title("Select Whisper models folder");
        if let Some(start_dir) = initial_dir {
            dialog = dialog.set_directory(start_dir);
        }
        dialog.blocking_pick_folder()
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
fn open_external_url(url: String) -> Result<(), String> {
    let value = url.trim();
    if value.is_empty() {
        return Err("URL is empty.".to_string());
    }
    if !(value.starts_with("https://") || value.starts_with("http://")) {
        return Err("Only http/https URLs are allowed.".to_string());
    }

    #[cfg(target_os = "macos")]
    {
        Command::new("open")
            .arg(value)
            .spawn()
            .map_err(|e| format!("Could not open URL on macOS: {e}"))?;
        return Ok(());
    }

    #[cfg(target_os = "windows")]
    {
        Command::new("cmd")
            .args(["/C", "start", "", value])
            .spawn()
            .map_err(|e| format!("Could not open URL on Windows: {e}"))?;
        return Ok(());
    }

    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        Command::new("xdg-open")
            .arg(value)
            .spawn()
            .map_err(|e| format!("Could not open URL on this platform: {e}"))?;
        Ok(())
    }
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
fn start_anchor_window_drag(app: tauri::AppHandle) -> Result<(), String> {
    if !is_anchor_floating_mode(&app) {
        return Ok(());
    }
    let anchor = app
        .get_webview_window("anchor")
        .ok_or_else(|| "Anchor window not found.".to_string())?;
    anchor
        .start_dragging()
        .map_err(|e| format!("Could not start anchor drag: {e}"))
}

#[tauri::command]
fn remember_anchor_window_position(app: tauri::AppHandle) -> Result<(), String> {
    let Some(anchor) = app.get_webview_window("anchor") else {
        return Ok(());
    };
    if let Ok(position) = anchor.outer_position() {
        let (x, y) = clamp_anchor_window_position(&anchor, position.x, position.y);
        let _ = anchor.set_position(Position::Physical(PhysicalPosition::new(x, y)));
        save_last_anchor_position(&app, AnchorPosition { x, y });
        save_last_anchor_timestamp(&app, now_millis());
    }
    Ok(())
}

#[tauri::command]
fn set_quick_window_expanded(app: tauri::AppHandle, expanded: bool) -> Result<(), String> {
    if let Some(window) = app.get_webview_window("quick") {
        let target_width = if expanded {
            QUICK_WINDOW_WIDTH_EXPANDED
        } else {
            QUICK_WINDOW_WIDTH_COMPACT
        };
        let target_height = if expanded {
            QUICK_WINDOW_HEIGHT_EXPANDED
        } else {
            QUICK_WINDOW_HEIGHT_COMPACT
        };

        window
            .set_size(Size::Logical(LogicalSize::new(target_width, target_height)))
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
        "translate" => "open-dictate-translate",
        "dictate" => "open-dictate-translate-record",
        "app" => "open-app",
        _ => return Err("Unknown panel mode.".to_string()),
    };
    open_quick_window_with_action(&app, Some(action.to_string()))
}

#[tauri::command]
fn capture_selected_text(app: tauri::AppHandle) -> Result<String, String> {
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

    if next.translate_system_prompt.is_empty() {
        return Err("Translate system prompt cannot be empty.".to_string());
    }

    let source_normalized = next.source_language.trim().to_lowercase();
    let target_normalized = next.target_language.trim().to_lowercase();
    if source_normalized == target_normalized {
        return Err("Source and target languages must be different.".to_string());
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
    let normalized_anchor_behavior = normalize_anchor_behavior(&ui_settings.anchor_behavior);
    config.ui_language_preference = normalized.clone();
    config.anchor_behavior = normalized_anchor_behavior.clone();
    save_config(&app, &config)?;
    set_anchor_behavior_mode(&app, &normalized_anchor_behavior);
    if config.onboarding_completed {
        if let Err(error) = ensure_anchor_window(&app) {
            log::warn!("Could not ensure anchor window after saving UI settings: {error}");
        }
        start_anchor_monitor_once(app.clone());
    }

    let payload = UiSettings {
        ui_language_preference: normalized,
        anchor_behavior: normalized_anchor_behavior,
    };
    app.emit("ui-language-changed", &payload)
        .map_err(|e| format!("Could not emit ui-language-changed: {e}"))?;
    app.emit("ui-settings-changed", &payload)
        .map_err(|e| format!("Could not emit ui-settings-changed: {e}"))?;
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
    let normalized_type = normalize_provider_type(&provider.provider_type);
    let normalized_base_url = normalize_provider_base_url(&provider.base_url);
    let openai_provider = normalized_type == "openai";
    let normalized_translate_model = provider.translate_model.trim().to_string();

    if normalized_name.is_empty() || normalized_type.is_empty() || normalized_base_url.is_empty() {
        return Err("Complete provider name, type and base URL before saving.".to_string());
    }
    if openai_provider && normalized_translate_model.is_empty() {
        return Err("Complete the text model before saving cloud provider.".to_string());
    }
    let transcribe_model = if openai_provider {
        let default_transcribe = default_transcribe_model();
        provider
            .transcribe_model
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .unwrap_or(default_transcribe.as_str())
            .to_string()
    } else {
        provider
            .transcribe_model
            .as_deref()
            .map(str::trim)
            .unwrap_or_default()
            .to_string()
    };
    let incoming_api_key = api_key
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string);
    let requires_api_key = provider_requires_api_key(&normalized_type);

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
                        && normalize_provider_base_url(&existing.base_url)
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
            existing.translate_model = normalized_translate_model.clone();
            existing.transcribe_model = transcribe_model.clone();
            updated = true;
            break;
        }
    }

    if !updated {
        if requires_api_key && incoming_api_key.is_none() {
            return Err("API key is required for new providers. Add it before saving.".to_string());
        }

        config.providers.push(ProviderConfig {
            id: provider_id.clone(),
            name: normalized_name,
            provider_type: normalized_type,
            base_url: normalized_base_url,
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
        } else if provider_requires_api_key(&saved_provider.provider_type) {
            return Err("API key is required for this provider. Add it before saving.".to_string());
        } else {
            saved_provider.api_key_fallback_b64 = None;
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
    let provider_type = normalize_provider_type(&provider.provider_type);
    let base_url = normalize_provider_base_url(&provider.base_url);

    let api_key = provider_api_key(&provider)?;
    if provider_type == "openai-compatible" {
        return test_local_provider_connection(&base_url, api_key.as_deref()).await;
    }
    let endpoint = provider_endpoint(&base_url, "models");

    let client = reqwest::Client::new();
    let response = with_optional_bearer_auth(client.get(endpoint), api_key.as_deref())
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
    let provider_type = normalize_provider_type(&provider.provider_type);
    let base_url = normalize_provider_base_url(&provider.base_url);

    if name.is_empty() || provider_type.is_empty() || base_url.is_empty() {
        return Err("Complete provider name, type and base URL before testing.".to_string());
    }

    let config = load_config(&app)?;
    let resolved_api_key = provider_api_key_for_input(&config, &provider, api_key)?;
    if provider_type == "openai-compatible" {
        return test_local_provider_connection(&base_url, resolved_api_key.as_deref()).await;
    }

    let endpoint = provider_endpoint(&base_url, "models");
    let client = reqwest::Client::new();
    let response = with_optional_bearer_auth(client.get(endpoint), resolved_api_key.as_deref())
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
    let endpoint = provider_endpoint(&provider.base_url, "audio/transcriptions");

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

    let mut form = reqwest::multipart::Form::new().part("file", file_part);
    if let Some(model_name) = non_empty_trimmed(&provider.transcribe_model) {
        form = form.text("model", model_name.to_string());
    }

    // Use source language from settings to improve transcription accuracy and latency
    let source = config.prompt_settings.source_language.trim();
    if let Some(iso639) = language_to_iso639(source) {
        form = form.text("language", iso639.to_string());
    }

    let client = reqwest::Client::new();
    let response = with_optional_bearer_auth(client.post(endpoint), api_key.as_deref())
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
fn auto_insert_text(
    app: tauri::AppHandle,
    text: String,
    prefer_replace_selection: Option<bool>,
) -> Result<InsertTextResult, String> {
    let total_started = Instant::now();
    let value = text.trim();
    if value.is_empty() {
        return Err("Nothing to insert.".to_string());
    }
    let prefer_replace_selection = prefer_replace_selection.unwrap_or(false);

    let previous_clipboard = app.clipboard().read_text().ok();

    app.clipboard()
        .write_text(value.to_string())
        .map_err(|e| format!("Could not copy text to clipboard: {e}"))?;

    hide_main_window(&app);

    restore_last_external_app(&app);
    thread::sleep(std::time::Duration::from_millis(180));
    let mut refocus_error_message: Option<String> = None;
    let refocus_attempt = if prefer_replace_selection {
        RefocusAttempt {
            attempted: false,
            ok: false,
            target_age_ms: None,
        }
    } else {
        refresh_last_input_focus_target_from_snapshot(&app);
        match refocus_last_input_target(&app) {
            Ok(attempt) => attempt,
            Err(error) => {
                refocus_error_message = Some(error);
                RefocusAttempt {
                    attempted: true,
                    ok: false,
                    target_age_ms: None,
                }
            }
        }
    };
    let target_age_ms = refocus_attempt.target_age_ms;
    let refocus_attempted = refocus_attempt.attempted;
    let refocus_ok = refocus_attempt.ok;
    if prefer_replace_selection {
        thread::sleep(std::time::Duration::from_millis(45));
    } else if !refocus_ok {
        thread::sleep(std::time::Duration::from_millis(70));
    }

    let mut paste_error_message: Option<String> = None;
    let result = match simulate_paste_shortcut() {
        Ok(()) => InsertTextResult {
            copied: true,
            pasted: true,
            message: if prefer_replace_selection {
                "Text copied and pasted in the active app, replacing the current selection."
                    .to_string()
            } else if refocus_ok {
                "Text copied and pasted in the active app.".to_string()
            } else {
                "Text copied and pasted in the active app. Focus target restore was skipped."
                    .to_string()
            },
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

    let trace_error_message = paste_error_message
        .as_deref()
        .or(refocus_error_message.as_deref());
    let trace_outcome = if result.pasted {
        if prefer_replace_selection {
            "ok-replace-selection"
        } else if refocus_ok {
            "ok"
        } else {
            "ok-fallback-no-refocus"
        }
    } else if refocus_error_message.is_some() {
        "paste-error-after-refocus-error"
    } else if !refocus_ok {
        "paste-error-no-refocus"
    } else {
        "paste-error"
    };

    log_auto_insert_trace(
        trace_outcome,
        trace_error_message,
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
    let system_prompt = "You are a writing assistant. Rewrite the provided text in the same language as the input. Improve grammar, clarity, and flow while preserving meaning, names, technical terms, and intent. Return only the final rewritten text.";
    let user_prompt = format!(
        "Mode: {mode_name}\nMode instruction: {mode_instruction}\n\nText:\n{}",
        input.trim()
    );

    run_chat_completion(
        &provider,
        api_key.as_deref(),
        &provider.translate_model,
        system_prompt,
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
        api_key.as_deref(),
        &provider.translate_model,
        &system_prompt,
        &user_prompt,
    )
    .await
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .manage(PendingQuickAction::default())
        .manage(LastExternalAppBundle::default())
        .manage(LastAnchorPosition::default())
        .manage(LastAnchorTimestamp::default())
        .manage(LastInputFocusTarget::default())
        .manage(AnchorBehaviorMode::default())
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

            let config = load_config(app.handle())?;
            set_anchor_behavior_mode(app.handle(), &config.anchor_behavior);
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
            list_providers,
            get_hotkeys,
            get_prompt_settings,
            get_ui_settings,
            get_transcription_config,
            save_transcription_config,
            list_whisper_models,
            download_whisper_model,
            pick_whisper_models_dir,
            get_onboarding_status,
            complete_onboarding,
            open_permission_settings,
            probe_auto_insert_permission,
            open_external_url,
            open_settings_window,
            open_widget_window,
            open_quick_window,
            start_anchor_window_drag,
            remember_anchor_window_position,
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
            consume_pending_quick_action,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

#[cfg(test)]
mod tests {
    use super::{
        download_progress_percent, local_prefers_openai_chat_endpoint, logical_to_physical,
        non_empty_trimmed, normalize_anchor_behavior, normalize_provider_base_url, point_in_rect,
        provider_endpoint, sanitize_scale_factor, scale_for_logical_point_in_rects,
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
}
