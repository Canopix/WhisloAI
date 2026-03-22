use base64::Engine as _;
use crate::*;
use std::thread;
use std::time::Instant;
use tauri_plugin_clipboard_manager::ClipboardExt;

#[tauri::command]
pub(crate) async fn transcribe_audio(
    app: tauri::AppHandle,
    audio_base64: String,
    mime_type: Option<String>,
) -> Result<String, String> {
    let base64_payload = audio_base64.trim();
    if base64_payload.is_empty() {
        return transcribe_error("Audio payload is empty.");
    }

    let audio_bytes = base64::engine::general_purpose::STANDARD
        .decode(base64_payload)
        .map_err(|e| {
            let message = format!("Could not decode audio payload: {e}");
            log::warn!("transcribe_audio failed: {message}");
            message
        })?;

    if audio_bytes.is_empty() {
        return transcribe_error("Audio payload is empty.");
    }

    let config = load_config(&app).map_err(|error| {
        log::warn!("transcribe_audio failed: {error}");
        error
    })?;

    if config.transcription.mode == "local" {
        if let Some(reason) =
            local_transcription_block_reason(cfg!(target_os = "macos"), is_running_under_rosetta())
        {
            return transcribe_error(reason.to_string());
        }

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
                    )
                    .map_err(|error| {
                        log::warn!("transcribe_audio failed: {error}");
                        error
                    });
                }
                #[cfg(not(feature = "local-transcription"))]
                {
                    return transcribe_error("Local transcription requires building with the 'local-transcription' feature (cmake needed). Use API mode or rebuild with: cargo build --features local-transcription".to_string());
                }
            }
        }
        return transcribe_error(
            "Local model path not set or file not found. Configure in Settings.".to_string(),
        );
    }

    let provider = active_provider(&config).map_err(|error| {
        log::warn!("transcribe_audio failed: {error}");
        error
    })?;
    let api_key = provider_api_key(&provider).map_err(|error| {
        log::warn!("transcribe_audio failed: {error}");
        error
    })?;
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
        .map_err(|e| {
            let message = format!("Invalid audio mime type: {e}");
            log::warn!("transcribe_audio failed: {message}");
            message
        })?;

    let mut form = reqwest::multipart::Form::new().part("file", file_part);
    if let Some(model_name) = non_empty_trimmed(&provider.transcribe_model) {
        form = form.text("model", model_name.to_string());
    }

    let source = config.prompt_settings.source_language.trim();
    if let Some(iso639) = language_to_iso639(source) {
        form = form.text("language", iso639.to_string());
    }

    let client = reqwest::Client::new();
    let response = with_optional_bearer_auth(client.post(endpoint), api_key.as_deref())
        .multipart(form)
        .send()
        .await
        .map_err(|e| {
            let message = format!("Transcription request failed: {e}");
            log::warn!("transcribe_audio failed: {message}");
            message
        })?;

    let status = response.status();
    let body = response
        .text()
        .await
        .unwrap_or_else(|_| "<empty body>".to_string());

    if !status.is_success() {
        return transcribe_error(format!(
            "Transcription failed with HTTP {}: {}",
            status.as_u16(),
            body
        ));
    }

    parse_transcription_text(&body).ok_or_else(|| {
        let message = "Transcription response was empty. Try recording again.".to_string();
        log::warn!("transcribe_audio failed: {message}");
        message
    })
}

#[tauri::command]
pub(crate) fn auto_insert_text(
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

    let restore_attempt = restore_last_external_app(&app);
    log_external_restore_trace("auto_insert_text", restore_attempt);
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
        restore_attempt.reason,
        total_started.elapsed().as_millis(),
    );

    Ok(result)
}

#[tauri::command]
pub(crate) async fn improve_text(
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
pub(crate) async fn translate_text(
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
