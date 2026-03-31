#!/usr/bin/env bash
# Outputs the project-scoped data directory path.
# Usage: WITNESS_DATA=$($CLAUDE_PLUGIN_ROOT/hooks/lib/resolve-project-dir.sh)
set -euo pipefail
DATA_DIR="${CLAUDE_PLUGIN_DATA:-${CLAUDE_PLUGIN_ROOT:-.}/.witness-data}"
source "$(dirname "$0")/project-scope.sh"
printf '%s' "$PROJECT_DIR"
