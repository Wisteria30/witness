#!/usr/bin/env bash
set -euo pipefail

PLUGIN_DIR="${CLAUDE_PLUGIN_ROOT:-$(cd "$(dirname "$0")/.." && pwd)}"
DATA_DIR="${CLAUDE_PLUGIN_DATA:-$PLUGIN_DIR/.code-guardrails-data}"
REPORT_DIR="$DATA_DIR/reports"
ENGINE_BIN="$PLUGIN_DIR/bin/code-guardrails-engine"

mkdir -p "$REPORT_DIR/pending" "$REPORT_DIR/history"

if [ ! -x "$ENGINE_BIN" ]; then
  exit 0
fi

set +e
"$ENGINE_BIN" scan-tree --root "$(pwd)" --config-dir "$PLUGIN_DIR" --report-dir "$REPORT_DIR" >/dev/null 2>&1
STATUS=$?
set -e

if [ "$STATUS" -eq 1 ]; then
  printf '%s\n' "{\"systemMessage\":\"code-guardrails audit refreshed pending reports under $REPORT_DIR/pending\"}"
fi

exit 0
