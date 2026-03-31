#!/usr/bin/env bash
# Sourced by hooks — requires DATA_DIR and CLAUDE_PROJECT_DIR to be set.
# Sets: PROJECT_HASH, PROJECT_DIR, PROJECT_REPORT_DIR, PROJECT_CHARTER_DIR

PROJECT_HASH=$(printf '%s' "$CLAUDE_PROJECT_DIR" | shasum -a 256 | cut -c1-16)
PROJECT_DIR="$DATA_DIR/projects/$PROJECT_HASH"
PROJECT_REPORT_DIR="$PROJECT_DIR/reports"
PROJECT_CHARTER_DIR="$PROJECT_DIR/charters"

if [ ! -f "$PROJECT_DIR/project.json" ]; then
  mkdir -p "$PROJECT_REPORT_DIR/pending" "$PROJECT_REPORT_DIR/history" "$PROJECT_REPORT_DIR/resolved"
  mkdir -p "$PROJECT_CHARTER_DIR/active" "$PROJECT_CHARTER_DIR/history"
  printf '{"path":"%s","name":"%s"}\n' \
    "$CLAUDE_PROJECT_DIR" "$(basename "$CLAUDE_PROJECT_DIR")" \
    > "$PROJECT_DIR/project.json"
fi
