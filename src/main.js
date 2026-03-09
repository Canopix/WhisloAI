const { invoke } = window.__TAURI__.core;
const { listen } = window.__TAURI__.event;

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
let onboardingDismissedForSession = false;
let onboardingAutoPromptTriggered = false;
let recordingAudioContext = null;
let recordingAnalyser = null;
let recordingDataArray = null;
let recordingFrameHandle = null;
let recordingBars = [];

const RECORDING_WAVE_BARS = 24;
const SUPPORTED_MODES = ["simple", "professional", "friendly", "casual", "formal"];

function formatError(error) {
  return typeof specErrorFor === "function" ? specErrorFor(error) : String(error || "").replace(/^Error: /, "");
}

function setStatus(message, isError = false) {
  statusEl.textContent = message;
  statusEl.dataset.tone = isError ? "error" : "neutral";
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

function setRecordingState(message, isError = false) {
  recordingStateEl.textContent = message;
  recordingStateEl.style.color = isError ? "#b91c1c" : "";
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

function setOnboardingVisible(visible) {
  onboardingPanel.hidden = !visible;

  const mainContent = document.getElementById("main-content");
  if (mainContent) {
    mainContent.hidden = visible;
  }
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
    setOnboardingStatusMessage(
      onboardingMicStatus,
      "Microphone permission request is not available in this runtime.",
      true,
    );
    return;
  }

  onboardingMicBtn.disabled = true;
  setOnboardingStatusMessage(onboardingMicStatus, "Requesting microphone access...");

  try {
    const stream = await navigator.mediaDevices.getUserMedia({ audio: true });
    stream.getTracks().forEach((track) => track.stop());
    setOnboardingStatusMessage(onboardingMicStatus, "Microphone access granted.");
  } catch (error) {
    setOnboardingStatusMessage(
      onboardingMicStatus,
      "Microphone access denied. Open system settings and allow access.",
      true,
    );
    setStatus(formatError(error), true);
  } finally {
    onboardingMicBtn.disabled = false;
  }
}

async function testOnboardingAccessibilityPermission() {
  onboardingAccessibilityBtn.disabled = true;
  setOnboardingStatusMessage(onboardingAccessibilityStatus, "Testing accessibility permission...");

  try {
    await invoke("probe_auto_insert_permission");
    setOnboardingStatusMessage(onboardingAccessibilityStatus, "Accessibility access looks ready.");
  } catch (error) {
    setOnboardingStatusMessage(
      onboardingAccessibilityStatus,
      "Accessibility access missing. Open system settings and allow BestText.",
      true,
    );
    setStatus(formatError(error), true);
  } finally {
    onboardingAccessibilityBtn.disabled = false;
  }
}

async function finishOnboarding() {
  try {
    await invoke("complete_onboarding");
    setOnboardingVisible(false);
    setStatus("Setup completed.");
  } catch (error) {
    setStatus(formatError(error), true);
  }
}

function skipOnboardingForSession() {
  onboardingDismissedForSession = true;
  setOnboardingVisible(false);
  setStatus("Setup skipped for now.");
}

function applyOnboardingStatus(status) {
  const needsAccessibility = Boolean(status && status.needsAccessibility);
  onboardingAccessibilityStep.hidden = !needsAccessibility;

  setOnboardingStatusMessage(onboardingMicStatus, "Not checked yet.");
  setOnboardingStatusMessage(onboardingAccessibilityStatus, "Not checked yet.");

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
  setStatus(`Text imported from ${source}.`);
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
    setRecordingState("No audio captured. Try recording again.", true);
    setStatus("No audio captured.", true);
    return;
  }

  isTranscribingAudio = true;
  setRecordingButtons(false);
  setRecordingState("Transcribing...");
  setStatus("Transcribing audio...");

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
    setRecordingState("Transcription ready. You can edit and translate.");
    setStatus("Audio transcribed.");
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
    mediaStream = await navigator.mediaDevices.getUserMedia({ audio: true });
    startRecordingVisualizer(mediaStream);
    mediaChunks = [];

    const preferredMime = pickRecorderMimeType();
    const options = preferredMime ? { mimeType: preferredMime } : undefined;
    mediaRecorder = new MediaRecorder(mediaStream, options);

    mediaRecorder.addEventListener("dataavailable", (event) => {
      if (event.data && event.data.size > 0) {
        mediaChunks.push(event.data);
      }
    });

    mediaRecorder.addEventListener("stop", async () => {
      const mimeType = mediaRecorder ? mediaRecorder.mimeType : preferredMime || "audio/webm";
      const blob = new Blob(mediaChunks, { type: mimeType });
      mediaRecorder = null;
      releaseMediaStream();
      setRecordingButtons(false);
      await transcribeRecordedBlob(blob);
    });

    mediaRecorder.start(250);
    setRecordingButtons(true);
    setRecordingState("Recording... Click stop when finished.");
    setStatus("Recording from microphone...");
  } catch (error) {
    mediaRecorder = null;
    releaseMediaStream();
    setRecordingButtons(false);
    const msg = formatError(error);
    setRecordingState(msg, true);
    setStatus(msg, true);
  }
}

function stopAudioRecording() {
  if (!mediaRecorder || mediaRecorder.state !== "recording") {
    releaseMediaStream();
    return;
  }

  stopAudioBtn.disabled = true;
  setRecordingState("Stopping recording...");
  mediaRecorder.stop();
}

async function handleImprove() {
  const input = improveInput.value.trim();
  if (!input) {
    setStatus("Write some English text first.", true);
    return;
  }

  improveBtn.disabled = true;
  setStatus("Improving text...");
  try {
    const output = await invoke("improve_text", {
      input,
      style: improveStyle.value,
    });
    improveOutput.value = output;
    setStatus("Text improved.");
  } catch (error) {
    setStatus(formatError(error), true);
  } finally {
    improveBtn.disabled = false;
  }
}

async function handleTranslate() {
  const input = translateInput.value.trim();
  if (!input) {
    setStatus("Write Spanish text first.", true);
    return;
  }

  translateBtn.disabled = true;
  setStatus("Translating text...");
  try {
    const output = await invoke("translate_text", {
      input,
      style: translateStyle.value,
    });
    translateOutput.value = output;
    setStatus("Text translated.");
  } catch (error) {
    setStatus(formatError(error), true);
  } finally {
    translateBtn.disabled = false;
  }
}

async function copyText(text) {
  if (!text.trim()) {
    setStatus("Nothing to copy.", true);
    return;
  }
  await navigator.clipboard.writeText(text);
  setStatus("Copied to clipboard.");
}

async function insertTextAtCursor(text) {
  const value = text.trim();
  if (!value) {
    setStatus("Nothing to insert.", true);
    return;
  }

  setStatus("Copying and inserting...");
  try {
    const result = await invoke("auto_insert_text", { text: value });
    if (result && result.pasted) {
      setStatus("Inserted in active app.");
      return;
    }

    setStatus(
      `Automatic paste failed. Text copied to clipboard. Paste manually with ${pasteShortcutHint()}.`,
      true,
    );
  } catch (error) {
    setStatus(
      `Automatic insert failed. Text may still be copied. Paste manually with ${pasteShortcutHint()}.`,
      true,
    );
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
window.addEventListener("beforeunload", releaseMediaStream);

async function bootstrap() {
  setRecordingButtons(false);
  stopRecordingVisualizer();
  setRecordingState("Microphone idle.");
  try {
    try {
      const promptSettings = await invoke("get_prompt_settings");
      const defaultMode = normalizeMode(promptSettings?.quickMode);
      improveStyle.value = defaultMode;
      translateStyle.value = defaultMode;
    } catch (_) {
      // keep UI defaults
    }

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
    setStatus("Ready.");
  } catch (error) {
    setStatus(`Error al iniciar: ${formatError(error)}`, true);
  }
}

bootstrap();
