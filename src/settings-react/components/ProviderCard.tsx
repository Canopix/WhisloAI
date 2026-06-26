import { Cloud, Server } from "lucide-react";
import { Badge } from "./ui/Badge";
import { Button } from "./ui/Button";
import { useT } from "../lib/i18n";
import type { ProviderView } from "../types";
import { cn } from "../lib/cn";

interface ProviderCardProps {
  provider: ProviderView;
  onOpen: (provider: ProviderView) => void;
}

export function ProviderCard({ provider, onOpen }: ProviderCardProps) {
  const t = useT();
  const isOpenAi = provider.providerType !== "openai-compatible";
  const Icon = isOpenAi ? Cloud : Server;

  const statusBadge = () => {
    if (provider.isActive) {
      return (
        <Badge variant="success">
          <span className="h-1.5 w-1.5 rounded-full bg-state-success-fg" />
          {t("settings.status.active")} · {t("settings.status.connected")}
        </Badge>
      );
    }
    if (isOpenAi && !provider.hasApiKey) {
      return <Badge variant="warning">{t("settings.status.missing_api_key")}</Badge>;
    }
    return <Badge variant="neutral">{t("settings.status.not_configured")}</Badge>;
  };

  const ctaLabel = provider.isActive
    ? t("settings.status.edit")
    : provider.hasApiKey || !isOpenAi
      ? t("settings.status.edit")
      : t("settings.status.configure");

  return (
    <button
      type="button"
      onClick={() => onOpen(provider)}
      className={cn(
        "group text-left rounded-2xl border bg-surface p-5 transition-all duration-duration-fast ease-ease-standard",
        "hover:-translate-y-0.5 hover:shadow-[0_12px_28px_rgba(16,38,35,0.1)]",
        provider.isActive
          ? "border-accent/40 ring-1 ring-accent/20"
          : "border-border-soft hover:border-accent/30",
      )}
    >
      <div className="flex items-start justify-between gap-3 mb-4">
        <div className="flex items-center gap-3">
          <div
            className={cn(
              "flex h-11 w-11 items-center justify-center rounded-xl",
              provider.isActive
                ? "bg-accent-soft text-accent"
                : "bg-surface-soft text-ink-muted",
            )}
          >
            <Icon className="h-5 w-5" />
          </div>
          <div>
            <div className="font-bold text-base text-ink leading-tight">
              {provider.name}
            </div>
            <div className="text-xs text-ink-muted mt-0.5">
              {isOpenAi ? t("settings.field.type.openai") : t("settings.field.type.openai_compatible")}
            </div>
          </div>
        </div>
        {statusBadge()}
      </div>

      <dl className="space-y-1.5 text-xs">
        <div className="flex items-baseline gap-2">
          <dt className="text-ink-muted shrink-0">{t("settings.field.translate_model")}:</dt>
          <dd className="font-mono text-ink truncate">
            {provider.translateModel || "—"}
          </dd>
        </div>
        <div className="flex items-baseline gap-2">
          <dt className="text-ink-muted shrink-0">{t("settings.field.transcribe_model")}:</dt>
          <dd className="font-mono text-ink truncate">
            {provider.transcribeModel || "—"}
          </dd>
        </div>
      </dl>

      <div className="mt-4 pt-3 border-t border-border-soft">
        <Button
          variant={provider.isActive ? "secondary" : "primary"}
          size="sm"
          className="w-full"
          onClick={(e) => {
            e.stopPropagation();
            onOpen(provider);
          }}
        >
          {ctaLabel}
        </Button>
      </div>
    </button>
  );
}
