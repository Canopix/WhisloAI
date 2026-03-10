const { invoke } = window.__TAURI__.core;
const { listen } = window.__TAURI__.event;
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

const tabs = document.querySelectorAll(".tab");
const panels = document.querySelectorAll(".panel");
const statusEl = document.getElementById("status");
const openSettingsBtn = document.getElementById("open-settings-btn");
const openWidgetBtn = document.getElementById("open-widget-btn");

const improveInput = document.getElementById("improve-input");
const improveStyle = document.getElementById("improve-style");
const improveOutput = document.getElementById("improve-output");
const improveBtn = document.getElementById("improve-btn");
const insertImproveBtn = document.getElementById("insert-improve-btn");
const copyImproveBtn = document.getElementById("copy-improve-btn");

const translateInput = document.getElementById("translate-input");
const translateStyle = document.getElementById("translate-style");
const translateOutput = document.getElementById("translate-output");
const translateBtn = document.getElementById("translate-btn");
const insertTranslateBtn = document.getElementById("insert-translate-btn");
const copyTranslateBtn = document.getElementById("copy-translate-btn");
const recordAudioBtn = document.getElementById("record-audio-btn");
const stopAudioBtn = document.getElementById("stop-audio-btn");
const recordingStateEl = document.getElementById("recording-state");
const recordingVisualizerEl = document.getElementById("recording-visualizer");
const recordingWaveformEl = document.getElementById("recording-waveform");

const onboardingPanel = document.getElementById("onboarding-panel");
const onboardingMicBtn = document.getElementById("onboarding-mic-btn");
const onboardingMicSettingsBtn = document.getElementById("onboarding-mic-settings-btn");
const onboardingMicStatus = document.getElementById("onboarding-mic-status");
const onboardingAccessibilityStep = document.getElementById("onboarding-accessibility-step");
const onboardingAccessibilityBtn = document.getElementById("onboarding-accessibility-btn");
const onboardingAccessibilitySettingsBtn = document.getElementById("onboarding-accessibility-settings-btn");
const onboardingAccessibilityStatus = document.getElementById("onboarding-accessibility-status");
const onboardingFinishBtn = document.getElementById("onboarding-finish-btn");
const onboardingSkipBtn = document.getElementById("onboarding-skip-btn");

let mediaRecorder = null;
let mediaStream = null;
let mediaChunks = [];
let isTranscribingAudio = false;
let isRecordingAudio = false;
let isRecordingAudioStarting = false;
let recordingStartNonce = 0;
let onboardingDismissedForSession = false;
let onboardingAutoPromptTriggered = false;
let recordingAudioContext = null;
let recordingAnalyser = null;
let recordingDataArray = null;
let recordingFrameHandle = null;
let recordingBars = [];
let currentUiLanguagePreference = "system";
let currentStatusState = { type: "key", key: "main.status.ready", params: null, isError: false };
let currentRecordingState = {
  type: "key",
  key: "main.recording.state.idle",
  params: null,
  isError: false,
};

const RECORDING_WAVE_BARS = 24;
const SUPPORTED_MODES = ["simple", "professional", "friendly", "casual", "formal"];

function formatError(error) {
  return typeof specErrorFor === "function" ? specErrorFor(error) : String(error || "").replace(/^Error: /, "");
}

function applyStatusMessage(message, isError = false, state = null) {
  statusEl.textContent = message;
  statusEl.dataset.tone = isError ? "error" : "neutral";
  currentStatusState = state || { type: "text", message, isError };
}

function setStatus(message, isError = false) {
  applyStatusMessage(message, isError, { type: "text", message, isError });
}

function setStatusKey(key, isError = false, params = null) {
  applyStatusMessage(t(key, params), isError, { type: "key", key, params, isError });
}

function refreshStatusTranslation() {
  if (currentStatusState.type === "key") {
    applyStatusMessage(t(currentStatusState.key, currentStatusState.params), currentStatusState.isError, {
      ...currentStatusState,
    });
  }
}

function normalizeMode(mode) {
  const value = String(mode || "")
    .trim()
    .toLowerCase();
  return SUPPORTED_MODES.includes(value) ? value : "simple";
}

function switchTab(tabName) {
  tabs.forEach((tab) => {
    const active = tab.dataset.tab === tabName;
    tab.classList.toggle("is-active", active);
    tab.setAttribute("aria-selected", active ? "true" : "false");
  });

  panels.forEach((panel) => {
    panel.classList.toggle("is-active", panel.dataset.panel === tabName);
  });
}

function applyRecordingState(message, isError = false, state = null) {
  recordingStateEl.textContent = message;
  recordingStateEl.style.color = isError ? "#b91c1c" : "";
  recordingStateEl.hidden = !message;
  currentRecordingState = state || { type: "text", message, isError };
}

function setRecordingState(message, isError = false) {
  applyRecordingState(message, isError, { type: "text", message, isError });
}

function setRecordingStateKey(key, isError = false, params = null) {
  applyRecordingState(t(key, params), isError, { type: "key", key, params, isError });
}

function refreshRecordingStateTranslation() {
  if (currentRecordingState.type === "key") {
    applyRecordingState(t(currentRecordingState.key, currentRecordingState.params), currentRecordingState.isError, {
      ...currentRecordingState,
    });
  }
}

function setRecordingVisualizerVisible(visible) {
  if (!recordingVisualizerEl) {
    return;
  }
  recordingVisualizerEl.hidden = !visible;
}

function ensureRecordingBars() {
  if (!recordingWaveformEl || recordingBars.length) {
    return;
  }

  const fragment = document.createDocumentFragment();
  for (let index = 0; index < RECORDING_WAVE_BARS; index += 1) {
    const bar = document.createElement("span");
    bar.className = "wave-bar";
    bar.style.setProperty("--bar-scale", "0.14");
    bar.style.setProperty("--wave-delay", `${(index * 0.04).toFixed(2)}s`);
    fragment.appendChild(bar);
    recordingBars.push(bar);
  }
  recordingWaveformEl.appendChild(fragment);
}

function resetRecordingBars() {
  recordingBars.forEach((bar) => {
    bar.style.setProperty("--bar-scale", "0.14");
  });
}

function stopRecordingVisualizer() {
  if (recordingFrameHandle) {
    cancelAnimationFrame(recordingFrameHandle);
    recordingFrameHandle = null;
  }

  recordingAnalyser = null;
  recordingDataArray = null;

  if (recordingAudioContext) {
    recordingAudioContext.close().catch(() => {
      // no-op
    });
    recordingAudioContext = null;
  }

  if (recordingWaveformEl) {
    recordingWaveformEl.classList.remove("is-fallback");
  }
  resetRecordingBars();
  setRecordingVisualizerVisible(false);
}

function renderRecordingWaveform() {
  if (!recordingAnalyser || !recordingDataArray.length || !recordingBars.length) {
    return;
  }

  recordingAnalyser.getByteFrequencyData(recordingDataArray);
  const binsPerBar = Math.max(1, Math.floor(recordingDataArray.length / recordingBars.length));

  for (let index = 0; index < recordingBars.length; index += 1) {
    const start = index * binsPerBar;
    const end = Math.min(recordingDataArray.length, start + binsPerBar);
    let sum = 0;
    let count = 0;

    for (let cursor = start; cursor < end; cursor += 1) {
      sum += recordingDataArray[cursor];
      count += 1;
    }

    const average = count ? sum / count : 0;
    const normalized = average / 255;
    const scale = Math.max(0.12, Math.min(1, 0.16 + normalized * 0.84));
    recordingBars[index].style.setProperty("--bar-scale", scale.toFixed(3));
  }

  recordingFrameHandle = requestAnimationFrame(renderRecordingWaveform);
}

function startRecordingVisualizer(stream) {
  if (!recordingWaveformEl) {
    return;
  }

  stopRecordingVisualizer();
  ensureRecordingBars();
  setRecordingVisualizerVisible(true);

  const AudioCtx = window.AudioContext || window.webkitAudioContext;
  if (!AudioCtx) {
    recordingWaveformEl.classList.add("is-fallback");
    return;
  }

  try {
    recordingWaveformEl.classList.remove("is-fallback");
    recordingAudioContext = new AudioCtx();
    const source = recordingAudioContext.createMediaStreamSource(stream);
    recordingAnalyser = recordingAudioContext.createAnalyser();
    recordingAnalyser.fftSize = 256;
    recordingAnalyser.smoothingTimeConstant = 0.82;
    source.connect(recordingAnalyser);
    recordingDataArray = new Uint8Array(recordingAnalyser.frequencyBinCount);
    renderRecordingWaveform();
  } catch (_) {
    recordingWaveformEl.classList.add("is-fallback");
  }
}

function isMacOS() {
  const platform = (navigator.userAgentData && navigator.userAgentData.platform) || navigator.platform || "";
  return /mac/i.test(platform);
}

function pasteShortcutHint() {
  return `${isMacOS() ? "Cmd" : "Ctrl"} + V`;
}

function setOnboardingStatusMessage(target, message, isError = false) {
  target.textContent = message;
  target.style.color = isError ? "#b91c1c" : "#1f2a37";
}

function setOnboardingStatusKey(target, key, isError = false, params = null) {
  setOnboardingStatusMessage(target, t(key, params), isError);
}

function setOnboardingVisible(visible) {
  onboardingPanel.hidden = !visible;

  const mainContent = document.getElementById("main-content");
  if (mainContent) {
    mainContent.hidden = visible;
  }
}

function localizeImportSource(source) {
  const value = String(source || "")
    .trim()
    .toLowerCase();
  if (value === "context action" || value === "context-action" || value === "single-instance") {
    return t("main.source.context_action");
  }
  if (value === "launch args" || value === "launch-args") {
    return t("main.source.launch_args");
  }
  return source;
}

function applyMainTranslations() {
  applyTranslations(document);
  refreshStatusTranslation();
  refreshRecordingStateTranslation();
}

async function loadUiSettings() {
  try {
    const settings = await invoke("get_ui_settings");
    currentUiLanguagePreference = String(settings?.uiLanguagePreference || "system");
  } catch (_) {
    currentUiLanguagePreference = "system";
  }
  setLanguagePreference(currentUiLanguagePreference);
  applyMainTranslations();
}

async function openPermissionSettings(permission) {
  try {
    await invoke("open_permission_settings", { permission });
  } catch (error) {
    setStatus(formatError(error), true);
  }
}

async function requestOnboardingMicrophonePermission() {
  if (!navigator.mediaDevices || typeof navigator.mediaDevices.getUserMedia !== "function") {
    setOnboardingStatusKey(onboardingMicStatus, "main.status.recording_permission_unavailable", true);
    return;
  }

  onboardingMicBtn.disabled = true;
  setOnboardingStatusKey(onboardingMicStatus, "main.status.requesting_microphone");

  try {
    const stream = await navigator.mediaDevices.getUserMedia({ audio: true });
    stream.getTracks().forEach((track) => track.stop());
    setOnboardingStatusKey(onboardingMicStatus, "main.status.microphone_granted");
  } catch (error) {
    setOnboardingStatusKey(onboardingMicStatus, "main.status.microphone_denied", true);
    setStatus(formatError(error), true);
  } finally {
    onboardingMicBtn.disabled = false;
  }
}

async function testOnboardingAccessibilityPermission() {
  onboardingAccessibilityBtn.disabled = true;
  setOnboardingStatusKey(onboardingAccessibilityStatus, "main.status.testing_accessibility");

  try {
    await invoke("probe_auto_insert_permission");
    setOnboardingStatusKey(onboardingAccessibilityStatus, "main.status.accessibility_ready");
  } catch (error) {
    setOnboardingStatusKey(onboardingAccessibilityStatus, "main.status.accessibility_missing", true);
    setStatus(formatError(error), true);
  } finally {
    onboardingAccessibilityBtn.disabled = false;
  }
}

async function finishOnboarding() {
  try {
    await invoke("complete_onboarding");
    setOnboardingVisible(false);
    setStatusKey("main.status.setup_completed");
  } catch (error) {
    setStatus(formatError(error), true);
  }
}

function skipOnboardingForSession() {
  onboardingDismissedForSession = true;
  setOnboardingVisible(false);
  setStatusKey("main.status.setup_skipped");
}

function applyOnboardingStatus(status) {
  const needsAccessibility = Boolean(status && status.needsAccessibility);
  onboardingAccessibilityStep.hidden = !needsAccessibility;

  setOnboardingStatusKey(onboardingMicStatus, "main.onboarding.status.not_checked");
  setOnboardingStatusKey(onboardingAccessibilityStatus, "main.onboarding.status.not_checked");

  const shouldShow = Boolean(status && !status.completed && !onboardingDismissedForSession);
  setOnboardingVisible(shouldShow);

  if (shouldShow && !onboardingAutoPromptTriggered) {
    onboardingAutoPromptTriggered = true;
    requestOnboardingMicrophonePermission();
    if (needsAccessibility) {
      setTimeout(() => {
        testOnboardingAccessibilityPermission();
      }, 250);
    }
  }
}

function releaseMediaStream() {
  if (mediaStream) {
    mediaStream.getTracks().forEach((track) => track.stop());
    mediaStream = null;
  }
  isRecordingAudio = false;
  isRecordingAudioStarting = false;
  stopRecordingVisualizer();
}

function setRecordingButtons(isRecording) {
  recordAudioBtn.disabled = isRecording || isTranscribingAudio;
  stopAudioBtn.disabled = !isRecording || isTranscribingAudio;
}

function pickRecorderMimeType() {
  if (typeof MediaRecorder === "undefined" || typeof MediaRecorder.isTypeSupported !== "function") {
    return null;
  }

  const candidates = ["audio/webm;codecs=opus", "audio/webm", "audio/mp4", "audio/ogg;codecs=opus"];
  return candidates.find((mime) => MediaRecorder.isTypeSupported(mime)) || null;
}

function applyExternalImproveText(text, source = "external") {
  if (!text || !text.trim()) {
    return;
  }

  switchTab("improve");
  improveInput.value = text.trim();
  improveInput.focus();
  improveInput.setSelectionRange(improveInput.value.length, improveInput.value.length);
  setStatusKey("main.status.imported", false, { source: localizeImportSource(source) });
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

async function transcribeRecordedBlob(blob) {
  if (!blob || blob.size === 0) {
    setRecordingStateKey("main.status.audio_empty_retry", true);
    setStatusKey("main.status.audio_empty", true);
    return;
  }

  isTranscribingAudio = true;
  setRecordingButtons(false);
  setRecordingStateKey("main.status.transcribing");
  setStatusKey("main.status.transcribing_audio");

  try {
    const base64Audio = arrayBufferToBase64(await blob.arrayBuffer());
    const transcript = await invoke("transcribe_audio", {
      audioBase64: base64Audio,
      mimeType: blob.type || null,
    });

    translateInput.value = transcript;
    switchTab("translate");
    translateInput.focus();
    translateInput.setSelectionRange(translateInput.value.length, translateInput.value.length);
    setRecordingStateKey("main.status.transcription_ready");
    setStatusKey("main.status.audio_transcribed");
  } catch (error) {
    const msg = formatError(error);
    setRecordingState(msg, true);
    setStatus(msg, true);
  } finally {
    isTranscribingAudio = false;
    setRecordingButtons(false);
  }
}

async function startAudioRecording() {
  if (isTranscribingAudio) {
    return;
  }
  if (isRecordingAudioStarting || isRecordingAudio || (mediaRecorder && mediaRecorder.state === "recording")) {
    return;
  }

  if (!navigator.mediaDevices || typeof navigator.mediaDevices.getUserMedia !== "function") {
    const msg = formatError("Microphone is not available");
    setRecordingState(msg, true);
    setStatus(msg, true);
    return;
  }

  if (typeof MediaRecorder === "undefined") {
    const msg = formatError("MediaRecorder not supported");
    setRecordingState(msg, true);
    setStatus(msg, true);
    return;
  }

  try {
    const startNonce = ++recordingStartNonce;
    isRecordingAudioStarting = true;
    recordAudioBtn.disabled = true;
    stopAudioBtn.disabled = true;
    mediaStream = await navigator.mediaDevices.getUserMedia({ audio: true });
    if (startNonce !== recordingStartNonce || document.hidden) {
      releaseMediaStream();
      setRecordingButtons(false);
      return;
    }
    isRecordingAudio = true;
    isRecordingAudioStarting = false;
    startRecordingVisualizer(mediaStream);
    mediaChunks = [];

    const preferredMime = pickRecorderMimeType();
    const options = preferredMime ? { mimeType: preferredMime } : undefined;
    mediaRecorder = new MediaRecorder(mediaStream, options);
    const recorder = mediaRecorder;
    let recorderErrored = false;

    recorder.addEventListener("dataavailable", (event) => {
      if (event.data && event.data.size > 0) {
        mediaChunks.push(event.data);
      }
    });

    recorder.addEventListener("error", () => {
      recorderErrored = true;
      isRecordingAudio = false;
      isRecordingAudioStarting = false;
      const msg = formatError("Audio recording failed");
      setRecordingState(msg, true);
      setStatus(msg, true);
      if (recorder.state !== "inactive") {
        try {
          recorder.stop();
        } catch (_) {
          releaseMediaStream();
          mediaRecorder = null;
          setRecordingButtons(false);
        }
      } else {
        releaseMediaStream();
        mediaRecorder = null;
        setRecordingButtons(false);
      }
    });

    recorder.addEventListener("stop", async () => {
      const mimeType = recorder.mimeType || preferredMime || "audio/webm";
      const blob = new Blob(mediaChunks, { type: mimeType });
      mediaChunks = [];
      isRecordingAudio = false;
      isRecordingAudioStarting = false;
      mediaRecorder = null;
      releaseMediaStream();
      setRecordingButtons(false);
      if (recorderErrored) {
        return;
      }
      await transcribeRecordedBlob(blob);
    });

    recorder.start(250);
    setRecordingButtons(true);
    setRecordingState("");
    setStatus("");
  } catch (error) {
    isRecordingAudio = false;
    isRecordingAudioStarting = false;
    mediaRecorder = null;
    releaseMediaStream();
    setRecordingButtons(false);
    const msg = formatError(error);
    setRecordingState(msg, true);
    setStatus(msg, true);
  }
}

function stopAudioRecording() {
  recordingStartNonce += 1;
  isRecordingAudioStarting = false;
  if (!mediaRecorder || mediaRecorder.state !== "recording") {
    releaseMediaStream();
    return;
  }

  stopAudioBtn.disabled = true;
  setRecordingState("");
  mediaRecorder.stop();
}

function handleMainVisibilityChange() {
  if (!document.hidden) {
    return;
  }
  if (mediaStream || isRecordingAudio || isRecordingAudioStarting || (mediaRecorder && mediaRecorder.state === "recording")) {
    stopAudioRecording();
  }
}

async function handleImprove() {
  const input = improveInput.value.trim();
  if (!input) {
    setStatusKey("main.status.write_english_first", true);
    return;
  }

  improveBtn.disabled = true;
  setStatusKey("main.status.improving");
  try {
    const output = await invoke("improve_text", {
      input,
      style: improveStyle.value,
    });
    improveOutput.value = output;
    setStatusKey("main.status.text_improved");
  } catch (error) {
    setStatus(formatError(error), true);
  } finally {
    improveBtn.disabled = false;
  }
}

async function handleTranslate() {
  const input = translateInput.value.trim();
  if (!input) {
    setStatusKey("main.status.write_source_first", true);
    return;
  }

  translateBtn.disabled = true;
  setStatusKey("main.status.translating");
  try {
    const output = await invoke("translate_text", {
      input,
      style: translateStyle.value,
    });
    translateOutput.value = output;
    setStatusKey("main.status.text_translated");
  } catch (error) {
    setStatus(formatError(error), true);
  } finally {
    translateBtn.disabled = false;
  }
}

async function copyText(text) {
  if (!text.trim()) {
    setStatusKey("main.status.nothing_to_copy", true);
    return;
  }
  await navigator.clipboard.writeText(text);
  setStatusKey("main.status.copied");
}

async function insertTextAtCursor(text) {
  const value = text.trim();
  if (!value) {
    setStatusKey("main.status.nothing_to_insert", true);
    return;
  }

  setStatusKey("main.status.copying_inserting");
  try {
    const result = await invoke("auto_insert_text", { text: value });
    if (result && result.pasted) {
      setStatusKey("main.status.inserted");
      return;
    }

    setStatusKey("main.status.paste_failed", true, { shortcut: pasteShortcutHint() });
  } catch (error) {
    setStatusKey("main.status.insert_failed", true, { shortcut: pasteShortcutHint() });
  }
}

async function openSettingsWindow() {
  try {
    await invoke("open_settings_window");
  } catch (error) {
    setStatus(formatError(error), true);
  }
}

async function openWidgetWindow() {
  try {
    await invoke("open_widget_window");
  } catch (error) {
    setStatus(formatError(error), true);
  }
}

tabs.forEach((tab) => {
  tab.addEventListener("click", () => switchTab(tab.dataset.tab));
});

const tablist = document.querySelector(".tabs");
if (tablist) {
  tablist.addEventListener("keydown", (e) => {
    const tabEls = Array.from(tablist.querySelectorAll('[role="tab"]'));
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
      switchTab(nextTab.dataset.tab);
    }
  });
}

openSettingsBtn.addEventListener("click", openSettingsWindow);
openWidgetBtn.addEventListener("click", openWidgetWindow);
improveBtn.addEventListener("click", handleImprove);
translateBtn.addEventListener("click", handleTranslate);
insertImproveBtn.addEventListener("click", () => insertTextAtCursor(improveOutput.value));
insertTranslateBtn.addEventListener("click", () => insertTextAtCursor(translateOutput.value));
recordAudioBtn.addEventListener("click", startAudioRecording);
stopAudioBtn.addEventListener("click", stopAudioRecording);
copyImproveBtn.addEventListener("click", () => copyText(improveOutput.value));
copyTranslateBtn.addEventListener("click", () => copyText(translateOutput.value));
onboardingMicBtn.addEventListener("click", requestOnboardingMicrophonePermission);
onboardingMicSettingsBtn.addEventListener("click", () => openPermissionSettings("microphone"));
onboardingAccessibilityBtn.addEventListener("click", testOnboardingAccessibilityPermission);
onboardingAccessibilitySettingsBtn.addEventListener("click", () => openPermissionSettings("accessibility"));
onboardingFinishBtn.addEventListener("click", finishOnboarding);
onboardingSkipBtn.addEventListener("click", skipOnboardingForSession);
document.addEventListener("visibilitychange", handleMainVisibilityChange);
window.addEventListener("beforeunload", releaseMediaStream);

async function bootstrap() {
  await loadUiSettings();
  setRecordingButtons(false);
  stopRecordingVisualizer();
  setRecordingStateKey("main.recording.state.idle");
  try {
    try {
      const promptSettings = await invoke("get_prompt_settings");
      const defaultMode = normalizeMode(promptSettings?.quickMode);
      improveStyle.value = defaultMode;
      translateStyle.value = defaultMode;
    } catch (_) {
      // keep UI defaults
    }

    await listen("ui-language-changed", (event) => {
      currentUiLanguagePreference = String(event?.payload?.uiLanguagePreference || "system");
      setLanguagePreference(currentUiLanguagePreference);
      applyMainTranslations();
    });

    await listen("external-improve-text", (event) => {
      const payload = event.payload;
      if (typeof payload === "string") {
        applyExternalImproveText(payload, "context action");
        return;
      }
      if (payload && typeof payload.text === "string") {
        applyExternalImproveText(payload.text, payload.source || "context action");
      }
    });

    const pendingText = await invoke("consume_pending_improve_text");
    if (pendingText) {
      applyExternalImproveText(pendingText, "launch args");
    }

    await listen("hotkey-triggered", (event) => {
      const payload = event.payload;
      const action = payload && typeof payload === "object" ? payload.action : String(payload || "");

      if (action === "open-improve") {
        switchTab("improve");
        improveInput.focus();
        return;
      }

      if (action === "open-dictate-translate") {
        switchTab("translate");
        translateInput.focus();
        return;
      }

      if (action === "open-dictate-translate-record") {
        switchTab("translate");
        translateInput.focus();
        setTimeout(() => {
          startAudioRecording();
        }, 140);
      }
    });

    const onboardingStatus = await invoke("get_onboarding_status");
    applyOnboardingStatus(onboardingStatus);
    setStatusKey("main.status.ready");
  } catch (error) {
    setStatusKey("main.status.startup_error", true, { error: formatError(error) });
  }
}

bootstrap();
