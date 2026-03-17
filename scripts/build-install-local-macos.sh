#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
APP_BUNDLE="${ROOT_DIR}/src-tauri/target/release/bundle/macos/WhisloAI.app"

if [[ "$(uname -s)" != "Darwin" ]]; then
  echo "Este script solo aplica en macOS."
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

# En local puede fallar por ausencia de TAURI_SIGNING_PRIVATE_KEY (updater),
# pero el .app queda generado y usable. Continuamos si el bundle existe.
if [[ $BUILD_EXIT -ne 0 && ! -d "$APP_BUNDLE" ]]; then
  echo "Falló el build y no se generó el .app."
  exit $BUILD_EXIT
fi

if [[ $BUILD_EXIT -ne 0 ]]; then
  echo "Build devolvió código $BUILD_EXIT (probablemente por firma de updater),"
  echo "pero el .app existe. Continuando con instalación local..."
fi

bash "${ROOT_DIR}/scripts/install-local-macos.sh"
