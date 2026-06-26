import { useEffect, useState } from "react";
import { Plus } from "lucide-react";
import { Button } from "../components/ui/Button";
import { ProviderCard } from "../components/ProviderCard";
import { ProviderDetail } from "../components/ProviderDetail";
import { useT } from "../lib/i18n";
import { tauri } from "../lib/tauri";
import type { ProviderView, TranscriptionConfig } from "../types";

type Selection =
  | { kind: "list" }
  | { kind: "detail"; provider: ProviderView | null; isNew: boolean };

export function ProvidersSection() {
  const t = useT();
  const [providers, setProviders] = useState<ProviderView[]>([]);
  const [loading, setLoading] = useState(true);
  const [transcriptionConfig, setTranscriptionConfig] =
    useState<TranscriptionConfig | null>(null);
  const [selection, setSelection] = useState<Selection>({ kind: "list" });

  const load = () => {
    setLoading(true);
    Promise.all([tauri.listProviders(), tauri.getTranscriptionConfig()])
      .then(([list, cfg]) => {
        setProviders(list);
        setTranscriptionConfig(cfg);
      })
      .catch(() => {})
      .finally(() => setLoading(false));
  };

  useEffect(() => {
    load();
  }, []);

  const openProvider = (provider: ProviderView) =>
    setSelection({ kind: "detail", provider, isNew: false });

  const newProvider = () =>
    setSelection({ kind: "detail", provider: null, isNew: true });

  const backToList = () => setSelection({ kind: "list" });

  const activeProvider = providers.find((p) => p.isActive);
  const total = providers.length;

  if (selection.kind === "detail") {
    return (
      <div className="py-8">
        <ProviderDetail
          provider={selection.provider}
          isNew={selection.isNew}
          transcriptionConfig={transcriptionConfig}
          onBack={backToList}
          onSaved={() => {
            load();
            backToList();
          }}
          onDeleted={() => {
            load();
            backToList();
          }}
        />
      </div>
    );
  }

  return (
    <div className="py-8">
      <div className="flex items-start justify-between gap-4 mb-6">
        <div>
          <h1 className="text-2xl font-bold text-ink">
            {t("settings.providers.saved.title")}
          </h1>
          <p className="text-sm text-ink-muted mt-1">
            {total === 0
              ? t("settings.providers.none")
              : total === 1
                ? t("settings.providers.summary_one", {
                    active: activeProvider?.name ?? t("settings.providers.active_none"),
                  })
                : t("settings.providers.summary_many", {
                    total,
                    active: activeProvider?.name ?? t("settings.providers.active_none"),
                  })}
          </p>
        </div>
        <Button variant="primary" onClick={newProvider}>
          <Plus className="h-4 w-4" />
          {t("settings.providers.new")}
        </Button>
      </div>

      {loading ? (
        <p className="text-sm text-ink-muted py-8 text-center">
          {t("settings.providers.loading")}
        </p>
      ) : total === 0 ? (
        <div className="rounded-2xl border border-dashed border-border-soft bg-surface-soft p-12 text-center">
          <p className="text-sm text-ink-muted mb-4">{t("settings.providers.none")}</p>
          <Button variant="primary" onClick={newProvider}>
            <Plus className="h-4 w-4" />
            {t("settings.providers.new")}
          </Button>
        </div>
      ) : (
        <div className="grid grid-cols-1 lg:grid-cols-2 gap-4">
          {providers.map((provider) => (
            <ProviderCard key={provider.id} provider={provider} onOpen={openProvider} />
          ))}
        </div>
      )}
    </div>
  );
}
