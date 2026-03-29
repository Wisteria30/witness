---
name: scan
description: Report-only guardrail scan. Lists violations by owner layer and file. Never repairs — use /witness:repair separately for that.
context: fork
allowed-tools: Bash, Read, Grep, Glob
argument-hint: [path]
---

Run a full guardrail scan for `$ARGUMENTS` if provided, otherwise for the current project root.

**REPORT ONLY. Do not fix, repair, or modify any code. Do not invoke repair. Do not suggest code changes. Just report and stop.**

Workflow:

1. Run `${CLAUDE_PLUGIN_ROOT}/bin/witness-engine scan-tree --root <target> --config-dir ${CLAUDE_PLUGIN_ROOT} --report-dir ${CLAUDE_PLUGIN_DATA}/reports`.
2. List files under `${CLAUDE_PLUGIN_DATA}/reports/pending/`.
3. Summarize findings by:
   - owner layer
   - violation class
   - file
4. Tell the user: "Run `/witness:repair` to batch-fix all pending reports in parallel."
5. **Stop here. Do not continue. Do not repair. Do not call any other skill or agent. Your job is done.**

Use [`../repair/doctrine.md`](../repair/doctrine.md) only for classification language, not for making changes.
