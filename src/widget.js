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
let quickModeLoadedAt = 0;
let recordingAborted = false;
let dictationStartNonce = 0;
let uiLanguagePreference = "system";

const QUICK_WAVE_BAR_COUNT = 20;
const SUPPORTED_MODES = ["simple", "professional", "friendly", "casual", "formal"];
const QUICK_MODE_CACHE_MS = 30_000;
const DICTATION_CHUNK_MS = 250;

function nowMs() {
  if (typeof performance !== "undefined" && typeof performance.now === "function") {
    return performance.now();
  }
  return Date.now();
}

function logDictationTrace(event, payload) {
  try {
    const data = {
      event,
      ...payload,
    };
    console.info("[dictation_trace]", JSON.stringify(data));
  } catch (_) {
    // no-op
  }
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

function applyWidgetDynamicTranslations() {
  const recordingVisible = quickToolbar.classList.contains("is-recording");
  closeBtn.title = recordingVisible ? t("widget.close.discard.title") : t("widget.close.default.title");
  closeBtn.setAttribute(
    "aria-label",
    recordingVisible ? t("widget.close.discard.aria") : t("widget.close.default.aria"),
  );
  const stopBtn = document.getElementById("quick-stop-btn");
  if (stopBtn) {
    stopBtn.title = t("widget.stopRecording.title");
    stopBtn.setAttribute("aria-label", t("widget.stopRecording.aria"));
  }
  updateDictateButton();
}

function setRecordingVisualizerVisible(visible) {
  if (dictateSpinnerEl) dictateSpinnerEl.hidden = true;
  if (dictateIconEl) dictateIconEl.hidden = visible;
  if (quickDictateWaveWrapEl) quickDictateWaveWrapEl.hidden = !visible;
  setCloseIcon(visible ? "trash-2" : "x");
  closeBtn.title = visible ? t("widget.close.discard.title") : t("widget.close.default.title");
  closeBtn.setAttribute(
    "aria-label",
    visible ? t("widget.close.discard.aria") : t("widget.close.default.aria"),
  );
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
  dictateBtn.setAttribute(
    "aria-label",
    isRecording ? t("widget.dictate.stop.aria") : t("widget.dictate.start.aria"),
  );
  dictateBtn.title = isRecording ? t("widget.dictate.stop.title") : t("widget.dictate.start.title");
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

  const modePromise = loadQuickMode();
  setBusy(true);

  try {
    const input = await invoke("capture_selected_text");
    if (!input || !String(input).trim()) {
      throw new Error("No selected text detected");
    }
    await modePromise;

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

  const clickStartedAt = nowMs();
  try {
    const startNonce = ++dictationStartNonce;
    setBusy(true);
    setPreparingSpinner(true);

    const getMediaStartedAt = nowMs();
    activeStream = await navigator.mediaDevices.getUserMedia({ audio: true });
    if (startNonce !== dictationStartNonce || document.hidden) {
      stopAndReleaseStream();
      setBusy(false);
      setPreparingSpinner(false);
      updateDictateButton();
      return;
    }
    const mediaReadyMs = nowMs() - getMediaStartedAt;

    mediaRecorder = new MediaRecorder(activeStream);
    const recorder = mediaRecorder;
    recordedChunks = [];
    let recorderErrored = false;

    recorder.ondataavailable = (event) => {
      if (event.data && event.data.size > 0) {
        recordedChunks.push(event.data);
      }
    };

    recorder.onerror = () => {
      recorderErrored = true;
      recordingAborted = true;
      if (recorder.state !== "inactive") {
        try {
          recorder.stop();
        } catch (_) {
          isRecording = false;
          mediaRecorder = null;
          stopAndReleaseStream();
          setBusy(false);
          setPreparingSpinner(false);
          updateDictateButton();
        }
      } else {
        isRecording = false;
        mediaRecorder = null;
        stopAndReleaseStream();
        setBusy(false);
        setPreparingSpinner(false);
        updateDictateButton();
      }
    };

    recorder.onstop = async () => {
      const stopStartedAt = nowMs();
      const aborted = recordingAborted;
      recordingAborted = false;

      const mimeType = recorder.mimeType || "audio/webm";
      const blobBuildStartedAt = nowMs();
      const blob = aborted ? null : new Blob(recordedChunks, { type: mimeType });
      const blobBuildMs = nowMs() - blobBuildStartedAt;
      recordedChunks = [];
      mediaRecorder = null;
      stopAndReleaseStream();
      isRecording = false;
      updateDictateButton();

      if (recorderErrored || aborted || !blob || !blob.size) {
        setBusy(false);
        setPreparingSpinner(false);
        return;
      }

      setBusy(true);
      setPreparingSpinner(true);

      try {
        const modePromise = loadQuickMode();
        const encodeStartedAt = nowMs();
        const audioBase64 = await blobToBase64(blob);
        const encodeMs = nowMs() - encodeStartedAt;

        const transcribeStartedAt = nowMs();
        const transcript = await invoke("transcribe_audio", {
          audioBase64,
          mimeType: blob.type || undefined,
        });
        const transcribeMs = nowMs() - transcribeStartedAt;

        await modePromise;
        const translateStartedAt = nowMs();
        const output = await invoke("translate_text", {
          input: transcript,
          style: quickMode,
        });
        const translateMs = nowMs() - translateStartedAt;

        const insertStartedAt = nowMs();
        await insertResultText(output);
        const insertMs = nowMs() - insertStartedAt;

        logDictationTrace("stop_to_insert", {
          aborted: false,
          blob_size_bytes: blob.size,
          blob_build_ms: Number(blobBuildMs.toFixed(1)),
          encode_ms: Number(encodeMs.toFixed(1)),
          transcribe_ms: Number(transcribeMs.toFixed(1)),
          translate_ms: Number(translateMs.toFixed(1)),
          insert_ms: Number(insertMs.toFixed(1)),
          total_ms: Number((nowMs() - stopStartedAt).toFixed(1)),
        });
      } catch (error) {
        logDictationTrace("stop_to_insert_error", {
          aborted: false,
          blob_size_bytes: blob.size,
          blob_build_ms: Number(blobBuildMs.toFixed(1)),
          total_ms: Number((nowMs() - stopStartedAt).toFixed(1)),
        });
      } finally {
        setBusy(false);
        setPreparingSpinner(false);
        updateDictateButton();
      }
    };

    recorder.start(DICTATION_CHUNK_MS);
    isRecording = true;
    setBusy(false);
    setPreparingSpinner(false);
    updateDictateButton();
    requestAnimationFrame(() => {
      if (activeStream && isRecording) {
        startRecordingVisualizer(activeStream);
      }
    });
    logDictationTrace("start_recording", {
      get_user_media_ms: Number(mediaReadyMs.toFixed(1)),
      click_to_recording_ms: Number((nowMs() - clickStartedAt).toFixed(1)),
    });
  } catch (error) {
    isRecording = false;
    updateDictateButton();
    stopAndReleaseStream();
    setBusy(false);
    setPreparingSpinner(false);
    logDictationTrace("start_recording_error", {
      click_to_error_ms: Number((nowMs() - clickStartedAt).toFixed(1)),
      message: String(error?.message || error || "unknown"),
    });
  }
}

function stopDictation() {
  dictationStartNonce += 1;
  if (!mediaRecorder || mediaRecorder.state !== "recording") {
    isRecording = false;
    updateDictateButton();
    stopAndReleaseStream();
    return;
  }
  mediaRecorder.stop();
}

function abortDictation() {
  dictationStartNonce += 1;
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

function handleWidgetVisibilityChange() {
  if (document.hidden && (isRecording || activeStream || (mediaRecorder && mediaRecorder.state === "recording"))) {
    abortDictation();
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

async function loadQuickMode(force = false) {
  const now = Date.now();
  if (!force && quickModeLoadedAt > 0 && now - quickModeLoadedAt < QUICK_MODE_CACHE_MS) {
    return quickMode;
  }
  try {
    const settings = await invoke("get_prompt_settings");
    quickMode = normalizeMode(settings?.quickMode);
  } catch (_) {
    quickMode = "simple";
  }
  quickModeLoadedAt = now;
  return quickMode;
}

async function loadUiLanguagePreference() {
  try {
    const settings = await invoke("get_ui_settings");
    uiLanguagePreference = String(settings?.uiLanguagePreference || "system");
  } catch (_) {
    uiLanguagePreference = "system";
  }
  setLanguagePreference(uiLanguagePreference);
  applyTranslations(document);
  applyWidgetDynamicTranslations();
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

document.addEventListener("visibilitychange", handleWidgetVisibilityChange);
window.addEventListener("beforeunload", stopAndReleaseStream);

async function bootstrap() {
  await loadUiLanguagePreference();
  await loadQuickMode();
  renderLucideIcons();
  stopRecordingVisualizer();
  if (quickDictateWaveWrapEl) quickDictateWaveWrapEl.hidden = true;
  setBusy(false);
  updateDictateButton();
  syncQuickWindowLayout(true);

  try {
    await listen("ui-language-changed", (event) => {
      const nextPreference = String(event?.payload?.uiLanguagePreference || "system");
      uiLanguagePreference = nextPreference;
      setLanguagePreference(uiLanguagePreference);
      applyTranslations(document);
      applyWidgetDynamicTranslations();
    });
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
