#!/usr/bin/env bash
set -euo pipefail

PLUGIN_DIR="${CLAUDE_PLUGIN_ROOT:-$(cd "$(dirname "$0")/.." && pwd)}"
DATA_DIR="${CLAUDE_PLUGIN_DATA:-$PLUGIN_DIR/.witness-data}"
REPORT_DIR="$DATA_DIR/reports"
CHARTER_DIR="$DATA_DIR/charters"
ENGINE_BIN="$PLUGIN_DIR/bin/witness-engine"
PLUGIN_JSON="$PLUGIN_DIR/.claude-plugin/plugin.json"

mkdir -p "$REPORT_DIR/pending" "$REPORT_DIR/history"
mkdir -p "$CHARTER_DIR/active" "$CHARTER_DIR/history"

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
