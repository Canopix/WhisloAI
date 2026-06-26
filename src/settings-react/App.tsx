import { useEffect, useState } from "react";
import { Sidebar } from "./components/Sidebar";
import { GeneralSection } from "./sections/GeneralSection";
import { ProvidersSection } from "./sections/ProvidersSection";
import { PromptsSection } from "./sections/PromptsSection";
import { ToolboxSection } from "./sections/ToolboxSection";
import { useT, notifyUiLanguageChanged } from "./lib/i18n";
import { tauri } from "./lib/tauri";
import { cn } from "./lib/cn";
import { useIsCompact } from "./lib/useViewport";
import type { SectionId } from "./types";

const VALID_SECTIONS: SectionId[] = ["general", "providers", "prompts", "toolbox"];

function sectionFromHash(): SectionId {
  const raw = window.location.hash.replace(/^#/, "") as SectionId;
  return VALID_SECTIONS.includes(raw) ? raw : "general";
}

export function App() {
  const t = useT();
  const compact = useIsCompact();
  const [section, setSection] = useState<SectionId>(() => sectionFromHash());
  const [appVersion, setAppVersion] = useState<string>("—");

  // The backend emits this after persisting a language preference change.
  useEffect(() => {
    const unlistenP = tauri.listen<unknown>("ui-language-changed", () => {
      notifyUiLanguageChanged();
    });
    return () => {
      unlistenP.then((un) => un()).catch(() => {});
    };
  }, []);

  useEffect(() => {
    tauri.getAppVersion().then((v) => setAppVersion(v || "—")).catch(() => {});
  }, []);

  // Keep URL hash in sync with the active section (enables deep-linking).
  useEffect(() => {
    const next = `#${section}`;
    if (window.location.hash !== next) {
      window.history.replaceState(null, "", next);
    }
  }, [section]);

  // Re-apply attribute-based translations on the document and force a re-render
  // whenever the language version bumps.
  useEffect(() => {
    window.WhisloAII18n?.applyTranslations(document);
  });

  return (
    <div className="flex h-screen w-screen overflow-hidden bg-bg">
      <div className="flex w-full mx-auto max-w-6xl">
        <Sidebar active={section} onSelect={setSection} />
        <main className="flex-1 flex flex-col min-w-0">
          <header
            className={cn(
              "flex items-center justify-between border-b border-border-soft py-4",
              compact ? "px-5" : "px-8 py-5",
            )}
          >
            <div className="min-w-0">
              <p className="text-xs font-semibold tracking-wide uppercase text-accent">
                {t("settings.hero.eyebrow")}
              </p>
              <h1 className="text-xl font-bold text-ink mt-0.5 truncate">
                {t("settings.hero.title")}
              </h1>
            </div>
            <span className="font-mono text-xs text-ink-muted shrink-0 ml-4 hidden sm:inline">
              {t("settings.version.installed", { version: appVersion })}
            </span>
          </header>
          <div className="flex-1 overflow-y-auto">
            <div
              className={cn(
                "mx-auto w-full",
                compact ? "px-5 max-w-3xl" : "px-8 max-w-4xl",
              )}
            >
              {section === "general" && <GeneralSection />}
              {section === "providers" && <ProvidersSection />}
              {section === "prompts" && <PromptsSection />}
              {section === "toolbox" && <ToolboxSection />}
            </div>
          </div>
        </main>
      </div>
    </div>
  );
}
