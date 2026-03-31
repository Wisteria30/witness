# Migration guide: witness v2 -> witness v3

This repository keeps the same core stack—Rust orchestration, `ripgrep` for discovery, and `ast-grep` for syntax rules—but upgrades the runtime contract from v2 guardrails to the v3 constitutional kernel.

## 1. Adopt the v3 constitution

Before:

- `ownership/defaults/adapters` were the only effective verifier inputs

After:

- the verifier consumes `policy/ownership.yml`
- the verifier consumes `policy/defaults.yml`
- the verifier consumes `policy/adapters.yml`
- the verifier consumes `policy/surfaces.yml`
- the verifier consumes `policy/contracts.yml`
- the verifier consumes `policy/contexts.yml`

## 2. Move from violations-only reports to finding kinds

Before:

- reports stored only `violations`
- stop gates blocked only on unresolved pending reports

After:

- reports store `findings`
- each finding has kind `violation`, `hole`, `drift`, or `obligation`
- stop gates block on any unresolved report, including pending charter decisions and charter obligations

## 3. Add charter-aware verification

Before:

- scan commands evaluated only repo policy
- no per-change constitutional delta could be supplied to the engine

After:

- `scan-file`, `scan-tree`, `scan-hook`, and `scan-stop` accept optional `--charter-dir`
- active charter files under `${CLAUDE_PLUGIN_DATA}/charters/active` are consumed automatically by the hooks
- charter-declared work can produce `obligation` findings until it is compiled into code or durable policy

## 4. Tighten approval semantics

Before:

- a registered approval id plus an allowed owner layer was enough

After:

- the approval id must exist in `policy/defaults.yml`
- the file must belong to an allowed owner layer
- the blessed symbol must match the call site

## 5. Expect breaking JSON changes

Before:

- scan JSON used `summary.violation_count`
- pending reports used `schema_version: 1` and `violations[]`

After:

- scan JSON uses `summary.{violations,holes,drift,obligations}`
- pending reports use `version: 3`, `status`, `charter_ref`, and `findings[]`
- hook block responses mention the full v3 finding mix

## 6. Update operational workflow

- run `/witness:charter` after the broad plan is approved when the change extends the constitution
- run `/witness:scan` to obtain v3 findings
- if there are `hole` findings, answer the narrow charter questions first
- if there are `violation`, `drift`, or `obligation` findings, run `/witness:repair`
