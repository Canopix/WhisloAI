const invoke = (() => {
  try {
    return window.__TAURI__?.core?.invoke;
  } catch (_) {
    return null;
  }
})();

const statusEl = document.getElementById("status");
const providersSummaryEl = document.getElementById("providers-summary");

const providersList = document.getElementById("providers-list");
const providerForm = document.getElementById("provider-form");
const providerId = document.getElementById("provider-id");
const providerName = document.getElementById("provider-name");
const providerType = document.getElementById("provider-type");
const providerBaseUrl = document.getElementById("provider-base-url");
const providerImproveModel = document.getElementById("provider-improve-model");
const providerTranslateModel = document.getElementById("provider-translate-model");
const providerTranscribeModel = document.getElementById("provider-transcribe-model");
const providerApiKey = document.getElementById("provider-api-key");
const testProviderBtn = document.getElementById("test-provider-btn");
const newProviderBtn = document.getElementById("new-provider-btn");
const activateProviderBtn = document.getElementById("activate-provider-btn");
const deleteProviderBtn = document.getElementById("delete-provider-btn");

const hotkeysForm = document.getElementById("hotkeys-form");
const hotkeyOpenApp = document.getElementById("hotkey-open-app");
const hotkeyOpenImprove = document.getElementById("hotkey-open-improve");
const hotkeyOpenDictate = document.getElementById("hotkey-open-dictate");
const resetHotkeysBtn = document.getElementById("reset-hotkeys-btn");

const promptForm = document.getElementById("prompt-form");
const promptImproveSystem = document.getElementById("prompt-improve-system");
const promptTranslateSystem = document.getElementById("prompt-translate-system");
const modeSimple = document.getElementById("mode-simple");
const modeProfessional = document.getElementById("mode-professional");
const modeFriendly = document.getElementById("mode-friendly");
const modeCasual = document.getElementById("mode-casual");
const modeFormal = document.getElementById("mode-formal");
const quickDefaultMode = document.getElementById("quick-default-mode");
const resetPromptBtn = document.getElementById("reset-prompt-btn");

const DEFAULT_HOTKEYS = {
  openApp: "CommandOrControl+Shift+Space",
  openImprove: "CommandOrControl+Shift+I",
  openDictateTranslate: "CommandOrControl+Shift+D",
};

const DEFAULT_PROMPT_SETTINGS = {
  improveSystemPrompt:
    "You are a writing assistant. Rewrite text in clear, concise, natural English. Keep intent and facts unchanged. Return only final text.",
  translateSystemPrompt:
    "You are a translation assistant. Convert Spanish text into clear, concise, natural English for workplace chat. Preserve names and technical terms. Return only final text.",
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

function buildProviderPayload() {
  return {
    id: providerId.value || null,
    name: providerName.value.trim(),
    providerType: providerType.value,
    baseUrl: providerBaseUrl.value.trim(),
    improveModel: providerImproveModel.value.trim(),
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
  providerImproveModel.value = provider.improveModel;
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
  providerImproveModel.value = "gpt-4.1-mini";
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
    providersSummaryEl.textContent = "No providers saved yet.";
    return;
  }
  providersSummaryEl.textContent = `${total} saved. Active: ${active ? active.name : "none"}.`;
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
    empty.textContent = "No providers yet. Create one from the form on the right.";
    providersList.appendChild(empty);
    resetProviderForm();
    return;
  }

  for (const provider of cachedProviders) {
    const item = document.createElement("li");
    const button = document.createElement("button");
    const keyClass = provider.hasApiKey ? "is-key-ok" : "is-key-missing";
    const keyLabel = provider.hasApiKey ? "API key saved locally" : "API key missing";
    button.type = "button";
    button.className = `provider-card-btn ${provider.isActive ? "is-active" : ""} ${
      provider.id === selectedProviderId ? "is-selected" : ""
    }`;
    button.innerHTML = `
      <span class="provider-card-top">
        <span class="provider-name">${escapeHtml(provider.name)}</span>
        <span class="provider-chip ${provider.isActive ? "is-active" : ""}">${provider.isActive ? "Active" : "Inactive"}</span>
      </span>
      <span class="provider-meta">${escapeHtml(provider.providerType)} · ${escapeHtml(providerHost(provider.baseUrl))}</span>
      <span class="provider-key ${keyClass}">${keyLabel}</span>
    `;

    button.addEventListener("click", () => {
      fillProviderForm(provider);
      renderProvidersList(provider.id);
      setStatus(`Editing provider: ${provider.name}.`, "neutral");
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
  renderProvidersList(preferredId || selectedProviderId || providerId.value || null);
}

async function saveProvider(event) {
  event.preventDefault();

  const payload = buildProviderPayload();
  if (
    !payload.name ||
    !payload.baseUrl ||
    !payload.improveModel ||
    !payload.translateModel ||
    !payload.transcribeModel
  ) {
    setStatus("Complete all provider fields before saving.", "error");
    return;
  }

  setStatus("Saving provider...", "loading");

  try {
    const keyValue = providerApiKey.value.trim();
    const saved = await invoke("save_provider", {
      provider: payload,
      apiKey: keyValue || null,
    });
    await loadProviders(saved.id);
    setStatus(
      saved.hasApiKey
        ? `Saved ${saved.name}.`
        : `Saved ${saved.name}. Add API key if you want to run requests.`,
      "success",
    );
  } catch (error) {
    setStatus(normalizeError(error), "error");
  }
}

async function activateProvider() {
  const currentId = providerId.value.trim();
  if (!currentId) {
    setStatus("Select a provider before setting it active.", "error");
    return;
  }

  const selected = cachedProviders.find((provider) => provider.id === currentId) || null;
  if (selected?.isActive) {
    setStatus(`${selected.name} is already active.`, "neutral");
    return;
  }

  setStatus("Setting active provider...", "loading");
  try {
    await invoke("set_active_provider", { providerId: currentId });
    await loadProviders(currentId);
    const provider = cachedProviders.find((item) => item.id === currentId);
    setStatus(`Active provider set: ${provider ? provider.name : "updated"}.`, "success");
  } catch (error) {
    setStatus(normalizeError(error), "error");
  }
}

async function deleteProvider() {
  const currentId = providerId.value.trim();
  if (!currentId) {
    setStatus("Select a provider before deleting it.", "error");
    return;
  }

  const selected = cachedProviders.find((provider) => provider.id === currentId) || null;
  if (!selected) {
    setStatus("Selected provider was not found.", "error");
    return;
  }

  const confirmed = window.confirm(`Delete provider "${selected.name}"?`);
  if (!confirmed) {
    return;
  }

  setStatus(`Deleting ${selected.name}...`, "loading");
  try {
    await invoke("delete_provider", { providerId: currentId });
    await loadProviders();
    setStatus(`Deleted provider: ${selected.name}.`, "success");
  } catch (error) {
    setStatus(normalizeError(error), "error");
  }
}

async function testProvider() {
  const payload = buildProviderPayload();
  if (!payload.name || !payload.baseUrl) {
    setStatus("Complete provider name and base URL before testing.", "error");
    return;
  }

  setStatus(`Testing ${payload.name}...`, "loading");
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

function fillHotkeysForm(hotkeys) {
  hotkeyOpenApp.value = hotkeys.openApp || DEFAULT_HOTKEYS.openApp;
  hotkeyOpenImprove.value = hotkeys.openImprove || DEFAULT_HOTKEYS.openImprove;
  hotkeyOpenDictate.value = hotkeys.openDictateTranslate || DEFAULT_HOTKEYS.openDictateTranslate;
}

async function loadHotkeys() {
  const hotkeys = await invoke("get_hotkeys");
  fillHotkeysForm(hotkeys);
}

async function saveHotkeys(event) {
  event.preventDefault();

  const payload = {
    openApp: hotkeyOpenApp.value.trim(),
    openImprove: hotkeyOpenImprove.value.trim(),
    openDictateTranslate: hotkeyOpenDictate.value.trim(),
  };

  if (!payload.openApp || !payload.openImprove || !payload.openDictateTranslate) {
    setStatus("Complete all hotkey fields before saving.", "error");
    return;
  }

  setStatus("Saving hotkeys...", "loading");
  try {
    const saved = await invoke("save_hotkeys", { hotkeys: payload });
    fillHotkeysForm(saved);
    setStatus("Hotkeys saved and applied.", "success");
  } catch (error) {
    setStatus(normalizeError(error), "error");
  }
}

function fillPromptForm(settings) {
  const value = settings || DEFAULT_PROMPT_SETTINGS;
  const modeInstructions = value.modeInstructions || DEFAULT_PROMPT_SETTINGS.modeInstructions;

  promptImproveSystem.value = value.improveSystemPrompt || DEFAULT_PROMPT_SETTINGS.improveSystemPrompt;
  promptTranslateSystem.value =
    value.translateSystemPrompt || DEFAULT_PROMPT_SETTINGS.translateSystemPrompt;

  modeSimple.value = modeInstructions.simple || DEFAULT_PROMPT_SETTINGS.modeInstructions.simple;
  modeProfessional.value =
    modeInstructions.professional || DEFAULT_PROMPT_SETTINGS.modeInstructions.professional;
  modeFriendly.value = modeInstructions.friendly || DEFAULT_PROMPT_SETTINGS.modeInstructions.friendly;
  modeCasual.value = modeInstructions.casual || DEFAULT_PROMPT_SETTINGS.modeInstructions.casual;
  modeFormal.value = modeInstructions.formal || DEFAULT_PROMPT_SETTINGS.modeInstructions.formal;
  quickDefaultMode.value = normalizeMode(value.quickMode || DEFAULT_PROMPT_SETTINGS.quickMode);
}

function buildPromptPayload() {
  return {
    improveSystemPrompt: promptImproveSystem.value.trim(),
    translateSystemPrompt: promptTranslateSystem.value.trim(),
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
    !payload.improveSystemPrompt ||
    !payload.translateSystemPrompt ||
    !payload.modeInstructions.simple ||
    !payload.modeInstructions.professional ||
    !payload.modeInstructions.friendly ||
    !payload.modeInstructions.casual ||
    !payload.modeInstructions.formal
  ) {
    setStatus("Complete all prompts and mode instructions before saving.", "error");
    return;
  }

  setStatus("Saving prompts and modes...", "loading");
  try {
    const saved = await invoke("save_prompt_settings", { promptSettings: payload });
    fillPromptForm(saved);
    setStatus("Prompts and modes saved.", "success");
  } catch (error) {
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
  setStatus("New provider form ready.", "neutral");
});
hotkeysForm.addEventListener("submit", saveHotkeys);
resetHotkeysBtn.addEventListener("click", () => {
  fillHotkeysForm(DEFAULT_HOTKEYS);
  setStatus("Hotkeys reset to defaults (not saved yet).", "neutral");
});
promptForm.addEventListener("submit", savePromptSettings);
resetPromptBtn.addEventListener("click", () => {
  fillPromptForm(DEFAULT_PROMPT_SETTINGS);
  setStatus("Prompt defaults restored (not saved yet).", "neutral");
});
const settingsNav = document.querySelector(".settings-nav");
if (settingsNav) {
  settingsNav.addEventListener("click", (e) => {
    const btn = e.target.closest(".settings-nav-btn");
    if (btn) {
      e.preventDefault();
      activateView(btn.dataset.view);
    }
  });
}
window.addEventListener("hashchange", () => activateView(currentHashView(), false));

async function bootstrap() {
  activateView(currentHashView(), false);
  resetProviderForm();
  fillPromptForm(DEFAULT_PROMPT_SETTINGS);
  if (!invoke) {
    setStatus("App not ready. View switching works.", "error");
    return;
  }
  try {
    await Promise.all([loadHotkeys(), loadProviders(), loadPromptSettings()]);
    setStatus("Ready.", "neutral");
  } catch (error) {
    setStatus(`Failed to load settings: ${normalizeError(error)}`, "error");
  }
}

bootstrap();
