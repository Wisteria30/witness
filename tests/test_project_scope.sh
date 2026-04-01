#!/usr/bin/env bash
set -euo pipefail

PASS=0
FAIL=0

assert_eq() {
  local desc="$1" expected="$2" actual="$3"
  if [ "$expected" = "$actual" ]; then
    echo "PASS: $desc"
    PASS=$((PASS + 1))
  else
    echo "FAIL: $desc (expected='$expected', actual='$actual')"
    FAIL=$((FAIL + 1))
  fi
}

assert_exists() {
  local desc="$1" path="$2"
  if [ -e "$path" ]; then
    echo "PASS: $desc"
    PASS=$((PASS + 1))
  else
    echo "FAIL: $desc (path does not exist: $path)"
    FAIL=$((FAIL + 1))
  fi
}

# Setup
TEST_DATA=$(mktemp -d)
trap 'rm -rf "$TEST_DATA"' EXIT

export CLAUDE_PROJECT_DIR="/Users/test/project-alpha"
export CLAUDE_PLUGIN_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
export CLAUDE_PLUGIN_DATA="$TEST_DATA"

# Test 1: resolver outputs correct path structure
RESOLVED=$(bash "$CLAUDE_PLUGIN_ROOT/hooks/lib/resolve-project-dir.sh")
HASH=$(printf '%s' "$CLAUDE_PROJECT_DIR" | shasum -a 256 | cut -c1-16)
assert_eq "resolver path contains hash" "$TEST_DATA/projects/$HASH" "$RESOLVED"

# Test 2: project.json created with correct content
assert_exists "project.json created" "$RESOLVED/project.json"
NAME=$(jq -r '.name' "$RESOLVED/project.json")
assert_eq "project.json name" "project-alpha" "$NAME"
PATH_VAL=$(jq -r '.path' "$RESOLVED/project.json")
assert_eq "project.json path" "/Users/test/project-alpha" "$PATH_VAL"

# Test 3: directory structure created
assert_exists "reports/pending" "$RESOLVED/reports/pending"
assert_exists "reports/history" "$RESOLVED/reports/history"
assert_exists "reports/resolved" "$RESOLVED/reports/resolved"
assert_exists "charters/active" "$RESOLVED/charters/active"
assert_exists "charters/history" "$RESOLVED/charters/history"

# Test 4: second project gets a different directory
export CLAUDE_PROJECT_DIR="/Users/test/project-beta"
RESOLVED_B=$(bash "$CLAUDE_PLUGIN_ROOT/hooks/lib/resolve-project-dir.sh")
if [ "$RESOLVED" != "$RESOLVED_B" ]; then
  echo "PASS: different projects get different directories"
  PASS=$((PASS + 1))
else
  echo "FAIL: different projects resolved to same directory"
  FAIL=$((FAIL + 1))
fi

# Test 5: idempotent — running resolver again doesn't fail
export CLAUDE_PROJECT_DIR="/Users/test/project-alpha"
RESOLVED_2=$(bash "$CLAUDE_PLUGIN_ROOT/hooks/lib/resolve-project-dir.sh")
assert_eq "idempotent resolver" "$RESOLVED" "$RESOLVED_2"

# Test 6: source-based helper sets expected variables
DATA_DIR="$TEST_DATA"
source "$CLAUDE_PLUGIN_ROOT/hooks/lib/project-scope.sh"
assert_eq "PROJECT_REPORT_DIR set" "$RESOLVED/reports" "$PROJECT_REPORT_DIR"
assert_eq "PROJECT_CHARTER_DIR set" "$RESOLVED/charters" "$PROJECT_CHARTER_DIR"

# Summary
echo ""
echo "Results: $PASS passed, $FAIL failed"
[ "$FAIL" -eq 0 ] || exit 1
