# Toolbox Audio-to-Text Implementation Plan

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a secondary experimental Toolbox section to the main app with a first Audio → Text tool that accepts uploaded audio files and transcribes them through the existing provider/local Whisper pipeline.

**Architecture:** Keep the main widget unchanged and extend only the compact main app. The Toolbox lives as a secondary panel inside the existing main shell, while transcription stays centralized in the existing `transcribe_audio` Tauri command so backend mode selection continues to come from current app settings.

**Tech Stack:** Tauri, vanilla HTML/CSS/JS frontend, Rust backend, existing i18n dictionary, existing local/provider transcription pipeline.

---

## Chunk 1: Plan the UI extension points

### Task 1: Add Toolbox panel structure and copy

**Files:**
- Modify: `src/index.html`
- Modify: `src/i18n.js`

- [ ] **Step 1: Add Toolbox section markup in `src/index.html`**

Add a new secondary panel after `#panel-translate` with:
- a calm section heading
- a small `Experimental` badge
- one `Audio → Text` tool block
- hidden file input
- transcript output textarea
- actions: Copy, Insert at cursor, Open in Translate

- [ ] **Step 2: Add i18n keys in `src/i18n.js`**

Add English and Spanish labels for:
- Toolbox title/subtitle
- Experimental badge
- Audio → Text title/hint
- Choose file CTA
- selected file metadata label
- transcript placeholder
- Open in Translate CTA
- success/loading/error status strings

- [ ] **Step 3: Run syntax check for i18n file**

Run: `node --check src/i18n.js`
Expected: exit 0

## Chunk 2: Add calm, design-system-aligned styling

### Task 2: Style the Toolbox as a secondary utility surface

**Files:**
- Modify: `src/styles.css`

- [ ] **Step 1: Add panel-level Toolbox styles**

Create styles for:
- toolbox header hierarchy
- experimental badge
- audio tool surface
- file picker row
- transcript action row

Use the existing design tokens and keep the visual tone secondary to Translate.

- [ ] **Step 2: Keep responsive behavior within the compact app width**

Ensure the new layout works inside the current compact main app width and does not create horizontal scroll.

## Chunk 3: Reuse the existing transcription path

### Task 3: Extract shared audio transcription helper in main app

**Files:**
- Modify: `src/main.js`

- [ ] **Step 1: Extract blob/file transcription helper**

Create a helper that accepts a `Blob` or file-like audio payload, converts it to base64, and calls `invoke("transcribe_audio", { audioBase64, mimeType })`.

- [ ] **Step 2: Refactor recorded audio flow to use the helper**

Update `transcribeRecordedBlob()` to use the new helper so recording behavior remains unchanged.

- [ ] **Step 3: Keep current status behavior intact for recording**

Preserve existing recording status messaging and button disabling/enabling.

## Chunk 4: Wire Toolbox Audio → Text

### Task 4: Implement uploaded audio flow and transcript actions

**Files:**
- Modify: `src/main.js`

- [ ] **Step 1: Add DOM refs and local state for Toolbox controls**

Wire:
- file input
- choose file button
- file metadata slot
- transcript textarea
- copy/insert/open-to-translate buttons

- [ ] **Step 2: Implement file selection and transcription flow**

When a user selects an audio file:
- validate presence
- show calm loading state
- transcribe via the shared helper
- populate transcript textarea
- show success/error status

- [ ] **Step 3: Reuse existing output actions**

Use the existing helpers for:
- copy transcript
- insert at cursor

Add an `Open in Translate` action that:
- copies transcript into `#translate-input`
- activates the Translate panel
- focuses the input

## Chunk 5: Add minimal backend coverage and verify

### Task 5: Cover MIME-to-filename mapping for uploaded files

**Files:**
- Modify: `src-tauri/src/domain/ai.rs`

- [ ] **Step 1: Add unit tests for `audio_file_name(...)`**

Cover at least:
- `audio/webm` → `recording.webm`
- `audio/ogg` → `recording.ogg`
- `audio/mp4` or `audio/m4a` → `recording.m4a`
- `audio/wav` → `recording.wav`
- uploaded MP3 support if UI accepts `audio/mpeg` (expected: `recording.mp3` after minimal implementation change)

- [ ] **Step 2: Make the smallest backend change only if a test proves it is needed**

Do not change `transcribe_audio` routing logic. Only extend MIME mapping if required for uploaded file compatibility.

- [ ] **Step 3: Verify backend tests**

Run: `cargo test --manifest-path src-tauri/Cargo.toml`
Expected: tests pass

## Final verification

- [ ] Run: `node --check src/main.js`
- [ ] Run: `node --check src/i18n.js`
- [ ] Run: `cargo test --manifest-path src-tauri/Cargo.toml`
- [ ] Manual check: recorded audio still fills Translate as before
- [ ] Manual check: Toolbox accepts a file and returns transcript text
- [ ] Manual check: Copy / Insert / Open in Translate work from Toolbox transcript
- [ ] Manual check: switching transcription mode in Settings affects both recording and uploaded audio because both call `transcribe_audio`

Plan complete and saved to `docs/superpowers/plans/2026-03-23-toolbox-audio-to-text.md`. Ready to execute.
