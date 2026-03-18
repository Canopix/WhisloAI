#!/usr/bin/env bash
set -euo pipefail

BUNDLE_ID="${WHISLOAI_BUNDLE_ID:-com.whisloai.desktop}"
APP_PATH="/Applications/WhisloAI.app"
DO_RESET_KEYCHAIN="${WHISLOAI_RESET_KEYCHAIN:-0}"
CONFIRM="${1:-}"

if [[ "$(uname -s)" != "Darwin" ]]; then
  echo "This script only works on macOS."
  exit 1
fi

if [[ "$CONFIRM" != "--yes" ]]; then
  cat <<EOF
This script resets WhisloAI to an almost "blank" local state for testing.

It will:
  - Close WhisloAI
  - Remove /Applications/WhisloAI.app
  - Remove local data in ~/Library for ${BUNDLE_ID}
  - Reset TCC (All, Accessibility, AppleEvents, Microphone) for ${BUNDLE_ID}

Optional:
  - Delete keychain credentials (WHISLOAI_RESET_KEYCHAIN=1)

To continue:
  bash ./scripts/reset-local-blank-state-macos.sh --yes
EOF
  exit 0
fi

echo "==> Closing WhisloAI (${BUNDLE_ID})..."
osascript -e "tell application id \"${BUNDLE_ID}\" to quit" >/dev/null 2>&1 || true
pkill -f "/Applications/WhisloAI.app/Contents/MacOS/app" >/dev/null 2>&1 || true
pkill -f "WhisloAI" >/dev/null 2>&1 || true
sleep 1

remove_if_exists() {
  local target="$1"
  if [[ -e "$target" ]]; then
    echo "==> Removing: $target"
    rm -rf "$target"
  else
    echo "==> Not found (ok): $target"
  fi
}

echo "==> Cleaning local install and app state..."
remove_if_exists "$APP_PATH"
remove_if_exists "$HOME/Library/Application Support/$BUNDLE_ID"
remove_if_exists "$HOME/Library/Caches/$BUNDLE_ID"
remove_if_exists "$HOME/Library/Preferences/$BUNDLE_ID.plist"
remove_if_exists "$HOME/Library/WebKit/$BUNDLE_ID"
remove_if_exists "$HOME/Library/Saved Application State/$BUNDLE_ID.savedState"

echo "==> Resetting TCC..."
tccutil reset All "$BUNDLE_ID" >/dev/null 2>&1 || true
tccutil reset Accessibility "$BUNDLE_ID" >/dev/null 2>&1 || true
tccutil reset AppleEvents "$BUNDLE_ID" >/dev/null 2>&1 || true
tccutil reset Microphone "$BUNDLE_ID" >/dev/null 2>&1 || true

if [[ "$DO_RESET_KEYCHAIN" == "1" ]]; then
  echo "==> Removing keychain credentials (service=whisloai)..."
  # Entry may not exist on every machine/version.
  security delete-generic-password -s whisloai >/dev/null 2>&1 || true
fi

echo "==> Restarting preferences cache..."
killall cfprefsd >/dev/null 2>&1 || true

cat <<EOF

Done. Local state cleaned for ${BUNDLE_ID}.

Recommended next step:
  npm run build:local

After opening WhisloAI:
  1) Enable Accessibility
  2) Enable Automation -> System Events
  3) Enable Microphone
  4) Quit and reopen WhisloAI once
EOF
