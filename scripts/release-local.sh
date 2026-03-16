#!/usr/bin/env bash
set -euo pipefail

usage() {
  cat <<'EOF'
Genera artifacts locales de release (sin tags ni GitHub Release).

Uso:
  ./scripts/release-local.sh [opciones]

Opciones:
  --no-install          No ejecuta npm ci
  --no-build            No ejecuta npx tauri build
  --output-dir <dir>    Carpeta base de salida (default: local-artifacts/releases)
  --dry-run             Muestra comandos sin ejecutarlos
  -h, --help            Muestra esta ayuda
EOF
}

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
OUTPUT_BASE_REL="local-artifacts/releases"
NO_INSTALL=0
NO_BUILD=0
DRY_RUN=0

run_cmd() {
  if [[ "$DRY_RUN" -eq 1 ]]; then
    printf '+'
    for arg in "$@"; do
      printf ' %q' "$arg"
    done
    printf '\n'
  else
    "$@"
  fi
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    --no-install)
      NO_INSTALL=1
      shift
      ;;
    --no-build)
      NO_BUILD=1
      shift
      ;;
    --output-dir)
      if [[ $# -lt 2 ]]; then
        echo "Falta valor para --output-dir" >&2
        exit 1
      fi
      OUTPUT_BASE_REL="$2"
      shift 2
      ;;
    --dry-run)
      DRY_RUN=1
      shift
      ;;
    -h|--help)
      usage
      exit 0
      ;;
    *)
      echo "Opcion desconocida: $1" >&2
      usage >&2
      exit 1
      ;;
  esac
done

require_cmd() {
  if ! command -v "$1" >/dev/null 2>&1; then
    echo "Comando requerido no encontrado: $1" >&2
    exit 1
  fi
}

require_cmd npm
require_cmd npx
require_cmd node

configure_macos_build_env() {
  if [[ "$(uname -s)" != "Darwin" ]]; then
    return
  fi

  local arch deployment_target
  arch="$(uname -m)"
  deployment_target="10.15"

  if [[ "$arch" == "arm64" ]]; then
    deployment_target="11.0"
    export CMAKE_GGML_CPU_ARM_ARCH="armv8.2-a+dotprod"
    export CFLAGS="-march=armv8.2-a+dotprod -U__ARM_FEATURE_MATMUL_INT8"
    export CXXFLAGS="-march=armv8.2-a+dotprod -U__ARM_FEATURE_MATMUL_INT8"
    export CFLAGS_aarch64_apple_darwin="$CFLAGS"
    export CXXFLAGS_aarch64_apple_darwin="$CXXFLAGS"
  fi

  export MACOSX_DEPLOYMENT_TARGET="$deployment_target"
  export CMAKE_OSX_DEPLOYMENT_TARGET="$deployment_target"
  export CMAKE_GGML_NATIVE="OFF"
  export CMAKE_GGML_CPU_ALL_VARIANTS="OFF"

  if [[ "$DRY_RUN" -eq 1 ]]; then
    echo "+ export MACOSX_DEPLOYMENT_TARGET=$MACOSX_DEPLOYMENT_TARGET"
    echo "+ export CMAKE_OSX_DEPLOYMENT_TARGET=$CMAKE_OSX_DEPLOYMENT_TARGET"
    if [[ "$arch" == "arm64" ]]; then
      echo "+ export CFLAGS=$CFLAGS"
      echo "+ export CXXFLAGS=$CXXFLAGS"
    fi
  else
    echo "macOS build env: arch=$arch, deployment_target=$MACOSX_DEPLOYMENT_TARGET"
  fi
}

cd "$ROOT_DIR"

VERSION="$(node -p "require('./package.json').version")"
STAMP="$(date +%Y%m%d-%H%M%S)"
OUTPUT_BASE_ABS="${ROOT_DIR}/${OUTPUT_BASE_REL}"
if [[ "$OUTPUT_BASE_REL" == /* ]]; then
  OUTPUT_BASE_ABS="$OUTPUT_BASE_REL"
fi
RELEASE_DIR="${OUTPUT_BASE_ABS}/v${VERSION}-${STAMP}"
BUNDLE_DIR="${ROOT_DIR}/src-tauri/target/release/bundle"

if [[ "$NO_INSTALL" -eq 0 ]]; then
  run_cmd npm ci
fi

if [[ "$NO_BUILD" -eq 0 ]]; then
  configure_macos_build_env
  run_cmd npx tauri build
fi

if [[ "$DRY_RUN" -eq 1 ]]; then
  run_cmd mkdir -p "$RELEASE_DIR"
  run_cmd cp -R "$BUNDLE_DIR" "$RELEASE_DIR/"
  echo
  echo "Dry run completado."
  echo "Artifacts quedarían en: $RELEASE_DIR"
  exit 0
fi

if [[ ! -d "$BUNDLE_DIR" ]]; then
  echo "No se encontro la carpeta de artifacts: $BUNDLE_DIR" >&2
  echo "Ejecuta el script sin --no-build o corre npx tauri build manualmente." >&2
  exit 1
fi

mkdir -p "$RELEASE_DIR"
cp -R "$BUNDLE_DIR" "$RELEASE_DIR/"

if command -v shasum >/dev/null 2>&1; then
  (
    cd "$RELEASE_DIR"
    find bundle -type f -print0 | sort -z | xargs -0 env LC_ALL=C LC_CTYPE=C LANG=C shasum -a 256 > SHA256SUMS.txt
  )
elif command -v sha256sum >/dev/null 2>&1; then
  (
    cd "$RELEASE_DIR"
    find bundle -type f -print0 | sort -z | xargs -0 sha256sum > SHA256SUMS.txt
  )
fi

cat > "${RELEASE_DIR}/README.txt" <<EOF
WhisloAI local release artifacts

Version: v${VERSION}
Generated: ${STAMP}

Contenido:
- bundle/          Artifacts generados por Tauri listos para subir manualmente
- SHA256SUMS.txt   Checksums (si shasum/sha256sum esta disponible)
EOF

echo "Listo."
echo "Artifacts locales en: $RELEASE_DIR"
