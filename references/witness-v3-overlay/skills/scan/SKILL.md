---
name: scan
description: Report-only constitutional scan. Lists violations, holes, drift, and obligations by owner layer and file. Never repairs.
context: fork
disable-model-invocation: true
allowed-tools: Bash, Read, Grep, Glob
argument-hint: [path]
---

Run a full witness scan for `$ARGUMENTS` if provided, otherwise for the current project root.

**REPORT ONLY.**
Do not fix, repair, or modify any code.
Do not invoke repair.
Do not suggest code changes beyond telling the user which witness workflow to run next.

The scan must evaluate the repo constitution and the active charter if present.

## Workflow

### 1. Locate the active charter directory
If `${CLAUDE_PLUGIN_DATA}/charters/active` exists, use it.
If it does not exist, continue without a charter.

### 2. Run the engine
Preferred v3 command:

```bash
${CLAUDE_PLUGIN_ROOT}/bin/witness-engine scan-tree \
  --root ${ARGUMENTS:-.} \
  --config-dir ${CLAUDE_PLUGIN_ROOT} \
  --charter-dir ${CLAUDE_PLUGIN_DATA}/charters/active \
  --report-dir ${CLAUDE_PLUGIN_DATA}/reports
```

If the current engine build does not yet support `--charter-dir`, fall back to the existing command without it and state that charter-aware verification is pending engine support.

### 3. Enumerate pending reports
List files under `${CLAUDE_PLUGIN_DATA}/reports/pending/`.

### 4. Summarize findings by category
Summarize by:

- owner layer
- finding kind (`violation`, `hole`, `drift`, `obligation`)
- violation class
- file

### 5. Tell the user exactly what to do next
- If only `violation`, `drift`, or `obligation` findings exist: tell the user to run `/witness:repair`.
- If `hole` findings exist: tell the user to run `/witness:charter` first, or answer the missing narrow constitutional decisions manually.
- If a structural diagnosis seems needed (mixed context debt, overloaded surface): suggest `/witness:shape`.

### 6. Stop
Do not continue.
Do not repair.
Do not call any other skill or agent.
Your job is done.
