#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
SRC_APP="${ROOT_DIR}/src-tauri/target/release/bundle/macos/WhisloAI.app"
DST_APP="/Applications/WhisloAI.app"
BUNDLE_ID="${WHISLOAI_BUNDLE_ID:-com.whisloai.desktop}"
SIGN_IDENTITY="${WHISLOAI_CODESIGN_IDENTITY:-}"

if [[ "$(uname -s)" != "Darwin" ]]; then
  echo "This script only works on macOS."
  exit 1
fi

if [[ ! -d "$SRC_APP" ]]; then
  echo "Local app bundle not found at: $SRC_APP"
  echo "First run: npm run tauri build -- --bundles app"
  exit 1
fi

# Close existing app instance to avoid locked files.
osascript -e 'tell application id "com.whisloai.desktop" to quit' >/dev/null 2>&1 || true
sleep 1

echo "Installing app to $DST_APP ..."
ditto "$SRC_APP" "$DST_APP"
xattr -dr com.apple.quarantine "$DST_APP" 2>/dev/null || true

if [[ -n "$SIGN_IDENTITY" ]]; then
  echo "Signing with identity: $SIGN_IDENTITY"
  codesign --force --deep --sign "$SIGN_IDENTITY" --identifier "$BUNDLE_ID" "$DST_APP"
else
  echo "No signing identity configured; using stable ad-hoc signature."
  codesign --force --deep --sign - --identifier "$BUNDLE_ID" "$DST_APP"
fi

echo "Final signature:"
codesign -dv --verbose=4 "$DST_APP" 2>&1 | sed -n '1,25p'

echo
echo "Opening installed app..."
open "$DST_APP"

echo
echo "If Accessibility still appears out of sync, run once:"
echo "  tccutil reset Accessibility $BUNDLE_ID"
echo "  tccutil reset AppleEvents $BUNDLE_ID"
echo "Then re-enable WhisloAI in Settings > Privacy & Security > Accessibility."
echo "In Automation, also enable System Events for WhisloAI."
