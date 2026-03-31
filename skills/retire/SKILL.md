---
name: retire
description: Archive compiled or stale active witness charters into history using the engine retirement flow. Use after repairs compile durable policy facts.
context: fork
disable-model-invocation: true
allowed-tools: Bash, Read, Grep, Glob
argument-hint: [change-id]
---

Archive active charters that no longer need to influence `scan` or `repair`.

## Current state

Active charters:
!`ls ${CLAUDE_PLUGIN_DATA}/charters/active/ 2>/dev/null || echo "(none)"`

Pending reports:
!`ls ${CLAUDE_PLUGIN_DATA}/reports/pending/ 2>/dev/null || echo "(none)"`

This skill is a thin user-facing wrapper around the authoritative engine command:

```bash
${CLAUDE_PLUGIN_ROOT}/bin/witness-engine retire-charters ...
```

Do not move charter files manually when the engine command can decide eligibility for you.

## Workflow

### 1. Discover active charters
Read `${CLAUDE_PLUGIN_DATA}/charters/active/` if it exists.

If there are no active charters, say `No active charters found.` and stop.

### 2. Choose target change ids
If `$ARGUMENTS` is present, use it as the target `change_id`.

Otherwise:

- read each active charter file
- extract its `change_id`
- ignore blank `change_id` values, but call them out in the summary
- prepare one `--change-id` argument per unique non-empty `change_id`

### 3. Run the authoritative retirement command
Run:

```bash
${CLAUDE_PLUGIN_ROOT}/bin/witness-engine retire-charters \
  --change-id <change-id> \
  --config-dir ${CLAUDE_PLUGIN_ROOT} \
  --charter-dir ${CLAUDE_PLUGIN_DATA}/charters/active \
  --report-dir ${CLAUDE_PLUGIN_DATA}/reports
```

Use one `--change-id` flag for each selected change id.

The engine decides whether the charter can move from `charters/active/` to `charters/history/`.
Do not second-guess or reimplement that decision in the skill.

### 4. Output summary
Report:

- which `change_id` values were archived
- which files now live under `${CLAUDE_PLUGIN_DATA}/charters/history/`
- which `change_id` values were skipped and why
- which active charters remain

Stop there.
