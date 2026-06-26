import { invoke as rawInvoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import type {
  ProviderView,
  ProviderInput,
  TranscriptionConfig,
  PromptSettings,
  UiSettings,
  WhisperModel,
  OnboardingStatus,
} from "../types";

// Thin typed wrappers around the Tauri command surface.
// Mirrors the commands used by the old settings.js but with types.

export const tauri = {
  // Providers
  listProviders: () => rawInvoke<ProviderView[]>("list_providers"),
  saveProvider: (provider: ProviderInput, apiKey: string | null) =>
    rawInvoke<ProviderView>("save_provider", { provider, apiKey }),
  setActiveProvider: (providerId: string) =>
    rawInvoke<void>("set_active_provider", { providerId }),
  deleteProvider: (providerId: string) =>
    rawInvoke<void>("delete_provider", { providerId }),
  testProviderConnection: (provider: ProviderInput, apiKey: string | null) =>
    rawInvoke<string>("test_provider_connection_input", { provider, apiKey }),

  // Transcription
  getTranscriptionConfig: () =>
    rawInvoke<TranscriptionConfig>("get_transcription_config"),
  saveTranscriptionConfig: (transcription: TranscriptionConfig) =>
    rawInvoke<TranscriptionConfig>("save_transcription_config", { transcription }),
  listWhisperModels: () => rawInvoke<WhisperModel[]>("list_whisper_models"),
  downloadWhisperModel: (modelId: string) =>
    rawInvoke<void>("download_whisper_model", { modelId }),
  pickWhisperModelsDir: () =>
    rawInvoke<string | null>("pick_whisper_models_dir"),

  // Prompts
  getPromptSettings: () => rawInvoke<PromptSettings>("get_prompt_settings"),
  savePromptSettings: (promptSettings: PromptSettings) =>
    rawInvoke<PromptSettings>("save_prompt_settings", { promptSettings }),

  // UI settings
  getUiSettings: () => rawInvoke<UiSettings>("get_ui_settings"),
  saveUiSettings: (uiSettings: UiSettings) =>
    rawInvoke<void>("save_ui_settings", { uiSettings }),

  // Misc
  getAppVersion: () => rawInvoke<string>("get_app_version"),
  getOnboardingStatus: () => rawInvoke<OnboardingStatus>("get_onboarding_status"),
  getBlockedBundleIds: () => rawInvoke<string[]>("get_blocked_bundle_ids"),
  blacklistCurrentApp: () => rawInvoke<string>("blacklist_current_app"),
  removeBlockedBundleId: (bundleId: string) =>
    rawInvoke<void>("remove_blocked_bundle_id", { bundleId }),
  openPermissionSettings: (permission: string) =>
    rawInvoke<void>("open_permission_settings", { permission }),
  probeAccessibilityPermission: () =>
    rawInvoke<boolean>("probe_accessibility_permission"),
  probeSystemEventsPermission: () =>
    rawInvoke<boolean>("probe_system_events_permission"),
  openExternalUrl: (url: string) =>
    rawInvoke<void>("open_external_url", { url }),

  // Audio toolbox
  transcribeAudio: (audioBase64: string, mimeType: string) =>
    rawInvoke<string>("transcribe_audio", { audioBase64, mimeType }),
  autoInsertText: (text: string) =>
    rawInvoke<{ pasted: boolean }>("auto_insert_text", { text }),

  // Events
  listen: <T>(
    event: string,
    handler: (payload: T) => void,
  ): Promise<UnlistenFn> => listen<T>(event, (e) => handler(e.payload)),
};

export type TauriApi = typeof tauri;
