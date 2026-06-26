/**
 * Maps backend/raw errors to localized user-facing messages.
 * @param {unknown} error - Raw error from invoke or exception
 * @returns {string} User-friendly message
 */
function specErrorFor(error) {
  const raw = String(error || "").trim();
  const lower = raw.toLowerCase();
  const t = window.WhisloAII18n?.t || ((key) => key);

  if (!raw) return t("error.generic");

  if (
    lower.includes("api key") ||
    lower.includes("missing api key") ||
    lower.includes("no provider configured") ||
    lower.includes("provider not found")
  ) {
    return t("error.api_key");
  }

  if (
    lower.includes("microphone") ||
    lower.includes("mediarecorder") ||
    lower.includes("getusermedia") ||
    lower.includes("not allowed") ||
    lower.includes("permission denied") ||
    lower.includes("could not access") ||
    lower.includes("recording failed")
  ) {
    return t("error.microphone");
  }

  if (
    lower.includes("audio payload is empty") ||
    lower.includes("no audio captured") ||
    lower.includes("transcription response was empty") ||
    lower.includes("local transcription was empty")
  ) {
    return t("error.transcription_empty");
  }

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
    return t("error.network");
  }

  if (lower.includes("automatic paste failed")) {
    return t("error.auto_paste");
  }

  if (lower.includes("no selected text")) {
    return t("error.no_selected_text");
  }

  if (raw.startsWith("Error: ")) {
    return raw.slice(7);
  }

  return raw;
}
