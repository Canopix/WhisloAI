# AGENTS.md

## Release Process

**Always use the bump script to sync versions — never edit version files manually.**

### Step 1: Bump version

```bash
./scripts/bump-version.sh <new-version>
# Example: ./scripts/bump-version.sh 0.1.26
```

This updates versions across the 3 files that matter:
- `package.json`
- `src-tauri/Cargo.toml`
- `src-tauri/tauri.conf.json`

### Step 2: Commit changes

```bash
git add package.json src-tauri/Cargo.toml src-tauri/tauri.conf.json
git commit -m "bump version to 0.1.26"
```

### Step 3: Tag and push

```bash
git tag v0.1.26
git push && git push --tags
```

The GitHub Actions workflow `release.yml` triggers automatically on the tag and generates the release with the correct `latest.json`.

### ⚠️ Critical

- **NEVER** set the `tauri.conf.json` version manually — always use the script
- The `latest.json` that GitHub Actions generates reads the version from `tauri.conf.json`
- If `tauri.conf.json` is not in sync with the tag, the app's updater won't detect the update
- `package-lock.json` and `Cargo.lock` are regenerated automatically during the build
