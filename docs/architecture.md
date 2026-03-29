# Architecture

code-guardrails vNext is built around a simple separation:

- **verifier**: fast, deterministic, cheap
- **doctrine**: reusable, explicit, human-readable
- **repairer**: isolated, deep, multi-file

## Verifier

The verifier is the Rust engine plus shell hook wrappers.

The synchronous `PostToolUse` path does four things:

1. discover the changed file from Claude Code hook JSON
2. run a cheap scan using ripgrep prefiltering + ast-grep rules
3. enrich findings with owner guesses, legal remedies, forbidden moves, and approval validation
4. persist a pending report and return only a short capsule to Claude

It does **not** try to reason across the whole system in the hot path.

## Doctrine

The doctrine lives in:

- `CLAUDE.md`
- `skills/repair-guardrail/SKILL.md`
- `skills/repair-guardrail/doctrine.md`

Its job is to teach a stable repair protocol:

- classify the owner
- choose one legal remedy
- add one witness
- refuse forbidden escapes

## Repairer

`agents/guardrail-repairer.md` is the high-context repair agent.

It runs in a worktree-isolated context and is designed for:

- owner-layer refactors
- boundary parser introduction
- typed error propagation
- composition-root adapter rewiring
- contract/property/stateful test additions

## Report lifecycle

Detailed findings are persisted under `${CLAUDE_PLUGIN_DATA}/reports/`:

- `pending/` holds one unresolved JSON report per source file
- `history/` stores immutable snapshots for auditability

Whenever a file is rescanned cleanly, its pending report is deleted.
`scan-stop` blocks if any pending reports remain.

## Why not “just block every bad line”?

Because not every quality property is prefix-safe.

A correct final repair may need several edits that temporarily move through intermediate states. `PostToolUse` gives fast local feedback, but `Stop` is the authoritative gate for unresolved work.

## Ownership model

The repository ships with six owner layers:

- `boundary`
- `domain`
- `application`
- `infrastructure`
- `composition_root`
- `tests`

These are configured by `policy/ownership.yml`.

A fallback is only lawful when it belongs to one of the owner mechanisms:

- approved policy API
- parser/settings default at the boundary
- explicit optional/union handling
- typed error / contract violation
- resilience adapter in infrastructure

A substitute is only lawful when it is either:

- confined to tests
- or promoted to a first-class runtime adapter with contract tests and composition-root selection
