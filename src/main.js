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

const statusEl = document.getElementById("status");
const openSettingsBtn = document.getElementById("open-settings-btn");
const openWidgetBtn = document.getElementById("open-widget-btn");

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
const onboardingWelcomeStep = document.getElementById("onboarding-welcome-step");
const onboardingGetStartedBtn = document.getElementById("onboarding-get-started-btn");
const onboardingBackBtn = document.getElementById("onboarding-back-btn");
const onboardingNextBtn = document.getElementById("onboarding-next-btn");
const onboardingProgressDots = document.querySelectorAll(".onboarding-progress-dot");
const onboardingFinishHint = document.getElementById("onboarding-finish-hint");
const onboardingMicBtn = document.getElementById("onboarding-mic-btn");
const onboardingMicSettingsBtn = document.getElementById("onboarding-mic-settings-btn");
const onboardingMicStatus = document.getElementById("onboarding-mic-status");
const onboardingMicStep = document.getElementById("onboarding-mic-step");
const onboardingMicStepState = document.getElementById("onboarding-mic-step-state");
const onboardingAccessibilityStep = document.getElementById("onboarding-accessibility-step");
const onboardingAccessibilityBtn = document.getElementById("onboarding-accessibility-btn");
const onboardingAccessibilitySettingsBtn = document.getElementById("onboarding-accessibility-settings-btn");
const onboardingAccessibilityStatus = document.getElementById("onboarding-accessibility-status");
const onboardingAccessibilityStepState = document.getElementById("onboarding-accessibility-step-state");
const onboardingAutomationStep = document.getElementById("onboarding-automation-step");
const onboardingAutomationBtn = document.getElementById("onboarding-automation-btn");
const onboardingAutomationSettingsBtn = document.getElementById("onboarding-automation-settings-btn");
const onboardingAutomationStatus = document.getElementById("onboarding-automation-status");
const onboardingAutomationStepState = document.getElementById("onboarding-automation-step-state");
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
let currentStatusState = { type: "key", key: "main.status.ready", params: null, tone: "neutral" };
let currentRecordingState = {
  type: "key",
  key: "main.recording.state.idle",
  params: null,
  tone: "neutral",
};
let onboardingCurrentStep = 0;
let onboardingVisibleSteps = [];
let onboardingNeedsAccessibility = false;
let onboardingNeedsAutomation = false;

const RECORDING_WAVE_BARS = 24;
const SUPPORTED_MODES = ["simple", "professional", "friendly", "casual", "formal"];
const STATUS_TONES = new Set(["neutral", "loading", "success", "error"]);
const ONBOARDING_STEP_I18N_KEY = {
  pending: "main.onboarding.step.pending",
  checking: "main.onboarding.step.checking",
  ready: "main.onboarding.step.ready",
  action_required: "main.onboarding.step.action_required",
};

function formatError(error) {
  return typeof specErrorFor === "function" ? specErrorFor(error) : String(error || "").replace(/^Error: /, "");
}

function resolveTone(value) {
  if (typeof value === "string" && STATUS_TONES.has(value)) {
    return value;
  }
  return value ? "error" : "neutral";
}

function applyStatusMessage(message, tone = "neutral", state = null) {
  const resolvedTone = resolveTone(tone);
  statusEl.textContent = message;
  statusEl.dataset.tone = resolvedTone;
  currentStatusState = state || { type: "text", message, tone: resolvedTone };
}

function setStatus(message, tone = "neutral") {
  const resolvedTone = resolveTone(tone);
  applyStatusMessage(message, resolvedTone, { type: "text", message, tone: resolvedTone });
}

function setStatusKey(key, tone = "neutral", params = null) {
  const resolvedTone = resolveTone(tone);
  applyStatusMessage(t(key, params), resolvedTone, { type: "key", key, params, tone: resolvedTone });
}

function refreshStatusTranslation() {
  if (currentStatusState.type === "key") {
    applyStatusMessage(t(currentStatusState.key, currentStatusState.params), currentStatusState.tone, {
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
  if (tabName !== "translate") {
    return;
  }
  const panel = document.querySelector('.panel[data-panel="translate"]');
  if (panel) {
    panel.classList.add("is-active");
  }
}

function applyRecordingState(message, tone = "neutral", state = null) {
  const resolvedTone = resolveTone(tone);
  recordingStateEl.textContent = message;
  recordingStateEl.dataset.tone = resolvedTone;
  recordingStateEl.hidden = !message;
  currentRecordingState = state || { type: "text", message, tone: resolvedTone };
}

function setRecordingState(message, tone = "neutral") {
  const resolvedTone = resolveTone(tone);
  applyRecordingState(message, resolvedTone, { type: "text", message, tone: resolvedTone });
}

function setRecordingStateKey(key, tone = "neutral", params = null) {
  const resolvedTone = resolveTone(tone);
  applyRecordingState(t(key, params), resolvedTone, { type: "key", key, params, tone: resolvedTone });
}

function refreshRecordingStateTranslation() {
  if (currentRecordingState.type === "key") {
    applyRecordingState(t(currentRecordingState.key, currentRecordingState.params), currentRecordingState.tone, {
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

function setOnboardingStatusMessage(target, message, tone = "neutral") {
  const resolvedTone = resolveTone(tone);
  target.textContent = message;
  target.dataset.tone = resolvedTone;
}

function setOnboardingStatusKey(target, key, tone = "neutral", params = null) {
  setOnboardingStatusMessage(target, t(key, params), tone);
}

function setOnboardingStepState(stepEl, stateEl, state) {
  if (!stepEl || !stateEl) {
    return;
  }
  const normalized = ONBOARDING_STEP_I18N_KEY[state] ? state : "pending";
  const i18nKey = ONBOARDING_STEP_I18N_KEY[normalized];
  stepEl.dataset.state = normalized;
  stateEl.dataset.state = normalized;
  stateEl.dataset.i18n = i18nKey;
  stateEl.textContent = t(i18nKey);
}

function refreshOnboardingStepStateTranslations() {
  const states = [
    [onboardingMicStep, onboardingMicStepState],
    [onboardingAccessibilityStep, onboardingAccessibilityStepState],
    [onboardingAutomationStep, onboardingAutomationStepState],
  ];
  states.forEach(([stepEl, stateEl]) => {
    if (!stepEl || !stateEl) {
      return;
    }
    const normalized = ONBOARDING_STEP_I18N_KEY[stepEl.dataset.state] ? stepEl.dataset.state : "pending";
    setOnboardingStepState(stepEl, stateEl, normalized);
  });
}

function setOnboardingVisible(visible) {
  onboardingPanel.hidden = !visible;

  const mainContent = document.getElementById("main-content");
  if (mainContent) {
    mainContent.hidden = visible;
  }
}

function buildOnboardingVisibleSteps() {
  const steps = ["mic"];
  if (onboardingNeedsAccessibility) {
    steps.push("accessibility");
  }
  if (onboardingNeedsAutomation) {
    steps.push("automation");
  }
  return steps;
}

function showOnboardingStep(stepIndex) {
  const allSteps = [
    onboardingWelcomeStep,
    onboardingMicStep,
    onboardingAccessibilityStep,
    onboardingAutomationStep,
  ];

  allSteps.forEach((step, index) => {
    if (!step) return;
    const isWelcome = index === 0;
    const isPermissionStep = !isWelcome;

    if (isPermissionStep) {
      const stepName = index === 1 ? "mic" : index === 2 ? "accessibility" : "automation";
      const shouldShow = stepName === "mic" || 
        (stepName === "accessibility" && onboardingNeedsAccessibility) ||
        (stepName === "automation" && onboardingNeedsAutomation);
      
      if (!shouldShow) {
        step.hidden = true;
        step.classList.remove("is-active");
        return;
      }
    }

    if (index === stepIndex) {
      step.hidden = false;
      step.classList.add("is-active");
    } else {
      step.hidden = true;
      step.classList.remove("is-active");
    }
  });

  onboardingCurrentStep = stepIndex;
  updateOnboardingNav();
  updateOnboardingProgressDots();
}

function updateOnboardingNav() {
  const visibleSteps = buildOnboardingVisibleSteps();
  const totalSteps = visibleSteps.length;
  const currentPermissionStep = onboardingCurrentStep - 1;
  const isWelcomeStep = onboardingCurrentStep === 0;
  const isLastPermissionStep = currentPermissionStep >= 0 && currentPermissionStep === totalSteps - 1;

  if (onboardingBackBtn) {
    onboardingBackBtn.hidden = isWelcomeStep;
  }

  if (onboardingNextBtn) {
    onboardingNextBtn.hidden = isWelcomeStep || isLastPermissionStep;
  }

  if (onboardingFinishBtn) {
    onboardingFinishBtn.hidden = !isLastPermissionStep;
    updateFinishButtonState();
  }

  if (onboardingSkipBtn) {
    onboardingSkipBtn.hidden = !isLastPermissionStep;
  }

  if (onboardingFinishHint) {
    onboardingFinishHint.hidden = !isLastPermissionStep || areAllPermissionsReady();
  }
}

function updateOnboardingProgressDots() {
  const visibleSteps = buildOnboardingVisibleSteps();
  const currentPermissionStep = onboardingCurrentStep - 1;

  onboardingProgressDots.forEach((dot, index) => {
    const stepName = visibleSteps[index];
    if (!stepName) {
      dot.hidden = true;
      return;
    }
    dot.hidden = false;
    dot.setAttribute("aria-selected", index === currentPermissionStep ? "true" : "false");
    dot.classList.toggle("is-active", index === currentPermissionStep);

    const stepEl = getOnboardingStepElement(stepName);
    dot.classList.toggle("is-complete", stepEl && stepEl.dataset.state === "ready");
  });
}

function getOnboardingStepElement(stepName) {
  if (stepName === "mic") return onboardingMicStep;
  if (stepName === "accessibility") return onboardingAccessibilityStep;
  if (stepName === "automation") return onboardingAutomationStep;
  return null;
}

function advanceOnboardingStep() {
  const visibleSteps = buildOnboardingVisibleSteps();
  const nextStep = onboardingCurrentStep + 1;
  const maxStep = visibleSteps.length;

  if (nextStep <= maxStep) {
    setTimeout(() => {
      showOnboardingStep(nextStep);
    }, 300);
  }
}

function goBackOnboardingStep() {
  const prevStep = Math.max(0, onboardingCurrentStep - 1);
  showOnboardingStep(prevStep);
}

function areAllPermissionsReady() {
  const visibleSteps = buildOnboardingVisibleSteps();
  return visibleSteps.every((stepName) => {
    const stepEl = getOnboardingStepElement(stepName);
    return stepEl && stepEl.dataset.state === "ready";
  });
}

function updateFinishButtonState() {
  if (!onboardingFinishBtn) return;
  const allReady = areAllPermissionsReady();
  onboardingFinishBtn.disabled = !allReady;
  if (onboardingFinishHint) {
    onboardingFinishHint.hidden = allReady;
  }
}

function applyMainTranslations() {
  applyTranslations(document);
  refreshStatusTranslation();
  refreshRecordingStateTranslation();
  refreshOnboardingStepStateTranslations();
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
    setStatus(formatError(error), "error");
  }
}

async function requestOnboardingMicrophonePermission() {
  if (!navigator.mediaDevices || typeof navigator.mediaDevices.getUserMedia !== "function") {
    setOnboardingStatusKey(onboardingMicStatus, "main.status.recording_permission_unavailable", "error");
    setOnboardingStepState(onboardingMicStep, onboardingMicStepState, "action_required");
    return;
  }

  onboardingMicBtn.disabled = true;
  setOnboardingStatusKey(onboardingMicStatus, "main.status.requesting_microphone", "loading");
  setOnboardingStepState(onboardingMicStep, onboardingMicStepState, "checking");

  try {
    const stream = await navigator.mediaDevices.getUserMedia({ audio: true });
    stream.getTracks().forEach((track) => track.stop());
    setOnboardingStatusKey(onboardingMicStatus, "main.status.microphone_granted", "success");
    setOnboardingStepState(onboardingMicStep, onboardingMicStepState, "ready");
    if (onboardingMicStep) {
      onboardingMicStep.classList.add("is-complete");
    }
    updateOnboardingProgressDots();
    updateFinishButtonState();
    advanceOnboardingStep();
  } catch (error) {
    setOnboardingStatusKey(onboardingMicStatus, "main.status.microphone_denied", "error");
    setOnboardingStepState(onboardingMicStep, onboardingMicStepState, "action_required");
    setStatus(formatError(error), "error");
  } finally {
    onboardingMicBtn.disabled = false;
  }
}

async function testOnboardingAccessibilityPermission() {
  onboardingAccessibilityBtn.disabled = true;
  setOnboardingStatusKey(onboardingAccessibilityStatus, "main.status.testing_accessibility", "loading");
  setOnboardingStepState(onboardingAccessibilityStep, onboardingAccessibilityStepState, "checking");

  try {
    await invoke("probe_accessibility_permission");
    setOnboardingStatusKey(onboardingAccessibilityStatus, "main.status.accessibility_ready", "success");
    setOnboardingStepState(onboardingAccessibilityStep, onboardingAccessibilityStepState, "ready");
    if (onboardingAccessibilityStep) {
      onboardingAccessibilityStep.classList.add("is-complete");
    }
    updateOnboardingProgressDots();
    updateFinishButtonState();
    advanceOnboardingStep();
  } catch (error) {
    setOnboardingStatusKey(onboardingAccessibilityStatus, "main.status.accessibility_missing", "error");
    setOnboardingStepState(onboardingAccessibilityStep, onboardingAccessibilityStepState, "action_required");
    setStatus(formatError(error), "error");
  } finally {
    onboardingAccessibilityBtn.disabled = false;
  }
}

async function testOnboardingAutomationPermission() {
  onboardingAutomationBtn.disabled = true;
  setOnboardingStatusKey(onboardingAutomationStatus, "main.status.testing_automation", "loading");
  setOnboardingStepState(onboardingAutomationStep, onboardingAutomationStepState, "checking");

  try {
    await invoke("probe_system_events_permission");
    setOnboardingStatusKey(onboardingAutomationStatus, "main.status.automation_ready", "success");
    setOnboardingStepState(onboardingAutomationStep, onboardingAutomationStepState, "ready");
    if (onboardingAutomationStep) {
      onboardingAutomationStep.classList.add("is-complete");
    }
    updateOnboardingProgressDots();
    updateFinishButtonState();
    advanceOnboardingStep();
  } catch (error) {
    setOnboardingStatusKey(onboardingAutomationStatus, "main.status.automation_missing", "error");
    setOnboardingStepState(onboardingAutomationStep, onboardingAutomationStepState, "action_required");
    setStatus(formatError(error), "error");
  } finally {
    onboardingAutomationBtn.disabled = false;
  }
}

async function finishOnboarding() {
  try {
    await invoke("complete_onboarding");
    setOnboardingVisible(false);
    setStatusKey("main.status.setup_completed", "success");
  } catch (error) {
    setStatus(formatError(error), "error");
  }
}

function skipOnboardingForSession() {
  onboardingDismissedForSession = true;
  setOnboardingVisible(false);
  setStatusKey("main.status.setup_skipped", "neutral");
}

function applyOnboardingStatus(status) {
  const platform = String(status?.platform || "")
    .trim()
    .toLowerCase();
  const supportsPermissionSettings = platform === "macos" || platform === "windows";
  onboardingMicSettingsBtn.hidden = !supportsPermissionSettings;
  onboardingAccessibilitySettingsBtn.hidden = !supportsPermissionSettings;
  onboardingAutomationSettingsBtn.hidden = !supportsPermissionSettings;

  onboardingNeedsAccessibility = Boolean(status && status.needsAccessibility);
  onboardingNeedsAutomation = Boolean(status && status.needsAutomation);
  onboardingAccessibilityStep.hidden = !onboardingNeedsAccessibility;
  onboardingAutomationStep.hidden = !onboardingNeedsAutomation;

  setOnboardingStatusKey(onboardingMicStatus, "main.onboarding.status.not_checked", "neutral");
  setOnboardingStatusKey(onboardingAccessibilityStatus, "main.onboarding.status.not_checked", "neutral");
  setOnboardingStatusKey(onboardingAutomationStatus, "main.onboarding.status.not_checked", "neutral");
  setOnboardingStepState(onboardingMicStep, onboardingMicStepState, "pending");
  setOnboardingStepState(onboardingAccessibilityStep, onboardingAccessibilityStepState, "pending");
  setOnboardingStepState(onboardingAutomationStep, onboardingAutomationStepState, "pending");

  if (onboardingMicStep) onboardingMicStep.classList.remove("is-complete");
  if (onboardingAccessibilityStep) onboardingAccessibilityStep.classList.remove("is-complete");
  if (onboardingAutomationStep) onboardingAutomationStep.classList.remove("is-complete");

  const shouldShow = Boolean(status && !status.completed && !onboardingDismissedForSession);
  setOnboardingVisible(shouldShow);

  if (shouldShow) {
    onboardingCurrentStep = 0;
    showOnboardingStep(0);
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
    setRecordingStateKey("main.status.audio_empty_retry", "error");
    setStatusKey("main.status.audio_empty", "error");
    return;
  }

  isTranscribingAudio = true;
  setRecordingButtons(false);
  setRecordingStateKey("main.status.transcribing", "loading");
  setStatusKey("main.status.transcribing_audio", "loading");

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
    setRecordingStateKey("main.status.transcription_ready", "success");
    setStatusKey("main.status.audio_transcribed", "success");
  } catch (error) {
    const msg = formatError(error);
    setRecordingState(msg, "error");
    setStatus(msg, "error");
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
    setRecordingState(msg, "error");
    setStatus(msg, "error");
    return;
  }

  if (typeof MediaRecorder === "undefined") {
    const msg = formatError("MediaRecorder not supported");
    setRecordingState(msg, "error");
    setStatus(msg, "error");
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
      setRecordingState(msg, "error");
      setStatus(msg, "error");
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
    setStatus("", "neutral");
  } catch (error) {
    isRecordingAudio = false;
    isRecordingAudioStarting = false;
    mediaRecorder = null;
    releaseMediaStream();
    setRecordingButtons(false);
    const msg = formatError(error);
    setRecordingState(msg, "error");
    setStatus(msg, "error");
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

async function handleTranslate() {
  const input = translateInput.value.trim();
  if (!input) {
    setStatusKey("main.status.write_source_first", "error");
    return;
  }

  translateBtn.disabled = true;
  setStatusKey("main.status.translating", "loading");
  try {
    const output = await invoke("translate_text", {
      input,
      style: translateStyle.value,
    });
    translateOutput.value = output;
    setStatusKey("main.status.text_translated", "success");
  } catch (error) {
    setStatus(formatError(error), "error");
  } finally {
    translateBtn.disabled = false;
  }
}

async function copyText(text) {
  if (!text.trim()) {
    setStatusKey("main.status.nothing_to_copy", "error");
    return;
  }
  await navigator.clipboard.writeText(text);
  setStatusKey("main.status.copied", "success");
}

async function insertTextAtCursor(text) {
  const value = text.trim();
  if (!value) {
    setStatusKey("main.status.nothing_to_insert", "error");
    return;
  }

  setStatusKey("main.status.copying_inserting", "loading");
  try {
    const result = await invoke("auto_insert_text", { text: value });
    if (result && result.pasted) {
      setStatusKey("main.status.inserted", "success");
      return;
    }

    setStatusKey("main.status.paste_failed", "error", { shortcut: pasteShortcutHint() });
  } catch (error) {
    setStatusKey("main.status.insert_failed", "error", { shortcut: pasteShortcutHint() });
  }
}

async function openSettingsWindow() {
  try {
    await invoke("open_settings_window");
  } catch (error) {
    setStatus(formatError(error), "error");
  }
}

async function openWidgetWindow() {
  try {
    await invoke("open_widget_window");
  } catch (error) {
    setStatus(formatError(error), "error");
  }
}

openSettingsBtn.addEventListener("click", openSettingsWindow);
openWidgetBtn.addEventListener("click", openWidgetWindow);
translateBtn.addEventListener("click", handleTranslate);
insertTranslateBtn.addEventListener("click", () => insertTextAtCursor(translateOutput.value));
recordAudioBtn.addEventListener("click", startAudioRecording);
stopAudioBtn.addEventListener("click", stopAudioRecording);
copyTranslateBtn.addEventListener("click", () => copyText(translateOutput.value));
onboardingMicBtn.addEventListener("click", requestOnboardingMicrophonePermission);
onboardingMicSettingsBtn.addEventListener("click", () => openPermissionSettings("microphone"));
onboardingAccessibilityBtn.addEventListener("click", testOnboardingAccessibilityPermission);
onboardingAccessibilitySettingsBtn.addEventListener("click", () => openPermissionSettings("accessibility"));
onboardingAutomationBtn.addEventListener("click", testOnboardingAutomationPermission);
onboardingAutomationSettingsBtn.addEventListener("click", () => openPermissionSettings("automation"));
onboardingFinishBtn.addEventListener("click", finishOnboarding);
onboardingSkipBtn.addEventListener("click", (event) => {
  event.preventDefault();
  skipOnboardingForSession();
});
if (onboardingGetStartedBtn) {
  onboardingGetStartedBtn.addEventListener("click", () => {
    showOnboardingStep(1);
  });
}
if (onboardingBackBtn) {
  onboardingBackBtn.addEventListener("click", goBackOnboardingStep);
}
if (onboardingNextBtn) {
  onboardingNextBtn.addEventListener("click", () => {
    const visibleSteps = buildOnboardingVisibleSteps();
    const currentPermissionStep = onboardingCurrentStep - 1;
    if (currentPermissionStep >= 0 && currentPermissionStep < visibleSteps.length - 1) {
      showOnboardingStep(onboardingCurrentStep + 1);
    }
  });
}
onboardingProgressDots.forEach((dot, index) => {
  dot.addEventListener("click", () => {
    const targetStep = index + 1;
    if (targetStep <= buildOnboardingVisibleSteps().length) {
      showOnboardingStep(targetStep);
    }
  });
});
document.addEventListener("visibilitychange", handleMainVisibilityChange);
window.addEventListener("beforeunload", releaseMediaStream);

async function bootstrap() {
  if (window.lucide && typeof window.lucide.createIcons === "function") {
    window.lucide.createIcons();
  }
  await loadUiSettings();
  setRecordingButtons(false);
  stopRecordingVisualizer();
  setRecordingStateKey("main.recording.state.idle", "neutral");
  try {
    try {
      const promptSettings = await invoke("get_prompt_settings");
      const defaultMode = normalizeMode(promptSettings?.quickMode);
      translateStyle.value = defaultMode;
    } catch (_) {
      // keep UI defaults
    }

    await listen("ui-language-changed", (event) => {
      currentUiLanguagePreference = String(event?.payload?.uiLanguagePreference || "system");
      setLanguagePreference(currentUiLanguagePreference);
      applyMainTranslations();
    });

    await listen("hotkey-triggered", (event) => {
      const payload = event.payload;
      const action = payload && typeof payload === "object" ? payload.action : String(payload || "");

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
    setStatusKey("main.status.ready", "neutral");
  } catch (error) {
    setStatusKey("main.status.startup_error", "error", { error: formatError(error) });
  }
}

bootstrap();
