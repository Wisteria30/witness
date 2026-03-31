---
name: repair
description: Batch repair all pending witness reports using 5 parallel worktree-isolated agents. Consumes the active charter when present.
disable-model-invocation: true
allowed-tools: Bash, Read, Grep, Glob, Write, Edit, Agent, AskUserQuestion
argument-hint: [report-dir]
---

Batch repair pending reports in parallel.

**Report directory**: Use `$ARGUMENTS` if provided, otherwise `$($CLAUDE_PLUGIN_ROOT/hooks/lib/resolve-project-dir.sh)/reports/pending`.

## Current state

Pending reports:
!`ls ${ARGUMENTS:-$($CLAUDE_PLUGIN_ROOT/hooks/lib/resolve-project-dir.sh)/reports/pending}/*.json 2>/dev/null || echo "(none)"`

Active charters:
!`ls $($CLAUDE_PLUGIN_ROOT/hooks/lib/resolve-project-dir.sh)/charters/active/ 2>/dev/null || echo "(none)"`

This skill consumes the active charter when present. It does not create or guess missing charter decisions.

## Workflow

### 1. Discover pending reports

```bash
WITNESS_DATA=$($CLAUDE_PLUGIN_ROOT/hooks/lib/resolve-project-dir.sh)
ls ${ARGUMENTS:-${WITNESS_DATA}/reports/pending}/*.json
```

If no reports exist, say `No pending reports found.` and stop.

### 2. Read and classify reports
For each `.json` file in the report directory:

- read the report
- extract `file`
- extract `findings` / `violations`
- note whether any finding kind is `hole`

If **all** reports contain `hole`, or reports containing `hole` are the majority, stop here.
Tell the user to run `/witness:charter` first, or answer the missing constitutional questions manually.
Do not dispatch repair agents in this case.

If both repairable reports and hole reports exist, continue with only the repairable reports.
Leave hole reports untouched and call them out explicitly in the summary.

Group repairable reports into **5 batches** by distributing evenly.
Reports affecting the same file **must** go in the same batch to avoid merge conflicts.

### 3. Load the active charter
If `${WITNESS_DATA}/charters/active` exists, read all active charter files.
For each agent batch, derive the smallest relevant charter slice:

- touched files
- affected public symbols
- contracts relevant to those files
- context assignments relevant to those files
- default / adapter decisions relevant to those files

### 4. Dispatch 5 parallel agents
Launch exactly **5 Agent calls in a single message** (parallel tool calls).
Each agent:

- **subagent_type**: `witness:guardrail-repairer`
- **isolation**: `worktree`
- **run_in_background**: `true`

For each agent prompt, include:

1. instruction to read the doctrine: `Read ${CLAUDE_PLUGIN_ROOT}/skills/repair/doctrine.md and follow it exactly.`
2. the full JSON content of each assigned report
3. the relevant charter slice, if any
4. the engine path for re-scan: `${CLAUDE_PLUGIN_ROOT}/bin/witness-engine`
5. the config dir: `${CLAUDE_PLUGIN_ROOT}`
6. the policy dir: `${CLAUDE_PLUGIN_ROOT}/policy/`

The agent definition already contains operating rules. Do not duplicate them in the prompt.

### 5. Wait and merge
After all 5 agents complete:

1. For every returned worktree path, apply the changes to the main workspace and remove the worktree.
2. Collect all `needs_charter_decision` items from all agents.
3. Collect all `compiled_constitution` items from all agents. Every item must include:
   - `change_id`
   - `policy_files`
4. Summarize:
   - how many reports were resolved
   - how many remain
   - which remedies were applied
   - which constitutional files were updated
   - which worktrees were merged and cleaned up

### 6. Charter decision loop
If there are `needs_charter_decision` items, process them **one at a time** using AskUserQuestion.
Only ask narrow constitutional questions.

Template:

```text
[file:line] code_snippet
This change requires a charter decision.
Question kind: <surface|context|contract|default_or_optionality|adapter>

Likely options:
1. option_a — brief explanation
2. option_b — brief explanation
3. Skip — leave as-is for now

Which approach?
```

Based on the answer:

- if the user chooses a remedy or judgement: update the active charter or the code, add the witness, re-scan to verify
- if they say `skip`: leave the pending report and move on
- if they provide new context: use it to resolve only that narrow constitutional judgement

### 7. Final summary
After all automated repairs and charter decisions are resolved:

1. re-scan to verify the repaired files are clean
2. collect the unique `change_id` values from `compiled_constitution`
3. run:

```bash
WITNESS_DATA=$($CLAUDE_PLUGIN_ROOT/hooks/lib/resolve-project-dir.sh)
${CLAUDE_PLUGIN_ROOT}/bin/witness-engine retire-charters \
  --change-id <change-id> \
  --config-dir ${CLAUDE_PLUGIN_ROOT} \
  --charter-dir ${WITNESS_DATA}/charters/active \
  --report-dir ${WITNESS_DATA}/reports
```

Only retire a charter when the command succeeds. If retirement is skipped, report why and leave the charter active.

After that, give a final summary and stop.
Do not take further action.
