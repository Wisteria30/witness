#!/usr/bin/env bash
set -euo pipefail

PLUGIN_DIR="${CLAUDE_PLUGIN_ROOT:-$(cd "$(dirname "$0")/.." && pwd)}"
DATA_DIR="${CLAUDE_PLUGIN_DATA:-$PLUGIN_DIR/.witness-data}"
source "$PLUGIN_DIR/hooks/lib/project-scope.sh"
REPORT_DIR="$PROJECT_REPORT_DIR"
CHARTER_DIR="$PROJECT_CHARTER_DIR"
ENGINE_BIN="$PLUGIN_DIR/bin/witness-engine"
PLUGIN_JSON="$PLUGIN_DIR/.claude-plugin/plugin.json"

# Snapshot existing pending reports so the stop gate only blocks on
# reports created during this session, not pre-existing ones.
mkdir -p "$REPORT_DIR/pending"
ls "$REPORT_DIR/pending/"*.json 2>/dev/null | sort > "$REPORT_DIR/.session-baseline" || true

EXPECTED_VERSION="$(jq -r '.version // ""' "$PLUGIN_JSON" 2>/dev/null || echo "")"

if [ -x "$ENGINE_BIN" ]; then
  CURRENT_VERSION="$("$ENGINE_BIN" --version 2>/dev/null || true)"
  if [ "$CURRENT_VERSION" = "$EXPECTED_VERSION" ]; then
    exit 0
  fi
  rm -f "$ENGINE_BIN"
fi

bash "$PLUGIN_DIR/setup" >/dev/null 2>&1 || {
  echo "witness: engine setup failed (guardrails running fail-open until setup succeeds)" >&2
  exit 0
}
