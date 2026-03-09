/**
 * Maps backend/raw errors to user-facing messages per spec section 9.
 * @param {unknown} error - Raw error from invoke or exception
 * @returns {string} User-friendly message in Spanish
 */
function specErrorFor(error) {
  const raw = String(error || "").trim();
  const lower = raw.toLowerCase();

  if (!raw) return "Ocurrió un error.";

  // 0. API key faltante o inválida
  if (
    lower.includes("api key") ||
    lower.includes("missing api key") ||
    lower.includes("no provider configured") ||
    lower.includes("provider not found")
  ) {
    return "Configurá una API key válida para continuar. Abrí Settings.";
  }

  // 1. Micrófono no disponible
  if (
    lower.includes("microphone") ||
    lower.includes("mediarecorder") ||
    lower.includes("getusermedia") ||
    lower.includes("not allowed") ||
    lower.includes("permission denied") ||
    lower.includes("could not access") ||
    lower.includes("recording failed")
  ) {
    return "No se pudo acceder al micrófono. Revisá permisos del sistema.";
  }

  // 2. Transcripción vacía o baja confianza
  if (
    lower.includes("audio payload is empty") ||
    lower.includes("no audio captured") ||
    lower.includes("transcription response was empty")
  ) {
    return "No pudimos transcribir claramente. Probá grabar de nuevo.";
  }

  // 3. Falla de red/API
  if (
    lower.includes("connection failed") ||
    lower.includes("provider request failed") ||
    lower.includes("provider returned http") ||
    lower.includes("provider connection failed") ||
    lower.includes("transcription request failed") ||
    lower.includes("transcription failed with http") ||
    lower.includes("could not parse") ||
    lower.includes("network") ||
    /http \d{3}/.test(lower)
  ) {
    return "No se pudo procesar el texto por un problema de conexión.";
  }

  // 4. Inserción fallida - handled in insertResultText, not from raw error
  if (lower.includes("automatic paste failed")) {
    return "No pudimos pegar automáticamente. El texto quedó copiado.";
  }

  // No selected text - keep clear but in Spanish
  if (lower.includes("no selected text")) {
    return "No se detectó texto seleccionado. Seleccioná y probá de nuevo.";
  }

  // Strip "Error: " prefix if present
  if (raw.startsWith("Error: ")) {
    return raw.slice(7);
  }
  return raw;
}
