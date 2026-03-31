#!/usr/bin/env bash
set -euo pipefail

PLUGIN_DIR="${CLAUDE_PLUGIN_ROOT:-$(cd "$(dirname "$0")/.." && pwd)}"
DATA_DIR="${CLAUDE_PLUGIN_DATA:-$PLUGIN_DIR/.witness-data}"
REPORT_DIR="$DATA_DIR/reports"
CHARTER_DIR="$DATA_DIR/charters/active"
ENGINE_BIN="$PLUGIN_DIR/bin/witness-engine"
TMP_INPUT="$(mktemp)"
trap 'rm -f "$TMP_INPUT"' EXIT

cat >"$TMP_INPUT"

# Skip if the edited file lives inside a git worktree (repair agent context)
_FP="$(jq -r '.tool_input.file_path // empty' "$TMP_INPUT" 2>/dev/null || true)"
if [ -n "$_FP" ] && [ -d "$(dirname "$_FP")" ]; then
  _GD="$(cd "$(dirname "$_FP")" && git rev-parse --git-dir 2>/dev/null || true)"
  if [ -n "$_GD" ] && [[ "$_GD" == *".git/worktrees/"* ]]; then
    exit 0
  fi
fi

mkdir -p "$REPORT_DIR/pending" "$REPORT_DIR/history"

if [ ! -x "$ENGINE_BIN" ]; then
  echo "witness: engine missing; run setup (fail-open)" >&2
  exit 0
fi

set +e
CMD=("$ENGINE_BIN" scan-hook --config-dir "$PLUGIN_DIR" --report-dir "$REPORT_DIR" --hook-response)
if [ -d "$CHARTER_DIR" ]; then
  CMD+=(--charter-dir "$CHARTER_DIR")
fi
OUTPUT="$("${CMD[@]}" <"$TMP_INPUT" 2>/dev/null)"
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
    echo "witness: scan error (fail-open)" >&2
    exit 0
    ;;
esac
