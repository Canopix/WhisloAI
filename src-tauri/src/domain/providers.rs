use base64::Engine as _;
use keyring::Entry;
use reqwest::RequestBuilder;
use std::collections::HashMap;

use super::config::*;

pub(crate) fn normalize_base_url(base_url: &str) -> String {
    base_url.trim().trim_end_matches('/').to_string()
}

pub(crate) fn normalize_provider_base_url(base_url: &str) -> String {
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

pub(crate) fn provider_endpoint(base_url: &str, path: &str) -> String {
    let root = normalize_provider_base_url(base_url);
    let suffix = path.trim().trim_start_matches('/');
    if suffix.is_empty() {
        root
    } else {
        format!("{root}/{suffix}")
    }
}

pub(crate) fn local_prefers_openai_chat_endpoint(base_url: &str) -> bool {
    normalize_provider_base_url(base_url)
        .to_lowercase()
        .ends_with("/v1")
}

pub(crate) fn keyring_entry(provider_id: &str) -> Result<Entry, String> {
    Entry::new(KEYRING_SERVICE, provider_id)
        .map_err(|e| format!("Could not create keyring entry for provider {provider_id}: {e}"))
}

pub(crate) fn read_keyring_secret(provider_id: &str) -> Option<String> {
    let entry = keyring_entry(provider_id).ok()?;
    let secret = entry.get_password().ok()?;
    let clean = secret.trim().to_string();
    if clean.is_empty() {
        None
    } else {
        Some(clean)
    }
}

pub(crate) fn encode_api_key_fallback(secret: &str) -> Option<String> {
    let clean = secret.trim();
    if clean.is_empty() {
        return None;
    }
    Some(base64::engine::general_purpose::STANDARD.encode(clean.as_bytes()))
}

pub(crate) fn decode_api_key_fallback(encoded: Option<&String>) -> Option<String> {
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

pub(crate) fn provider_api_key_from_config(provider: &ProviderConfig) -> Option<String> {
    decode_api_key_fallback(provider.api_key_fallback_b64.as_ref())
        .or_else(|| read_keyring_secret(&provider.id))
}

pub(crate) fn provider_dedupe_signature(provider: &ProviderConfig) -> String {
    format!(
        "{}|{}|{}",
        provider.name.trim().to_lowercase(),
        normalize_provider_type(&provider.provider_type),
        normalize_provider_base_url(&provider.base_url).to_lowercase(),
    )
}

pub(crate) fn dedupe_providers(config: &mut AppConfig) -> bool {
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

pub(crate) fn normalize_provider_type(value: &str) -> String {
    match value.trim().to_lowercase().as_str() {
        "openai" | "openai-compatible" | "local" => "openai-compatible".to_string(),
        _ => "openai-compatible".to_string(),
    }
}

pub(crate) fn provider_requires_api_key(provider_type: &str) -> bool {
    normalize_provider_type(provider_type) == "openai-compatible"
}

pub(crate) fn with_optional_bearer_auth(
    builder: RequestBuilder,
    api_key: Option<&str>,
) -> RequestBuilder {
    match api_key.map(str::trim).filter(|value| !value.is_empty()) {
        Some(key) => builder.bearer_auth(key),
        None => builder,
    }
}

pub(crate) fn provider_to_view(provider: &ProviderConfig) -> ProviderView {
    let api_key = provider_api_key_from_config(provider);
    ProviderView {
        id: provider.id.clone(),
        name: provider.name.clone(),
        provider_type: provider.provider_type.clone(),
        base_url: provider.base_url.clone(),
        translate_model: provider.translate_model.clone(),
        transcribe_model: provider.transcribe_model.clone(),
        is_active: provider.is_active,
        has_api_key: api_key.is_some(),
        api_key,
    }
}

pub(crate) fn provider_api_key(provider: &ProviderConfig) -> Result<Option<String>, String> {
    let resolved = provider_api_key_from_config(provider);
    if provider_requires_api_key(&provider.provider_type) {
        resolved.map(Some).ok_or_else(|| {
            "Missing API key. Configure a valid API key in Settings > Providers.".to_string()
        })
    } else {
        Ok(resolved)
    }
}

pub(crate) fn provider_api_key_for_input(
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

pub(crate) fn active_provider(config: &AppConfig) -> Result<ProviderConfig, String> {
    config
        .providers
        .iter()
        .find(|p| p.is_active)
        .cloned()
        .ok_or_else(|| "No active provider configured.".to_string())
}
