import test from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import path from "node:path";

const ROOT = path.resolve(path.dirname(new URL(import.meta.url).pathname), "..");

function read(relativePath) {
  return readFileSync(path.join(ROOT, relativePath), "utf8");
}

function expectContainsAll(haystack, needles, messagePrefix) {
  needles.forEach((needle) => {
    assert.ok(
      haystack.includes(needle),
      `${messagePrefix}: missing "${needle}"`,
    );
  });
}

function extractI18nValue(source, key) {
  const escaped = key.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
  const re = new RegExp(`"${escaped}"\\s*:\\s*"([^"]*)"`, "g");
  const matches = Array.from(source.matchAll(re));
  return matches.map((match) => String(match[1] || ""));
}

test("settings includes a dedicated permissions section", () => {
  const html = read("src/settings.html");
  expectContainsAll(
    html,
    [
      'id="permissions-card"',
      'id="permissions-microphone-status"',
      'id="permissions-accessibility-status"',
      'id="permissions-open-microphone-btn"',
      'id="permissions-open-accessibility-btn"',
      'id="permissions-check-microphone-btn"',
      'id="permissions-check-accessibility-btn"',
    ],
    "settings.html",
  );
});

test("settings i18n includes permissions labels and statuses", () => {
  const i18n = read("src/i18n.js");
  expectContainsAll(
    i18n,
    [
      '"settings.nav.general": "General & Permissions"',
      '"settings.nav.general": "General y permisos"',
      '"settings.permissions.title":',
      '"settings.permissions.hint":',
      '"settings.permissions.restart_hint":',
      '"settings.permissions.microphone.title":',
      '"settings.permissions.accessibility.title":',
      '"settings.permissions.open_settings":',
      '"settings.permissions.check":',
      '"settings.permissions.status.not_checked":',
      '"settings.permissions.status.settings_opened":',
      '"settings.status.checking_microphone_permission":',
      '"settings.status.microphone_permission_ready_restart":',
      '"settings.status.microphone_permission_denied":',
      '"settings.status.checking_accessibility_permission":',
      '"settings.status.accessibility_permission_ready_restart":',
      '"settings.status.accessibility_permission_missing":',
    ],
    "src/i18n.js",
  );
});

test("settings logic wires permission actions", () => {
  const js = read("src/settings.js");
  expectContainsAll(
    js,
    [
      'document.getElementById("permissions-microphone-status")',
      'document.getElementById("permissions-accessibility-status")',
      'document.getElementById("permissions-open-microphone-btn")',
      'document.getElementById("permissions-open-accessibility-btn")',
      'document.getElementById("permissions-check-microphone-btn")',
      'document.getElementById("permissions-check-accessibility-btn")',
      "setPermissionInlineStatus(",
      'invoke("open_permission_settings"',
      'invoke("probe_auto_insert_permission")',
    ],
    "src/settings.js",
  );
});

test("onboarding success messages explain app relaunch", () => {
  const i18n = read("src/i18n.js");

  const microphoneGrantedValues = extractI18nValue(i18n, "main.status.microphone_granted");
  const accessibilityReadyValues = extractI18nValue(i18n, "main.status.accessibility_ready");

  assert.equal(microphoneGrantedValues.length, 2, "Expected EN + ES for microphone status");
  assert.equal(accessibilityReadyValues.length, 2, "Expected EN + ES for accessibility status");

  const allValues = [...microphoneGrantedValues, ...accessibilityReadyValues].map((value) =>
    value.toLowerCase(),
  );

  assert.ok(
    allValues.some((value) => value.includes("reopen")),
    "Expected English copy to mention reopening the app",
  );
  assert.ok(
    allValues.some((value) => value.includes("cerr")),
    "Expected Spanish copy to mention closing/reopening the app",
  );
});
