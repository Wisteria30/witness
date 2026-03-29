---
name: repair
description: Batch repair all pending guardrail reports using 5 parallel worktree-isolated agents. Run /witness:scan first.
disable-model-invocation: true
allowed-tools: Bash, Read, Grep, Glob, Agent, AskUserQuestion
argument-hint: [report-dir]
---

Batch repair pending guardrail reports in parallel.

**Report directory**: Use `$ARGUMENTS` if provided, otherwise `${CLAUDE_PLUGIN_DATA}/reports/pending`.

## Workflow

### 1. Discover pending reports

```bash
ls <report-dir>/*.json
```

If no reports exist, say "No pending reports found." and stop.

### 2. Read and group reports

For each `.json` file in the report directory:
- Read the report
- Extract `file` (the source file path) and `violations` array

Group reports into **5 batches** by distributing evenly. Reports affecting the same file MUST go in the same batch to avoid merge conflicts.

### 3. Dispatch 5 parallel agents

Launch exactly **5 Agent calls in a single message** (parallel tool calls). Each agent:

- **subagent_type**: `guardrail-repairer`
- **isolation**: `worktree`
- **run_in_background**: `true`

For each agent's prompt, include:
1. Instruction to read the doctrine: `Read ${CLAUDE_PLUGIN_ROOT}/skills/repair/doctrine.md and follow it exactly.`
2. The full JSON content of each assigned report
3. The engine path for re-scan: `${CLAUDE_PLUGIN_ROOT}/bin/witness-engine`
4. The config dir: `${CLAUDE_PLUGIN_ROOT}`
5. The policy dir: `${CLAUDE_PLUGIN_ROOT}/policy/` (for surfaces.yml reference)

The agent definition (`guardrail-repairer`) already contains operating rules.
Do not duplicate doctrine or operating rules in the prompt.

### 4. Wait and cleanup

After all 5 agents complete:

1. Each agent's result includes a worktree path if changes were made. For every returned worktree path, apply the changes to the main workspace and remove the worktree:
   ```bash
   # For each worktree that has changes:
   cd <worktree-path> && git diff HEAD > /tmp/witness-patch-N.patch
   cd <main-repo> && git apply /tmp/witness-patch-N.patch
   git worktree remove <worktree-path> --force
   ```

2. Collect all `needs_human_decision` items from all agents.

3. Summarize the automated results:
   - How many reports were resolved
   - How many remain
   - Which remedies were applied
   - Which worktrees were merged and cleaned up

### 5. Human decision loop

If there are `needs_human_decision` items, process them **one at a time** using AskUserQuestion.

For each item, ask:

```
[file:line] code_snippet

This fallback's optionality has no clear spec. Two likely remedies:
1. remedy_a — (brief explanation)
2. remedy_b — (brief explanation)
3. Skip — leave as-is for now

Which approach?
```

Based on the user's answer:
- If they choose a remedy: apply it, add a witness, re-scan to verify.
- If they say "skip": leave the pending report and move on.
- If they provide additional context (e.g., "this field is always present"): use that to choose `eliminate_optionality` or the appropriate remedy.

After all human decisions are resolved, give a final summary.

**Do not take further action after the final summary.**
