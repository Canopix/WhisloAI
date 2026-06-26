import { useEffect, useState } from "react";
import { Card, CardContent, CardHeader, CardTitle, CardDescription } from "../components/ui/Card";
import { Button } from "../components/ui/Button";
import { Badge } from "../components/ui/Badge";
import { Input, Label, Select } from "../components/ui/Field";
import { StatusInline, type SectionStatus } from "../components/StatusInline";
import { TranscriptionCard } from "../components/TranscriptionCard";
import { useT, useLanguagePreference } from "../lib/i18n";
import { tauri } from "../lib/tauri";
import type { OnboardingStatus, UiSettings } from "../types";

export function GeneralSection() {
  const t = useT();
  return (
    <div className="py-8 space-y-6">
      <div>
        <h1 className="text-2xl font-bold text-ink">{t("settings.general.title")}</h1>
        <p className="text-sm text-ink-muted mt-1">{t("settings.general.hint")}</p>
      </div>
      <LanguageCard />
      <AnchorBehaviorCard />
      <TranscriptionCard />
      <BlockedAppsCard />
      <PermissionsCard />
    </div>
  );
}

/* ---------- Language ---------- */

function LanguageCard() {
  const t = useT();
  const [preference, setPreference] = useLanguagePreference();
  const [status, setStatus] = useState<SectionStatus>({ message: "", tone: "neutral" });

  const handleChange = (value: string) => {
    setPreference(value);
    const next: UiSettings = {
      uiLanguagePreference: value,
      anchorBehavior: "",
    };
    setStatus({ message: t("settings.status.saving"), tone: "loading" });
    tauri.saveUiSettings(next)
      .then(() => {
        setStatus({ message: t("settings.status.saved"), tone: "success" });
      })
      .catch((err) => setStatus({ message: String(err), tone: "error" }));
  };

  return (
    <Card>
      <CardHeader>
        <CardTitle>{t("settings.general.language.label")}</CardTitle>
      </CardHeader>
      <CardContent className="space-y-3">
        <Select
          value={preference}
          onChange={(e) => handleChange(e.target.value)}
        >
          <option value="system">{t("settings.general.language.system")}</option>
          <option value="en">{t("settings.general.language.en")}</option>
          <option value="es">{t("settings.general.language.es")}</option>
        </Select>
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

/* ---------- Anchor behavior ---------- */

function AnchorBehaviorCard() {
  const t = useT();
  const [behavior, setBehavior] = useState<string>("contextual");
  const [supportsContextual, setSupportsContextual] = useState<boolean>(true);
  const [status, setStatus] = useState<SectionStatus>({ message: "", tone: "neutral" });

  useEffect(() => {
    Promise.all([
      tauri.getUiSettings(),
      tauri.getOnboardingStatus(),
    ]).then(([ui, onb]) => {
      setBehavior(ui.anchorBehavior || "contextual");
      setSupportsContextual(onb.supportsContextualAnchor !== false);
    }).catch(() => {});
  }, []);

  const save = (next: string) => {
    setBehavior(next);
    setStatus({ message: t("settings.status.saving"), tone: "loading" });
    tauri.saveUiSettings({ uiLanguagePreference: "system", anchorBehavior: next })
      .then(() => setStatus({ message: t("settings.status.saved"), tone: "success" }))
      .catch((err) => setStatus({ message: String(err), tone: "error" }));
  };

  const options = [
    {
      key: "contextual",
      label: t("settings.general.anchor_behavior.contextual"),
      copy: t("settings.general.anchor_behavior.contextual_copy"),
      gif: "./on-input.gif",
      fallback: t("settings.general.anchor_behavior.contextual_preview_fallback"),
      show: supportsContextual,
    },
    {
      key: "floating",
      label: t("settings.general.anchor_behavior.floating"),
      copy: t("settings.general.anchor_behavior.floating_copy"),
      gif: "./free.gif",
      fallback: t("settings.general.anchor_behavior.floating_preview_fallback"),
      show: true,
    },
  ].filter((o) => o.show);

  return (
    <Card>
      <CardHeader>
        <CardTitle>{t("settings.general.anchor_behavior.label")}</CardTitle>
      </CardHeader>
      <CardContent className="space-y-3">
        <div className="grid grid-cols-1 sm:grid-cols-2 gap-3">
          {options.map((opt) => {
            const selected = behavior === opt.key;
            return (
              <button
                key={opt.key}
                type="button"
                onClick={() => save(opt.key)}
                aria-pressed={selected}
                className={`text-left rounded-xl border p-3 transition-all ${
                  selected
                    ? "border-accent bg-accent-soft ring-2 ring-accent/20"
                    : "border-border-soft hover:border-accent/50"
                }`}
              >
                <div className="aspect-video rounded-lg bg-surface-soft border border-border-soft overflow-hidden mb-3 flex items-center justify-center">
                  <img
                    src={opt.gif}
                    alt=""
                    className="w-full h-full object-cover"
                    onError={(e) => {
                      (e.currentTarget as HTMLImageElement).style.display = "none";
                      const parent = e.currentTarget.parentElement;
                      if (parent) parent.textContent = opt.fallback;
                    }}
                  />
                </div>
                <div className="font-semibold text-sm text-ink">{opt.label}</div>
                <div className="text-xs text-ink-muted mt-0.5">{opt.copy}</div>
                {selected && (
                  <Badge variant="accent" className="mt-2">
                    {t("settings.status.active")}
                  </Badge>
                )}
              </button>
            );
          })}
        </div>
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

/* ---------- Blocked apps ---------- */

function BlockedAppsCard() {
  const t = useT();
  const [bundleIds, setBundleIds] = useState<string[]>([]);
  const [status, setStatus] = useState<SectionStatus>({ message: "", tone: "neutral" });

  const load = () => {
    tauri.getBlockedBundleIds().then(setBundleIds).catch(() => {});
  };

  useEffect(() => {
    load();
    const un = tauri.listen<string[]>("blocked-bundle-ids-updated", load);
    return () => { un.then((f) => f()).catch(() => {}); };
  }, []);

  const blockCurrent = () => {
    setStatus({ message: t("settings.status.saving"), tone: "loading" });
    tauri.blacklistCurrentApp()
      .then((id) => {
        setStatus({ message: t("settings.status.blocked_app_added", { bundleId: id }), tone: "success" });
        load();
      })
      .catch((err) => setStatus({ message: String(err), tone: "error" }));
  };

  const remove = (bundleId: string) => {
    tauri.removeBlockedBundleId(bundleId)
      .then(() => {
        setStatus({ message: t("settings.status.blocked_app_removed", { bundleId }), tone: "success" });
        load();
      })
      .catch((err) => setStatus({ message: String(err), tone: "error" }));
  };

  return (
    <Card>
      <CardHeader>
        <div className="flex items-start justify-between gap-3">
          <div>
            <CardTitle>{t("settings.general.blocked_apps.label")}</CardTitle>
            <CardDescription>{t("settings.general.blocked_apps.hint")}</CardDescription>
          </div>
          <Button size="sm" onClick={blockCurrent}>
            {t("settings.general.blocked_apps.block_current")}
          </Button>
        </div>
      </CardHeader>
      <CardContent className="space-y-3">
        {bundleIds.length === 0 ? (
          <p className="text-sm text-ink-muted py-4 text-center rounded-xl border border-dashed border-border-soft bg-surface-soft">
            {t("settings.general.blocked_apps.empty")}
          </p>
        ) : (
          <ul className="space-y-2">
            {bundleIds.map((id) => (
              <li
                key={id}
                className="flex items-center justify-between gap-3 rounded-xl border border-border-soft px-3 py-2"
              >
                <code className="font-mono text-xs text-ink">{id}</code>
                <Button variant="ghost" size="sm" onClick={() => remove(id)}>
                  {t("settings.general.blocked_apps.remove")}
                </Button>
              </li>
            ))}
          </ul>
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

/* ---------- Permissions ---------- */

function PermissionsCard() {
  const t = useT();
  const [onb, setOnb] = useState<OnboardingStatus | null>(null);

  useEffect(() => {
    tauri.getOnboardingStatus().then(setOnb).catch(() => {});
  }, []);

  const rows = [
    {
      key: "microphone",
      title: t("settings.permissions.microphone.title"),
      show: true,
      open: () => tauri.openPermissionSettings("microphone"),
      check: async () => {
        try {
          await navigator.mediaDevices.getUserMedia({ audio: true });
          return "granted";
        } catch {
          return "denied";
        }
      },
    },
    {
      key: "accessibility",
      title: t("settings.permissions.accessibility.title"),
      show: onb?.needsAccessibility !== false,
      open: () => tauri.openPermissionSettings("accessibility"),
      check: () => tauri.probeAccessibilityPermission().then((ok) => (ok ? "granted" : "denied")),
    },
    {
      key: "automation",
      title: t("settings.permissions.automation.title"),
      show: onb?.needsAutomation !== false,
      open: () => tauri.openPermissionSettings("automation"),
      check: () => tauri.probeSystemEventsPermission().then((ok) => (ok ? "granted" : "denied")),
    },
  ].filter((r) => r.show);

  return (
    <Card>
      <CardHeader>
        <CardTitle>{t("settings.permissions.title")}</CardTitle>
        <CardDescription>{t("settings.permissions.explainer")}</CardDescription>
      </CardHeader>
      <CardContent className="space-y-3">
        {rows.map((row) => (
          <PermissionRow key={row.key} title={row.title} onOpen={row.open} onCheck={row.check} />
        ))}
        <p className="text-xs text-ink-muted pt-2">{t("settings.permissions.restart_hint")}</p>
      </CardContent>
    </Card>
  );
}

function PermissionRow({
  title,
  onOpen,
  onCheck,
}: {
  title: string;
  onOpen: () => void | Promise<void>;
  onCheck: () => Promise<string>;
}) {
  const t = useT();
  const [state, setState] = useState<"idle" | "checking" | "granted" | "denied">("idle");

  const check = async () => {
    setState("checking");
    try {
      const result = await onCheck();
      setState(result === "granted" ? "granted" : "denied");
    } catch {
      setState("idle");
    }
  };

  const badge = () => {
    if (state === "granted") return <Badge variant="success">OK</Badge>;
    if (state === "denied") return <Badge variant="error">{t("settings.permissions.status.not_checked")}</Badge>;
    return <Badge variant="neutral">{t("settings.permissions.status.not_checked")}</Badge>;
  };

  return (
    <div className="flex items-center justify-between gap-3 rounded-xl border border-border-soft px-3 py-2.5">
      <div className="flex items-center gap-3">
        <span className="font-medium text-sm text-ink">{title}</span>
        {badge()}
      </div>
      <div className="flex gap-2">
        <Button variant="secondary" size="sm" onClick={() => onOpen()}>
          {t("settings.permissions.open_settings")}
        </Button>
        <Button variant="ghost" size="sm" onClick={check} disabled={state === "checking"}>
          {t("settings.permissions.check")}
        </Button>
      </div>
    </div>
  );
}
