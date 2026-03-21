use crate::*;
use std::collections::HashMap;
use std::fs;
use std::io::Write;
use std::path::PathBuf;
use tauri::Emitter;
use tauri_plugin_dialog::DialogExt;

#[tauri::command]
pub(crate) fn get_hotkeys(app: tauri::AppHandle) -> Result<HotkeyConfig, String> {
    let config = load_config(&app)?;
    Ok(config.hotkeys)
}

#[tauri::command]
pub(crate) fn get_app_version(app: tauri::AppHandle) -> Result<String, String> {
    Ok(app.package_info().version.to_string())
}

#[tauri::command]
pub(crate) fn get_prompt_settings(app: tauri::AppHandle) -> Result<PromptSettings, String> {
    let config = load_config(&app)?;
    Ok(config.prompt_settings)
}

#[tauri::command]
pub(crate) fn get_ui_settings(app: tauri::AppHandle) -> Result<UiSettings, String> {
    let config = load_config(&app)?;
    Ok(UiSettings {
        ui_language_preference: normalize_ui_language_preference(&config.ui_language_preference),
        anchor_behavior: normalize_anchor_behavior(&config.anchor_behavior),
    })
}

#[tauri::command]
pub(crate) fn get_transcription_config(app: tauri::AppHandle) -> Result<TranscriptionConfig, String> {
    let config = load_config(&app)?;
    Ok(config.transcription)
}

#[tauri::command]
pub(crate) fn save_transcription_config(
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
pub(crate) fn list_whisper_models(app: tauri::AppHandle) -> Vec<WhisperModelItem> {
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
pub(crate) async fn download_whisper_model(
    app: tauri::AppHandle,
    model_id: String,
) -> Result<String, String> {
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
pub(crate) async fn pick_whisper_models_dir(
    app: tauri::AppHandle,
) -> Result<Option<String>, String> {
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
pub(crate) fn save_hotkeys(
    app: tauri::AppHandle,
    hotkeys: HotkeyConfig,
) -> Result<HotkeyConfig, String> {
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
pub(crate) fn save_prompt_settings(
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
pub(crate) fn save_ui_settings(
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
