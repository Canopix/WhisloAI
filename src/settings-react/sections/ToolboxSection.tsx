import { useRef, useState } from "react";
import { Upload, Copy, CornerDownLeft, RotateCcw } from "lucide-react";
import { Card, CardContent, CardHeader, CardTitle, CardDescription } from "../components/ui/Card";
import { Button } from "../components/ui/Button";
import { Badge } from "../components/ui/Badge";
import { Textarea } from "../components/ui/Field";
import { StatusInline, type SectionStatus } from "../components/StatusInline";
import { useT } from "../lib/i18n";
import { tauri } from "../lib/tauri";

const ACCEPTED = ".webm,.mp4,.mp3,.ogg,.opus,.wav,.m4a,audio/webm,audio/mp4,audio/mpeg,audio/ogg,audio/opus,audio/wav,audio/x-wav,audio/m4a,audio/x-m4a";

export function ToolboxSection() {
  const t = useT();
  const fileInputRef = useRef<HTMLInputElement>(null);
  const [fileName, setFileName] = useState<string>("");
  const [transcript, setTranscript] = useState<string>("");
  const [status, setStatus] = useState<SectionStatus>({ message: "", tone: "neutral" });

  const pickFile = () => fileInputRef.current?.click();

  const handleFile = async (e: React.ChangeEvent<HTMLInputElement>) => {
    const file = e.target.files?.[0];
    if (!file) return;
    setFileName(file.name);
    setTranscript("");
    setStatus({ message: t("settings.toolbox.audio_to_text.preparing"), tone: "loading" });

    try {
      const arrayBuffer = await file.arrayBuffer();
      const bytes = new Uint8Array(arrayBuffer);
      let binary = "";
      const chunk = 0x8000;
      for (let i = 0; i < bytes.length; i += chunk) {
        binary += String.fromCharCode.apply(null, Array.from(bytes.subarray(i, i + chunk)) as unknown as number[]);
      }
      const base64 = btoa(binary);
      const mimeType = file.type || "audio/webm";
      setStatus({ message: t("settings.toolbox.audio_to_text.transcribing"), tone: "loading" });
      const result = await tauri.transcribeAudio(base64, mimeType);
      setTranscript(result || "");
      setStatus({ message: t("settings.toolbox.audio_to_text.done"), tone: "success" });
    } catch (err) {
      setStatus({ message: String(err), tone: "error" });
    }
    e.target.value = "";
  };

  const copyTranscript = async () => {
    if (!transcript) {
      setStatus({ message: t("settings.toolbox.audio_to_text.no_transcript"), tone: "error" });
      return;
    }
    try {
      await navigator.clipboard.writeText(transcript);
      setStatus({ message: t("main.status.copied"), tone: "success" });
    } catch (err) {
      setStatus({ message: String(err), tone: "error" });
    }
  };

  const insertTranscript = async () => {
    if (!transcript) {
      setStatus({ message: t("settings.toolbox.audio_to_text.no_transcript"), tone: "error" });
      return;
    }
    setStatus({ message: t("main.status.copying_inserting"), tone: "loading" });
    try {
      const result = await tauri.autoInsertText(transcript);
      if (result?.pasted) {
        setStatus({ message: t("main.status.inserted"), tone: "success" });
      } else {
        setStatus({ message: t("main.status.paste_failed", { shortcut: "Cmd+V" }), tone: "error" });
      }
    } catch {
      setStatus({ message: t("main.status.insert_failed", { shortcut: "Cmd+V" }), tone: "error" });
    }
  };

  const reset = () => {
    setFileName("");
    setTranscript("");
    setStatus({ message: "", tone: "neutral" });
  };

  return (
    <div className="py-8 space-y-6">
      <div>
        <h1 className="text-2xl font-bold text-ink flex items-center gap-3">
          {t("settings.toolbox.title")}
          <Badge variant="warning">{t("settings.toolbox.experimental")}</Badge>
        </h1>
        <p className="text-sm text-ink-muted mt-1">{t("settings.toolbox.hint")}</p>
      </div>

      <Card>
        <CardHeader>
          <CardTitle>{t("settings.toolbox.audio_to_text.title")}</CardTitle>
          <CardDescription>{t("settings.toolbox.audio_to_text.hint")}</CardDescription>
        </CardHeader>
        <CardContent className="space-y-4">
          <input
            ref={fileInputRef}
            type="file"
            accept={ACCEPTED}
            onChange={handleFile}
            className="hidden"
          />

          <button
            type="button"
            onClick={pickFile}
            className="w-full rounded-xl border border-dashed border-border-soft bg-surface-soft hover:border-accent/50 hover:bg-accent-soft/30 transition-colors p-6 flex flex-col items-center gap-2"
          >
            <Upload className="h-6 w-6 text-ink-muted" />
            <span className="text-sm font-medium text-ink">
              {t("settings.toolbox.audio_to_text.choose_file")}
            </span>
            {fileName && (
              <span className="text-xs text-ink-muted">{fileName}</span>
            )}
          </button>

          <div>
            <label className="block mb-1.5 text-xs font-bold text-ink/80">
              {t("settings.toolbox.audio_to_text.transcript")}
            </label>
            <Textarea
              rows={6}
              value={transcript}
              readOnly
              placeholder={t("settings.toolbox.audio_to_text.transcript_placeholder")}
            />
          </div>

          {transcript && (
            <div className="flex items-center justify-end gap-2">
              <Button variant="ghost" size="sm" onClick={reset}>
                <RotateCcw className="h-3.5 w-3.5" />
                {t("settings.toolbox.audio_to_text.reset")}
              </Button>
              <Button variant="secondary" size="sm" onClick={copyTranscript}>
                <Copy className="h-3.5 w-3.5" />
                {t("main.copy_output")}
              </Button>
              <Button variant="secondary" size="sm" onClick={insertTranscript}>
                <CornerDownLeft className="h-3.5 w-3.5" />
                {t("main.translate.insert")}
              </Button>
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
