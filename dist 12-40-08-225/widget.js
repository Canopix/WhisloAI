const { invoke } = window.__TAURI__.core;
const { listen } = window.__TAURI__.event;

const quickShell = document.getElementById("quick-shell");
const quickToolbar = document.querySelector(".quick-toolbar");
const translateSelectionBtn = document.getElementById("quick-translate-selection-btn");
const improveSelectionBtn = document.getElementById("quick-improve-selection-btn");
const dictateBtn = document.getElementById("quick-dictate-btn");
const dictateIconEl = document.getElementById("quick-dictate-icon");
const quickDictateWaveWrapEl = document.getElementById("quick-dictate-wave-wrap");
const settingsBtn = document.getElementById("quick-settings-btn");
const statusEl = document.getElementById("quick-status");
const quickRecordingWaveformEl = document.getElementById("quick-recording-waveform");

let mediaRecorder = null;
let recordedChunks = [];
let activeStream = null;
let isRecording = false;
let isBusy = false;
let recordingAudioContext = null;
let recordingAnalyser = null;
let recordingDataArray = null;
let recordingFrameHandle = null;
let recordingBars = [];
let lastExpandedState = null;
let quickMode = "simple";

const QUICK_WAVE_BAR_COUNT = 20;
const SUPPORTED_MODES = ["simple", "professional", "friendly", "casual", "formal"];

function normalizeError(error) {
  const text = String(error || "Unknown error.");
  if (text.startsWith("Error: ")) {
    return text.slice(7);
  }
  return text;
}

function normalizeMode(mode) {
  const value = String(mode || "")
    .trim()
    .toLowerCase();
  return SUPPORTED_MODES.includes(value) ? value : "simple";
}

function renderLucideIcons() {
  if (!window.lucide || typeof window.lucide.createIcons !== "function") {
    return;
  }
  window.lucide.createIcons({
    attrs: {
      width: "16",
      height: "16",
      "stroke-width": "2",
    },
  });
}

function setDictateGlyph(icon) {
  dictateIconEl.innerHTML = `<i class="icon-lucide" data-lucide="${icon}" aria-hidden="true"></i>`;
  renderLucideIcons();
}

function isMacOS() {
  const platform = (navigator.userAgentData && navigator.userAgentData.platform) || navigator.platform || "";
  return /mac/i.test(platform);
}

function pasteHint() {
  return `${isMacOS() ? "Cmd" : "Ctrl"} + V`;
}

function syncQuickWindowLayout(force = false) {
  const expanded = !statusEl.hidden;
  if (!force && expanded === lastExpandedState) {
    return;
  }
  lastExpandedState = expanded;
  invoke("set_quick_window_expanded", { expanded }).catch(() => {
    // ignore resize failures
  });
}

function setStatus(message, state = "neutral") {
  statusEl.textContent = message;
  statusEl.dataset.state = state;

  const visible = state === "error" || state === "warning";
  statusEl.hidden = !visible;
  syncQuickWindowLayout();
}

function setRecordingVisualizerVisible(visible) {
  quickDictateWaveWrapEl.hidden = !visible;
  quickToolbar.classList.toggle("is-recording", visible);
  syncQuickWindowLayout();
}

function ensureRecordingBars() {
  if (!quickRecordingWaveformEl || recordingBars.length) {
    return;
  }

  const fragment = document.createDocumentFragment();
  for (let index = 0; index < QUICK_WAVE_BAR_COUNT; index += 1) {
    const bar = document.createElement("span");
    bar.className = "wave-bar";
    bar.style.setProperty("--bar-scale", "0.14");
    bar.style.setProperty("--wave-delay", `${(index * 0.04).toFixed(2)}s`);
    fragment.appendChild(bar);
    recordingBars.push(bar);
  }
  quickRecordingWaveformEl.appendChild(fragment);
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

  quickRecordingWaveformEl.classList.remove("is-fallback");
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
  stopRecordingVisualizer();
  ensureRecordingBars();
  setRecordingVisualizerVisible(true);

  const AudioCtx = window.AudioContext || window.webkitAudioContext;
  if (!AudioCtx) {
    quickRecordingWaveformEl.classList.add("is-fallback");
    return;
  }

  try {
    quickRecordingWaveformEl.classList.remove("is-fallback");
    recordingAudioContext = new AudioCtx();
    const source = recordingAudioContext.createMediaStreamSource(stream);
    recordingAnalyser = recordingAudioContext.createAnalyser();
    recordingAnalyser.fftSize = 256;
    recordingAnalyser.smoothingTimeConstant = 0.82;
    source.connect(recordingAnalyser);
    recordingDataArray = new Uint8Array(recordingAnalyser.frequencyBinCount);
    renderRecordingWaveform();
  } catch (_) {
    quickRecordingWaveformEl.classList.add("is-fallback");
  }
}

function setBusy(busy) {
  isBusy = busy;
  quickShell.classList.toggle("is-busy", busy);
  translateSelectionBtn.disabled = busy || isRecording;
  improveSelectionBtn.disabled = busy || isRecording;
  settingsBtn.disabled = busy || isRecording;
  dictateBtn.disabled = busy && !isRecording;
}

function updateDictateButton() {
  setDictateGlyph(isRecording ? "square" : "mic");
  dictateBtn.classList.toggle("is-recording", isRecording);
  dictateBtn.setAttribute("aria-label", isRecording ? "Stop dictation" : "Start dictation");
  dictateBtn.title = isRecording ? "Stop dictation" : "Start dictation";
  dictateBtn.disabled = isBusy && !isRecording;
}

function blobToBase64(blob) {
  return new Promise((resolve, reject) => {
    const reader = new FileReader();
    reader.onload = () => {
      const result = reader.result || "";
      const commaIndex = result.indexOf(",");
      if (commaIndex === -1) {
        reject(new Error("Could not encode audio."));
        return;
      }
      resolve(result.slice(commaIndex + 1));
    };
    reader.onerror = () => reject(new Error("Could not read recorded audio."));
    reader.readAsDataURL(blob);
  });
}

function stopAndReleaseStream() {
  if (activeStream) {
    activeStream.getTracks().forEach((track) => track.stop());
  }
  activeStream = null;
  stopRecordingVisualizer();
}

async function insertResultText(output) {
  const result = await invoke("auto_insert_text", { text: output });
  if (result && result.pasted) {
    setStatus("Inserted in the active field.", "success");
  } else {
    try {
      await invoke("open_quick_window");
    } catch (_) {
      // ignore
    }
    setStatus(`Copied. Paste manually with ${pasteHint()}.`, "warning");
  }
}

async function runSelectionAction(mode) {
  if (isBusy || isRecording) {
    return;
  }

  await loadQuickMode();
  setBusy(true);
  setStatus("Capturing selected text...", "loading");

  try {
    const input = await invoke("capture_selected_text");
    if (!input || !String(input).trim()) {
      throw new Error("No selected text detected.");
    }

    setStatus(mode === "improve" ? "Improving selected text..." : "Translating selected text...", "loading");
    const output =
      mode === "improve"
        ? await invoke("improve_text", { input, style: quickMode })
        : await invoke("translate_text", { input, style: quickMode });

    await insertResultText(output);
    setStatus("Ready.", "neutral");
  } catch (error) {
    try {
      await invoke("open_quick_window");
    } catch (_) {
      // ignore
    }
    setStatus(normalizeError(error), "error");
  } finally {
    setBusy(false);
    updateDictateButton();
  }
}

async function startDictation() {
  if (isBusy || isRecording) {
    return;
  }

  try {
    setBusy(true);
    setStatus("Preparing microphone...", "loading");

    activeStream = await navigator.mediaDevices.getUserMedia({ audio: true });
    mediaRecorder = new MediaRecorder(activeStream);
    recordedChunks = [];

    startRecordingVisualizer(activeStream);
    isRecording = true;
    setBusy(false);
    updateDictateButton();
    setStatus("Recording...", "neutral");

    mediaRecorder.ondataavailable = (event) => {
      if (event.data && event.data.size > 0) {
        recordedChunks.push(event.data);
      }
    };

    mediaRecorder.onerror = () => {
      setStatus("Recording failed. Check microphone permissions.", "error");
    };

    mediaRecorder.onstop = async () => {
      const blob = new Blob(recordedChunks, { type: mediaRecorder.mimeType || "audio/webm" });
      recordedChunks = [];
      mediaRecorder = null;
      stopAndReleaseStream();
      isRecording = false;
      updateDictateButton();

      if (!blob.size) {
        setStatus("No audio captured. Try again.", "error");
        return;
      }

      setBusy(true);
      updateDictateButton();
      try {
        setStatus("Transcribing...", "loading");
        const audioBase64 = await blobToBase64(blob);
        const transcript = await invoke("transcribe_audio", {
          audioBase64,
          mimeType: blob.type || undefined,
        });

        await loadQuickMode();
        setStatus("Translating transcript...", "loading");
        const output = await invoke("translate_text", {
          input: transcript,
          style: quickMode,
        });

        await insertResultText(output);
        setStatus("Ready.", "neutral");
      } catch (error) {
        setStatus(normalizeError(error), "error");
      } finally {
        setBusy(false);
        updateDictateButton();
      }
    };

    mediaRecorder.start();
  } catch (error) {
    isRecording = false;
    updateDictateButton();
    stopAndReleaseStream();
    setBusy(false);
    setStatus(normalizeError(error), "error");
  }
}

function stopDictation() {
  if (!mediaRecorder || mediaRecorder.state !== "recording") {
    isRecording = false;
    updateDictateButton();
    stopAndReleaseStream();
    setStatus("Ready.", "neutral");
    return;
  }
  setStatus("Stopping...", "loading");
  mediaRecorder.stop();
}

async function openSettings() {
  if (isBusy || isRecording) {
    return;
  }
  try {
    await invoke("open_settings_window");
    await invoke("close_quick_window");
  } catch (error) {
    setStatus(normalizeError(error), "error");
  }
}

async function loadQuickMode() {
  try {
    const settings = await invoke("get_prompt_settings");
    quickMode = normalizeMode(settings?.quickMode);
  } catch (_) {
    quickMode = "simple";
  }
}

async function closeQuick() {
  if (isRecording) {
    stopDictation();
  }
  try {
    await invoke("close_quick_window");
  } catch (_) {
    // no-op
  }
}

function runQuickAction(action) {
  if (!action) {
    return;
  }
  if (action === "open-improve") {
    runSelectionAction("improve");
    return;
  }
  if (action === "open-dictate-translate" || action === "open-dictate-translate-record") {
    if (isRecording) {
      stopDictation();
    } else {
      startDictation();
    }
    return;
  }
  if (action === "open-app") {
    setStatus("Ready.", "neutral");
    improveSelectionBtn.focus();
  }
}

translateSelectionBtn.addEventListener("click", () => runSelectionAction("translate"));
improveSelectionBtn.addEventListener("click", () => runSelectionAction("improve"));
dictateBtn.addEventListener("click", () => {
  if (isRecording) {
    stopDictation();
  } else {
    startDictation();
  }
});
settingsBtn.addEventListener("click", openSettings);

document.addEventListener("keydown", (event) => {
  if (event.key === "Escape") {
    closeQuick();
  }
});

window.addEventListener("beforeunload", stopAndReleaseStream);

async function bootstrap() {
  await loadQuickMode();
  renderLucideIcons();
  stopRecordingVisualizer();
  setBusy(false);
  updateDictateButton();
  setStatus("Ready.", "neutral");
  syncQuickWindowLayout(true);

  try {
    await listen("quick-action", (event) => {
      runQuickAction(event?.payload?.action);
    });
    const pending = await invoke("consume_pending_quick_action");
    runQuickAction(pending || "open-app");
  } catch (_) {
    improveSelectionBtn.focus();
  }
}

bootstrap();
