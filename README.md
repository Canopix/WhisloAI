# BestText (MVP bootstrap)

Minimal Tauri desktop app focused on:
- Improving English text (`Improve`)
- Translating Spanish text to English (`Translate`)
- Configuring AI providers with user API keys (`OpenAI` and `OpenAI-compatible`)

## Tech choices (lightweight)
- Tauri (Rust backend)
- Plain HTML/CSS/JavaScript frontend (no React/Vue)
- Secure API key storage via OS keychain (`keyring` crate)

## Build requirements

- **cmake** – required for local Whisper transcription. On macOS: `brew install cmake`

## Run

```bash
npm install
npm run dev
```

## Build

```bash
npm run build
npm run tauri build
```

## Current status
Implemented:
- Overlay mode on macOS: tiny floating anchor near focused text fields + quick popover
- Quick popover as primary UX (independent from `main`) + separate large `Settings` window
- One-click selected-text actions from popover:
  - `Translate selection` (Spanish -> English)
  - `Improve selection` (English rewrite)
- Provider CRUD (basic add/update + set active)
- `Test connection` (`GET /models`)
- `Improve` and `Translate` calls through OpenAI-compatible Chat Completions API
- Microphone dictation from quick popover (`Record -> transcribe -> translate -> auto-insert`)
- Auto-insert at cursor (`copy + simulated Cmd/Ctrl+V`) with manual paste fallback
- First-run onboarding for permissions (`Microphone` and `Accessibility` on macOS)
- API key secure storage in system credential store
- External text import into `Improve` via launch args/event (`--improve-text` / `--improve-text-file`)
- Global hotkey registration with runtime configurable shortcuts in `Settings`
  - Open `Settings` from the button on the main compact panel

## macOS right-click action (phase 1)
This uses a macOS Quick Action that sends selected text to BestText.

1. Open `Automator` and create a new `Quick Action`.
2. Configure:
   - `Workflow receives current`: `text`
   - `in`: `any application`
3. Add action `Run Shell Script`.
4. Set:
   - `Shell`: `/bin/zsh`
   - `Pass input`: `as arguments`
5. Use this script body:

```bash
/Users/emanuelcanova/Projects/best-text/scripts/macos-context-action.sh "$1"
```

6. Save it as `Improve English with BestText`.

Then, select text in Slack/Teams/any app, right click, and run the quick action.

## Windows context action (phase 1)
This uses an AutoHotkey helper as a practical equivalent for right-click workflows in Windows apps.

Trigger:
- `Shift + Right Click` on selected text
- The helper copies selection and opens BestText with that text in `Improve`

Install (PowerShell):

```powershell
powershell -ExecutionPolicy Bypass -File .\scripts\install-windows-context-action.ps1
```

Uninstall:

```powershell
powershell -ExecutionPolicy Bypass -File .\scripts\uninstall-windows-context-action.ps1
```

Notes:
- Requires AutoHotkey v2 installed.
- You can set `BESTTEXT_EXE` env var if your app binary is in a custom path.

Not implemented yet:
- Native Windows shell extension context menu (current phase 1 uses AutoHotkey helper)
