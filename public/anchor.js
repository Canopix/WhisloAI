const { invoke } = window.__TAURI__.core;
const { listen } = window.__TAURI__.event;

const anchorBtn = document.getElementById("anchor-btn");
const DRAG_THRESHOLD = 5;

let anchorBehavior = "contextual";
let pointerStart = null;
let dragTriggered = false;
let suppressNextClick = false;
let dragInProgress = false;

function normalizeAnchorBehavior(value) {
  return String(value || "").trim().toLowerCase() === "floating" ? "floating" : "contextual";
}

async function loadUiSettings() {
  try {
    const settings = await invoke("get_ui_settings");
    anchorBehavior = normalizeAnchorBehavior(settings?.anchorBehavior);
  } catch (_) {
    anchorBehavior = "contextual";
  }
}

if (window.lucide && typeof window.lucide.createIcons === "function") {
  window.lucide.createIcons({
    attrs: {
      width: "14",
      height: "14",
      "stroke-width": "2.25",
    },
  });
}

anchorBtn.addEventListener("pointerdown", (event) => {
  if (event.button !== 0 || anchorBehavior !== "floating") {
    return;
  }
  pointerStart = {
    pointerId: event.pointerId,
    x: event.clientX,
    y: event.clientY,
  };
  dragTriggered = false;
});

anchorBtn.addEventListener("pointermove", async (event) => {
  if (
    anchorBehavior !== "floating" ||
    dragInProgress ||
    !pointerStart ||
    dragTriggered ||
    event.pointerId !== pointerStart.pointerId
  ) {
    return;
  }

  const deltaX = event.clientX - pointerStart.x;
  const deltaY = event.clientY - pointerStart.y;
  if (Math.hypot(deltaX, deltaY) < DRAG_THRESHOLD) {
    return;
  }

  dragTriggered = true;
  dragInProgress = true;
  suppressNextClick = true;

  try {
    await invoke("start_anchor_window_drag");
    await invoke("remember_anchor_window_position");
  } catch (_) {
    // Keep anchor silent to avoid noisy popups while user types.
  } finally {
    dragInProgress = false;
    pointerStart = null;
  }
});

anchorBtn.addEventListener("pointerup", async (event) => {
  if (!pointerStart || event.pointerId !== pointerStart.pointerId) {
    return;
  }

  pointerStart = null;
  if (!dragTriggered || anchorBehavior !== "floating") {
    return;
  }

  dragTriggered = false;
  try {
    await invoke("remember_anchor_window_position");
  } catch (_) {
    // no-op
  }
});

anchorBtn.addEventListener("pointercancel", () => {
  pointerStart = null;
  dragTriggered = false;
});

anchorBtn.addEventListener("click", async () => {
  if (suppressNextClick) {
    suppressNextClick = false;
    return;
  }

  try {
    await invoke("open_quick_window");
  } catch (_) {
    // Keep anchor silent to avoid noisy popups while user types.
  }
});

loadUiSettings();

if (typeof listen === "function") {
  listen("ui-settings-changed", (event) => {
    anchorBehavior = normalizeAnchorBehavior(event?.payload?.anchorBehavior);
  }).catch(() => {});
}
