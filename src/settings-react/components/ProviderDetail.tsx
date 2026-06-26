import { useState } from "react";
import { ChevronLeft, MoreHorizontal, Trash2, CheckCircle2 } from "lucide-react";
import { Card, CardContent, CardHeader, CardTitle } from "./ui/Card";
import { Button } from "./ui/Button";
import { Badge } from "./ui/Badge";
import { Input, Select, Label, FieldError } from "./ui/Field";
import { StatusInline, type SectionStatus } from "./StatusInline";
import { useT } from "../lib/i18n";
import { tauri } from "../lib/tauri";
import type { ProviderView, ProviderInput, TranscriptionConfig } from "../types";
import { cn } from "../lib/cn";

interface ProviderDetailProps {
  provider: ProviderView | null;
  isNew: boolean;
  transcriptionConfig: TranscriptionConfig | null;
  onBack: () => void;
  onSaved: () => void;
  onDeleted: () => void;
}

interface FieldErrors {
  [key: string]: string;
}

const EMPTY_INPUT: ProviderInput = {
  id: null,
  name: "",
  providerType: "openai",
  baseUrl: "https://api.openai.com/v1",
  translateModel: "gpt-4.1-mini",
  transcribeModel: "gpt-4o-mini-transcribe",
};

export function ProviderDetail({
  provider,
  isNew,
  transcriptionConfig,
  onBack,
  onSaved,
  onDeleted,
}: ProviderDetailProps) {
  const t = useT();
  const [mode, setMode] = useState<"view" | "edit">(isNew ? "edit" : "view");
  const [menuOpen, setMenuOpen] = useState(false);
  const [confirmDelete, setConfirmDelete] = useState(false);
  const [status, setStatus] = useState<SectionStatus>({ message: "", tone: "neutral" });
  const [errors, setErrors] = useState<FieldErrors>({});

  const [form, setForm] = useState<ProviderInput>(
    provider
      ? {
          id: provider.id,
          name: provider.name,
          providerType: provider.providerType,
          baseUrl: provider.baseUrl,
          translateModel: provider.translateModel,
          transcribeModel: provider.transcribeModel,
        }
      : EMPTY_INPUT,
  );
  const [apiKey, setApiKey] = useState<string>("");

  const isOpenAi = form.providerType !== "openai-compatible";
  const transcriptionUsesApi = transcriptionConfig?.mode !== "local";

  const update = (patch: Partial<ProviderInput>) => {
    setForm((f) => ({ ...f, ...patch }));
    setErrors({});
  };

  const validate = (): FieldErrors => {
    const e: FieldErrors = {};
    if (!form.name.trim()) e.name = t("settings.field.error.name_required");
    if (!form.baseUrl.trim()) e.baseUrl = t("settings.field.error.base_url_required");
    if (isOpenAi && !form.translateModel.trim())
      e.translateModel = t("settings.field.error.translate_model_required");
    if (isOpenAi && transcriptionUsesApi && !form.transcribeModel.trim())
      e.transcribeModel = t("settings.field.error.transcribe_model_required");
    return e;
  };

  const handleSave = () => {
    const e = validate();
    if (Object.keys(e).length) {
      setErrors(e);
      setStatus({ message: t("settings.status.complete_provider_fields"), tone: "error" });
      return;
    }
    setErrors({});
    setStatus({ message: t("settings.status.saving_provider"), tone: "loading" });
    tauri.saveProvider(form, apiKey.trim() || null)
      .then(() => {
        setStatus({ message: t("settings.status.saved_provider", { name: form.name }), tone: "success" });
        onSaved();
      })
      .catch((err) => setStatus({ message: String(err), tone: "error" }));
  };

  const handleTest = () => {
    const e: FieldErrors = {};
    if (!form.name.trim()) e.name = t("settings.field.error.name_required");
    if (!form.baseUrl.trim()) e.baseUrl = t("settings.field.error.base_url_required");
    if (Object.keys(e).length) {
      setErrors(e);
      setStatus({ message: t("settings.status.complete_name_url_test"), tone: "error" });
      return;
    }
    setStatus({ message: t("settings.status.testing_provider", { name: form.name }), tone: "loading" });
    tauri.testProviderConnection(form, apiKey.trim() || null)
      .then((msg) => setStatus({ message: msg, tone: "success" }))
      .catch((err) => setStatus({ message: String(err), tone: "error" }));
  };

  const handleSetActive = () => {
    if (!provider) return;
    setStatus({ message: t("settings.status.setting_active_provider"), tone: "loading" });
    tauri.setActiveProvider(provider.id)
      .then(() => {
        setStatus({ message: t("settings.status.active_provider_set", { name: provider.name }), tone: "success" });
        onSaved();
      })
      .catch((err) => setStatus({ message: String(err), tone: "error" }));
    setMenuOpen(false);
  };

  const handleDelete = () => {
    if (!provider) return;
    if (!confirmDelete) {
      setConfirmDelete(true);
      return;
    }
    setStatus({ message: t("settings.status.deleting_provider", { name: provider.name }), tone: "loading" });
    tauri.deleteProvider(provider.id)
      .then(() => {
        setStatus({ message: t("settings.status.deleted_provider", { name: provider.name }), tone: "success" });
        onDeleted();
      })
      .catch((err) => setStatus({ message: String(err), tone: "error" }));
  };

  const title = isNew
    ? t("settings.provider.mode.creating")
    : mode === "view"
      ? provider?.name ?? ""
      : t("settings.provider.mode.editing", { name: provider?.name ?? "" });

  return (
    <div className="w-full">
      {/* Breadcrumb */}
      <button
        type="button"
        onClick={onBack}
        className="inline-flex items-center gap-1.5 text-sm text-ink-muted hover:text-ink mb-4 transition-colors"
      >
        <ChevronLeft className="h-4 w-4" />
        {t("settings.providers.saved.title")}
      </button>

      <Card>
        <CardHeader>
          <div className="flex items-start justify-between gap-3">
            <div>
              <CardTitle>{title}</CardTitle>
              {provider?.isActive && (
                <Badge variant="success" className="mt-2">
                  <CheckCircle2 className="h-3 w-3" />
                  {t("settings.status.active")}
                </Badge>
              )}
            </div>

            {/* Action menu — only for existing providers */}
            {!isNew && mode === "view" && (
              <div className="relative">
                <Button
                  variant="ghost"
                  size="icon"
                  onClick={() => setMenuOpen((v) => !v)}
                  aria-label="More actions"
                >
                  <MoreHorizontal className="h-4 w-4" />
                </Button>
                {menuOpen && (
                  <>
                    <div
                      className="fixed inset-0 z-10"
                      onClick={() => setMenuOpen(false)}
                    />
                    <div className="absolute right-0 top-full mt-1 z-20 w-52 rounded-xl border border-border-soft bg-surface shadow-lg py-1">
                      <button
                        type="button"
                        onClick={handleSetActive}
                        disabled={!provider || provider.isActive}
                        className="w-full text-left px-3 py-2 text-sm text-ink hover:bg-surface-soft disabled:opacity-50"
                      >
                        {t("settings.action.set_active")}
                      </button>
                      <button
                        type="button"
                        onClick={() => {
                          setConfirmDelete(false);
                          handleDelete();
                        }}
                        className="w-full text-left px-3 py-2 text-sm text-state-error-fg hover:bg-state-error-bg"
                      >
                        {t("settings.action.delete_provider")}
                      </button>
                    </div>
                  </>
                )}
              </div>
            )}
          </div>
        </CardHeader>

        <CardContent className="space-y-5">
          {mode === "view" && provider ? (
            <ProviderReadView provider={provider} onEdit={() => setMode("edit")} />
          ) : (
            <ProviderForm
              form={form}
              apiKey={apiKey}
              errors={errors}
              isOpenAi={isOpenAi}
              transcriptionUsesApi={transcriptionUsesApi}
              isEditing={!isNew}
              onForm={update}
              onApiKey={setApiKey}
              onCancel={() => (isNew ? onBack() : setMode("view"))}
              onSave={handleSave}
              onTest={handleTest}
            />
          )}

          {confirmDelete && (
            <div className="rounded-xl border border-state-error-border bg-state-error-bg p-3 flex items-center justify-between gap-3">
              <span className="text-sm text-state-error-fg">
                {t("settings.confirm.delete_provider", { name: provider?.name ?? "" })}
              </span>
              <div className="flex gap-2">
                <Button variant="ghost" size="sm" onClick={() => setConfirmDelete(false)}>
                  {t("settings.action.cancel")}
                </Button>
                <Button variant="danger" size="sm" onClick={handleDelete}>
                  <Trash2 className="h-3.5 w-3.5" />
                  {t("settings.action.delete_provider")}
                </Button>
              </div>
            </div>
          )}

          <StatusInline
            message={status.message}
            tone={status.tone}
            autoClearMs={status.tone === "success" ? 3000 : 0}
            onAutoClear={() => setStatus({ message: "", tone: "neutral" })}
          />
        </CardContent>
      </Card>
    </div>
  );
}

/* ---------- Read mode ---------- */

function ProviderReadView({
  provider,
  onEdit,
}: {
  provider: ProviderView;
  onEdit: () => void;
}) {
  const t = useT();
  const isOpenAi = provider.providerType !== "openai-compatible";

  const rows: { label: string; value: string }[] = [
    { label: t("settings.field.name"), value: provider.name },
    {
      label: t("settings.field.type"),
      value: isOpenAi
        ? t("settings.field.type.openai")
        : t("settings.field.type.openai_compatible"),
    },
    { label: t("settings.field.base_url"), value: provider.baseUrl },
    { label: t("settings.field.translate_model"), value: provider.translateModel || "—" },
    { label: t("settings.field.transcribe_model"), value: provider.transcribeModel || "—" },
  ];

  return (
    <div className="space-y-4">
      <dl className="divide-y divide-border-soft rounded-xl border border-border-soft overflow-hidden">
        {rows.map((row) => (
          <div key={row.label} className="flex items-baseline gap-4 px-4 py-2.5 bg-surface">
            <dt className="text-xs font-semibold text-ink-muted w-40 shrink-0">
              {row.label}
            </dt>
            <dd className="text-sm text-ink font-mono break-all">{row.value}</dd>
          </div>
        ))}
        <div className="flex items-baseline gap-4 px-4 py-2.5 bg-surface">
          <dt className="text-xs font-semibold text-ink-muted w-40 shrink-0">
            {t("settings.field.api_key")}
          </dt>
          <dd className="text-sm text-ink">
            {isOpenAi ? (
              provider.hasApiKey ? (
                <Badge variant="success">{t("settings.status.key_saved")}</Badge>
              ) : (
                <Badge variant="warning">{t("settings.status.missing_api_key")}</Badge>
              )
            ) : (
              <Badge variant="neutral">{t("settings.status.key_not_required")}</Badge>
            )}
          </dd>
        </div>
      </dl>

      <Button variant="primary" onClick={onEdit} className="w-full">
        {t("settings.status.edit")}
      </Button>
    </div>
  );
}

/* ---------- Edit/Create mode ---------- */

function ProviderForm({
  form,
  apiKey,
  errors,
  isOpenAi,
  transcriptionUsesApi,
  isEditing,
  onForm,
  onApiKey,
  onCancel,
  onSave,
  onTest,
}: {
  form: ProviderInput;
  apiKey: string;
  errors: FieldErrors;
  isOpenAi: boolean;
  transcriptionUsesApi: boolean;
  isEditing: boolean;
  onForm: (patch: Partial<ProviderInput>) => void;
  onApiKey: (v: string) => void;
  onCancel: () => void;
  onSave: () => void;
  onTest: () => void;
}) {
  const t = useT();

  return (
    <div className="space-y-4">
      <div className="grid grid-cols-1 sm:grid-cols-2 gap-4">
        <div>
          <Label htmlFor="p-name">{t("settings.field.name")}</Label>
          <Input
            id="p-name"
            value={form.name}
            onChange={(e) => onForm({ name: e.target.value })}
            placeholder="OpenAI"
          />
          <FieldError>{errors.name}</FieldError>
        </div>
        <div>
          <Label htmlFor="p-type">{t("settings.field.type")}</Label>
          <Select
            id="p-type"
            value={form.providerType}
            onChange={(e) => onForm({ providerType: e.target.value })}
          >
            <option value="openai">{t("settings.field.type.openai")}</option>
            <option value="openai-compatible">
              {t("settings.field.type.openai_compatible")}
            </option>
          </Select>
        </div>
      </div>

      <div>
        <Label htmlFor="p-url">{t("settings.field.base_url")}</Label>
        <Input
          id="p-url"
          type="url"
          value={form.baseUrl}
          onChange={(e) => onForm({ baseUrl: e.target.value })}
          placeholder="https://api.openai.com/v1"
        />
        <FieldError>{errors.baseUrl}</FieldError>
      </div>

      <div className="grid grid-cols-1 sm:grid-cols-2 gap-4">
        <div>
          <Label htmlFor="p-translate">{t("settings.field.translate_model")}</Label>
          <Input
            id="p-translate"
            value={form.translateModel}
            onChange={(e) => onForm({ translateModel: e.target.value })}
            placeholder="gpt-4.1-mini"
            disabled={!isOpenAi}
          />
          <FieldError>{errors.translateModel}</FieldError>
        </div>
        <div>
          <Label htmlFor="p-transcribe">{t("settings.field.transcribe_model")}</Label>
          <Input
            id="p-transcribe"
            value={form.transcribeModel}
            onChange={(e) => onForm({ transcribeModel: e.target.value })}
            placeholder="gpt-4o-mini-transcribe"
            disabled={!isOpenAi || !transcriptionUsesApi}
          />
          <FieldError>{errors.transcribeModel}</FieldError>
        </div>
      </div>

      <div>
        <Label htmlFor="p-key">{t("settings.field.api_key")}</Label>
        <Input
          id="p-key"
          type="password"
          value={apiKey}
          onChange={(e) => onApiKey(e.target.value)}
          placeholder={isOpenAi ? "sk-..." : t("settings.field.api_key_optional_placeholder")}
          autoComplete="off"
        />
        {isEditing && (
          <p className="mt-1.5 text-xs text-ink-muted">
            {t("settings.field.api_key_hint")}
          </p>
        )}
        <FieldError>{errors.apiKey}</FieldError>
      </div>

      <div className="flex items-center justify-between gap-3 pt-2">
        <Button variant="ghost" onClick={onCancel}>
          {t("settings.action.cancel")}
        </Button>
        <div className="flex gap-2">
          <Button variant="secondary" onClick={onTest}>
            {t("settings.action.test_connection")}
          </Button>
          <Button variant="primary" onClick={onSave}>
            {t("settings.action.save_provider")}
          </Button>
        </div>
      </div>
    </div>
  );
}
