#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
SRC_APP="${ROOT_DIR}/src-tauri/target/release/bundle/macos/WhisloAI.app"
DST_APP="/Applications/WhisloAI.app"
BUNDLE_ID="${WHISLOAI_BUNDLE_ID:-com.whisloai.desktop}"
SIGN_IDENTITY="${WHISLOAI_CODESIGN_IDENTITY:-}"

if [[ "$(uname -s)" != "Darwin" ]]; then
  echo "Este script solo aplica en macOS."
  exit 1
fi

if [[ ! -d "$SRC_APP" ]]; then
  echo "No se encontró el bundle local en: $SRC_APP"
  echo "Primero ejecutá: npm run tauri build -- --bundles app"
  exit 1
fi

# Cerrar una instancia previa para evitar archivos bloqueados.
osascript -e 'tell application id "com.whisloai.desktop" to quit' >/dev/null 2>&1 || true
sleep 1

echo "Instalando app en $DST_APP ..."
ditto "$SRC_APP" "$DST_APP"
xattr -dr com.apple.quarantine "$DST_APP" 2>/dev/null || true

if [[ -n "$SIGN_IDENTITY" ]]; then
  echo "Firmando con identidad: $SIGN_IDENTITY"
  codesign --force --deep --sign "$SIGN_IDENTITY" --identifier "$BUNDLE_ID" "$DST_APP"
else
  echo "No hay identidad de firma configurada; usando firma ad-hoc estable."
  codesign --force --deep --sign - --identifier "$BUNDLE_ID" "$DST_APP"
fi

echo "Firma final:"
codesign -dv --verbose=4 "$DST_APP" 2>&1 | sed -n '1,25p'

echo
echo "Abriendo app instalada..."
open "$DST_APP"

echo
echo "Si Accessibility sigue desincronizado, ejecutá una sola vez:"
echo "  tccutil reset Accessibility $BUNDLE_ID"
echo "  tccutil reset AppleEvents $BUNDLE_ID"
echo "y volvé a habilitar WhisloAI en Settings > Privacy & Security > Accessibility."
echo "En Automation, habilitá también System Events para WhisloAI."
