#!/usr/bin/env bash
set -euo pipefail

PLUGIN_DIR="${CLAUDE_PLUGIN_ROOT:-$(cd "$(dirname "$0")/.." && pwd)}"
DATA_DIR="${CLAUDE_PLUGIN_DATA:-$PLUGIN_DIR/.code-guardrails-data}"
REPORT_DIR="$DATA_DIR/reports"
ENGINE_BIN="$PLUGIN_DIR/bin/code-guardrails-engine"
TMP_INPUT="$(mktemp)"
trap 'rm -f "$TMP_INPUT"' EXIT

mkdir -p "$REPORT_DIR/pending" "$REPORT_DIR/history"

cat >"$TMP_INPUT"

if [ ! -x "$ENGINE_BIN" ]; then
  echo "code-guardrails: engine missing; run setup (fail-open)" >&2
  exit 0
fi

set +e
OUTPUT="$("$ENGINE_BIN" scan-hook --config-dir "$PLUGIN_DIR" --report-dir "$REPORT_DIR" --hook-response <"$TMP_INPUT" 2>/dev/null)"
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
    echo "code-guardrails: scan error (fail-open)" >&2
    exit 0
    ;;
esac
