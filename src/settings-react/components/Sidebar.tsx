import {
  Settings,
  Plug,
  Brain,
  Wrench,
  type LucideIcon,
} from "lucide-react";
import { cn } from "../lib/cn";
import { useT } from "../lib/i18n";
import { useIsCompact } from "../lib/useViewport";
import type { SectionId } from "../types";

interface SidebarProps {
  active: SectionId;
  onSelect: (id: SectionId) => void;
}

const NAV_ITEMS: { id: SectionId; icon: LucideIcon; i18nKey: string }[] = [
  { id: "general", icon: Settings, i18nKey: "settings.nav.general" },
  { id: "providers", icon: Plug, i18nKey: "settings.nav.providers" },
  { id: "prompts", icon: Brain, i18nKey: "settings.nav.prompts" },
  { id: "toolbox", icon: Wrench, i18nKey: "settings.nav.toolbox" },
];

export function Sidebar({ active, onSelect }: SidebarProps) {
  const t = useT();
  const compact = useIsCompact();

  return (
    <nav
      role="tablist"
      aria-label={t("settings.nav.aria")}
      className={cn(
        "flex flex-col gap-1 p-3 shrink-0 border-r border-border-soft bg-surface/60 transition-[width] duration-duration-medium ease-ease-standard",
        compact ? "w-[64px]" : "w-[220px]",
      )}
    >
      {NAV_ITEMS.map(({ id, icon: Icon, i18nKey }) => {
        const isActive = active === id;
        const label = t(i18nKey);
        return (
          <button
            key={id}
            type="button"
            role="tab"
            aria-selected={isActive}
            aria-label={label}
            title={compact ? label : undefined}
            onClick={() => onSelect(id)}
            className={cn(
              "group flex items-center rounded-xl text-sm font-medium transition-all duration-duration-fast ease-ease-standard",
              compact ? "justify-center p-2.5" : "gap-3 px-3 py-2.5",
              isActive
                ? "bg-accent-soft text-accent-strong border border-accent/20"
                : "text-ink-muted hover:bg-surface-soft hover:text-ink border border-transparent",
            )}
          >
            <Icon
              className={cn(
                "h-5 w-5 shrink-0",
                isActive ? "text-accent" : "text-ink-muted group-hover:text-ink",
              )}
            />
            {!compact && <span className="truncate">{label}</span>}
          </button>
        );
      })}
    </nav>
  );
}
