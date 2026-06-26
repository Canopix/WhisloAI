// Types mirroring the Rust structs in src-tauri/src/domain/config.rs.
// These match the shapes returned by list_providers, get_transcription_config,
// get_prompt_settings, get_ui_settings, etc.

export interface ProviderView {
  id: string;
  name: string;
  providerType: string; // "openai" | "openai-compatible"
  baseUrl: string;
  translateModel: string;
  transcribeModel: string;
  isActive: boolean;
  hasApiKey: boolean;
  apiKey: string | null;
}

export interface ProviderInput {
  id: string | null;
  name: string;
  providerType: string;
  baseUrl: string;
  translateModel: string;
  transcribeModel: string;
}

export interface TranscriptionConfig {
  mode: string; // "api" | "local"
  localModelPath: string | null;
  localModelsDir: string | null;
}

export interface PromptSettings {
  translateSystemPrompt: string;
  sourceLanguage: string;
  targetLanguage: string;
  modeInstructions: {
    simple: string;
    professional: string;
    friendly: string;
    casual: string;
    formal: string;
  };
  quickMode: string;
}

export interface UiSettings {
  uiLanguagePreference: string; // "system" | "en" | "es"
  anchorBehavior: string; // "contextual" | "floating"
}

export interface WhisperModel {
  id: string;
  fileName: string;
  sizeLabel: string;
  status: "available" | "downloading" | "downloaded" | "selected" | "error";
  downloaded: boolean;
  isSelected: boolean;
  downloadProgress: number; // 0..100
}

export interface OnboardingStatus {
  needsAccessibility: boolean;
  needsAutomation: boolean;
  supportsContextualAnchor: boolean;
}

export type SectionId = "general" | "providers" | "prompts" | "toolbox";
