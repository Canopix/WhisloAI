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

test("tauri config enables updater plugin with GitHub latest endpoint", () => {
  const conf = read("src-tauri/tauri.conf.json");
  expectContainsAll(
    conf,
    [
      '"plugins"',
      '"updater"',
      '"endpoints"',
      '"https://github.com/Canopix/WhisloAI/releases/latest/download/latest.json"',
      '"pubkey"',
    ],
    "src-tauri/tauri.conf.json",
  );
});

test("tauri capabilities allow updater commands", () => {
  const capabilities = read("src-tauri/capabilities/default.json");
  expectContainsAll(
    capabilities,
    [
      '"updater:default"',
    ],
    "src-tauri/capabilities/default.json",
  );
});

test("rust app wires updater plugin, startup auto-check and tray menu action", () => {
  const lib = read("src-tauri/src/lib.rs");
  expectContainsAll(
    lib,
    [
      'const TRAY_MENU_CHECK_UPDATES: &str = "tray-check-updates";',
      '.text(TRAY_MENU_CHECK_UPDATES, "Check for updates")',
      '.plugin(tauri_plugin_updater::Builder::new().build())',
      'start_background_update_check(app.handle().clone(), UpdateCheckTrigger::Startup);',
      'start_background_update_check(app.clone(), UpdateCheckTrigger::TrayMenu);',
    ],
    "src-tauri/src/lib.rs",
  );
});

test("release workflow exports updater signing secrets", () => {
  const workflow = read(".github/workflows/release.yml");
  expectContainsAll(
    workflow,
    [
      'TAURI_SIGNING_PRIVATE_KEY: ${{ secrets.TAURI_SIGNING_PRIVATE_KEY }}',
      'TAURI_SIGNING_PRIVATE_KEY_PASSWORD: ${{ secrets.TAURI_SIGNING_PRIVATE_KEY_PASSWORD }}',
    ],
    ".github/workflows/release.yml",
  );
});
