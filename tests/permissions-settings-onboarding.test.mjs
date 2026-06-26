import test from "node:test";
import assert from "node:assert/strict";
import { readFileSync, existsSync } from "node:fs";
import path from "node:path";

const ROOT = path.resolve(path.dirname(new URL(import.meta.url).pathname), "..");

function read(relativePath) {
  return readFileSync(path.join(ROOT, relativePath), "utf8");
}

function readOpt(relativePath) {
  const full = path.join(ROOT, relativePath);
  return existsSync(full) ? readFileSync(full, "utf8") : null;
}

function expectContainsAll(haystack, needles, messagePrefix) {
  needles.forEach((needle) => {
    assert.ok(haystack.includes(needle), `${messagePrefix}: missing "${needle}"`);
  });
}

test("settings.html is the React SPA entry", () => {
  const html = read("settings.html");
  expectContainsAll(html, ['id="root"', "./src/settings-react/main.tsx", "/i18n.js"], "settings.html");
});

test("React shell and sections exist", () => {
  const required = [
    "src/settings-react/main.tsx",
    "src/settings-react/App.tsx",
    "src/settings-react/sections/GeneralSection.tsx",
    "src/settings-react/sections/ProvidersSection.tsx",
    "src/settings-react/sections/PromptsSection.tsx",
    "src/settings-react/sections/ToolboxSection.tsx",
    "src/settings-react/components/Sidebar.tsx",
    "src/settings-react/components/ProviderCard.tsx",
    "src/settings-react/components/ProviderDetail.tsx",
    "src/settings-react/components/TranscriptionCard.tsx",
    "src/settings-react/lib/tauri.ts",
    "src/settings-react/lib/i18n.ts",
  ];
  required.forEach((f) => {
    assert.ok(existsSync(path.join(ROOT, f)), `missing React source: ${f}`);
  });
});

test("General section covers language, anchor, transcription, blocked apps, permissions", () => {
  const src = read("src/settings-react/sections/GeneralSection.tsx");
  expectContainsAll(src, ["LanguageCard", "AnchorBehaviorCard", "BlockedAppsCard", "PermissionsCard"], "GeneralSection");
  const transcription = read("src/settings-react/components/TranscriptionCard.tsx");
  expectContainsAll(transcription, ["transcription.mode", "whisper"], "TranscriptionCard");
});

test("Providers section uses card + detail view pattern", () => {
  const section = read("src/settings-react/sections/ProvidersSection.tsx");
  expectContainsAll(section, ["ProviderCard", "ProviderDetail", "kind: \"list\"", "kind: \"detail\""], "ProvidersSection");
  const detail = read("src/settings-react/components/ProviderDetail.tsx");
  expectContainsAll(detail, ["\"view\"", "\"edit\"", "handleSetActive", "handleDelete"], "ProviderDetail");
});

test("Build config is set up for Vite multi-entry", () => {
  const vite = read("vite.config.ts");
  expectContainsAll(vite, ["@vitejs/plugin-react", "settings.html", "outDir: \"dist\""], "vite.config.ts");
});

test("vanilla windows are untouched in public/", () => {
  ["public/index.html", "public/widget.html", "public/anchor.html", "public/styles.css", "public/main.js", "public/widget.js", "public/anchor.js"].forEach(
    (f) => {
      assert.ok(existsSync(path.join(ROOT, f)), `missing vanilla asset: ${f}`);
    },
  );
});

test("old vanilla settings.js has been removed", () => {
  assert.ok(!existsSync(path.join(ROOT, "src/settings.js")), "src/settings.js should be deleted");
});

test("i18n keys used by the React UI exist in both languages", () => {
  const i18n = read("public/i18n.js");
  const keys = [
    "settings.nav.general",
    "settings.nav.providers",
    "settings.nav.prompts",
    "settings.nav.toolbox",
    "settings.status.saving",
    "settings.status.saved",
    "settings.status.active",
    "settings.status.connected",
    "settings.status.not_configured",
    "settings.status.missing_api_key",
    "settings.transcription.title",
    "settings.providers.saved.title",
    "settings.providers.new",
  ];
  expectContainsAll(i18n, keys, "i18n.js");
});
