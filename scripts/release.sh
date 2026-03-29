#!/usr/bin/env bash
set -euo pipefail

if [ "$#" -ne 1 ]; then
  echo "usage: scripts/release.sh <version>" >&2
  exit 1
fi

VERSION="$1"
ROOT="$(cd "$(dirname "$0")/.." && pwd)"

if ! command -v jq >/dev/null 2>&1; then
  echo "witness: jq is required for version sync" >&2
  exit 1
fi

# Cargo.toml (sed)
sed -i.bak "s/^version = \"[^\"]*\"/version = \"$VERSION\"/" "$ROOT/Cargo.toml"
rm -f "$ROOT/Cargo.toml.bak"

# plugin.json (jq)
jq --arg v "$VERSION" '.version = $v' "$ROOT/.claude-plugin/plugin.json" > "$ROOT/.claude-plugin/plugin.json.tmp"
mv "$ROOT/.claude-plugin/plugin.json.tmp" "$ROOT/.claude-plugin/plugin.json"

# marketplace.json (jq)
jq --arg v "$VERSION" '.plugins[0].version = $v' "$ROOT/marketplace.json" > "$ROOT/marketplace.json.tmp"
mv "$ROOT/marketplace.json.tmp" "$ROOT/marketplace.json"

echo "witness: version synced to $VERSION"
