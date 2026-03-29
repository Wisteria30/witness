---
name: scan
description: Run a full guardrail scan in a forked context, inspect pending reports, and summarize owner-layer remediation work.
context: fork
disable-model-invocation: true
allowed-tools: Bash, Read, Grep, Glob
argument-hint: [path]
---

Run a full guardrail scan for `$ARGUMENTS` if provided, otherwise for the current project root.

**This skill is REPORT ONLY. Do not fix, repair, or modify any code. Do not invoke repair. Just report and stop.**

Workflow:

1. Run `${CLAUDE_PLUGIN_ROOT}/bin/witness-engine scan-tree --root <target> --config-dir ${CLAUDE_PLUGIN_ROOT} --report-dir ${CLAUDE_PLUGIN_DATA}/reports`.
2. List files under `${CLAUDE_PLUGIN_DATA}/reports/pending/`.
3. Summarize findings by:
   - owner layer
   - violation class
   - file
4. At the end, tell the user: "Run `/witness:repair` to batch-fix all pending reports in parallel."
5. End your response here. **Do not take any further action. The user will decide when to repair.**

Use [`../repair/doctrine.md`](../repair/doctrine.md) only for classification language, not for making changes.
