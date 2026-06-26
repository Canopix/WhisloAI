import { useEffect } from "react";
import { cn } from "../lib/cn";

export type StatusTone = "neutral" | "loading" | "success" | "error";

interface StatusInlineProps {
  message: string;
  tone: StatusTone;
  className?: string;
  // When tone is success, auto-clear after this many ms (0 = no auto-clear)
  autoClearMs?: number;
  onAutoClear?: () => void;
}

const toneClasses: Record<StatusTone, string> = {
  neutral: "border-border-soft bg-surface-soft text-ink-muted",
  loading:
    "border-state-loading-border bg-state-loading-bg text-state-loading-fg",
  success:
    "border-state-success-border bg-state-success-bg text-state-success-fg",
  error: "border-state-error-border bg-state-error-bg text-state-error-fg",
};

export function StatusInline({
  message,
  tone,
  className,
  autoClearMs = 0,
  onAutoClear,
}: StatusInlineProps) {
  useEffect(() => {
    if (tone === "success" && autoClearMs > 0 && message && onAutoClear) {
      const id = setTimeout(onAutoClear, autoClearMs);
      return () => clearTimeout(id);
    }
  }, [tone, message, autoClearMs, onAutoClear]);

  if (!message) return null;

  return (
    <div
      role="status"
      aria-live="polite"
      className={cn(
        "rounded-xl border px-3 py-2 text-xs font-medium transition-colors",
        toneClasses[tone],
        className,
      )}
    >
      {tone === "loading" && (
        <span className="inline-block h-3 w-3 mr-2 align-middle animate-spin rounded-full border-2 border-current border-t-transparent" />
      )}
      <span className="align-middle">{message}</span>
    </div>
  );
}

export type SectionStatus = {
  message: string;
  tone: StatusTone;
};
