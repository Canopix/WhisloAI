import { useEffect, useState } from "react";
import { Card, CardContent, CardHeader, CardTitle, CardDescription } from "./ui/Card";
import { Button } from "./ui/Button";
import { Badge } from "./ui/Badge";
import { Label, Select } from "./ui/Field";
import { StatusInline, type SectionStatus } from "./StatusInline";
import { useT } from "../lib/i18n";
import { tauri } from "../lib/tauri";
import type { TranscriptionConfig, WhisperModel } from "../types";

export function TranscriptionCard() {
  const t = useT();
  const [config, setConfig] = useState<TranscriptionConfig | null>(null);
  const [models, setModels] = useState<WhisperModel[]>([]);
  const [modelsDir, setModelsDir] = useState<string>("");
  const [status, setStatus] = useState<SectionStatus>({ message: "", tone: "neutral" });

  const load = () => {
    tauri.getTranscriptionConfig().then((c) => {
      setConfig(c);
      setModelsDir(c.localModelsDir || "");
    }).catch(() => {});
    tauri.listWhisperModels().then(setModels).catch(() => setModels([]));
  };

  useEffect(() => {
    load();
    const un = tauri.listen<{ id: string; progress: number }>(
      "whisper-download-progress",
      (payload) => {
        setModels((prev) =>
          prev.map((m) =>
            m.id === payload.id
              ? { ...m, status: "downloading", downloadProgress: payload.progress }
              : m,
          ),
        );
      },
    );
    return () => { un.then((f) => f()).catch(() => {}); };
  }, []);

  if (!config) return null;

  const isLocal = config.mode === "local";

  const saveMode = (mode: string) => {
    const next: TranscriptionConfig = { ...config, mode };
    setConfig(next);
    persist(next);
  };

  const persist = (next: TranscriptionConfig) => {
    setStatus({ message: t("settings.status.saving"), tone: "loading" });
    tauri.saveTranscriptionConfig(next)
      .then((saved) => {
        setConfig(saved);
        setModelsDir(saved.localModelsDir || "");
        setStatus({ message: t("settings.status.saved"), tone: "success" });
        tauri.listWhisperModels().then(setModels).catch(() => {});
      })
      .catch((err) => setStatus({ message: String(err), tone: "error" }));
  };

  const pickDir = () => {
    tauri.pickWhisperModelsDir()
      .then((dir) => {
        if (dir) {
          setModelsDir(dir);
          persist({ ...config, localModelsDir: dir });
        }
      })
      .catch((err) => setStatus({ message: String(err), tone: "error" }));
  };

  const downloadModel = (modelId: string) => {
    setStatus({
      message: t("settings.status.downloading_model_progress", { id: modelId, percent: 0 }),
      tone: "loading",
    });
    tauri.downloadWhisperModel(modelId).catch((err) =>
      setStatus({ message: String(err), tone: "error" }),
    );
  };

  const dirLocked = !modelsDir.trim();

  return (
    <Card>
      <CardHeader>
        <CardTitle>{t("settings.transcription.title")}</CardTitle>
        <CardDescription>{t("settings.transcription.hint")}</CardDescription>
      </CardHeader>
      <CardContent className="space-y-5">
        <div>
          <Label>{t("settings.transcription.mode")}</Label>
          <Select value={config.mode} onChange={(e) => saveMode(e.target.value)}>
            <option value="api">{t("settings.transcription.mode.api")}</option>
            <option value="local">{t("settings.transcription.mode.local")}</option>
          </Select>
        </div>

        {isLocal && (
          <div className="space-y-4 pt-2 border-t border-border-soft">
            <div>
              <h3 className="text-sm font-bold text-ink mb-1">
                {t("settings.transcription.local_model")}
              </h3>
              <p className="text-xs text-ink-muted">
                {t("settings.transcription.local_hint")}{" "}
                <a
                  href="https://huggingface.co/ggerganov/whisper.cpp"
                  target="_blank"
                  rel="noopener"
                  className="text-accent underline"
                >
                  ggerganov/whisper.cpp
                </a>
              </p>
            </div>

            <div>
              <Label>{t("settings.transcription.models_dir")}</Label>
              <div className="flex gap-2">
                <input
                  type="text"
                  readOnly
                  value={modelsDir || t("settings.transcription.no_models_dir")}
                  className="flex-1 rounded-xl border border-border-soft bg-surface px-3 py-2 text-sm text-ink"
                />
                <Button variant="secondary" size="default" onClick={pickDir}>
                  {t("settings.transcription.select_folder")}
                </Button>
              </div>
              <p className="mt-1.5 text-xs text-ink-muted">
                {t("settings.transcription.models_dir_hint")}
              </p>
            </div>

            <div>
              <Label>{t("settings.transcription.download")}</Label>
              <ul className="space-y-2">
                {models.map((m) => (
                  <li
                    key={m.id}
                    className="flex items-center justify-between gap-3 rounded-xl border border-border-soft px-3 py-2"
                  >
                    <div className="min-w-0">
                      <div className="flex items-center gap-2">
                        <span className="font-mono text-sm text-ink">{m.id}</span>
                        <Badge variant="neutral">{m.sizeLabel}</Badge>
                      </div>
                      {m.status === "downloading" && (
                        <div className="mt-1.5 h-1.5 w-full rounded-full bg-surface-soft overflow-hidden">
                          <div
                            className="h-full bg-accent transition-all"
                            style={{ width: `${m.downloadProgress}%` }}
                          />
                        </div>
                      )}
                      {m.isSelected && (
                        <p className="text-xs text-state-success-fg mt-1">
                          {t("settings.transcription.selected_model_status")}
                        </p>
                      )}
                      {!m.isSelected && m.downloaded && (
                        <p className="text-xs text-ink-muted mt-1">
                          {t("settings.transcription.downloaded_model_status")}
                        </p>
                      )}
                    </div>
                    <Button
                      size="sm"
                      variant={m.isSelected ? "secondary" : "primary"}
                      disabled={m.isSelected || dirLocked || m.status === "downloading"}
                      onClick={() => downloadModel(m.id)}
                    >
                      {m.isSelected
                        ? t("settings.status.active")
                        : m.downloaded
                          ? t("settings.status.active")
                          : t("settings.transcription.download")}
                    </Button>
                  </li>
                ))}
              </ul>
              <p className="mt-1.5 text-xs text-ink-muted">
                {t("settings.transcription.auto_select_hint")}
              </p>
            </div>
          </div>
        )}

        <StatusInline
          message={status.message}
          tone={status.tone}
          autoClearMs={3000}
          onAutoClear={() => setStatus({ message: "", tone: "neutral" })}
        />
      </CardContent>
    </Card>
  );
}
