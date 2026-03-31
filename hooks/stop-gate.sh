#!/usr/bin/env bash
set -euo pipefail

PLUGIN_DIR="${CLAUDE_PLUGIN_ROOT:-$(cd "$(dirname "$0")/.." && pwd)}"
DATA_DIR="${CLAUDE_PLUGIN_DATA:-$PLUGIN_DIR/.witness-data}"
REPORT_DIR="$DATA_DIR/reports"
CHARTER_DIR="$DATA_DIR/charters/active"
ENGINE_BIN="$PLUGIN_DIR/bin/witness-engine"

if [ ! -x "$ENGINE_BIN" ]; then
  exit 0
fi

set +e
CMD=("$ENGINE_BIN" scan-stop --config-dir "$PLUGIN_DIR" --report-dir "$REPORT_DIR" --hook-response)
if [ -d "$CHARTER_DIR" ]; then
  CMD+=(--charter-dir "$CHARTER_DIR")
fi
OUTPUT="$("${CMD[@]}" 2>/dev/null)"
STATUS=$?
set -e

case "$STATUS" in
  0)
    exit 0
    ;;
  1)
    printf '%s\n' "$OUTPUT"
    exit 0
    ;;
  *)
    echo "witness: stop gate error (fail-open)" >&2
    exit 0
    ;;
esac
