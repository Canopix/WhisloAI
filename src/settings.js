const invoke = (() => {
  try {
    return window.__TAURI__?.core?.invoke;
  } catch (_) {
    return null;
  }
})();

const listen = (() => {
  try {
    return window.__TAURI__?.event?.listen || null;
  } catch (_) {
    return null;
  }
})();

const i18n = window.WhisloAII18n || null;
const t = (key, params) => (i18n ? i18n.t(key, params) : key);
const applyTranslations = (root) => {
  if (i18n && typeof i18n.applyTranslations === "function") {
    i18n.applyTranslations(root);
  }
};
const setLanguagePreference = (preference) => {
  if (i18n && typeof i18n.setLanguagePreference === "function") {
    i18n.setLanguagePreference(preference, navigator.language);
  }
};

const statusEl = document.getElementById("status");
const providersSummaryEl = document.getElementById("providers-summary");
const uiLanguagePreferenceEl = document.getElementById("ui-language-preference");

const providersList = document.getElementById("providers-list");
const providerForm = document.getElementById("provider-form");
const providerId = document.getElementById("provider-id");
const providerName = document.getElementById("provider-name");
const providerType = document.getElementById("provider-type");
const providerBaseUrl = document.getElementById("provider-base-url");
const providerTranslateModel = document.getElementById("provider-translate-model");
const providerTranscribeModel = document.getElementById("provider-transcribe-model");
const providerApiKey = document.getElementById("provider-api-key");
const testProviderBtn = document.getElementById("test-provider-btn");
const newProviderBtn = document.getElementById("new-provider-btn");
const activateProviderBtn = document.getElementById("activate-provider-btn");
const deleteProviderBtn = document.getElementById("delete-provider-btn");

const promptForm = document.getElementById("prompt-form");
const promptTranslateSystem = document.getElementById("prompt-translate-system");
const sourceLanguage = document.getElementById("source-language");
const targetLanguage = document.getElementById("target-language");
const modeSimple = document.getElementById("mode-simple");
const modeProfessional = document.getElementById("mode-professional");
const modeFriendly = document.getElementById("mode-friendly");
const modeCasual = document.getElementById("mode-casual");
const modeFormal = document.getElementById("mode-formal");
const quickDefaultMode = document.getElementById("quick-default-mode");
const resetPromptBtn = document.getElementById("reset-prompt-btn");

const transcriptionMode = document.getElementById("transcription-mode");
const transcriptionLocalSection = document.getElementById("transcription-local-section");
const localModelPath = document.getElementById("local-model-path");
const pickModelBtn = document.getElementById("pick-model-btn");
const saveTranscriptionBtn = document.getElementById("save-transcription-btn");
const whisperModelsContainer = document.getElementById("whisper-models-container");

const DEFAULT_PROMPT_SETTINGS = {
  translateSystemPrompt:
    "You are a translation assistant. Convert text from {source} into clear, concise, natural {target} for workplace chat. Preserve names and technical terms. Return only final text.",
  sourceLanguage: "Spanish",
  targetLanguage: "English",
  modeInstructions: {
    simple: "Use clear, concise wording with everyday vocabulary.",
    professional: "Use a polished workplace tone with direct and confident wording.",
    friendly: "Use a warm, approachable tone while staying concise.",
    casual: "Use a relaxed conversational tone with natural phrasing.",
    formal: "Use a formal, respectful tone with complete sentences.",
  },
  quickMode: "simple",
};

let cachedProviders = [];
let selectedProviderId = null;
let providersLoaded = false;
let currentUiLanguagePreference = "system";

function normalizeError(error) {
  const text = String(error || "Unknown error.");
  if (text.startsWith("Error: ")) {
    return text.slice(7);
  }
  return text;
}

function escapeHtml(value) {
  return String(value || "")
    .replaceAll("&", "&amp;")
    .replaceAll("<", "&lt;")
    .replaceAll(">", "&gt;")
    .replaceAll('"', "&quot;")
    .replaceAll("'", "&#039;");
}

function providerHost(baseUrl) {
  try {
    return new URL(baseUrl).host;
  } catch (_) {
    return baseUrl || "unknown host";
  }
}

function setStatus(message, tone = "neutral") {
  statusEl.textContent = message;
  statusEl.dataset.tone = tone;
}

function setStatusKey(key, tone = "neutral", params) {
  setStatus(t(key, params), tone);
}

function hasView(view) {
  return document.querySelector(`.settings-view[data-view="${view}"]`) != null;
}

function currentHashView() {
  const view = window.location.hash.replace(/^#/, "").trim();
  return hasView(view) ? view : "providers";
}

function activateView(view, syncHash = true) {
  const views = Array.from(document.querySelectorAll(".settings-view"));
  const nextView = views.some((s) => s.dataset.view === view) ? view : "providers";
  document.querySelectorAll(".settings-nav-btn").forEach((btn) => {
    const isActive = btn.dataset.view === nextView;
    btn.classList.toggle("is-active", isActive);
    btn.setAttribute("aria-selected", isActive ? "true" : "false");
  });
  views.forEach((section) => {
    const isActive = section.dataset.view === nextView;
    section.classList.toggle("is-active", isActive);
  });

  if (syncHash) {
    const nextHash = `#${nextView}`;
    if (window.location.hash !== nextHash) {
      try {
        window.history.replaceState(null, "", nextHash);
      } catch (_) {}
    }
  }

  if (nextView !== "providers") {
    setStatusKey("settings.status.ready", "neutral");
  }
}

function normalizeMode(mode) {
  const value = String(mode || "")
    .trim()
    .toLowerCase();
  if (!value) {
    return "simple";
  }
  return ["simple", "professional", "friendly", "casual", "formal"].includes(value)
    ? value
    : "simple";
}

function quickModeOptionLabel(mode) {
  const labels = {
    simple: ["✍️", "settings.mode.simple.label"],
    professional: ["🧑‍💻", "settings.mode.professional.label"],
    friendly: ["📢", "settings.mode.friendly.label"],
    casual: ["💬", "settings.mode.casual.label"],
    formal: ["🧠", "settings.mode.formal.label"],
  };
  const [emoji, key] = labels[mode] || labels.simple;
  return `${emoji} ${t(key)}`;
}

function renderQuickModeOptions() {
  const selected = normalizeMode(quickDefaultMode.value);
  quickDefaultMode.innerHTML = "";
  ["simple", "professional", "friendly", "casual", "formal"].forEach((mode) => {
    const option = document.createElement("option");
    option.value = mode;
    option.textContent = quickModeOptionLabel(mode);
    quickDefaultMode.appendChild(option);
  });
  quickDefaultMode.value = selected;
}

function applySettingsTranslations() {
  applyTranslations(document);
  renderQuickModeOptions();
  if (providersLoaded) {
    renderProvidersList(selectedProviderId);
  }
}

function buildProviderPayload() {
  return {
    id: providerId.value || null,
    name: providerName.value.trim(),
    providerType: providerType.value,
    baseUrl: providerBaseUrl.value.trim(),
    translateModel: providerTranslateModel.value.trim(),
    transcribeModel: providerTranscribeModel.value.trim(),
  };
}

function syncProviderButtons() {
  const selected = cachedProviders.find((provider) => provider.id === selectedProviderId) || null;
  activateProviderBtn.disabled = !selected || selected.isActive;
  deleteProviderBtn.disabled = !selected;
}

function fillProviderForm(provider) {
  selectedProviderId = provider.id;
  providerId.value = provider.id;
  providerName.value = provider.name;
  providerType.value = provider.providerType;
  providerBaseUrl.value = provider.baseUrl;
  providerTranslateModel.value = provider.translateModel;
  providerTranscribeModel.value = provider.transcribeModel || "gpt-4o-mini-transcribe";
  providerApiKey.value = provider.apiKey || "";
  providerApiKey.placeholder = "sk-...";
  syncProviderButtons();
}

function resetProviderForm() {
  selectedProviderId = null;
  providerId.value = "";
  providerName.value = "";
  providerType.value = "openai";
  providerBaseUrl.value = "https://api.openai.com/v1";
  providerTranslateModel.value = "gpt-4.1-mini";
  providerTranscribeModel.value = "gpt-4o-mini-transcribe";
  providerApiKey.value = "";
  providerApiKey.placeholder = "sk-...";
  syncProviderButtons();
  providerName.focus();
}

function renderProvidersSummary() {
  const total = cachedProviders.length;
  const active = cachedProviders.find((provider) => provider.isActive);
  if (!total) {
    providersSummaryEl.textContent = t("settings.providers.none");
    return;
  }
  providersSummaryEl.textContent = t("settings.providers.summary", {
    total,
    active: active ? active.name : t("settings.providers.active_none"),
  });
}

function renderProvidersList(preferredId) {
  const selected =
    cachedProviders.find((provider) => provider.id === preferredId) ||
    cachedProviders.find((provider) => provider.id === selectedProviderId) ||
    cachedProviders.find((provider) => provider.isActive) ||
    cachedProviders[0] ||
    null;
  selectedProviderId = selected ? selected.id : null;

  providersList.innerHTML = "";
  renderProvidersSummary();

  if (!cachedProviders.length) {
    const empty = document.createElement("li");
    empty.className = "providers-empty";
    empty.textContent = t("settings.providers.none_list");
    providersList.appendChild(empty);
    resetProviderForm();
    return;
  }

  for (const provider of cachedProviders) {
    const item = document.createElement("li");
    const button = document.createElement("button");
    const keyClass = provider.hasApiKey ? "is-key-ok" : "is-key-missing";
    const keyLabel = provider.hasApiKey ? t("settings.card.key_saved") : t("settings.card.key_missing");
    button.type = "button";
    button.className = `provider-card-btn ${provider.isActive ? "is-active" : ""} ${
      provider.id === selectedProviderId ? "is-selected" : ""
    }`;
    button.innerHTML = `
      <span class="provider-card-top">
        <span class="provider-name">${escapeHtml(provider.name)}</span>
        <span class="provider-chip ${provider.isActive ? "is-active" : ""}">${provider.isActive ? t("settings.card.active") : t("settings.card.inactive")}</span>
      </span>
      <span class="provider-meta">${escapeHtml(provider.providerType)} · ${escapeHtml(providerHost(provider.baseUrl))}</span>
      <span class="provider-key ${keyClass}">${keyLabel}</span>
    `;

    button.addEventListener("click", () => {
      fillProviderForm(provider);
      renderProvidersList(provider.id);
      setStatus(t("settings.status.editing_provider", { name: provider.name }), "neutral");
    });

    item.appendChild(button);
    providersList.appendChild(item);
  }

  if (selected) {
    fillProviderForm(selected);
  }
}

async function loadProviders(preferredId = null) {
  cachedProviders = await invoke("list_providers");
  providersLoaded = true;
  renderProvidersList(preferredId || selectedProviderId || providerId.value || null);
}

async function saveProvider(event) {
  event.preventDefault();

  const payload = buildProviderPayload();
  if (
    !payload.name ||
    !payload.baseUrl ||
    !payload.translateModel ||
    !payload.transcribeModel
  ) {
    setStatusKey("settings.status.complete_provider_fields", "error");
    return;
  }

  setStatusKey("settings.status.saving_provider", "loading");

  try {
    const keyValue = providerApiKey.value.trim();
    const saved = await invoke("save_provider", {
      provider: payload,
      apiKey: keyValue || null,
    });
    await loadProviders(saved.id);
    setStatus(
      saved.hasApiKey
        ? t("settings.status.saved_provider", { name: saved.name })
        : t("settings.status.saved_provider_no_key", { name: saved.name }),
      "success",
    );
  } catch (error) {
    setStatus(normalizeError(error), "error");
  }
}

async function activateProvider() {
  const currentId = providerId.value.trim();
  if (!currentId) {
    setStatusKey("settings.status.select_provider_activate", "error");
    return;
  }

  const selected = cachedProviders.find((provider) => provider.id === currentId) || null;
  if (selected?.isActive) {
    setStatus(t("settings.status.provider_already_active", { name: selected.name }), "neutral");
    return;
  }

  setStatusKey("settings.status.setting_active_provider", "loading");
  try {
    await invoke("set_active_provider", { providerId: currentId });
    await loadProviders(currentId);
    const provider = cachedProviders.find((item) => item.id === currentId);
    setStatus(
      t("settings.status.active_provider_set", {
        name: provider ? provider.name : t("settings.status.active_provider_updated"),
      }),
      "success",
    );
  } catch (error) {
    setStatus(normalizeError(error), "error");
  }
}

async function deleteProvider() {
  const currentId = providerId.value.trim();
  if (!currentId) {
    setStatusKey("settings.status.select_provider_delete", "error");
    return;
  }

  const selected = cachedProviders.find((provider) => provider.id === currentId) || null;
  if (!selected) {
    setStatusKey("settings.status.selected_provider_not_found", "error");
    return;
  }

  const confirmed = window.confirm(t("settings.confirm.delete_provider", { name: selected.name }));
  if (!confirmed) {
    return;
  }

  setStatus(t("settings.status.deleting_provider", { name: selected.name }), "loading");
  try {
    await invoke("delete_provider", { providerId: currentId });
    await loadProviders();
    setStatus(t("settings.status.deleted_provider", { name: selected.name }), "success");
  } catch (error) {
    setStatus(normalizeError(error), "error");
  }
}

async function testProvider() {
  const payload = buildProviderPayload();
  if (!payload.name || !payload.baseUrl) {
    setStatusKey("settings.status.complete_name_url_test", "error");
    return;
  }

  setStatus(t("settings.status.testing_provider", { name: payload.name }), "loading");
  try {
    const message = await invoke("test_provider_connection_input", {
      provider: payload,
      apiKey: providerApiKey.value.trim() || null,
    });
    setStatus(message, "success");
  } catch (error) {
    setStatus(normalizeError(error), "error");
  }
}

function fillPromptForm(settings) {
  const value = settings || DEFAULT_PROMPT_SETTINGS;
  const modeInstructions = value.modeInstructions || DEFAULT_PROMPT_SETTINGS.modeInstructions;

  promptTranslateSystem.value =
    value.translateSystemPrompt || DEFAULT_PROMPT_SETTINGS.translateSystemPrompt;
  sourceLanguage.value = value.sourceLanguage || DEFAULT_PROMPT_SETTINGS.sourceLanguage;
  targetLanguage.value = value.targetLanguage || DEFAULT_PROMPT_SETTINGS.targetLanguage;

  modeSimple.value = modeInstructions.simple || DEFAULT_PROMPT_SETTINGS.modeInstructions.simple;
  modeProfessional.value =
    modeInstructions.professional || DEFAULT_PROMPT_SETTINGS.modeInstructions.professional;
  modeFriendly.value = modeInstructions.friendly || DEFAULT_PROMPT_SETTINGS.modeInstructions.friendly;
  modeCasual.value = modeInstructions.casual || DEFAULT_PROMPT_SETTINGS.modeInstructions.casual;
  modeFormal.value = modeInstructions.formal || DEFAULT_PROMPT_SETTINGS.modeInstructions.formal;
  quickDefaultMode.value = normalizeMode(value.quickMode || DEFAULT_PROMPT_SETTINGS.quickMode);
  renderQuickModeOptions();
}

function buildPromptPayload() {
  return {
    translateSystemPrompt: promptTranslateSystem.value.trim(),
    sourceLanguage: sourceLanguage.value.trim() || DEFAULT_PROMPT_SETTINGS.sourceLanguage,
    targetLanguage: targetLanguage.value.trim() || DEFAULT_PROMPT_SETTINGS.targetLanguage,
    modeInstructions: {
      simple: modeSimple.value.trim(),
      professional: modeProfessional.value.trim(),
      friendly: modeFriendly.value.trim(),
      casual: modeCasual.value.trim(),
      formal: modeFormal.value.trim(),
    },
    quickMode: normalizeMode(quickDefaultMode.value),
  };
}

async function loadPromptSettings() {
  const settings = await invoke("get_prompt_settings");
  fillPromptForm(settings);
}

async function savePromptSettings(event) {
  event.preventDefault();
  const payload = buildPromptPayload();
  if (
    !payload.translateSystemPrompt ||
    !payload.modeInstructions.simple ||
    !payload.modeInstructions.professional ||
    !payload.modeInstructions.friendly ||
    !payload.modeInstructions.casual ||
    !payload.modeInstructions.formal
  ) {
    setStatusKey("settings.status.complete_prompts", "error");
    return;
  }

  setStatusKey("settings.status.saving_prompts", "loading");
  try {
    const saved = await invoke("save_prompt_settings", { promptSettings: payload });
    fillPromptForm(saved);
    setStatusKey("settings.status.prompts_saved", "success");
  } catch (error) {
    setStatus(normalizeError(error), "error");
  }
}

async function loadUiSettings() {
  if (!invoke) {
    currentUiLanguagePreference = "system";
    if (uiLanguagePreferenceEl) {
      uiLanguagePreferenceEl.value = currentUiLanguagePreference;
    }
    setLanguagePreference(currentUiLanguagePreference);
    applySettingsTranslations();
    return;
  }

  try {
    const settings = await invoke("get_ui_settings");
    currentUiLanguagePreference = String(settings?.uiLanguagePreference || "system");
  } catch (_) {
    currentUiLanguagePreference = "system";
  }

  if (uiLanguagePreferenceEl) {
    uiLanguagePreferenceEl.value = currentUiLanguagePreference;
  }
  setLanguagePreference(currentUiLanguagePreference);
  applySettingsTranslations();
}

async function saveUiLanguagePreference(nextPreference) {
  if (!invoke) {
    currentUiLanguagePreference = nextPreference;
    setLanguagePreference(currentUiLanguagePreference);
    applySettingsTranslations();
    return;
  }

  try {
    const saved = await invoke("save_ui_settings", {
      uiSettings: {
        uiLanguagePreference: nextPreference,
      },
    });
    currentUiLanguagePreference = String(saved?.uiLanguagePreference || nextPreference);
    if (uiLanguagePreferenceEl) {
      uiLanguagePreferenceEl.value = currentUiLanguagePreference;
    }
    setLanguagePreference(currentUiLanguagePreference);
    applySettingsTranslations();
  } catch (error) {
    if (uiLanguagePreferenceEl) {
      uiLanguagePreferenceEl.value = currentUiLanguagePreference;
    }
    setStatus(normalizeError(error), "error");
  }
}

providerForm.addEventListener("submit", saveProvider);
testProviderBtn.addEventListener("click", testProvider);
activateProviderBtn.addEventListener("click", activateProvider);
deleteProviderBtn.addEventListener("click", deleteProvider);
newProviderBtn.addEventListener("click", () => {
  resetProviderForm();
  activateView("providers");
  setStatusKey("settings.status.new_provider_ready", "neutral");
});
promptForm.addEventListener("submit", savePromptSettings);
resetPromptBtn.addEventListener("click", () => {
  fillPromptForm(DEFAULT_PROMPT_SETTINGS);
  setStatusKey("settings.status.prompt_defaults_restored", "neutral");
});

if (uiLanguagePreferenceEl) {
  uiLanguagePreferenceEl.addEventListener("change", () => {
    saveUiLanguagePreference(String(uiLanguagePreferenceEl.value || "system"));
  });
}

function syncTranscriptionLocalVisibility() {
  transcriptionLocalSection.hidden = transcriptionMode.value !== "local";
}

async function loadTranscriptionConfig() {
  if (!invoke) return;
  try {
    const config = await invoke("get_transcription_config");
    transcriptionMode.value = config.mode || "api";
    localModelPath.value = config.localModelPath || "";
    syncTranscriptionLocalVisibility();
  } catch (_) {
    transcriptionMode.value = "api";
    syncTranscriptionLocalVisibility();
  }
}

async function renderWhisperModels() {
  if (!invoke || !whisperModelsContainer) return;
  try {
    const models = await invoke("list_whisper_models");
    whisperModelsContainer.innerHTML = "";
    for (const m of models) {
      const row = document.createElement("div");
      row.className = "whisper-model-row";
      const btn = document.createElement("button");
      btn.type = "button";
      btn.className = "settings-btn-secondary";
      btn.textContent = t("settings.action.download");
      btn.dataset.modelId = m.id;
      btn.addEventListener("click", async () => {
        btn.disabled = true;
        setStatus(t("settings.status.downloading_model", { id: m.id }), "loading");
        try {
          const path = await invoke("download_whisper_model", { modelId: m.id });
          localModelPath.value = path;
          await saveTranscriptionConfig();
          setStatus(t("settings.status.downloaded_model", { id: m.id }), "success");
        } catch (err) {
          setStatus(normalizeError(err), "error");
        } finally {
          btn.disabled = false;
        }
      });
      row.innerHTML = `<span class="model-info">${escapeHtml(m.id)} <span class="model-size">${escapeHtml(m.size)}</span></span>`;
      row.appendChild(btn);
      whisperModelsContainer.appendChild(row);
    }
  } catch (_) {
    whisperModelsContainer.innerHTML = `<p class="hint">${escapeHtml(t("settings.transcription.load_models_failed"))}</p>`;
  }
}

async function saveTranscriptionConfig() {
  if (!invoke) return;
  setStatusKey("settings.status.saving_transcription", "loading");
  try {
    const path = transcriptionMode.value === "local" ? localModelPath.value.trim() : "";
    await invoke("save_transcription_config", {
      transcription: {
        mode: transcriptionMode.value,
        localModelPath: path || null,
      },
    });
    setStatusKey("settings.status.transcription_saved", "success");
  } catch (error) {
    setStatus(normalizeError(error), "error");
  }
}

transcriptionMode.addEventListener("change", syncTranscriptionLocalVisibility);
pickModelBtn.addEventListener("click", async () => {
  if (!invoke) return;
  try {
    const path = await invoke("pick_local_whisper_model");
    if (path) {
      localModelPath.value = path;
      transcriptionMode.value = "local";
      syncTranscriptionLocalVisibility();
      await saveTranscriptionConfig();
    }
  } catch (error) {
    setStatus(normalizeError(error), "error");
  }
});
saveTranscriptionBtn.addEventListener("click", saveTranscriptionConfig);

const settingsNav = document.querySelector(".settings-nav");
if (settingsNav) {
  settingsNav.addEventListener("click", (e) => {
    const btn = e.target.closest(".settings-nav-btn");
    if (btn) {
      e.preventDefault();
      activateView(btn.dataset.view);
    }
  });
  settingsNav.addEventListener("keydown", (e) => {
    const tabEls = Array.from(settingsNav.querySelectorAll('[role="tab"]'));
    const idx = tabEls.indexOf(e.target);
    if (idx === -1) return;
    let nextIdx = idx;
    if (e.key === "ArrowLeft" || e.key === "ArrowUp") {
      e.preventDefault();
      nextIdx = idx <= 0 ? tabEls.length - 1 : idx - 1;
    } else if (e.key === "ArrowRight" || e.key === "ArrowDown") {
      e.preventDefault();
      nextIdx = idx >= tabEls.length - 1 ? 0 : idx + 1;
    } else if (e.key === "Home") {
      e.preventDefault();
      nextIdx = 0;
    } else if (e.key === "End") {
      e.preventDefault();
      nextIdx = tabEls.length - 1;
    } else {
      return;
    }
    const nextTab = tabEls[nextIdx];
    if (nextTab) {
      nextTab.focus();
      activateView(nextTab.dataset.view);
    }
  });
}
window.addEventListener("hashchange", () => activateView(currentHashView(), false));

async function bootstrap() {
  activateView(currentHashView(), false);
  resetProviderForm();
  fillPromptForm(DEFAULT_PROMPT_SETTINGS);
  if (window.lucide && typeof window.lucide.createIcons === "function") {
    window.lucide.createIcons();
  }

  await loadUiSettings();

  if (!invoke) {
    setStatusKey("settings.status.app_not_ready", "error");
    return;
  }

  try {
    if (listen) {
      await listen("ui-language-changed", async (event) => {
        const nextPreference = String(event?.payload?.uiLanguagePreference || "system");
        currentUiLanguagePreference = nextPreference;
        if (uiLanguagePreferenceEl) {
          uiLanguagePreferenceEl.value = nextPreference;
        }
        setLanguagePreference(currentUiLanguagePreference);
        applySettingsTranslations();
        await renderWhisperModels();
      });
    }

    await Promise.all([
      loadProviders(),
      loadPromptSettings(),
      loadTranscriptionConfig(),
      renderWhisperModels(),
    ]);
    setStatusKey("settings.status.ready", "neutral");
  } catch (error) {
    setStatus(t("settings.status.failed_load", { error: normalizeError(error) }), "error");
  }
}

bootstrap();
