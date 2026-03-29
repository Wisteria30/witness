# Releasing

## Quick Start

```bash
# 1. Sync version across Cargo.toml, plugin.json, marketplace.json
scripts/release.sh 1.1.0

# 2. Commit & PR
git add -A && git commit -m "release: v1.1.0"
git push origin HEAD

# 3. Merge PR -> GitHub Actions auto-creates release with 4-platform binaries
```

## How it works

### Version management

| File | Role |
|------|------|
| `Cargo.toml` | Single source of truth. Embedded in binary (`--version`) |
| `.claude-plugin/plugin.json` | SessionStart version check against binary |
| `marketplace.json` | Marketplace display |

`scripts/release.sh` syncs all three.

### CI protection (on PR)

`version-lint` enforces:
1. Version bump required when `src/`, `hooks/`, `rules/`, `skills/`, etc. change
2. `Cargo.toml` version must differ from base branch

### CD (on main merge)

`release.yml` auto-triggers:
1. Detects `Cargo.toml` version change
2. Builds for 4 platforms (macOS aarch64/x86_64, Linux aarch64/x86_64)
3. Creates `v{version}` tag and GitHub Release with binaries

No manual tagging needed.

### User auto-update

On Claude Code session start, `session-start.sh`:
1. Compares `plugin.json` version with binary `--version`
2. On mismatch: deletes binary, re-runs `setup`
3. `setup` downloads matching release binary (falls back to cargo build)

## Flow

```
scripts/release.sh 1.1.0
  |
Cargo.toml = 1.1.0
plugin.json = 1.1.0
marketplace.json = 1.1.0
  |
PR -> CI: version-lint pass
  |
Merge to main
  |
CD: version change detected -> build 4 platforms -> v1.1.0 tag -> GitHub Release
  |
User updates plugin -> plugin.json = 1.1.0
  |
Claude Code session start -> session-start.sh
  |
Binary 1.0.0 != plugin.json 1.1.0 -> setup -> download v1.1.0
```
