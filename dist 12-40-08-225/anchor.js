const { invoke } = window.__TAURI__.core;

const anchorBtn = document.getElementById("anchor-btn");

if (window.lucide && typeof window.lucide.createIcons === "function") {
  window.lucide.createIcons({
    attrs: {
      width: "14",
      height: "14",
      "stroke-width": "2.25",
    },
  });
}

anchorBtn.addEventListener("click", async () => {
  try {
    await invoke("open_quick_window");
  } catch (_) {
    // Keep anchor silent to avoid noisy popups while user types.
  }
});
