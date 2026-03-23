use serde::{Deserialize, Serialize};

use super::config::{non_empty_trimmed, AudioTranscriptionResponse, ProviderConfig};
use super::providers::{
    local_prefers_openai_chat_endpoint, normalize_provider_base_url, normalize_provider_type,
    provider_endpoint, with_optional_bearer_auth,
};
use crate::platform;

#[derive(Debug, Serialize)]
pub(crate) struct ChatRequest<'a> {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) model: Option<&'a str>,
    pub(crate) messages: Vec<ChatMessage<'a>>,
    pub(crate) temperature: f32,
}

#[derive(Debug, Serialize)]
pub(crate) struct ChatMessage<'a> {
    pub(crate) role: &'a str,
    pub(crate) content: &'a str,
}

#[derive(Debug, Deserialize)]
pub(crate) struct ChatResponse {
    pub(crate) choices: Vec<ChatChoice>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct ChatChoice {
    pub(crate) message: ChatOutput,
}

#[derive(Debug, Deserialize)]
pub(crate) struct ChatOutput {
    pub(crate) content: serde_json::Value,
}

pub(crate) fn extract_content(content: &serde_json::Value) -> Option<String> {
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

pub(crate) async fn run_chat_completion(
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

pub(crate) async fn run_openai_chat_completion(
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

pub(crate) fn extract_local_rest_chat_content(payload: &serde_json::Value) -> Option<String> {
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

pub(crate) async fn run_local_rest_chat(
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

pub(crate) async fn test_local_provider_connection(
    base_url: &str,
    api_key: Option<&str>,
    model: Option<&str>,
) -> Result<String, String> {
    let system_prompt = "You are a connection test assistant.";
    let ping_message = "ping";

    if local_prefers_openai_chat_endpoint(base_url) {
        let openai_probe =
            run_openai_chat_completion(base_url, api_key, model, system_prompt, ping_message).await;
        if openai_probe.is_ok() {
            return Ok("Connected successfully via /chat/completions.".to_string());
        }

        let local_probe =
            run_local_rest_chat(base_url, api_key, model, system_prompt, ping_message).await;
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
        run_local_rest_chat(base_url, api_key, model, system_prompt, ping_message).await;
    if local_probe.is_ok() {
        return Ok("Connected successfully via /chat.".to_string());
    }

    let openai_probe =
        run_openai_chat_completion(base_url, api_key, model, system_prompt, ping_message).await;
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

pub(crate) fn parse_transcription_text(raw_body: &str) -> Option<String> {
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
pub(crate) fn language_to_iso639(language: &str) -> Option<&'static str> {
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

pub(crate) fn audio_file_name(mime_type: Option<&str>) -> String {
    let normalized = mime_type
        .unwrap_or_default()
        .split(';')
        .next()
        .unwrap_or_default()
        .trim()
        .to_ascii_lowercase();

    let extension = match normalized.as_str() {
        "audio/webm" => "webm",
        "audio/ogg" => "ogg",
        "audio/mp4" | "audio/m4a" | "audio/x-m4a" => "m4a",
        "audio/wav" | "audio/x-wav" | "audio/wave" => "wav",
        _ => "bin",
    };

    format!("recording.{extension}")
}

pub(crate) fn simulate_modifier_shortcut(character: char) -> Result<(), String> {
    platform::backend().simulate_modifier_shortcut(character)
}

pub(crate) fn simulate_paste_shortcut() -> Result<(), String> {
    simulate_modifier_shortcut('v')
}

pub(crate) fn simulate_copy_shortcut() -> Result<(), String> {
    simulate_modifier_shortcut('c')
}

pub(crate) fn ensure_accessibility_permission() -> Result<(), String> {
    platform::backend().ensure_accessibility_permission()
}

pub(crate) fn ensure_system_events_permission() -> Result<(), String> {
    platform::backend().ensure_system_events_permission()
}

pub(crate) fn probe_input_automation_permission() -> Result<(), String> {
    ensure_accessibility_permission()?;
    if platform::capabilities().needs_automation {
        ensure_system_events_permission()?;
    }
    Ok(())
}

pub(crate) fn transcribe_with_local_whisper(
    model_path: &str,
    audio_bytes: &[u8],
    _mime_type: Option<&str>,
    source_language: &str,
) -> Result<String, String> {
    use symphonia::core::audio::Signal;
    use symphonia::core::io::MediaSourceStream;
    use symphonia::core::probe::Hint;
    use whisper_rs::{FullParams, SamplingStrategy, WhisperContext, WhisperContextParameters};

    log::info!(
        "local_whisper:start model_path='{}' model_exists={} audio_bytes={} source_language={}",
        model_path,
        std::path::Path::new(model_path).exists(),
        audio_bytes.len(),
        source_language
    );

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
    let decoded_samples_len = samples.len();
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
    log::info!(
        "local_whisper:decoded sample_rate={} decoded_samples={} resampled_samples={}",
        sample_rate,
        decoded_samples_len,
        resampled.len()
    );
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
    log::info!(
        "local_whisper:done segments={} transcript_chars={}",
        state.as_iter().count(),
        text.chars().count()
    );
    normalize_local_transcription_output(text)
}

pub(crate) fn local_transcription_block_reason(
    _is_macos: bool,
    _is_rosetta_translated: bool,
) -> Option<&'static str> {
    None
}

pub(crate) fn transcribe_error(message: impl Into<String>) -> Result<String, String> {
    let message = message.into();
    log::warn!("transcribe_audio failed: {message}");
    Err(message)
}

pub(crate) fn normalize_local_transcription_output(text: String) -> Result<String, String> {
    let value = text.trim().to_string();
    if value.is_empty() {
        return Err(
            "Local transcription was empty. Try recording again, speak closer to the microphone, or select a larger Whisper model."
                .to_string(),
        );
    }
    Ok(value)
}
