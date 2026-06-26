import { useCallback, useEffect, useState, useSyncExternalStore } from "react";

interface I18nBundle {
  t: (key: string, params?: Record<string, string | number>) => string;
  applyTranslations: (root?: Document | Element) => void;
  setLanguagePreference: (preference: string) => void;
  getLanguage: () => string;
  getLanguagePreference: () => string;
  resolveLanguage: (preference: string) => string;
}

declare global {
  interface Window {
    WhisloAII18n?: I18nBundle;
  }
}

// Minimal pub/sub so every useT() subscriber re-renders when the language
// changes. The Tauri backend emits "ui-language-changed", but we also bump the
// version here for synchronous in-window changes (and for the dev server where
// there is no backend event).
let languageVersion = 0;
const listeners = new Set<() => void>();

function subscribe(cb: () => void) {
  listeners.add(cb);
  return () => listeners.delete(cb);
}

function getSnapshot() {
  return languageVersion;
}

function notifyLanguageChange() {
  languageVersion++;
  listeners.forEach((l) => l());
}

export function useT() {
  useSyncExternalStore(subscribe, getSnapshot, getSnapshot);
  return useCallback(
    (key: string, params?: Record<string, string | number>) => {
      const i18n = window.WhisloAII18n;
      return i18n ? i18n.t(key, params) : key;
    },
    [],
  );
}

export function useLanguagePreference() {
  const [preference, setPreference] = useState<string>(
    () => window.WhisloAII18n?.getLanguagePreference() ?? "system",
  );

  useEffect(() => {
    setPreference(window.WhisloAII18n?.getLanguagePreference() ?? "system");
  }, []);

  const changePreference = useCallback((next: string) => {
    window.WhisloAII18n?.setLanguagePreference(next);
    setPreference(next);
    notifyLanguageChange();
  }, []);

  return [preference, changePreference] as const;
}

// To be called when the backend confirms a language change (ui-language-changed
// event). Ensures all subscribers re-render with the new language.
export function notifyUiLanguageChanged() {
  notifyLanguageChange();
}
