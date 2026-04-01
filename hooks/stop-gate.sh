#!/usr/bin/env bash
set -euo pipefail

PLUGIN_DIR="${CLAUDE_PLUGIN_ROOT:-$(cd "$(dirname "$0")/.." && pwd)}"
DATA_DIR="${CLAUDE_PLUGIN_DATA:-$PLUGIN_DIR/.witness-data}"
source "$PLUGIN_DIR/hooks/lib/project-scope.sh"
REPORT_DIR="$PROJECT_REPORT_DIR"
CHARTER_DIR="$PROJECT_CHARTER_DIR/active"
ENGINE_BIN="$PLUGIN_DIR/bin/witness-engine"

if [ ! -x "$ENGINE_BIN" ]; then
  exit 0
fi

# If every pending report already existed at session start, pass.
# The stop gate only blocks on reports created during this session.
BASELINE="$REPORT_DIR/.session-baseline"
if [ -f "$BASELINE" ]; then
  CURRENT="$(ls "$REPORT_DIR/pending/"*.json 2>/dev/null | sort)"
  if [ -n "$CURRENT" ]; then
    NEW_REPORTS="$(comm -23 <(echo "$CURRENT") "$BASELINE")"
    if [ -z "$NEW_REPORTS" ]; then
      exit 0
    fi
  else
    exit 0
  fi
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
