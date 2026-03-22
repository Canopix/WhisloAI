use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use tauri::{Emitter, Manager};

pub(crate) const KEYRING_SERVICE: &str = "whisloai";
pub(crate) const SUPPORTED_STYLE_MODES: [&str; 5] =
    ["simple", "professional", "friendly", "casual", "formal"];
pub(crate) const WHISPER_MODELS: &[(&str, &str, &str)] = &[
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

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub(crate) struct HotkeyConfig {
    pub(crate) open_app: String,
    pub(crate) open_dictate_translate: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ProviderConfig {
    pub(crate) id: String,
    pub(crate) name: String,
    pub(crate) provider_type: String,
    pub(crate) base_url: String,
    pub(crate) translate_model: String,
    #[serde(default = "default_transcribe_model")]
    pub(crate) transcribe_model: String,
    #[serde(default)]
    pub(crate) api_key_fallback_b64: Option<String>,
    pub(crate) is_active: bool,
}

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ProviderView {
    pub(crate) id: String,
    pub(crate) name: String,
    pub(crate) provider_type: String,
    pub(crate) base_url: String,
    pub(crate) translate_model: String,
    pub(crate) transcribe_model: String,
    pub(crate) is_active: bool,
    pub(crate) has_api_key: bool,
    pub(crate) api_key: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ProviderInput {
    pub(crate) id: Option<String>,
    pub(crate) name: String,
    pub(crate) provider_type: String,
    pub(crate) base_url: String,
    pub(crate) translate_model: String,
    #[serde(default)]
    pub(crate) transcribe_model: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PromptSettings {
    #[serde(default = "default_translate_system_prompt")]
    pub(crate) translate_system_prompt: String,
    #[serde(default = "default_source_language")]
    pub(crate) source_language: String,
    #[serde(default = "default_target_language")]
    pub(crate) target_language: String,
    #[serde(default = "default_mode_instructions")]
    pub(crate) mode_instructions: HashMap<String, String>,
    #[serde(default = "default_quick_mode")]
    pub(crate) quick_mode: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PromptSettingsInput {
    pub(crate) translate_system_prompt: String,
    #[serde(default)]
    pub(crate) source_language: String,
    #[serde(default)]
    pub(crate) target_language: String,
    pub(crate) mode_instructions: HashMap<String, String>,
    pub(crate) quick_mode: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub(crate) struct UiSettings {
    #[serde(default = "default_ui_language_preference")]
    pub(crate) ui_language_preference: String,
    #[serde(default = "default_anchor_behavior")]
    pub(crate) anchor_behavior: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct UiSettingsInput {
    #[serde(default = "default_ui_language_preference")]
    pub(crate) ui_language_preference: String,
    #[serde(default = "default_anchor_behavior")]
    pub(crate) anchor_behavior: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub(crate) struct TranscriptionConfig {
    #[serde(default)]
    pub(crate) mode: String,
    #[serde(default)]
    pub(crate) local_model_path: Option<String>,
    #[serde(default)]
    pub(crate) local_models_dir: Option<String>,
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
pub(crate) struct AppConfig {
    #[serde(default = "default_hotkeys")]
    pub(crate) hotkeys: HotkeyConfig,
    #[serde(default)]
    pub(crate) onboarding_completed: bool,
    #[serde(default)]
    pub(crate) providers: Vec<ProviderConfig>,
    #[serde(default = "default_prompt_settings")]
    pub(crate) prompt_settings: PromptSettings,
    #[serde(default)]
    pub(crate) transcription: TranscriptionConfig,
    #[serde(default = "default_ui_language_preference")]
    pub(crate) ui_language_preference: String,
    #[serde(default = "default_anchor_behavior")]
    pub(crate) anchor_behavior: String,
}

#[derive(Debug, Deserialize)]
pub(crate) struct OpenAiModelsResponse {
    pub(crate) data: Vec<serde_json::Value>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct AudioTranscriptionResponse {
    pub(crate) text: Option<String>,
}

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub(crate) struct HotkeyTriggerEvent {
    pub(crate) action: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct InsertTextResult {
    pub(crate) copied: bool,
    pub(crate) pasted: bool,
    pub(crate) message: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct OnboardingStatus {
    pub(crate) completed: bool,
    pub(crate) platform: String,
    pub(crate) needs_accessibility: bool,
    pub(crate) needs_automation: bool,
    pub(crate) supports_contextual_anchor: bool,
}

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub(crate) struct WhisperDownloadProgress {
    pub(crate) model_id: String,
    pub(crate) downloaded_bytes: u64,
    pub(crate) total_bytes: Option<u64>,
    pub(crate) percent: Option<u8>,
    pub(crate) done: bool,
    pub(crate) destination: Option<String>,
}

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub(crate) struct WhisperModelItem {
    pub(crate) id: String,
    pub(crate) filename: String,
    pub(crate) size: String,
    pub(crate) downloaded: bool,
    pub(crate) local_path: Option<String>,
}

#[derive(Debug, Clone)]
pub(crate) struct InputFocusTarget {
    pub(crate) bundle_id: String,
    pub(crate) x: i32,
    pub(crate) y: i32,
    pub(crate) captured_at_ms: u128,
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

pub(crate) fn default_hotkeys() -> HotkeyConfig {
    HotkeyConfig {
        open_app: "CommandOrControl+Shift+Space".to_string(),
        open_dictate_translate: "CommandOrControl+Shift+D".to_string(),
    }
}

pub(crate) fn default_provider() -> ProviderConfig {
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

pub(crate) fn default_transcribe_model() -> String {
    "gpt-4o-mini-transcribe".to_string()
}

pub(crate) fn default_translate_system_prompt() -> String {
    "You are a translation assistant. Convert text from {source} into clear, concise, natural {target} for workplace chat. Preserve names and technical terms. Return only final text.".to_string()
}

pub(crate) fn default_source_language() -> String {
    "Spanish".to_string()
}

pub(crate) fn default_target_language() -> String {
    "English".to_string()
}

pub(crate) fn default_quick_mode() -> String {
    "simple".to_string()
}

pub(crate) fn default_ui_language_preference() -> String {
    "system".to_string()
}

pub(crate) fn default_anchor_behavior() -> String {
    if cfg!(target_os = "macos") {
        "contextual".to_string()
    } else {
        "floating".to_string()
    }
}

pub(crate) fn default_mode_instruction_for(mode: &str) -> Option<&'static str> {
    match mode {
        "simple" => Some("Use clear, concise wording with everyday vocabulary."),
        "professional" => Some("Use a polished workplace tone with direct and confident wording."),
        "friendly" => Some("Use a warm, approachable tone while staying concise."),
        "casual" => Some("Use a relaxed conversational tone with natural phrasing."),
        "formal" => Some("Use a formal, respectful tone with complete sentences."),
        _ => None,
    }
}

pub(crate) fn default_mode_instructions() -> HashMap<String, String> {
    let mut map = HashMap::new();
    for mode in SUPPORTED_STYLE_MODES {
        if let Some(value) = default_mode_instruction_for(mode) {
            map.insert(mode.to_string(), value.to_string());
        }
    }
    map
}

pub(crate) fn default_prompt_settings() -> PromptSettings {
    PromptSettings {
        translate_system_prompt: default_translate_system_prompt(),
        source_language: default_source_language(),
        target_language: default_target_language(),
        mode_instructions: default_mode_instructions(),
        quick_mode: default_quick_mode(),
    }
}

pub(crate) fn normalize_mode_name(mode: &str) -> String {
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

pub(crate) fn normalize_ui_language_preference(value: &str) -> String {
    match value.trim().to_lowercase().as_str() {
        "system" | "en" | "es" => value.trim().to_lowercase(),
        _ => default_ui_language_preference(),
    }
}

pub(crate) fn normalize_anchor_behavior(value: &str) -> String {
    match value.trim().to_lowercase().as_str() {
        "floating" => "floating".to_string(),
        _ => default_anchor_behavior(),
    }
}

pub(crate) fn normalize_prompt_settings(settings: &mut PromptSettings) -> bool {
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

pub(crate) fn mode_instruction_for(settings: &PromptSettings, style: &str) -> (String, String) {
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

pub(crate) fn non_empty_trimmed(value: &str) -> Option<&str> {
    let clean = value.trim();
    if clean.is_empty() {
        None
    } else {
        Some(clean)
    }
}

pub(crate) fn config_path(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    let config_dir = app
        .path()
        .app_config_dir()
        .map_err(|e| format!("Could not resolve app config directory: {e}"))?;

    fs::create_dir_all(&config_dir)
        .map_err(|e| format!("Could not create app config directory: {e}"))?;

    Ok(config_dir.join("providers.json"))
}

pub(crate) fn default_models_dir(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    let dir = app
        .path()
        .app_config_dir()
        .map_err(|e| format!("Could not resolve app config directory: {e}"))?
        .join("whisper-models");
    fs::create_dir_all(&dir).map_err(|e| format!("Could not create models directory: {e}"))?;
    Ok(dir)
}

pub(crate) fn resolved_transcription_models_dir(
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

pub(crate) fn download_progress_percent(
    downloaded_bytes: u64,
    total_bytes: Option<u64>,
) -> Option<u8> {
    let total = total_bytes?;
    if total == 0 {
        return None;
    }
    let pct = ((downloaded_bytes as f64 / total as f64) * 100.0).round() as i64;
    Some(pct.clamp(0, 100) as u8)
}

pub(crate) fn emit_whisper_download_progress(
    app: &tauri::AppHandle,
    progress: WhisperDownloadProgress,
) {
    if let Err(error) = app.emit("whisper-download-progress", progress) {
        log::debug!("Could not emit whisper-download-progress event: {error}");
    }
}

pub(crate) fn load_config(app: &tauri::AppHandle) -> Result<AppConfig, String> {
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
    if crate::domain::providers::dedupe_providers(&mut config) {
        needs_save = true;
    }

    for provider in &mut config.providers {
        let normalized_type =
            crate::domain::providers::normalize_provider_type(&provider.provider_type);
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

pub(crate) fn save_config(app: &tauri::AppHandle, config: &AppConfig) -> Result<(), String> {
    let path = config_path(app)?;
    let payload = serde_json::to_string_pretty(config)
        .map_err(|e| format!("Could not serialize settings: {e}"))?;
    fs::write(&path, payload)
        .map_err(|e| format!("Could not write settings file {}: {e}", path.display()))
}
