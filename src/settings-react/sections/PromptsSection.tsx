import { useEffect, useState } from "react";
import { Card, CardContent, CardHeader, CardTitle, CardDescription } from "../components/ui/Card";
import { Button } from "../components/ui/Button";
import { Input, Label, Select, Textarea, FieldError } from "../components/ui/Field";
import { StatusInline, type SectionStatus } from "../components/StatusInline";
import { useT } from "../lib/i18n";
import { tauri } from "../lib/tauri";
import type { PromptSettings } from "../types";

const DEFAULT_PROMPT_SETTINGS: PromptSettings = {
  translateSystemPrompt:
    "You are a translation assistant. Convert text from {source} into clear, concise, natural {target} for workplace chat. Preserve names and technical terms. Return only final text.",
  sourceLanguage: "Spanish",
  targetLanguage: "English",
  modeInstructions: {
    simple: "Use clear, concise wording with everyday vocabulary.",
    professional: "Use a polished workplace tone with direct and confident wording.",
    friendly: "Use a warm, approachable tone while staying concise.",
    casual: "Use a relaxed conversational tone with natural phrasing.",
    formal: "Use a formal, respectful tone with complete sentences.",
  },
  quickMode: "simple",
};

const LANGUAGES = [
  "Spanish",
  "English",
  "Portuguese",
  "French",
  "German",
  "Italian",
  "Japanese",
  "Chinese",
];

const MODES: { key: keyof PromptSettings["modeInstructions"]; emoji: string; labelKey: string; placeholderKey: string }[] = [
  { key: "simple", emoji: "✍️", labelKey: "settings.mode.simple.label", placeholderKey: "settings.mode.simple.placeholder" },
  { key: "professional", emoji: "🧑‍💻", labelKey: "settings.mode.professional.label", placeholderKey: "settings.mode.professional.placeholder" },
  { key: "friendly", emoji: "📢", labelKey: "settings.mode.friendly.label", placeholderKey: "settings.mode.friendly.placeholder" },
  { key: "casual", emoji: "💬", labelKey: "settings.mode.casual.label", placeholderKey: "settings.mode.casual.placeholder" },
  { key: "formal", emoji: "🧠", labelKey: "settings.mode.formal.label", placeholderKey: "settings.mode.formal.placeholder" },
];

export function PromptsSection() {
  const t = useT();
  const [settings, setSettings] = useState<PromptSettings | null>(null);
  const [status, setStatus] = useState<SectionStatus>({ message: "", tone: "neutral" });
  const [error, setError] = useState<string>("");

  useEffect(() => {
    tauri.getPromptSettings().then(setSettings).catch(() => {});
  }, []);

  if (!settings) return null;

  const update = (patch: Partial<PromptSettings>) => {
    setSettings({ ...settings, ...patch });
    setError("");
  };

  const updateModeInstruction = (key: keyof PromptSettings["modeInstructions"], value: string) => {
    setSettings({
      ...settings,
      modeInstructions: { ...settings.modeInstructions, [key]: value },
    });
    setError("");
  };

  const validate = (): string => {
    if (!settings.translateSystemPrompt.trim()) return t("settings.status.complete_prompts");
    for (const m of MODES) {
      if (!settings.modeInstructions[m.key].trim()) return t("settings.status.complete_prompts");
    }
    if (
      settings.sourceLanguage.trim().toLowerCase() ===
      settings.targetLanguage.trim().toLowerCase()
    ) {
      return t("settings.status.source_target_must_differ");
    }
    return "";
  };

  const save = () => {
    const err = validate();
    if (err) {
      setError(err);
      setStatus({ message: err, tone: "error" });
      return;
    }
    setStatus({ message: t("settings.status.saving_prompts"), tone: "loading" });
    tauri.savePromptSettings(settings)
      .then((saved) => {
        setSettings(saved);
        setStatus({ message: t("settings.status.prompts_saved"), tone: "success" });
      })
      .catch((e) => setStatus({ message: String(e), tone: "error" }));
  };

  const reset = () => {
    setSettings(DEFAULT_PROMPT_SETTINGS);
    setStatus({ message: t("settings.status.prompt_defaults_restored"), tone: "neutral" });
  };

  return (
    <div className="py-8 space-y-6">
      <div>
        <h1 className="text-2xl font-bold text-ink">{t("settings.prompts.title")}</h1>
        <p className="text-sm text-ink-muted mt-1">{t("settings.prompts.hint")}</p>
      </div>

      {/* Language conversion */}
      <Card>
        <CardHeader>
          <CardTitle>{t("settings.prompts.language_conversion")}</CardTitle>
          <CardDescription>{t("settings.prompts.language_hint")}</CardDescription>
        </CardHeader>
        <CardContent className="grid grid-cols-1 sm:grid-cols-2 gap-4">
          <div>
            <Label htmlFor="src-lang">{t("settings.field.source_language")}</Label>
            <Select
              id="src-lang"
              value={settings.sourceLanguage}
              onChange={(e) => update({ sourceLanguage: e.target.value })}
            >
              {LANGUAGES.map((l) => (
                <option key={l} value={l}>{l}</option>
              ))}
            </Select>
          </div>
          <div>
            <Label htmlFor="tgt-lang">{t("settings.field.target_language")}</Label>
            <Select
              id="tgt-lang"
              value={settings.targetLanguage}
              onChange={(e) => update({ targetLanguage: e.target.value })}
            >
              {LANGUAGES.map((l) => (
                <option key={l} value={l}>{l}</option>
              ))}
            </Select>
          </div>
        </CardContent>
      </Card>

      {/* System prompt */}
      <Card>
        <CardHeader>
          <CardTitle>{t("settings.prompts.system")}</CardTitle>
        </CardHeader>
        <CardContent>
          <Label htmlFor="translate-prompt">{t("settings.field.translate_prompt")}</Label>
          <Textarea
            id="translate-prompt"
            rows={3}
            value={settings.translateSystemPrompt}
            onChange={(e) => update({ translateSystemPrompt: e.target.value })}
            placeholder={t("settings.placeholder.translate_prompt")}
          />
        </CardContent>
      </Card>

      {/* Writing modes */}
      <Card>
        <CardHeader>
          <CardTitle>{t("settings.prompts.writing_modes")}</CardTitle>
          <CardDescription>{t("settings.prompts.writing_modes_hint")}</CardDescription>
        </CardHeader>
        <CardContent className="space-y-3">
          {MODES.map((m) => (
            <div key={m.key} className="flex items-start gap-3">
              <span className="text-xl pt-2.5" aria-hidden="true">{m.emoji}</span>
              <div className="flex-1">
                <Label htmlFor={`mode-${m.key}`}>{t(m.labelKey)}</Label>
                <Input
                  id={`mode-${m.key}`}
                  value={settings.modeInstructions[m.key]}
                  onChange={(e) => updateModeInstruction(m.key, e.target.value)}
                  placeholder={t(m.placeholderKey)}
                />
              </div>
            </div>
          ))}

          <div className="pt-2">
            <Label htmlFor="quick-default">{t("settings.quick_mode.label")}</Label>
            <Select
              id="quick-default"
              value={settings.quickMode}
              onChange={(e) => update({ quickMode: e.target.value })}
            >
              {MODES.map((m) => (
                <option key={m.key} value={m.key}>
                  {m.emoji} {t(m.labelKey)}
                </option>
              ))}
            </Select>
          </div>
        </CardContent>
      </Card>

      {error && <FieldError>{error}</FieldError>}

      <div className="flex items-center justify-between gap-3">
        <Button variant="ghost" onClick={reset}>
          {t("settings.action.reset_defaults")}
        </Button>
        <Button variant="primary" onClick={save}>
          {t("settings.action.save_prompts")}
        </Button>
      </div>

      <StatusInline
        message={status.message}
        tone={status.tone}
        autoClearMs={status.tone === "success" ? 3000 : 0}
        onAutoClear={() => setStatus({ message: "", tone: "neutral" })}
      />
    </div>
  );
}
