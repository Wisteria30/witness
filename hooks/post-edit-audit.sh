#!/usr/bin/env bash
set -euo pipefail

PLUGIN_DIR="${CLAUDE_PLUGIN_ROOT:-$(cd "$(dirname "$0")/.." && pwd)}"
DATA_DIR="${CLAUDE_PLUGIN_DATA:-$PLUGIN_DIR/.witness-data}"
source "$PLUGIN_DIR/hooks/lib/project-scope.sh"
REPORT_DIR="$PROJECT_REPORT_DIR"
CHARTER_DIR="$PROJECT_CHARTER_DIR/active"
ENGINE_BIN="$PLUGIN_DIR/bin/witness-engine"

# Skip if running inside a git worktree (repair agent context)
_GD="$(git rev-parse --git-dir 2>/dev/null || true)"
if [ -n "$_GD" ] && [[ "$_GD" == *".git/worktrees/"* ]]; then
  exit 0
fi

if [ ! -x "$ENGINE_BIN" ]; then
  exit 0
fi

set +e
CMD=("$ENGINE_BIN" scan-tree --root "$(pwd)" --config-dir "$PLUGIN_DIR" --report-dir "$REPORT_DIR")
if [ -d "$CHARTER_DIR" ]; then
  CMD+=(--charter-dir "$CHARTER_DIR")
fi
"${CMD[@]}" >/dev/null 2>&1
STATUS=$?
set -e

if [ "$STATUS" -eq 1 ]; then
  printf '%s\n' "{\"systemMessage\":\"witness audit refreshed pending reports under $REPORT_DIR/pending\"}"
fi

exit 0
