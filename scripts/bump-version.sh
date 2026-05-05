#!/usr/bin/env bash
set -euo pipefail

# Usage: ./scripts/bump-version.sh <new-version>
# Example: ./scripts/bump-version.sh 0.1.25

if [ $# -ne 1 ]; then
  echo "Usage: $0 <new-version>"
  echo "Example: $0 0.1.25"
  exit 1
fi

NEW_VERSION="$1"

# Validate semver format (basic check)
if ! echo "$NEW_VERSION" | grep -qE '^[0-9]+\.[0-9]+\.[0-9]+$'; then
  echo "Error: Version must be in semver format: X.Y.Z"
  echo "Got: $NEW_VERSION"
  exit 1
fi

ROOT_DIR="$(cd "$(dirname "$0")/.." && pwd)"

# Files to update
FILES=(
  "$ROOT_DIR/package.json"
  "$ROOT_DIR/src-tauri/Cargo.toml"
  "$ROOT_DIR/src-tauri/tauri.conf.json"
)

# Read current versions to verify after
OLD_VERSIONS=()
for file in "${FILES[@]}"; do
  if [ ! -f "$file" ]; then
    echo "Error: File not found: $file"
    exit 1
  fi
done

echo "Bumping version to $NEW_VERSION"
echo ""

# Update each file
for file in "${FILES[@]}"; do
  case "$file" in
    *.json)
      # Use node to properly update JSON (handles whitespace/formatting)
      node -e "
        const fs = require('fs');
        const path = '$file';
        const data = JSON.parse(fs.readFileSync(path, 'utf8'));
        data.version = '$NEW_VERSION';
        fs.writeFileSync(path, JSON.stringify(data, null, 2) + '\n');
      "
      ;;
    *.toml)
      # Use sed for TOML
      sed -i '' "s/^version = \".*\"/version = \"$NEW_VERSION\"/" "$file"
      ;;
  esac
  echo "  Updated: $(basename "$(dirname "$file")/$file")"
done

echo ""

# Verify all files match
echo "Verifying..."
echo ""

ALL_OK=true
for file in "${FILES[@]}"; do
  case "$file" in
    *.json)
      CURRENT=$(node -e "const fs = require('fs'); console.log(JSON.parse(fs.readFileSync('$file','utf8')).version)")
      ;;
    *.toml)
      CURRENT=$(grep '^version = ' "$file" | head -1 | sed 's/version = "\(.*\)"/\1/')
      ;;
  esac
  if [ "$CURRENT" = "$NEW_VERSION" ]; then
    echo "  ✓ $(basename "$file"): $CURRENT"
  else
    echo "  ✗ $(basename "$file"): expected $NEW_VERSION, got $CURRENT"
    ALL_OK=false
  fi
done

echo ""

if [ "$ALL_OK" = true ]; then
  echo "All versions synced to $NEW_VERSION"
  echo ""
  echo "Next steps:"
  echo "  1. git add package.json src-tauri/Cargo.toml src-tauri/tauri.conf.json"
  echo "  2. git commit -m \"bump version to $NEW_VERSION\""
  echo "  3. git tag v$NEW_VERSION"
  echo "  4. git push && git push --tags"
else
  echo "Error: Version mismatch detected! Please fix manually."
  exit 1
fi
