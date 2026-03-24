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
const anchorBehaviorEl = document.getElementById("anchor-behavior");
const anchorBehaviorOptionEls = Array.from(document.querySelectorAll(".anchor-behavior-option"));
const anchorBehaviorToggleEls = Array.from(document.querySelectorAll(".anchor-behavior-option-toggle"));
const anchorGifContextualEl = document.getElementById("anchor-gif-contextual");
const anchorGifFloatingEl = document.getElementById("anchor-gif-floating");
const creatorProfileLink = document.getElementById("creator-profile-link");
const settingsInstalledVersionEl = document.getElementById("settings-installed-version");
const permissionsMicrophoneStatusEl = document.getElementById("permissions-microphone-status");
const permissionsAccessibilityStatusEl = document.getElementById("permissions-accessibility-status");
const permissionsAutomationStatusEl = document.getElementById("permissions-automation-status");
const permissionsOpenMicrophoneBtn = document.getElementById("permissions-open-microphone-btn");
const permissionsOpenAccessibilityBtn = document.getElementById("permissions-open-accessibility-btn");
const permissionsOpenAutomationBtn = document.getElementById("permissions-open-automation-btn");
const permissionsCheckMicrophoneBtn = document.getElementById("permissions-check-microphone-btn");
const permissionsCheckAccessibilityBtn = document.getElementById("permissions-check-accessibility-btn");
const permissionsCheckAutomationBtn = document.getElementById("permissions-check-automation-btn");
const permissionsAccessibilityRowEl = permissionsAccessibilityStatusEl
  ? permissionsAccessibilityStatusEl.closest(".permissions-row")
  : null;
const permissionsAutomationRowEl = permissionsAutomationStatusEl
  ? permissionsAutomationStatusEl.closest(".permissions-row")
  : null;
const anchorBehaviorContextualOptionEl = document.querySelector(
  '.anchor-behavior-option[data-anchor-behavior-option="contextual"]',
);

const providersList = document.getElementById("providers-list");
const providerForm = document.getElementById("provider-form");
const providerId = document.getElementById("provider-id");
const providerModeBanner = document.getElementById("provider-mode-banner");
const providerModeTitle = document.getElementById("provider-mode-title");
const providerModeHint = document.getElementById("provider-mode-hint");
const providerDetailsHint = document.getElementById("provider-details-hint");
const providerName = document.getElementById("provider-name");
const providerType = document.getElementById("provider-type");
const providerBaseUrl = document.getElementById("provider-base-url");
const providerTranslateModel = document.getElementById("provider-translate-model");
const providerTranscribeModel = document.getElementById("provider-transcribe-model");
const providerTranscribeModelHint = document.getElementById("provider-transcribe-model-hint");
const providerTranslateModelField = document.getElementById("provider-translate-model-field");
const providerTranscribeModelField = document.getElementById("provider-transcribe-model-field");
const providerApiKey = document.getElementById("provider-api-key");
const providerApiKeyHint = document.getElementById("provider-api-key-hint");
const providerApiKeyField = document.getElementById("provider-api-key-field");
const saveProviderBtn = document.getElementById("save-provider-btn");
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
const localModelsDir = document.getElementById("local-models-dir");
const pickModelsDirBtn = document.getElementById("pick-models-dir-btn");
const saveTranscriptionBtn = document.getElementById("save-transcription-btn");
const whisperModelsContainer = document.getElementById("whisper-models-container");
const pipelineGuideEls = Array.from(document.querySelectorAll("[data-pipeline-guide]"));

const settingsAudioFileInput = document.getElementById("settings-audio-file-input");
const settingsChooseAudioBtn = document.getElementById("settings-choose-audio-btn");
const settingsAudioFileName = document.getElementById("settings-audio-file-name");
const settingsAudioTranscribeStatus = document.getElementById("settings-audio-transcribe-status");
const settingsAudioTranscriptOutput = document.getElementById("settings-audio-transcript-output");
const settingsTranscriptActions = document.getElementById("settings-transcript-actions");
const settingsResetTranscriptBtn = document.getElementById("settings-reset-transcript-btn");
const settingsCopyTranscriptBtn = document.getElementById("settings-copy-transcript-btn");
const settingsInsertTranscriptBtn = document.getElementById("settings-insert-transcript-btn");

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
let currentAnchorBehavior = "contextual";
let installedAppVersion = "";
let savedLocalModelsDir = "";
let localModelPathValue = "";
let activeWhisperDownloadModelId = null;
const whisperModelProgressEls = new Map();
let settingsToolboxAudioTranscribing = false;
let settingsToolboxUploadRequestNonce = 0;

function renderInstalledVersion() {
  if (!settingsInstalledVersionEl) {
    return;
  }
  const version = installedAppVersion || "—";
  settingsInstalledVersionEl.textContent = t("settings.version.installed", { version });
}

function normalizeAnchorBehavior(value) {
  return String(value || "").trim().toLowerCase() === "floating" ? "floating" : "contextual";
}

function syncAnchorBehaviorSelectionUi() {
  const normalized = normalizeAnchorBehavior(currentAnchorBehavior);
  anchorBehaviorOptionEls.forEach((option) => {
    const value = normalizeAnchorBehavior(option.dataset.anchorBehaviorOption);
    const selected = value === normalized;
    const toggle = option.querySelector(".anchor-behavior-option-toggle");
    option.classList.toggle("is-selected", selected);
    if (toggle) {
      toggle.setAttribute("aria-checked", selected ? "true" : "false");
      toggle.tabIndex = selected ? 0 : -1;
    }
  });
  if (anchorBehaviorEl) {
    anchorBehaviorEl.value = normalized;
  }
}

function setAnchorPreviewState(imageEl, state) {
  const slot = imageEl?.closest(".anchor-behavior-gif-slot");
  const option = imageEl?.closest(".anchor-behavior-option");
  if (slot) {
    slot.dataset.previewState = state;
  }
  if (option) {
    option.dataset.previewState = state;
  }
}

function setAnchorBehaviorGifPreview(imageEl, candidatePaths) {
  if (!imageEl) {
    return;
  }
  const slot = imageEl.closest(".anchor-behavior-gif-slot");
  const emptyStateEl = slot ? slot.querySelector(".anchor-behavior-gif-empty") : null;
  const candidates = (candidatePaths || []).map((value) => String(value || "").trim()).filter(Boolean);
  let index = 0;
  setAnchorPreviewState(imageEl, candidates.length ? "loading" : "fallback");

  const showFallback = () => {
    imageEl.hidden = true;
    if (emptyStateEl) {
      emptyStateEl.hidden = false;
    }
    setAnchorPreviewState(imageEl, "fallback");
  };

  const tryNext = () => {
    if (index >= candidates.length) {
      showFallback();
      return;
    }
    const nextSrc = candidates[index];
    index += 1;
    imageEl.onload = () => {
      imageEl.onload = null;
      imageEl.onerror = null;
      imageEl.hidden = false;
      if (emptyStateEl) {
        emptyStateEl.hidden = true;
      }
      setAnchorPreviewState(imageEl, "ready");
    };
    imageEl.onerror = () => {
      imageEl.onload = null;
      imageEl.onerror = null;
      tryNext();
    };
    imageEl.src = nextSrc;
  };

  tryNext();
}

function initializeAnchorBehaviorGifPreviews() {
  setAnchorBehaviorGifPreview(anchorGifContextualEl, [
    "./on-input.gif",
    "./on%20input.gif",
    "./on_input.gif",
  ]);
  setAnchorBehaviorGifPreview(anchorGifFloatingEl, [
    "./free.gif",
  ]);
}

function primaryPermissionSettingsButton() {
  const candidates = [
    permissionsOpenMicrophoneBtn,
    permissionsOpenAccessibilityBtn,
    permissionsOpenAutomationBtn,
  ].filter((button) => {
    if (!button) {
      return false;
    }
    const row = button.closest(".permissions-row");
    return !row || !row.hidden;
  });

  const pending = candidates.find((button) => {
    const row = button.closest(".permissions-row");
    return row?.dataset.state !== "ready";
  });

  return pending || candidates[0] || null;
}

function wirePermissionsPrimaryCta() {
  const primaryCta = document.getElementById("permissions-primary-cta");
  if (!primaryCta || primaryCta.dataset.boundPrimaryPermissionsCta === "true") {
    return;
  }
  primaryCta.dataset.boundPrimaryPermissionsCta = "true";
  primaryCta.addEventListener("click", () => {
    const target = primaryPermissionSettingsButton();
    if (!target) {
      setStatusKey("settings.status.app_not_ready", "error");
      return;
    }
    target.click();
  });
}

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

async function openExternalUrl(url) {
  const value = String(url || "").trim();
  if (!value) return;
  if (!invoke) {
    window.open(value, "_blank", "noopener");
    return;
  }
  try {
    await invoke("open_external_url", { url: value });
  } catch (_) {
    window.open(value, "_blank", "noopener");
  }
}

function setPermissionInlineStatus(statusEl, key, tone = "neutral", rowState = "pending") {
  if (!statusEl) {
    return;
  }
  statusEl.dataset.tone = tone;
  statusEl.dataset.i18n = key;
  statusEl.textContent = t(key);
  const row = statusEl.closest(".permissions-row");
  if (row) {
    row.dataset.state = rowState;
  }
}

function applyRuntimePermissionUi(status) {
  const platform = String(status?.platform || "").trim().toLowerCase();
  const needsAccessibility = Boolean(status?.needsAccessibility);
  const needsAutomation = Boolean(status?.needsAutomation);
  const supportsContextualAnchor = Boolean(
    status?.supportsContextualAnchor ?? platform === "macos",
  );
  if (permissionsAccessibilityRowEl) {
    permissionsAccessibilityRowEl.hidden = !needsAccessibility;
  }
  if (permissionsAutomationRowEl) {
    permissionsAutomationRowEl.hidden = !needsAutomation;
  }
  if (!needsAccessibility) {
    setPermissionInlineStatus(
      permissionsAccessibilityStatusEl,
      "settings.permissions.status.not_required",
      "neutral",
      "ready",
    );
  }
  if (!needsAutomation) {
    setPermissionInlineStatus(
      permissionsAutomationStatusEl,
      "settings.permissions.status.not_required",
      "neutral",
      "ready",
    );
  }

  if (anchorBehaviorContextualOptionEl) {
    anchorBehaviorContextualOptionEl.hidden = !supportsContextualAnchor;
  }
  const contextualSelectOption = anchorBehaviorEl?.querySelector('option[value="contextual"]');
  if (contextualSelectOption) {
    contextualSelectOption.hidden = !supportsContextualAnchor;
  }
  if (!supportsContextualAnchor) {
    if (currentAnchorBehavior !== "floating") {
      currentAnchorBehavior = "floating";
    }
  }
  syncAnchorBehaviorSelectionUi();
}

async function openPermissionSettingsFromSettings(permission, statusEl) {
  if (!invoke) {
    setStatusKey("settings.status.app_not_ready", "error");
    return;
  }
  try {
    await invoke("open_permission_settings", { permission });
    setPermissionInlineStatus(
      statusEl,
      "settings.permissions.status.settings_opened",
      "neutral",
      "pending",
    );
    setStatusKey("settings.permissions.status.settings_opened", "neutral");
  } catch (error) {
    setStatus(normalizeError(error), "error");
  }
}

async function checkMicrophonePermissionFromSettings(statusEl) {
  if (!navigator.mediaDevices || typeof navigator.mediaDevices.getUserMedia !== "function") {
    setPermissionInlineStatus(
      statusEl,
      "main.status.recording_permission_unavailable",
      "error",
      "action_required",
    );
    setStatusKey("main.status.recording_permission_unavailable", "error");
    return;
  }

  setPermissionInlineStatus(
    statusEl,
    "settings.status.checking_microphone_permission",
    "loading",
    "checking",
  );
  setStatusKey("settings.status.checking_microphone_permission", "loading");
  try {
    const stream = await navigator.mediaDevices.getUserMedia({ audio: true });
    stream.getTracks().forEach((track) => track.stop());
    setPermissionInlineStatus(
      statusEl,
      "settings.status.microphone_permission_ready_restart",
      "success",
      "ready",
    );
    setStatusKey("settings.status.microphone_permission_ready_restart", "success");
  } catch (_) {
    setPermissionInlineStatus(
      statusEl,
      "settings.status.microphone_permission_denied",
      "error",
      "action_required",
    );
    setStatusKey("settings.status.microphone_permission_denied", "error");
  }
}

async function checkAccessibilityPermissionFromSettings(statusEl) {
  if (!invoke) {
    setStatusKey("settings.status.app_not_ready", "error");
    return;
  }

  setPermissionInlineStatus(
    statusEl,
    "settings.status.checking_accessibility_permission",
    "loading",
    "checking",
  );
  setStatusKey("settings.status.checking_accessibility_permission", "loading");
  try {
    await invoke("probe_accessibility_permission");
    setPermissionInlineStatus(
      statusEl,
      "settings.status.accessibility_permission_ready_restart",
      "success",
      "ready",
    );
    setStatusKey("settings.status.accessibility_permission_ready_restart", "success");
  } catch (_) {
    setPermissionInlineStatus(
      statusEl,
      "settings.status.accessibility_permission_missing",
      "error",
      "action_required",
    );
    setStatusKey("settings.status.accessibility_permission_missing", "error");
  }
}

async function checkAutomationPermissionFromSettings(statusEl) {
  if (!invoke) {
    setStatusKey("settings.status.app_not_ready", "error");
    return;
  }

  setPermissionInlineStatus(
    statusEl,
    "settings.status.checking_automation_permission",
    "loading",
    "checking",
  );
  setStatusKey("settings.status.checking_automation_permission", "loading");
  try {
    await invoke("probe_system_events_permission");
    setPermissionInlineStatus(
      statusEl,
      "settings.status.automation_permission_ready_restart",
      "success",
      "ready",
    );
    setStatusKey("settings.status.automation_permission_ready_restart", "success");
  } catch (_) {
    setPermissionInlineStatus(
      statusEl,
      "settings.status.automation_permission_missing",
      "error",
      "action_required",
    );
    setStatusKey("settings.status.automation_permission_missing", "error");
  }
}

function hasView(view) {
  return document.querySelector(`.settings-view[data-view="${view}"]`) != null;
}

function defaultSettingsView() {
  if (hasView("general")) {
    return "general";
  }
  if (hasView("providers")) {
    return "providers";
  }
  return "general";
}

function currentHashView() {
  const view = window.location.hash.replace(/^#/, "").trim();
  if (hasView(view)) {
    return view;
  }
  return defaultSettingsView();
}

function activateView(view, syncHash = true) {
  const views = Array.from(document.querySelectorAll(".settings-view"));
  const nextView = views.some((s) => s.dataset.view === view) ? view : defaultSettingsView();
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

function ensureSelectControlValue(control, desiredValue, fallbackValue) {
  const nextValue = String(desiredValue || "").trim() || String(fallbackValue || "").trim();
  if (!control) {
    return;
  }
  if (control.tagName !== "SELECT") {
    control.value = nextValue;
    return;
  }
  const exists = Array.from(control.options).some((option) => option.value === nextValue);
  if (!exists && nextValue) {
    const option = document.createElement("option");
    option.value = nextValue;
    option.textContent = nextValue;
    control.appendChild(option);
  }
  control.value = nextValue;
}

function applySettingsTranslations() {
  applyTranslations(document);
  renderInstalledVersion();
  renderQuickModeOptions();
  bindPipelineScenarioInteractions();
  if (providersLoaded) {
    renderProvidersList(selectedProviderId);
  }
  syncProviderTypeInputs();
  renderPipelineGuides();
}

function isOpenAiCompatibleType(value) {
  return String(value || "").trim().toLowerCase() === "openai-compatible";
}

function isOpenAiType(value) {
  return String(value || "").trim().toLowerCase() === "openai";
}

function setProviderMode(isCreatingNew, providerNameValue) {
  if (providerForm) {
    providerForm.dataset.mode = isCreatingNew ? "create" : "edit";
  }

  const editOnlyButtons = providerForm ? providerForm.querySelectorAll(".provider-edit-only") : [];
  editOnlyButtons.forEach((btn) => {
    btn.hidden = isCreatingNew;
  });

  if (providerModeBanner) {
    providerModeBanner.dataset.providerMode = isCreatingNew ? "create" : "edit";
  }
  if (providerModeTitle) {
    const key = isCreatingNew
      ? "settings.provider.mode.creating"
      : "settings.provider.mode.editing";
    providerModeTitle.dataset.i18n = key;
    providerModeTitle.textContent = isCreatingNew ? t(key) : t(key, { name: providerNameValue || "" });
  }
  if (providerModeHint) {
    providerModeHint.hidden = !isCreatingNew;
  }
  if (providerDetailsHint) {
    providerDetailsHint.hidden = isCreatingNew;
  }
  if (providerApiKeyHint) {
    providerApiKeyHint.hidden = isCreatingNew;
  }
  if (saveProviderBtn) {
    const btnKey = isCreatingNew
      ? "settings.action.save_provider"
      : "settings.action.update_provider";
    saveProviderBtn.dataset.i18n = btnKey;
    saveProviderBtn.textContent = t(btnKey);
  }
}

function normalizedTranscriptionMode() {
  return String(transcriptionMode?.value || "api").trim().toLowerCase() === "local" ? "local" : "api";
}

function selectedPersistedProvider() {
  const currentId = String(providerId?.value || selectedProviderId || "").trim();
  if (currentId) {
    const byId = cachedProviders.find((provider) => provider.id === currentId);
    if (byId) {
      return byId;
    }
  }
  return cachedProviders.find((provider) => provider.isActive) || cachedProviders[0] || null;
}

function activeProviderView(persistedProvider = selectedPersistedProvider()) {
  const hasTypedApiKey = String(providerApiKey?.value || "").trim().length > 0;
  return {
    id: persistedProvider?.id || null,
    providerType: String(providerType?.value || persistedProvider?.providerType || "").trim(),
    baseUrl: String(providerBaseUrl?.value || persistedProvider?.baseUrl || "").trim(),
    translateModel: String(
      providerTranslateModel?.value || persistedProvider?.translateModel || "",
    ).trim(),
    transcribeModel: String(
      providerTranscribeModel?.value || persistedProvider?.transcribeModel || "",
    ).trim(),
    hasApiKey: hasTypedApiKey || persistedProvider?.hasApiKey === true,
  };
}

function isLikelyLocalProviderBaseUrl(baseUrl) {
  const value = String(baseUrl || "").trim();
  if (!value) {
    return false;
  }
  try {
    const url = new URL(value);
    const host = String(url.hostname || "").toLowerCase();
    return host === "localhost" || host === "127.0.0.1" || host === "0.0.0.0" || host === "::1";
  } catch (_) {
    const lower = value.toLowerCase();
    return (
      lower.includes("localhost") ||
      lower.includes("127.0.0.1") ||
      lower.includes("0.0.0.0") ||
      lower.includes("::1")
    );
  }
}

function resolvePipelineBlockers(state) {
  const blockers = [];
  if (!state.hasPersistedProvider) {
    blockers.push({
      id: "provider_missing",
      messageKey: "settings.pipeline.blocker.provider_missing",
      actionKey: "settings.pipeline.action.open_providers",
      tab: "providers",
      focusId: "new-provider-btn",
      statusKey: "settings.pipeline.next.create_provider",
      nextActionKey: "settings.pipeline.next.create_provider",
    });
    return blockers;
  }

  if (!state.translateModel) {
    blockers.push({
      id: "text_model_missing",
      messageKey: "settings.pipeline.blocker.text_model_missing",
      actionKey: "settings.pipeline.action.configure_text_model",
      tab: "providers",
      focusId: "provider-translate-model",
      statusKey: "settings.pipeline.next.configure_text_model",
      nextActionKey: "settings.pipeline.next.configure_text_model",
    });
  }

  if (state.requiresApiKey && !state.hasApiKey) {
    blockers.push({
      id: "api_key_missing",
      messageKey: "settings.pipeline.blocker.api_key_missing",
      actionKey: "settings.pipeline.action.configure_api_key",
      tab: "providers",
      focusId: "provider-api-key",
      statusKey: "settings.pipeline.next.configure_api_key",
      nextActionKey: "settings.pipeline.next.configure_api_key",
    });
  }

  if (state.requiresTranscribeModel && !state.transcribeModel) {
    blockers.push({
      id: "transcribe_model_missing",
      messageKey: "settings.pipeline.blocker.transcribe_model_missing",
      actionKey: "settings.pipeline.action.configure_transcribe_model",
      tab: "providers",
      focusId: "provider-transcribe-model",
      statusKey: "settings.pipeline.next.configure_transcribe_model",
      nextActionKey: "settings.pipeline.next.configure_transcribe_model",
    });
  }

  if (state.transcriptionMode === "local" && !state.localModelConfigured) {
    blockers.push({
      id: "local_model_missing",
      messageKey: "settings.pipeline.blocker.local_model_missing",
      actionKey: "settings.pipeline.action.configure_local_model",
      tab: "providers",
      focusId: "pick-models-dir-btn",
      statusKey: "settings.pipeline.next.configure_local_model",
      nextActionKey: "settings.pipeline.next.configure_local_model",
    });
  }

  return blockers;
}

function runtimePipelineState() {
  const transcriptionValue = normalizedTranscriptionMode();
  const persistedProvider = selectedPersistedProvider();
  const activeProvider = activeProviderView(persistedProvider);
  const providerType = String(activeProvider?.providerType || "").trim().toLowerCase();
  const translateModel = String(activeProvider?.translateModel || "").trim();
  const transcribeModel = String(activeProvider?.transcribeModel || "").trim();
  const localModelConfigured = String(localModelPathValue || "").trim().length > 0;
  const providerLooksLocal = isLikelyLocalProviderBaseUrl(activeProvider?.baseUrl);
  const requiresApiKey = isOpenAiType(providerType);
  const requiresTranscribeModel = transcriptionValue === "api" && isOpenAiType(providerType);
  const hasApiKey = activeProvider?.hasApiKey === true;

  let scenario = transcriptionValue === "api" ? "api_full" : "mixed";
  if (
    transcriptionValue === "local" &&
    isOpenAiCompatibleType(providerType) &&
    providerLooksLocal
  ) {
    scenario = "all_local";
  }

  const state = {
    hasPersistedProvider: !!persistedProvider,
    transcriptionMode: transcriptionValue,
    activeProvider,
    scenario,
    requiresApiKey,
    requiresTranscribeModel,
    hasApiKey,
    translateModel,
    transcribeModel,
    localModelConfigured,
  };
  const blockers = resolvePipelineBlockers(state);
  return {
    ...state,
    blockers,
    nextActionKey: blockers[0]?.nextActionKey || `settings.pipeline.next.${scenario}`,
  };
}

function applyScenarioPreset(scenarioId) {
  const scenario = String(scenarioId || "").trim();
  if (!scenario) {
    return;
  }

  if (scenario === "api_full") {
    transcriptionMode.value = "api";
  } else {
    transcriptionMode.value = "local";
  }

  if (scenario === "all_local") {
    providerType.value = "openai-compatible";
    if (!isLikelyLocalProviderBaseUrl(providerBaseUrl.value)) {
      providerBaseUrl.value = "http://127.0.0.1:1234/v1";
    }
  }

  if (scenario === "mixed") {
    if (isLikelyLocalProviderBaseUrl(providerBaseUrl.value) && isOpenAiCompatibleType(providerType.value)) {
      providerType.value = "openai";
      providerBaseUrl.value = "https://api.openai.com/v1";
    }
  }

  syncTranscriptionLocalVisibility();
  renderPipelineGuides();
  setStatus(t(`settings.pipeline.next.${scenario}`), "neutral");
}

function focusPipelineTarget(blocker) {
  const nextView = String(blocker?.tab || "providers");
  activateView(nextView);
  const focusId = String(blocker?.focusId || "").trim();
  window.setTimeout(() => {
    if (!focusId) {
      return;
    }
    const target = document.getElementById(focusId);
    if (!target) {
      return;
    }
    if (typeof target.scrollIntoView === "function") {
      target.scrollIntoView({ block: "center", behavior: "smooth" });
    }
    if (typeof target.focus === "function") {
      target.focus();
    }
  }, 20);
  if (blocker?.statusKey) {
    setStatus(t(blocker.statusKey), "neutral");
  }
}

function renderPipelineGuideBlock(guide, state) {
  if (!guide) {
    return;
  }

  const preflight = guide.querySelector("[data-pipeline-preflight]");
  const preflightTitle = guide.querySelector("[data-pipeline-preflight-title]");
  const nextAction = guide.querySelector("[data-pipeline-next-action]");
  const blockersEl = guide.querySelector("[data-pipeline-blockers]");

  const hasBlockers = state.blockers.length > 0;
  if (preflight) {
    preflight.dataset.state = hasBlockers ? "needs_attention" : "ready";
  }
  if (preflightTitle) {
    preflightTitle.textContent = hasBlockers
      ? t("settings.pipeline.status.missing", { count: state.blockers.length })
      : t("settings.pipeline.status.ready");
  }
  guide.classList.toggle("is-ready", !hasBlockers);
  guide.classList.toggle("has-blockers", hasBlockers);

  if (nextAction) {
    nextAction.textContent = t(state.nextActionKey);
  }

  const scenariosEl = guide.querySelector(".pipeline-scenarios");
  if (scenariosEl) {
    scenariosEl.hidden = !hasBlockers;
  }

  guide.querySelectorAll(".pipeline-scenario").forEach((card) => {
    const isCurrent = card.dataset.scenario === state.scenario;
    card.classList.toggle("is-active", isCurrent);
    card.setAttribute("aria-current", isCurrent ? "true" : "false");
    card.setAttribute("aria-pressed", isCurrent ? "true" : "false");
  });

  if (!blockersEl) {
    return;
  }

  blockersEl.innerHTML = "";
  blockersEl.hidden = !hasBlockers;
  if (!hasBlockers) {
    return;
  }

  for (const blocker of state.blockers) {
    const item = document.createElement("li");
    item.className = "pipeline-blocker-item";

    const message = document.createElement("span");
    message.className = "pipeline-blocker-message";
    message.textContent = t(blocker.messageKey);

    const action = document.createElement("button");
    action.type = "button";
    action.className = "pipeline-blocker-btn";
    action.textContent = t(blocker.actionKey);
    action.addEventListener("click", () => {
      focusPipelineTarget(blocker);
    });

    item.appendChild(message);
    item.appendChild(action);
    blockersEl.appendChild(item);
  }
}

function renderPipelineGuides() {
  if (!pipelineGuideEls.length) {
    return;
  }
  const state = runtimePipelineState();
  for (const guide of pipelineGuideEls) {
    renderPipelineGuideBlock(guide, state);
  }
}

function bindPipelineScenarioInteractions() {
  for (const guide of pipelineGuideEls) {
    const cards = guide.querySelectorAll(".pipeline-scenario");
    cards.forEach((card) => {
      if (card.dataset.boundScenarioClick === "true") {
        return;
      }
      card.dataset.boundScenarioClick = "true";
      card.addEventListener("click", () => {
        applyScenarioPreset(card.dataset.scenario);
      });
    });
  }
}

function syncProviderTypeInputs() {
  const openAiProvider = !isOpenAiCompatibleType(providerType.value);
  const transcriptionUsesApi = normalizedTranscriptionMode() === "api";
  const requiresTranscribeModel = openAiProvider && transcriptionUsesApi;
  if (providerTranslateModelField) {
    providerTranslateModelField.hidden = false;
  }
  if (providerTranscribeModelField) {
    providerTranscribeModelField.hidden = false;
    providerTranscribeModelField.classList.toggle("is-disabled", !transcriptionUsesApi);
  }
  providerTranslateModel.required = openAiProvider;
  providerTranscribeModel.required = requiresTranscribeModel;
  providerTranscribeModel.disabled = !transcriptionUsesApi;
  if (providerTranscribeModelHint) {
    const transcribeHintKey = transcriptionUsesApi
      ? "settings.field.transcribe_model_hint"
      : "settings.field.transcribe_model_local_disabled";
    providerTranscribeModelHint.dataset.i18n = transcribeHintKey;
    providerTranscribeModelHint.textContent = t(transcribeHintKey);
  }
  if (providerApiKeyField) {
    providerApiKeyField.hidden = false;
  }
  providerApiKey.disabled = false;
  providerApiKey.required = openAiProvider;
  providerApiKey.placeholder = openAiProvider ? "sk-..." : t("settings.field.api_key_optional_placeholder");
  renderPipelineGuides();
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
  setProviderMode(false, provider.name);
  providerId.value = provider.id;
  providerName.value = provider.name;
  providerType.value = provider.providerType;
  providerBaseUrl.value = provider.baseUrl;
  providerTranslateModel.value = provider.translateModel;
  providerTranscribeModel.value =
    provider.transcribeModel ||
    (isOpenAiCompatibleType(provider.providerType) ? "" : "gpt-4o-mini-transcribe");
  providerApiKey.value = provider.apiKey || "";
  providerApiKey.placeholder = "sk-...";
  syncProviderTypeInputs();
  syncProviderButtons();
}

function resetProviderForm() {
  selectedProviderId = null;
  setProviderMode(true);
  providerId.value = "";
  providerName.value = "";
  providerType.value = "openai";
  providerBaseUrl.value = "https://api.openai.com/v1";
  providerTranslateModel.value = "gpt-4.1-mini";
  providerTranscribeModel.value = "gpt-4o-mini-transcribe";
  providerApiKey.value = "";
  providerApiKey.placeholder = "sk-...";
  syncProviderTypeInputs();
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
  const activeName = active ? active.name : t("settings.providers.active_none");
  const key = total === 1 ? "settings.providers.summary_one" : "settings.providers.summary_many";
  providersSummaryEl.textContent = t(key, { total, active: activeName });
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
    const compatibleProvider = isOpenAiCompatibleType(provider.providerType);
    const keyClass = compatibleProvider || provider.hasApiKey ? "is-key-ok" : "is-key-missing";
    const keyLabel = compatibleProvider
      ? t("settings.card.key_not_required")
      : provider.hasApiKey
        ? t("settings.card.key_saved")
        : t("settings.card.key_missing");
    const iconClass = compatibleProvider ? "provider-card-icon-Local" : "provider-card-icon-cloud";
    button.type = "button";
    button.className = `provider-card-btn ${provider.isActive ? "is-active" : ""} ${
      provider.id === selectedProviderId ? "is-selected" : ""
    }`;
    button.innerHTML = `
      <span class="provider-card-top">
        <span class="provider-card-name-row">
          <span class="provider-card-icon ${iconClass}" aria-hidden="true">
            <i class="icon-lucide" data-lucide="${compatibleProvider ? "server" : "cloud"}"></i>
          </span>
          <span class="provider-name">${escapeHtml(provider.name)}</span>
        </span>
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

  if (window.lucide && typeof window.lucide.createIcons === "function") {
    window.lucide.createIcons();
  }
}

async function loadProviders(preferredId = null) {
  cachedProviders = await invoke("list_providers");
  providersLoaded = true;
  renderProvidersList(preferredId || selectedProviderId || providerId.value || null);
  renderPipelineGuides();
}

async function saveProvider(event) {
  event.preventDefault();

  const payload = buildProviderPayload();
  const openAiProvider = !isOpenAiCompatibleType(payload.providerType);
  const requiresTranscribeModel = openAiProvider && normalizedTranscriptionMode() === "api";
  if (
    !payload.name ||
    !payload.baseUrl ||
    (openAiProvider && !payload.translateModel) ||
    (requiresTranscribeModel && !payload.transcribeModel)
  ) {
    setStatusKey("settings.status.complete_provider_fields", "error");
    return;
  }

  setStatusKey("settings.status.saving_provider", "loading");

  try {
    const keyValue = providerApiKey.value.trim();
    const apiKeyPayload = keyValue || null;
    const saved = await invoke("save_provider", {
      provider: payload,
      apiKey: apiKeyPayload,
    });
    await loadProviders(saved.id);
    const compatibleProvider = isOpenAiCompatibleType(saved.providerType);
    setStatus(
      compatibleProvider || saved.hasApiKey
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
    const apiKeyPayload = providerApiKey.value.trim() || null;
    const message = await invoke("test_provider_connection_input", {
      provider: payload,
      apiKey: apiKeyPayload,
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
  ensureSelectControlValue(
    sourceLanguage,
    value.sourceLanguage,
    DEFAULT_PROMPT_SETTINGS.sourceLanguage,
  );
  ensureSelectControlValue(
    targetLanguage,
    value.targetLanguage,
    DEFAULT_PROMPT_SETTINGS.targetLanguage,
  );

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

  const sourceLanguageNormalized = payload.sourceLanguage.trim().toLowerCase();
  const targetLanguageNormalized = payload.targetLanguage.trim().toLowerCase();
  if (sourceLanguageNormalized === targetLanguageNormalized) {
    setStatusKey("settings.status.source_target_must_differ", "error");
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
    currentAnchorBehavior = "contextual";
    if (uiLanguagePreferenceEl) {
      uiLanguagePreferenceEl.value = currentUiLanguagePreference;
    }
    syncAnchorBehaviorSelectionUi();
    setLanguagePreference(currentUiLanguagePreference);
    applySettingsTranslations();
    return;
  }

  try {
    const settings = await invoke("get_ui_settings");
    currentUiLanguagePreference = String(settings?.uiLanguagePreference || "system");
    currentAnchorBehavior = normalizeAnchorBehavior(settings?.anchorBehavior);
  } catch (_) {
    currentUiLanguagePreference = "system";
    currentAnchorBehavior = "contextual";
  }

  if (uiLanguagePreferenceEl) {
    uiLanguagePreferenceEl.value = currentUiLanguagePreference;
  }
  syncAnchorBehaviorSelectionUi();
  setLanguagePreference(currentUiLanguagePreference);
  applySettingsTranslations();
}

async function loadAppVersion() {
  if (!invoke) {
    installedAppVersion = "";
    renderInstalledVersion();
    return;
  }
  try {
    const version = await invoke("get_app_version");
    installedAppVersion = String(version || "").trim();
  } catch (_) {
    installedAppVersion = "";
  }
  renderInstalledVersion();
}

async function saveUiPreferences(nextPreference, nextAnchorBehavior) {
  const normalizedLanguagePreference = String(nextPreference || "system");
  const normalizedAnchorBehavior = normalizeAnchorBehavior(nextAnchorBehavior);

  if (!invoke) {
    currentUiLanguagePreference = normalizedLanguagePreference;
    currentAnchorBehavior = normalizedAnchorBehavior;
    syncAnchorBehaviorSelectionUi();
    setLanguagePreference(currentUiLanguagePreference);
    applySettingsTranslations();
    return;
  }

  try {
    const saved = await invoke("save_ui_settings", {
      uiSettings: {
        uiLanguagePreference: normalizedLanguagePreference,
        anchorBehavior: normalizedAnchorBehavior,
      },
    });
    currentUiLanguagePreference = String(saved?.uiLanguagePreference || normalizedLanguagePreference);
    currentAnchorBehavior = normalizeAnchorBehavior(saved?.anchorBehavior || normalizedAnchorBehavior);
    if (uiLanguagePreferenceEl) {
      uiLanguagePreferenceEl.value = currentUiLanguagePreference;
    }
    syncAnchorBehaviorSelectionUi();
    setLanguagePreference(currentUiLanguagePreference);
    applySettingsTranslations();
  } catch (error) {
    if (uiLanguagePreferenceEl) {
      uiLanguagePreferenceEl.value = currentUiLanguagePreference;
    }
    syncAnchorBehaviorSelectionUi();
    setStatus(normalizeError(error), "error");
  }
}

providerForm.addEventListener("submit", saveProvider);
testProviderBtn.addEventListener("click", testProvider);
activateProviderBtn.addEventListener("click", activateProvider);
deleteProviderBtn.addEventListener("click", deleteProvider);
providerType.addEventListener("change", () => syncProviderTypeInputs());
for (const control of [
  providerName,
  providerBaseUrl,
  providerTranslateModel,
  providerTranscribeModel,
  providerApiKey,
]) {
  if (!control) {
    continue;
  }
  control.addEventListener("input", () => {
    renderPipelineGuides();
  });
  control.addEventListener("change", () => {
    renderPipelineGuides();
  });
}
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
    saveUiPreferences(String(uiLanguagePreferenceEl.value || "system"), currentAnchorBehavior);
  });
}

if (anchorBehaviorEl) {
  anchorBehaviorEl.addEventListener("change", () => {
    saveUiPreferences(currentUiLanguagePreference, normalizeAnchorBehavior(anchorBehaviorEl.value));
  });
}

for (const optionToggle of anchorBehaviorToggleEls) {
  optionToggle.addEventListener("click", () => {
    const nextBehavior = normalizeAnchorBehavior(optionToggle.dataset.anchorBehaviorOption);
    if (nextBehavior === currentAnchorBehavior) {
      syncAnchorBehaviorSelectionUi();
      return;
    }
    if (anchorBehaviorEl) {
      anchorBehaviorEl.value = nextBehavior;
    }
    saveUiPreferences(currentUiLanguagePreference, nextBehavior);
  });
  optionToggle.addEventListener("keydown", (event) => {
    const keysPrev = ["ArrowLeft", "ArrowUp"];
    const keysNext = ["ArrowRight", "ArrowDown"];
    const visibleToggles = anchorBehaviorToggleEls.filter((t) => {
      const option = t.closest(".anchor-behavior-option");
      return !option || option.hidden === false;
    });

    const currentIndexVisible = visibleToggles.indexOf(optionToggle);
    if (currentIndexVisible === -1) {
      return;
    }

    if (event.key === " " || event.key === "Enter") {
      event.preventDefault();
      optionToggle.click();
      return;
    }

    if (!keysPrev.includes(event.key) && !keysNext.includes(event.key)) {
      return;
    }

    event.preventDefault();
    const delta = keysPrev.includes(event.key) ? -1 : 1;
    const nextIndex = (currentIndexVisible + delta + visibleToggles.length) % visibleToggles.length;
    const nextToggle = visibleToggles[nextIndex];
    if (!nextToggle) {
      return;
    }
    nextToggle.focus();
    nextToggle.click();
  });
}

if (creatorProfileLink) {
  creatorProfileLink.addEventListener("click", async (event) => {
    event.preventDefault();
    await openExternalUrl(creatorProfileLink.getAttribute("href"));
  });
}

if (permissionsOpenMicrophoneBtn) {
  permissionsOpenMicrophoneBtn.addEventListener("click", async () => {
    await openPermissionSettingsFromSettings("microphone", permissionsMicrophoneStatusEl);
  });
}

if (permissionsOpenAccessibilityBtn) {
  permissionsOpenAccessibilityBtn.addEventListener("click", async () => {
    await openPermissionSettingsFromSettings("accessibility", permissionsAccessibilityStatusEl);
  });
}

if (permissionsOpenAutomationBtn) {
  permissionsOpenAutomationBtn.addEventListener("click", async () => {
    await openPermissionSettingsFromSettings("automation", permissionsAutomationStatusEl);
  });
}

if (permissionsCheckMicrophoneBtn) {
  permissionsCheckMicrophoneBtn.addEventListener("click", async () => {
    await checkMicrophonePermissionFromSettings(permissionsMicrophoneStatusEl);
  });
}

if (permissionsCheckAccessibilityBtn) {
  permissionsCheckAccessibilityBtn.addEventListener("click", async () => {
    await checkAccessibilityPermissionFromSettings(permissionsAccessibilityStatusEl);
  });
}

if (permissionsCheckAutomationBtn) {
  permissionsCheckAutomationBtn.addEventListener("click", async () => {
    await checkAutomationPermissionFromSettings(permissionsAutomationStatusEl);
  });
}

function syncTranscriptionLocalVisibility() {
  transcriptionLocalSection.hidden = transcriptionMode.value !== "local";
  syncProviderTypeInputs();
}

function normalizeDirectoryPath(value) {
  return String(value || "").trim();
}

function whisperDownloadLockStatusKey() {
  const savedDir = normalizeDirectoryPath(savedLocalModelsDir);
  const selectedDir = normalizeDirectoryPath(localModelsDir?.value);
  if (!savedDir) {
    return selectedDir ? "settings.status.save_models_dir_first" : "settings.status.select_models_dir_first";
  }
  if (selectedDir !== savedDir) {
    return "settings.status.save_models_dir_first";
  }
  return null;
}

function formatByteCount(bytes) {
  const value = Number(bytes);
  if (!Number.isFinite(value) || value < 0) {
    return "0 MB";
  }
  if (value < 1024 * 1024) {
    return `${Math.max(1, Math.round(value / 1024))} KB`;
  }
  return `${(value / (1024 * 1024)).toFixed(value >= 10 * 1024 * 1024 ? 0 : 1)} MB`;
}

function whisperModelIdFromPath(pathValue, models) {
  const normalizedPath = String(pathValue || "").trim().replaceAll("\\", "/").toLowerCase();
  if (!normalizedPath) {
    return null;
  }
  const match = models.find((model) => {
    const candidatePath = String(model?.localPath || "")
      .trim()
      .replaceAll("\\", "/")
      .toLowerCase();
    return candidatePath && candidatePath === normalizedPath;
  });
  if (match) {
    return String(match.id || "");
  }
  const fileName = normalizedPath.split("/").pop();
  const downloadedByName = models.find((model) => {
    const candidateName = String(model?.filename || "").trim().toLowerCase();
    return model?.downloaded === true && candidateName && candidateName === fileName;
  });
  if (downloadedByName) {
    return String(downloadedByName.id || "");
  }
  const anyByName = models.find((model) => String(model?.filename || "").trim().toLowerCase() === fileName);
  if (anyByName?.downloaded === true && anyByName?.localPath) {
    localModelPathValue = String(anyByName.localPath).trim();
  }
  return null;
}

function setWhisperModelProgress(modelId, message = "") {
  const el = whisperModelProgressEls.get(String(modelId || "").trim());
  if (!el) {
    return;
  }
  el.textContent = message;
}

function onWhisperDownloadProgress(payload) {
  const modelId = String(payload?.modelId || "").trim();
  if (!modelId) {
    return;
  }
  if (activeWhisperDownloadModelId && modelId !== activeWhisperDownloadModelId) {
    return;
  }
  const percentRaw = Number(payload?.percent);
  const percent = Number.isFinite(percentRaw) ? Math.max(0, Math.min(100, Math.round(percentRaw))) : null;
  const done = payload?.done === true;
  const downloadedLabel = formatByteCount(payload?.downloadedBytes);

  if (done) {
    setWhisperModelProgress(modelId, "");
    return;
  }

  if (percent != null) {
    setWhisperModelProgress(modelId, `${percent}%`);
    setStatus(t("settings.status.downloading_model_progress", { id: modelId, percent }), "loading");
    return;
  }

  setWhisperModelProgress(modelId, downloadedLabel);
  setStatus(t("settings.status.downloading_model_bytes", { id: modelId, downloaded: downloadedLabel }), "loading");
}

async function loadTranscriptionConfig() {
  if (!invoke) return;
  try {
    const config = await invoke("get_transcription_config");
    transcriptionMode.value = config.mode || "api";
    localModelPathValue = String(config.localModelPath || "").trim();
    savedLocalModelsDir = normalizeDirectoryPath(config.localModelsDir);
    if (localModelsDir) {
      localModelsDir.value = savedLocalModelsDir;
    }
    syncTranscriptionLocalVisibility();
  } catch (_) {
    transcriptionMode.value = "api";
    localModelPathValue = "";
    savedLocalModelsDir = "";
    if (localModelsDir) {
      localModelsDir.value = "";
    }
    syncTranscriptionLocalVisibility();
  }
}

async function renderWhisperModels() {
  if (!invoke || !whisperModelsContainer) return;
  whisperModelProgressEls.clear();
  try {
    const lockKey = whisperDownloadLockStatusKey();
    const models = await invoke("list_whisper_models");
    const selectedModelId = whisperModelIdFromPath(localModelPathValue, models);
    whisperModelsContainer.innerHTML = "";
    for (const m of models) {
      const downloaded = m?.downloaded === true;
      const isSelectedModel = selectedModelId && selectedModelId === m.id;
      const row = document.createElement("div");
      row.className = "whisper-model-row";
      if (lockKey) {
        row.classList.add("is-locked");
      }
      if (isSelectedModel) {
        row.classList.add("is-selected-model");
      }
      const info = document.createElement("span");
      info.className = "model-info";
      info.innerHTML = `${escapeHtml(m.id)} <span class="model-size">${escapeHtml(m.size)}</span>`;
      const progress = document.createElement("small");
      progress.className = "model-progress";
      progress.textContent =
        activeWhisperDownloadModelId === m.id
          ? "0%"
          : isSelectedModel
            ? t("settings.transcription.selected_model_status")
            : downloaded
              ? t("settings.transcription.downloaded_model_status")
            : "";
      info.appendChild(progress);
      whisperModelProgressEls.set(m.id, progress);

      const btn = document.createElement("button");
      btn.type = "button";
      btn.className = "settings-btn-secondary";
      btn.dataset.modelId = m.id;
      const downloadBusy = !!activeWhisperDownloadModelId;
      if (downloaded) {
        btn.textContent = isSelectedModel ? t("settings.action.selected_model") : t("settings.action.use_model");
      } else {
        btn.textContent = t("settings.action.download");
      }
      btn.disabled =
        !!lockKey ||
        (downloadBusy && activeWhisperDownloadModelId !== m.id) ||
        (downloaded && isSelectedModel);
      if (lockKey) {
        btn.title = t(lockKey);
      }
      btn.addEventListener("click", async () => {
        const currentLockKey = whisperDownloadLockStatusKey();
        if (currentLockKey) {
          setStatusKey(currentLockKey, "error");
          return;
        }
        if (m?.downloaded && m?.localPath) {
          localModelPathValue = String(m.localPath).trim();
          transcriptionMode.value = "local";
          syncTranscriptionLocalVisibility();
          await saveTranscriptionConfig();
          setStatus(t("settings.status.selected_model", { id: m.id }), "success");
          return;
        }
        activeWhisperDownloadModelId = m.id;
        await renderWhisperModels();
        setWhisperModelProgress(m.id, "0%");
        setStatus(t("settings.status.downloading_model_progress", { id: m.id, percent: 0 }), "loading");
        try {
          const path = await invoke("download_whisper_model", { modelId: m.id });
          localModelPathValue = String(path || "").trim();
          await saveTranscriptionConfig();
          setWhisperModelProgress(m.id, "100%");
          setStatus(t("settings.status.downloaded_model", { id: m.id }), "success");
        } catch (err) {
          setStatus(normalizeError(err), "error");
        } finally {
          activeWhisperDownloadModelId = null;
          await renderWhisperModels();
        }
      });
      row.appendChild(info);
      row.appendChild(btn);
      whisperModelsContainer.appendChild(row);
    }
    renderPipelineGuides();
  } catch (_) {
    whisperModelsContainer.innerHTML = `<p class="hint">${escapeHtml(t("settings.transcription.load_models_failed"))}</p>`;
    renderPipelineGuides();
  }
}

async function saveTranscriptionConfig() {
  if (!invoke) return;
  setStatusKey("settings.status.saving_transcription", "loading");
  try {
    const path = transcriptionMode.value === "local" ? localModelPathValue : "";
    const modelsDir = normalizeDirectoryPath(localModelsDir?.value);
    const savedConfig = await invoke("save_transcription_config", {
      transcription: {
        mode: transcriptionMode.value,
        localModelPath: path || null,
        localModelsDir: modelsDir || null,
      },
    });
    savedLocalModelsDir = normalizeDirectoryPath(savedConfig?.localModelsDir);
    if (localModelsDir) {
      localModelsDir.value = savedLocalModelsDir;
    }
    await renderWhisperModels();
    setStatusKey("settings.status.transcription_saved", "success");
  } catch (error) {
    setStatus(normalizeError(error), "error");
  }
}

transcriptionMode.addEventListener("change", async () => {
  syncTranscriptionLocalVisibility();
  await renderWhisperModels();
});
if (pickModelsDirBtn) {
  pickModelsDirBtn.addEventListener("click", async () => {
    if (!invoke) return;
    try {
      const folder = await invoke("pick_whisper_models_dir");
      if (folder && localModelsDir) {
        localModelsDir.value = folder;
        transcriptionMode.value = "local";
        syncTranscriptionLocalVisibility();
        await saveTranscriptionConfig();
      }
    } catch (error) {
      setStatus(normalizeError(error), "error");
    }
  });
}
saveTranscriptionBtn.addEventListener("click", saveTranscriptionConfig);

function inferAudioMimeType(file) {
  const providedType = String(file?.type || "")
    .trim()
    .toLowerCase();
  if (providedType) {
    return providedType;
  }

  const fileName = String(file?.name || "").trim().toLowerCase();
  if (fileName.endsWith(".webm")) return "audio/webm";
  if (fileName.endsWith(".mp4")) return "audio/mp4";
  if (fileName.endsWith(".mp3")) return "audio/mpeg";
  if (fileName.endsWith(".ogg")) return "audio/ogg";
  if (fileName.endsWith(".wav")) return "audio/wav";
  if (fileName.endsWith(".m4a")) return "audio/m4a";
  return "";
}

function audioFormatLabel(mimeType) {
  const normalized = String(mimeType || "").trim().toLowerCase();
  switch (normalized) {
    case "audio/webm":
      return "WEBM";
    case "audio/mp4":
      return "MP4";
    case "audio/mpeg":
    case "audio/mp3":
      return "MP3";
    case "audio/ogg":
      return "OGG";
    case "audio/wav":
    case "audio/x-wav":
      return "WAV";
    case "audio/m4a":
    case "audio/x-m4a":
      return "M4A";
    default:
      return "AUDIO";
  }
}

function formatFileSize(size) {
  if (!Number.isFinite(size) || size <= 0) {
    return "0 KB";
  }
  if (size >= 1024 * 1024) {
    return `${(size / (1024 * 1024)).toFixed(1)} MB`;
  }
  return `${Math.max(1, Math.round(size / 1024))} KB`;
}

function arrayBufferToBase64(arrayBuffer) {
  const bytes = new Uint8Array(arrayBuffer);
  const chunkSize = 0x8000;
  let binary = "";

  for (let i = 0; i < bytes.length; i += chunkSize) {
    const chunk = bytes.subarray(i, i + chunkSize);
    binary += String.fromCharCode(...chunk);
  }

  return btoa(binary);
}

function setSettingsToolboxAudioStatus(message, tone = "neutral") {
  if (!settingsAudioTranscribeStatus) return;
  settingsAudioTranscribeStatus.textContent = message;
  settingsAudioTranscribeStatus.dataset.tone = tone;
  settingsAudioTranscribeStatus.hidden = !message;
}

function setSettingsToolboxAudioStatusKey(key, tone = "neutral", params = null) {
  setSettingsToolboxAudioStatus(t(key, params), tone);
}

function setSettingsToolboxBusy(isBusy) {
  settingsToolboxAudioTranscribing = isBusy;
  if (settingsChooseAudioBtn) {
    settingsChooseAudioBtn.disabled = isBusy;
  }
  if (settingsAudioFileInput) {
    settingsAudioFileInput.disabled = isBusy;
  }
}

function resetSettingsToolboxState() {
  settingsToolboxUploadRequestNonce += 1;
  setSettingsToolboxBusy(false);

  if (settingsAudioFileInput) {
    settingsAudioFileInput.value = "";
  }
  if (settingsAudioFileName) {
    settingsAudioFileName.textContent = "";
    settingsAudioFileName.hidden = true;
  }
  if (settingsAudioTranscriptOutput) {
    settingsAudioTranscriptOutput.value = "";
  }
  if (settingsTranscriptActions) {
    settingsTranscriptActions.hidden = true;
  }
  setSettingsToolboxAudioStatus("", "neutral");
  setStatusKey("settings.status.ready", "neutral");
}

async function transcribeAudioBlobInSettings(blob, mimeTypeOverride = null) {
  if (!blob || blob.size === 0) {
    throw new Error(t("settings.toolbox.audio_to_text.no_file"));
  }

  const base64Audio = arrayBufferToBase64(await blob.arrayBuffer());
  return invoke("transcribe_audio", {
    audioBase64: base64Audio,
    mimeType: mimeTypeOverride || blob.type || null,
  });
}

async function handleSettingsAudioFileUpload(file) {
  if (!file) {
    setSettingsToolboxAudioStatusKey("settings.toolbox.audio_to_text.no_file", "error");
    return;
  }

  const effectiveMimeType = inferAudioMimeType(file);
  const validTypes = [
    "audio/webm",
    "audio/mp4",
    "audio/mpeg",
    "audio/mp3",
    "audio/ogg",
    "audio/wav",
    "audio/x-wav",
    "audio/x-m4a",
    "audio/m4a",
  ];
  if (!validTypes.includes(effectiveMimeType) && !file.name.match(/\.(webm|mp4|mp3|ogg|wav|m4a)$/i)) {
    setSettingsToolboxAudioStatusKey("settings.toolbox.audio_to_text.invalid_format", "error");
    return;
  }

  const requestNonce = ++settingsToolboxUploadRequestNonce;
  setSettingsToolboxBusy(true);

  if (settingsAudioFileName) {
    settingsAudioFileName.textContent = t("settings.toolbox.audio_to_text.selected_file", {
      name: file.name,
      format: audioFormatLabel(effectiveMimeType),
      size: formatFileSize(file.size),
    });
    settingsAudioFileName.hidden = false;
  }

  setSettingsToolboxAudioStatusKey("settings.toolbox.audio_to_text.preparing", "loading");
  settingsAudioTranscriptOutput.value = "";
  if (settingsTranscriptActions) settingsTranscriptActions.hidden = true;

  try {
    setSettingsToolboxAudioStatusKey("settings.toolbox.audio_to_text.transcribing", "loading");

    const transcript = await transcribeAudioBlobInSettings(file, effectiveMimeType || null);

    if (requestNonce !== settingsToolboxUploadRequestNonce) {
      return;
    }

    settingsAudioTranscriptOutput.value = transcript;
    if (settingsTranscriptActions) settingsTranscriptActions.hidden = false;
    setSettingsToolboxAudioStatusKey("settings.toolbox.audio_to_text.done", "success");
  } catch (error) {
    if (requestNonce !== settingsToolboxUploadRequestNonce) {
      return;
    }
    setSettingsToolboxAudioStatus(normalizeError(error), "error");
  } finally {
    if (requestNonce === settingsToolboxUploadRequestNonce) {
      setSettingsToolboxBusy(false);
    }
  }
}

function handleSettingsChooseAudioFile() {
  if (settingsAudioFileInput && !settingsToolboxAudioTranscribing) {
    settingsAudioFileInput.click();
  }
}

function handleSettingsAudioFileInputChange(event) {
  const file = event.target.files?.[0];
  if (file) {
    handleSettingsAudioFileUpload(file);
  }
  event.target.value = "";
}

async function copySettingsTranscript() {
  const text = settingsAudioTranscriptOutput?.value?.trim();
  if (!text) {
    setStatusKey("main.status.nothing_to_copy", "error");
    return;
  }
  await navigator.clipboard.writeText(text);
  setStatusKey("main.status.copied", "success");
}

async function insertSettingsTranscript() {
  const text = settingsAudioTranscriptOutput?.value?.trim();
  if (!text) {
    setStatusKey("main.status.nothing_to_insert", "error");
    return;
  }

  setStatusKey("main.status.copying_inserting", "loading");
  try {
    const result = await invoke("auto_insert_text", { text });
    if (result && result.pasted) {
      setStatusKey("main.status.inserted", "success");
      return;
    }
    setStatusKey("main.status.paste_failed", "error", { shortcut: "Cmd+V" });
  } catch (error) {
    setStatusKey("main.status.insert_failed", "error", { shortcut: "Cmd+V" });
  }
}

if (settingsChooseAudioBtn) {
  settingsChooseAudioBtn.addEventListener("click", handleSettingsChooseAudioFile);
}
if (settingsAudioFileInput) {
  settingsAudioFileInput.addEventListener("change", handleSettingsAudioFileInputChange);
}
if (settingsCopyTranscriptBtn) {
  settingsCopyTranscriptBtn.addEventListener("click", copySettingsTranscript);
}
if (settingsInsertTranscriptBtn) {
  settingsInsertTranscriptBtn.addEventListener("click", insertSettingsTranscript);
}
if (settingsResetTranscriptBtn) {
  settingsResetTranscriptBtn.addEventListener("click", resetSettingsToolboxState);
}

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
  initializeAnchorBehaviorGifPreviews();
  wirePermissionsPrimaryCta();
  setPermissionInlineStatus(
    permissionsMicrophoneStatusEl,
    "settings.permissions.status.not_checked",
    "neutral",
    "pending",
  );
  setPermissionInlineStatus(
    permissionsAccessibilityStatusEl,
    "settings.permissions.status.not_checked",
    "neutral",
    "pending",
  );
  setPermissionInlineStatus(
    permissionsAutomationStatusEl,
    "settings.permissions.status.not_checked",
    "neutral",
    "pending",
  );
  if (window.lucide && typeof window.lucide.createIcons === "function") {
    window.lucide.createIcons();
  }
  bindPipelineScenarioInteractions();

  await loadUiSettings();
  await loadAppVersion();

  if (!invoke) {
    renderPipelineGuides();
    setStatusKey("settings.status.app_not_ready", "error");
    return;
  }

  try {
    try {
      const onboardingStatus = await invoke("get_onboarding_status");
      applyRuntimePermissionUi(onboardingStatus);
    } catch (_) {
      // Keep default UI if runtime status is unavailable.
    }

    if (listen) {
      await listen("ui-language-changed", async (event) => {
        const nextPreference = String(event?.payload?.uiLanguagePreference || "system");
        const nextAnchorBehavior = normalizeAnchorBehavior(event?.payload?.anchorBehavior);
        currentUiLanguagePreference = nextPreference;
        currentAnchorBehavior = nextAnchorBehavior;
        if (uiLanguagePreferenceEl) {
          uiLanguagePreferenceEl.value = nextPreference;
        }
        syncAnchorBehaviorSelectionUi();
        setLanguagePreference(currentUiLanguagePreference);
        applySettingsTranslations();
        await renderWhisperModels();
      });
      await listen("whisper-download-progress", (event) => {
        onWhisperDownloadProgress(event?.payload);
      });
    }

    await Promise.all([loadProviders(), loadPromptSettings(), loadTranscriptionConfig()]);
    await renderWhisperModels();
    renderPipelineGuides();
    setStatusKey("settings.status.ready", "neutral");
  } catch (error) {
    setStatus(t("settings.status.failed_load", { error: normalizeError(error) }), "error");
  }
}

bootstrap();
