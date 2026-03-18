#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
APP_BUNDLE="${ROOT_DIR}/src-tauri/target/release/bundle/macos/WhisloAI.app"

if [[ "$(uname -s)" != "Darwin" ]]; then
  echo "This script only works on macOS."
  exit 1
fi

cd "$ROOT_DIR"

echo "Build frontend..."
npm run build

echo "Build Tauri bundle app..."
set +e
npx tauri build --bundles app
BUILD_EXIT=$?
set -e

# Local builds may fail without TAURI_SIGNING_PRIVATE_KEY (updater),
# but the .app may still be generated and usable. Continue if bundle exists.
if [[ $BUILD_EXIT -ne 0 && ! -d "$APP_BUNDLE" ]]; then
  echo "Build failed and no .app bundle was generated."
  exit $BUILD_EXIT
fi

if [[ $BUILD_EXIT -ne 0 ]]; then
  echo "Build returned exit code $BUILD_EXIT (likely updater signing),"
  echo "but the .app exists. Continuing with local installation..."
fi

bash "${ROOT_DIR}/scripts/install-local-macos.sh"
