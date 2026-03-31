---
name: charter
description: Compile the minimal witness-relevant intent ΔK from the current approved plan or a supplied plan file. Never create a full implementation plan.
disable-model-invocation: true
allowed-tools: Read, Write, Edit, Grep, Glob, AskUserQuestion
argument-hint: [plan-file-or-change-id]
---

Compile a **sparse witness charter** (`ΔK_w`) from an already-approved broad plan.

This skill is **not** a general planner.
Do **not** produce implementation sequencing, task breakdowns, rollout plans, test plans, or milestones.
Only extract the witness-relevant constitutional delta.

Resolve the project-scoped data directory first:
```bash
WITNESS_DATA=$($CLAUDE_PLUGIN_ROOT/hooks/lib/resolve-project-dir.sh)
```

The active charter must be written to:

```text
${WITNESS_DATA}/charters/active/<change-id>.yml
```

Rules:

- create `${WITNESS_DATA}/charters/active/` if it does not exist
- if a charter with the same `change_id` already exists, update it in place instead of creating duplicates
- keep the filename stable and derived from `change_id` alone
- never leave anonymous files such as `.yml` or timestamp-only files in `active/`

## Inputs

Use one of these, in order:

1. `$ARGUMENTS` if it points to a readable plan file
2. a fenced `witness-delta` block if the user provided one
3. the currently approved plan in the conversation

If none of these exists, do **not** invent a broad plan. Ask only the minimum narrow questions needed to determine whether a charter is needed.

## What belongs in the charter

Only these judgement classes:

- surface decisions for new public symbols
- bounded-context assignments
- contracts to add/change and their compatibility mode
- approved default / optionality decisions that are not already encoded in `policy/defaults.yml`
- lawful adapter additions

## What must never go into the charter

Never duplicate:

- file-by-file task ordering
- implementation steps
- rollout phases
- migration schedules
- broad acceptance criteria
- teammate assignments
- test execution order

## Workflow

### 1. Load the durable constitution
Read:

- `${CLAUDE_PLUGIN_ROOT}/policy/ownership.yml`
- `${CLAUDE_PLUGIN_ROOT}/policy/defaults.yml`
- `${CLAUDE_PLUGIN_ROOT}/policy/adapters.yml`
- `${CLAUDE_PLUGIN_ROOT}/policy/surfaces.yml`
- `${CLAUDE_PLUGIN_ROOT}/policy/contracts.yml` if it exists
- `${CLAUDE_PLUGIN_ROOT}/policy/contexts.yml` if it exists

### 2. Decide whether this change is constitution-preserving or constitution-extending
If the approved plan introduces none of the following, the change is likely constitution-preserving and no charter is needed:

- new public symbols
- new or changed boundary/inter-context contracts
- new default or optionality decisions not already registry-backed
- new lawful runtime adapters
- new bounded-context edges or context assignments

If constitution-preserving, say so explicitly and stop. Do not create an empty charter file.

### 3. Project the plan into witness intent
Produce only:

- `surfaces.public_symbols`
- `contexts.assignments`
- `contracts.add`
- `defaults.approvals`
- `adapters.add`
- `holes`

### 4. Ask only narrow questions when underdetermined
If any of the following is unclear, ask exactly one question at a time:

- Is this symbol public or internal?
- Which bounded context owns this concept?
- What contract kind is this (`shape`, `interaction`, `law`)?
- What compatibility mode applies?
- Is this absent case genuinely specified or should optionality be eliminated?
- Is this alternate implementation lawful or only test convenience?

Do not ask broad planning questions.

### 5. Choose or derive `change_id`
Use `$ARGUMENTS` if it looks like a change id.
Otherwise derive a stable one from the plan title or the smallest distinctive noun phrase.

### 6. Write the active charter
Write YAML to:

```text
${WITNESS_DATA}/charters/active/<change-id>.yml
```

Target shape:

```yaml
version: 1
change_id: CHG-...
constitution_mode: extend
source:
  kind: approved-plan
  ref: conversation
surfaces:
  public_symbols:
    src/api/tool_use.py:
      ToolUsePayload: public_concept
      parse_tool_use: public_concept
contexts:
  assignments:
    src/api/tool_use.py: api_boundary
contracts:
  add:
    - id: http.tool_use_payload.v1
      kind: shape
      compatibility: exact
defaults:
  approvals: []
adapters:
  add: []
holes: []
```

### 7. Output summary
Report:

- whether a charter was needed
- which constitutional judgements were captured
- which holes remain, if any
- the saved charter path
- whether the charter is expected to stay temporary until it is compiled into durable policy
- the next step: implement, then run `/witness:scan`

If the change later compiles durable constitutional facts into `policy/*.yml`, retire the matching charter from `${WITNESS_DATA}/charters/active/` into `${WITNESS_DATA}/charters/history/`.

Stop there.
