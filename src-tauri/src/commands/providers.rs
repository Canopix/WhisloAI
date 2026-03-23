use crate::*;

#[tauri::command]
pub(crate) fn list_providers(app: tauri::AppHandle) -> Result<Vec<ProviderView>, String> {
    let config = load_config(&app)?;
    Ok(config.providers.iter().map(provider_to_view).collect())
}

#[tauri::command]
pub(crate) fn save_provider(
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
pub(crate) fn set_active_provider(
    app: tauri::AppHandle,
    provider_id: String,
) -> Result<(), String> {
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
pub(crate) fn delete_provider(app: tauri::AppHandle, provider_id: String) -> Result<(), String> {
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
pub(crate) async fn test_provider_connection(
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
        let model = non_empty_trimmed(&provider.translate_model);
        return test_local_provider_connection(&base_url, api_key.as_deref(), model).await;
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
pub(crate) async fn test_provider_connection_input(
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
        let model = non_empty_trimmed(&provider.translate_model);
        return test_local_provider_connection(&base_url, resolved_api_key.as_deref(), model).await;
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
