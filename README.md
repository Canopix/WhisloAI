<div align="center">
  <img src="./src/Banner.png" alt="WhisloAI banner" width="100%" />
  <h1>WhisloAI</h1>
  <p>Desktop AI assistant for fast translation and dictation from any app.</p>
</div>

<p align="center">
  <a href="https://github.com/Canopix/WhisloAI/stargazers"><img alt="GitHub stars" src="https://img.shields.io/github/stars/Canopix/WhisloAI?style=social"></a>
  <a href="https://github.com/Canopix/WhisloAI/issues"><img alt="GitHub issues" src="https://img.shields.io/github/issues/Canopix/WhisloAI"></a>
  <a href="https://github.com/Canopix/WhisloAI"><img alt="Platform" src="https://img.shields.io/badge/platform-desktop-blue"></a>
</p>

## Download

Download the latest prebuilt app for macOS, Windows, or Linux:

- https://github.com/Canopix/WhisloAI/releases

## Overview

WhisloAI helps you translate and dictate text from anywhere in your desktop workflow.

## Why I Built This

I built WhisloAI because I want to keep improving my English every day.

I used to do the same loop all the time: write something, translate it, tweak the tone, paste it back, and repeat. After a while, I thought: why not automate this?

So this app started as a personal tool for my daily workflow. I am sharing it publicly because I want honest feedback, I want to see if it is useful to others, and I want to keep learning by building in public with the community.

Current MVP focuses on:

- `Translate`: source language to target language (configurable in Settings)
- `Dictate`: record voice, transcribe, translate, and insert back
- Bring-your-own provider credentials (`OpenAI` and OpenAI-compatible endpoints)
- Quick overlay actions near focused text inputs

## Key Features

- Lightweight desktop app built with Tauri
- macOS floating anchor + quick popover workflow
- One-click selected text translation (`Translate selection`)
- Provider management (add/update/select active provider)
- Connection test against `/models`
- Voice dictation path (`Record -> transcribe -> translate -> insert`)
- Auto-insert at cursor with manual paste fallback
- First-run onboarding for required permissions
- Local API key persistence for configured providers

## Tech Stack

- Tauri (Rust backend)
- HTML/CSS/JavaScript frontend
- Local settings + provider configuration persisted on disk

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

## Publish a Release (Maintainers)

1. Update version values:
   - `package.json`
   - `src-tauri/Cargo.toml`
   - `src-tauri/tauri.conf.json`
2. Commit and push your changes.
3. Create and push a release tag:

```bash
git tag v0.1.1
git push origin v0.1.1
```

4. GitHub Actions workflow `.github/workflows/release.yml` builds artifacts for macOS, Windows, and Linux, then attaches them to a GitHub Release.

## Usage

### Core flow

1. Open a text field in any app
2. Open the WhisloAI widget
3. Choose `Translate selection` (or use dictation)
4. Insert output back into your active text field

### Widget workflow

- Keep the widget open while writing
- Use quick actions to translate selected text or dictate
- Configure provider, languages, and writing modes from `Settings`

## Privacy and Security

- API keys are currently persisted locally in app configuration (base64-encoded, not encrypted) to keep setup simple during MVP
- You control provider selection and credentials
- Recommended before public deployment: add explicit telemetry and data-handling documentation

## Project Status

WhisloAI is an active MVP and moving toward a broader open source release.

## License

This project is licensed under the PolyForm Noncommercial 1.0.0 license.

- Noncommercial use is allowed under the terms in [LICENSE](./LICENSE).
- Commercial use requires a separate commercial license from the maintainer.

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
