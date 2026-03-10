<div align="center">
  <img src="./app-icon.png" alt="WhisloAI logo" width="120" />
  <h1>WhisloAI</h1>
  <p>Desktop AI writing assistant for instant text improvement and translation.</p>
</div>

<p align="center">
  <a href="https://github.com/Canopix/WhisloAI/stargazers"><img alt="GitHub stars" src="https://img.shields.io/github/stars/Canopix/WhisloAI?style=social"></a>
  <a href="https://github.com/Canopix/WhisloAI/issues"><img alt="GitHub issues" src="https://img.shields.io/github/issues/Canopix/WhisloAI"></a>
  <a href="https://github.com/Canopix/WhisloAI"><img alt="Platform" src="https://img.shields.io/badge/platform-desktop-blue"></a>
</p>

## Overview

WhisloAI helps you rewrite and translate text from anywhere in your desktop workflow.

Current MVP focuses on:

- `Improve`: rewrite English text
- `Translate`: Spanish to English
- Bring-your-own provider credentials (`OpenAI` and OpenAI-compatible endpoints)
- Quick overlay actions near focused text inputs

## Key Features

- Lightweight desktop app built with Tauri
- macOS floating anchor + quick popover workflow
- One-click selected text actions (`Translate selection`, `Improve selection`)
- Provider management (add/update/select active provider)
- Connection test against `/models`
- Voice dictation path (`Record -> transcribe -> translate -> insert`)
- Auto-insert at cursor with manual paste fallback
- First-run onboarding for required permissions
- Secure API key storage via system credential store
- External text import (`--improve-text`, `--improve-text-file`)
- Configurable global hotkeys from Settings

## Tech Stack

- Tauri (Rust backend)
- HTML/CSS/JavaScript frontend
- Secure key storage with Rust `keyring`

## Requirements

- Node.js 20+
- Rust toolchain (stable)
- Tauri system dependencies for your OS
- `cmake` (required for local Whisper transcription)
  - macOS: `brew install cmake`

## Quick Start

```bash
npm install
npm run dev
```

## Build

```bash
npm run build
npm run tauri build
```

## Usage

### Core flow

1. Open a text field in any app
2. Trigger WhisloAI popover/anchor
3. Choose `Improve selection` or `Translate selection`
4. Insert output back into your active text field

### macOS right-click action (phase 1)

Use a macOS Quick Action to send selected text to WhisloAI.

1. Open Automator and create a new `Quick Action`
2. Set:
   - `Workflow receives current`: `text`
   - `in`: `any application`
3. Add action `Run Shell Script`
4. Set:
   - `Shell`: `/bin/zsh`
   - `Pass input`: `as arguments`
5. Script body:

```bash
<absolute-path-to-repo>/scripts/macos-context-action.sh "$1"
```

6. Save it as `Improve English with WhisloAI`

### Windows context action (phase 1)

Current implementation uses an AutoHotkey helper.

- Trigger: `Shift + Right Click` on selected text
- Installs helper scripts that copy selection and open WhisloAI in `Improve`

Install:

```powershell
powershell -ExecutionPolicy Bypass -File .\scripts\install-windows-context-action.ps1
```

Uninstall:

```powershell
powershell -ExecutionPolicy Bypass -File .\scripts\uninstall-windows-context-action.ps1
```

Notes:

- Requires AutoHotkey v2
- Optional env var: `WHISLOAI_EXE` for custom executable path

## Privacy and Security

- API keys are stored in the OS credential store (not plain text config files)
- You control provider selection and credentials
- Recommended before public deployment: add explicit telemetry and data-handling documentation

## Project Status

WhisloAI is an active MVP and moving toward a broader open source release.

Not implemented yet:

- Native Windows shell extension context menu (phase 1 currently uses AutoHotkey)

## Documentation

- Product spec (Spanish): [`docs/especificacion-funcional-mvp.md`](./docs/especificacion-funcional-mvp.md)

## Contributing

Contributions are welcome.

Suggested flow:

1. Open an issue with context and expected behavior
2. Submit a focused PR with clear testing notes
3. Keep changes scoped and avoid mixing refactors with feature work

## Maintainer

- Built by [@emanuel_build](https://x.com/emanuel_build)

<p align="center">
  <img src="./white-icon.png" alt="WhisloAI mark" width="140" />
</p>
