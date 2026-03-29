---
name: scan
description: Run a full guardrail scan in a forked context, inspect pending reports, and summarize owner-layer remediation work.
context: fork
disable-model-invocation: true
allowed-tools: Bash, Read, Grep, Glob
argument-hint: [path]
---

Run a full guardrail scan for `$ARGUMENTS` if provided, otherwise for the current project root.

Workflow:

1. Run `./bin/code-guardrails-engine scan-tree --root <target> --config-dir ${CLAUDE_PLUGIN_ROOT:-.}`.
2. If `${CLAUDE_PLUGIN_DATA}` exists, inspect `${CLAUDE_PLUGIN_DATA}/reports/pending`.
3. Summarize findings by:
   - owner layer
   - violation class
   - file
4. Do **not** patch code in this skill. This skill is for triage only.
5. If fixes are required, recommend `/repair-guardrail <report-path-or-file>`.

Use [`../repair-guardrail/doctrine.md`](../repair-guardrail/doctrine.md) only for classification language, not for making changes.
