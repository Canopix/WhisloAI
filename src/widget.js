const { invoke } = window.__TAURI__.core;
const { listen } = window.__TAURI__.event;

const quickShell = document.getElementById("quick-shell");
const quickToolbar = document.querySelector(".quick-toolbar");
const translateSelectionBtn = document.getElementById("quick-translate-selection-btn");
const improveSelectionBtn = document.getElementById("quick-improve-selection-btn");
const dictateBtn = document.getElementById("quick-dictate-btn");
const dictateIconEl = document.getElementById("quick-dictate-icon");
const dictateSpinnerEl = document.getElementById("quick-dictate-spinner");
const quickDictateWaveWrapEl = document.getElementById("quick-dictate-wave-wrap");
const settingsBtn = document.getElementById("quick-settings-btn");
const closeBtn = document.getElementById("quick-close-btn");
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
let recordingAborted = false;

const QUICK_WAVE_BAR_COUNT = 20;
const SUPPORTED_MODES = ["simple", "professional", "friendly", "casual", "formal"];

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

function syncQuickWindowLayout(force = false, expanded = true) {
  if (!force && lastExpandedState === expanded) {
    return;
  }
  lastExpandedState = expanded;
  invoke("set_quick_window_expanded", { expanded }).catch(() => {});
}

function setCloseIcon(icon) {
  const i = closeBtn.querySelector("i.icon-lucide");
  if (i) i.setAttribute("data-lucide", icon);
  renderLucideIcons();
}

function setRecordingVisualizerVisible(visible) {
  if (dictateSpinnerEl) dictateSpinnerEl.hidden = true;
  if (dictateIconEl) dictateIconEl.hidden = visible;
  if (quickDictateWaveWrapEl) quickDictateWaveWrapEl.hidden = !visible;
  setCloseIcon(visible ? "trash-2" : "x");
  closeBtn.title = visible ? "Discard recording" : "Close widget (Escape)";
  closeBtn.setAttribute("aria-label", visible ? "Discard recording" : "Close widget");
  quickToolbar.classList.toggle("is-recording", visible);
  syncQuickWindowLayout(true, visible);
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

  if (quickRecordingWaveformEl) quickRecordingWaveformEl.classList.remove("is-fallback");
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

function setPreparingSpinner(visible) {
  if (dictateIconEl) dictateIconEl.hidden = visible;
  if (dictateSpinnerEl) dictateSpinnerEl.hidden = !visible;
}

function updateDictateButton() {
  setDictateGlyph(isRecording ? "circle-stop" : "mic");
  dictateBtn.classList.toggle("is-recording", isRecording);
  dictateBtn.setAttribute("aria-label", isRecording ? "Stop dictation" : "Start dictation — Record speech, transcribe, then translate");
  dictateBtn.title = isRecording ? "Stop dictation" : "Start dictation — Record speech, transcribe, then translate";
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
  if (!result || !result.pasted) {
    try {
      await invoke("open_quick_window");
    } catch (_) {
      // ignore
    }
  }
}

async function runSelectionAction(mode) {
  if (isBusy || isRecording) {
    return;
  }

  await loadQuickMode();
  setBusy(true);

  try {
    const input = await invoke("capture_selected_text");
    if (!input || !String(input).trim()) {
      throw new Error("No selected text detected");
    }

    const output =
      mode === "improve"
        ? await invoke("improve_text", { input, style: quickMode })
        : await invoke("translate_text", { input, style: quickMode });

    await insertResultText(output);
  } catch (error) {
    try {
      await invoke("open_quick_window");
    } catch (_) {
      // ignore
    }
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
    setPreparingSpinner(true);

    activeStream = await navigator.mediaDevices.getUserMedia({ audio: true });
    mediaRecorder = new MediaRecorder(activeStream);
    recordedChunks = [];

    startRecordingVisualizer(activeStream);
    isRecording = true;
    setBusy(false);
    updateDictateButton();

    mediaRecorder.ondataavailable = (event) => {
      if (event.data && event.data.size > 0) {
        recordedChunks.push(event.data);
      }
    };

    mediaRecorder.onerror = () => {};

    mediaRecorder.onstop = async () => {
      const aborted = recordingAborted;
      recordingAborted = false;

      const mimeType = mediaRecorder.mimeType || "audio/webm";
      const blob = aborted ? null : new Blob(recordedChunks, { type: mimeType });
      recordedChunks = [];
      mediaRecorder = null;
      stopAndReleaseStream();
      isRecording = false;
      updateDictateButton();

      if (aborted || !blob || !blob.size) {
        setBusy(false);
        setPreparingSpinner(false);
        return;
      }

      setBusy(true);
      setPreparingSpinner(true);

      try {
        const audioBase64 = await blobToBase64(blob);
        const transcript = await invoke("transcribe_audio", {
          audioBase64,
          mimeType: blob.type || undefined,
        });

        await loadQuickMode();
        const output = await invoke("translate_text", {
          input: transcript,
          style: quickMode,
        });

        await insertResultText(output);
      } catch (error) {
        // silent
      } finally {
        setBusy(false);
        setPreparingSpinner(false);
        updateDictateButton();
      }
    };

    mediaRecorder.start();
  } catch (error) {
    isRecording = false;
    updateDictateButton();
    stopAndReleaseStream();
    setBusy(false);
  }
}

function stopDictation() {
  if (!mediaRecorder || mediaRecorder.state !== "recording") {
    isRecording = false;
    updateDictateButton();
    stopAndReleaseStream();
    return;
  }
  mediaRecorder.stop();
}

function abortDictation() {
  recordingAborted = true;
  if (mediaRecorder && mediaRecorder.state === "recording") {
    mediaRecorder.stop();
  } else {
    isRecording = false;
    updateDictateButton();
    stopAndReleaseStream();
    setBusy(false);
    setPreparingSpinner(false);
  }
}

async function openSettings() {
  if (isBusy || isRecording) {
    return;
  }
  try {
    await invoke("open_settings_window");
    await invoke("close_quick_window");
  } catch (error) {
    // silent
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
    abortDictation();
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
    improveSelectionBtn.focus();
  }
}

translateSelectionBtn.addEventListener("click", () => runSelectionAction("translate"));
improveSelectionBtn.addEventListener("click", () => runSelectionAction("improve"));
dictateBtn.addEventListener("click", () => {
  if (isRecording) stopDictation();
  else startDictation();
});
document.addEventListener("click", (event) => {
  const stop = event.target.closest("#quick-stop-btn");
  if (stop) {
    event.stopPropagation();
    stopDictation();
  }
});
settingsBtn.addEventListener("click", openSettings);
closeBtn.addEventListener("click", closeQuick);

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
  if (quickDictateWaveWrapEl) quickDictateWaveWrapEl.hidden = true;
  setBusy(false);
  updateDictateButton();
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
