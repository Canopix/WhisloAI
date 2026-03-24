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
      'id="permissions-automation-status"',
      'id="permissions-open-microphone-btn"',
      'id="permissions-open-accessibility-btn"',
      'id="permissions-open-automation-btn"',
      'id="permissions-check-microphone-btn"',
      'id="permissions-check-accessibility-btn"',
      'id="permissions-check-automation-btn"',
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
      '"settings.permissions.automation.title":',
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
      '"settings.status.checking_automation_permission":',
      '"settings.status.automation_permission_ready_restart":',
      '"settings.status.automation_permission_missing":',
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
      'document.getElementById("permissions-automation-status")',
      'document.getElementById("permissions-open-microphone-btn")',
      'document.getElementById("permissions-open-accessibility-btn")',
      'document.getElementById("permissions-open-automation-btn")',
      'document.getElementById("permissions-check-microphone-btn")',
      'document.getElementById("permissions-check-accessibility-btn")',
      'document.getElementById("permissions-check-automation-btn")',
      "setPermissionInlineStatus(",
      'invoke("open_permission_settings"',
      'invoke("probe_accessibility_permission")',
      'invoke("probe_system_events_permission")',
    ],
    "src/settings.js",
  );
});

test("onboarding includes dedicated automation step", () => {
  const html = read("src/index.html");
  expectContainsAll(
    html,
    [
      'id="onboarding-accessibility-step"',
      'id="onboarding-automation-step"',
      'id="onboarding-automation-btn"',
      'id="onboarding-automation-settings-btn"',
      'id="onboarding-automation-status"',
    ],
    "src/index.html",
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

test("settings hero reframes General + Permissions as guided setup", () => {
  const html = read("src/settings.html");
  const i18n = read("src/i18n.js");

  expectContainsAll(
    html,
    [
      'class="settings-hero"',
      'data-i18n="settings.hero.eyebrow"',
      'data-i18n="settings.hero.title"',
      'data-i18n="settings.hero.subtitle"',
    ],
    "src/settings.html",
  );

  const heroEyebrowValues = extractI18nValue(i18n, "settings.hero.eyebrow");
  const heroTitleValues = extractI18nValue(i18n, "settings.hero.title");
  const heroSubtitleValues = extractI18nValue(i18n, "settings.hero.subtitle");

  assert.equal(heroEyebrowValues.length, 2, "Expected EN + ES for settings hero eyebrow");
  assert.equal(heroTitleValues.length, 2, "Expected EN + ES for settings hero title");
  assert.equal(heroSubtitleValues.length, 2, "Expected EN + ES for settings hero subtitle");

  const heroValues = [...heroEyebrowValues, ...heroTitleValues, ...heroSubtitleValues].map((value) =>
    value.toLowerCase(),
  );

  assert.ok(
    heroValues.some((value) => value.includes("setup")),
    "Expected English hero copy to frame Settings as setup",
  );
  assert.ok(
    heroValues.some((value) => value.includes("configur")),
    "Expected Spanish hero copy to frame Settings as configuration/setup",
  );
  assert.ok(
    heroValues.some((value) => value.includes("permission") || value.includes("permis")),
    "Expected hero copy to call out permissions explicitly",
  );
});

test("settings General and Permissions sections expose stronger hierarchy hooks", () => {
  const html = read("src/settings.html");

  expectContainsAll(
    html,
    [
      'id="general-card"',
      'data-settings-section="general"',
      'id="general-heading"',
      'aria-labelledby="general-heading"',
      'data-settings-section="permissions"',
      'id="permissions-heading"',
      'aria-labelledby="permissions-heading"',
    ],
    "src/settings.html",
  );
});

test("toolbox lives in settings and no longer in the hidden main surface", () => {
  const settingsHtml = read("src/settings.html");
  const mainHtml = read("src/index.html");
  const settingsJs = read("src/settings.js");
  const i18n = read("src/i18n.js");

  expectContainsAll(
    settingsHtml,
    [
      'id="nav-toolbox"',
      'data-view="toolbox"',
      'id="view-toolbox"',
      'id="settings-audio-file-input"',
      'id="settings-choose-audio-btn"',
      'id="settings-audio-transcript-output"',
      'id="settings-transcript-actions"',
      'id="settings-reset-transcript-btn"',
    ],
    "src/settings.html",
  );

  expectContainsAll(
    settingsJs,
    [
      'document.getElementById("settings-audio-file-input")',
      'document.getElementById("settings-choose-audio-btn")',
      'document.getElementById("settings-audio-transcript-output")',
      'document.getElementById("settings-reset-transcript-btn")',
      'function resetSettingsToolboxState(',
      'invoke("transcribe_audio", {',
    ],
    "src/settings.js",
  );

  expectContainsAll(
    i18n,
    [
      '"settings.nav.toolbox": "Toolbox"',
      '"settings.toolbox.audio_to_text.title":',
      '"settings.toolbox.audio_to_text.choose_file":',
      '"settings.toolbox.audio_to_text.reset":',
    ],
    "src/i18n.js",
  );

  assert.equal(
    mainHtml.includes('id="open-toolbox-btn"'),
    false,
    "src/index.html should not keep the old Toolbox button",
  );
  assert.equal(
    mainHtml.includes('id="panel-toolbox"'),
    false,
    "src/index.html should not keep the old Toolbox panel",
  );
});

test("permissions redesign adds richer explainer and CTA hooks", () => {
  const html = read("src/settings.html");
  const i18n = read("src/i18n.js");

  expectContainsAll(
    html,
    [
      'data-i18n="settings.permissions.explainer"',
      'data-i18n="settings.permissions.cta"',
      'id="permissions-primary-cta"',
    ],
    "src/settings.html",
  );

  const explainerValues = extractI18nValue(i18n, "settings.permissions.explainer");
  const ctaValues = extractI18nValue(i18n, "settings.permissions.cta");

  assert.equal(explainerValues.length, 2, "Expected EN + ES for permissions explainer");
  assert.equal(ctaValues.length, 2, "Expected EN + ES for permissions CTA");

  const explainerLower = explainerValues.map((value) => value.toLowerCase());
  const ctaLower = ctaValues.map((value) => value.toLowerCase());

  assert.ok(
    explainerLower.some(
      (value) => value.includes("microphone") && value.includes("accessibility") && value.includes("automation"),
    ),
    "Expected English explainer to describe microphone, accessibility, and automation together",
  );
  assert.ok(
    explainerLower.some((value) => value.includes("micr") && value.includes("acces") && value.includes("automat")),
    "Expected Spanish explainer to describe microphone, accessibility, and automation together",
  );
  assert.ok(
    ctaLower.some((value) => value.includes("system settings")),
    "Expected English CTA to direct users into system settings",
  );
  assert.ok(
    ctaLower.some((value) => value.includes("configuración") || value.includes("ajustes")),
    "Expected Spanish CTA to direct users into system settings",
  );
});

test("anchor behavior preview uses meaningful fallback copy hooks", () => {
  const html = read("src/settings.html");
  const i18n = read("src/i18n.js");

  expectContainsAll(
    html,
    [
      'data-i18n="settings.general.anchor_behavior.contextual_preview_fallback"',
      'data-i18n="settings.general.anchor_behavior.floating_preview_fallback"',
    ],
    "src/settings.html",
  );

  const contextualValues = extractI18nValue(i18n, "settings.general.anchor_behavior.contextual_preview_fallback");
  const floatingValues = extractI18nValue(i18n, "settings.general.anchor_behavior.floating_preview_fallback");

  assert.equal(contextualValues.length, 2, "Expected EN + ES for contextual preview fallback");
  assert.equal(floatingValues.length, 2, "Expected EN + ES for floating preview fallback");

  const contextualLower = contextualValues.map((value) => value.toLowerCase());
  const floatingLower = floatingValues.map((value) => value.toLowerCase());

  assert.ok(
    contextualLower.some((value) => value.includes("input") || value.includes("cursor")),
    "Expected contextual fallback copy to describe the anchor near active input/cursor",
  );
  assert.ok(
    contextualLower.some((value) => value.includes("campo") || value.includes("cursor") || value.includes("entrada")),
    "Expected Spanish contextual fallback copy to describe the active input/cursor",
  );
  assert.ok(
    floatingLower.some((value) => value.includes("drag") || value.includes("screen")),
    "Expected floating fallback copy to describe the movable on-screen anchor",
  );
  assert.ok(
    floatingLower.some((value) => value.includes("arrastr") || value.includes("pantalla") || value.includes("mover")),
    "Expected Spanish floating fallback copy to describe the movable on-screen anchor",
  );
});

test("anchor behavior preview images cannot be natively dragged", () => {
  const css = read("src/styles.css");
  const gifRuleIndex = css.indexOf(".anchor-behavior-gif {");
  assert.ok(
    gifRuleIndex !== -1,
    "Expected .anchor-behavior-gif rule to exist in styles.css",
  );
  const nextBrace = css.indexOf("}", gifRuleIndex);
  const ruleBlock = css.slice(gifRuleIndex, nextBrace);
  assert.ok(
    ruleBlock.includes("pointer-events: none"),
    `Expected .anchor-behavior-gif rule block to contain pointer-events: none, got: ${ruleBlock}`,
  );
});

test("creator treatment moves into a dedicated settings footer surface", () => {
  const html = read("src/settings.html");

  expectContainsAll(
    html,
    [
      '<footer',
      'id="settings-creator-footer"',
      'data-i18n="settings.creator.title"',
      'data-i18n="settings.creator.follow"',
      'data-i18n="settings.creator.support"',
      'id="creator-profile-link"',
    ],
    "src/settings.html",
  );
});
