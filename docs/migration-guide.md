# Migration guide: current witness -> vNext

This repository keeps the same core stack as the current public plugin—Rust orchestration, `ripgrep` for discovery, and `ast-grep` for syntax rules—but changes what happens after detection.

## 1. Replace comment-only approval with registry-backed approval

Before:

- adjacent `policy-approved:` comments suppressed findings on their own

After:

- `policy-approved:` comments are only valid when the ID is registered in `policy/defaults.yml`
- the file must belong to an allowed owner layer in `policy/ownership.yml`
- invalid approval IDs still surface as violations

## 2. Split detection from repair

Before:

- hot-path hooks pushed the full scan output into `additionalContext`

After:

- the sync hook stores a detailed JSON report under `${CLAUDE_PLUGIN_DATA}/reports/pending/`
- Claude only receives a short capsule plus the report path
- the repair doctrine lives in `skills/repair/`
- the heavy fix path is delegated to `agents/guardrail-repairer.md`

## 3. Add authoritative stop gates

Before:

- only `PostToolUse` blocked

After:

- `PostToolUse` still gives immediate feedback
- `Stop` and `SubagentStop` refuse to finish while unresolved reports remain
- this supports multi-edit owner-layer repairs without rewarding local escape rewrites

## 4. Add ownership policy

Create or adapt `policy/ownership.yml` so the verifier can distinguish:

- boundaries
- domain
- application
- infrastructure
- composition root
- tests

Without this file, the verifier can still classify by rule hint, but it cannot validate approvals or composition-root-only adapter selection precisely.

## 5. Add adapter registry + contract suites

Populate `policy/adapters.yml` with lawful runtime adapters and the contract tests that justify them.

Anything not listed there should be treated as suspect runtime substitution.

## 6. Update your project instructions

Keep `CLAUDE.md` short and stable:

- a fallback is an effect handler, not a convenience
- a production substitute is an adapter, not a fake
- when a guardrail fires, repair at the owner layer and add one witness
- forbidden: rename-only, equivalent rewrites, invented approval IDs, runtime test-support imports

## 7. Use the new commands

- `/scan` for triage in forked context
- `/repair <report-path-or-file>` for actual repair work
