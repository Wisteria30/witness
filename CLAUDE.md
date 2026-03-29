# CLAUDE.md

This file guides Claude Code while working in this repository.

## What is this project?

witness is a Claude Code plugin that detects and repairs two specific design failures in AI-generated production code:

- **unowned elimination** of absence or failure into a value
- **unproved substitution** of runtime implementations

In concrete terms, the project targets:

- implicit fallbacks such as `.get(key, default)`, `??`, `||`, `catch { return default }`, and equivalent rewrites
- runtime test doubles or test-only semantics leaking into non-test code

The plugin is intentionally repair-oriented. The verifier should classify violations, point to the owner layer, and demand a lawful remedy plus one witness. It should never incentivize rename-only or syntax-equivalent escapes.

## Commands

```bash
cargo build --release
cargo test --all-targets
cargo fmt --check
cargo clippy -- -D warnings
cargo test --test metadata_validation

./bin/witness-engine scan-tree --root . --config-dir .
./bin/witness-engine scan-file --file path/to/file.py --config-dir .
cat hook-input.json | ./bin/witness-engine scan-hook --config-dir . --report-dir /tmp/cg-reports
./bin/witness-engine scan-stop --config-dir . --report-dir /tmp/cg-reports

scripts/release.sh <version>
```

## Architecture

### Engine

All engine code lives in `src/main.rs`.

Subcommands:

- `scan-file` — scan one file and emit a structured JSON result
- `scan-tree` — full project scan with ripgrep prefiltering and grouped ast-grep invocation
- `scan-hook` — read Claude Code hook JSON from stdin, scan the changed file, and optionally emit a hook-ready JSON block response
- `scan-stop` — inspect pending report files and block `Stop` / `SubagentStop` when unresolved work remains

The engine exit contract is strict:

- `0` = clean
- `1` = violations or unresolved pending reports
- `2` = verifier/tool failure

The hook wrappers are responsible for fail-open behavior on engine errors.

### Rules

`rules/` holds ast-grep rules. Rules should stay focused on cheap syntactic surfaces. They are not the place for deep semantics or project-scale reasoning.

Every rule must declare metadata for:

- `policy_group`
- `violation_class`
- `owner_hint`
- `approval_mode`

### Policies

`policy/ownership.yml`, `policy/defaults.yml`, and `policy/adapters.yml` are the source of truth for layer ownership, approved defaults, and lawful runtime adapters.

The approval model is **registry-backed**:

- adjacent `policy-approved: REQ-123` comments are only valid when the ID exists in `policy/defaults.yml`
- the current file must belong to an allowed owner layer for that ID
- invalid approval comments must never suppress findings

### Hooks

- `hooks/post-edit-classify.sh` is the synchronous hot path
- `hooks/post-edit-audit.sh` is async and only refreshes pending reports / emits a short system message
- `hooks/stop-gate.sh` is authoritative for unresolved reports
- `hooks/session-start.sh` ensures the binary exists and matches the plugin version

Never dump full scan output into `additionalContext`. The main session should only receive a compact capsule and a path to a persisted report.

### Skills and agent

- `/scan` runs a full guardrail scan in a forked context and summarizes owner-layer work
- `/repair` dispatches 5 parallel `guardrail-repairer` agents (worktree-isolated) to fix all pending reports at once
- `guardrail-repairer` is the dedicated repair subagent for isolated, high-volume owner-layer refactors

**After `/scan` returns, never auto-start repairs.** Present the report and wait for the user to choose which reports to repair. The user controls the repair workflow.

## Hot-path constraints

`scan-file` and `scan-hook` are latency-sensitive. Protect them.

- Prefer ripgrep for file discovery and keyword prefiltering
- Prefer ast-grep for syntax-aware matching
- Keep Rust as orchestration, policy validation, report writing, and light semantic enrichment
- Do not move heavy reasoning into the synchronous hook path
- Do not introduce network calls anywhere in the engine or hooks

## AI repair doctrine

When a fallback/test-double violation fires, do not preserve the current line.

1. Find the owner layer:
   - boundary
   - domain
   - application
   - infrastructure
   - composition root
   - tests

2. Choose exactly one legal remedy:
   - approved policy API
   - boundary parser / settings model
   - Optional/union + exhaustive handling
   - typed exception / contract violation
   - explicit resilience adapter
   - move double to tests
   - promote substitute to a first-class adapter + contract tests

3. Add one witness:
   - parser/schema
   - exhaustiveness check
   - architecture/import rule
   - contract/property/stateful test
   - registered approval id
   - explicit export manifest

4. Challenge the interface — classify new symbols as public concepts (owner-layer nouns: Payload, Policy, Adapter, Error, etc.) or internal mechanics; add an export manifest witness (`__all__`, named exports)

Forbidden moves:

- rename mock/stub/fake
- syntax-equivalent fallback rewrites
- adding a new inline default
- inventing a new approval id
- importing test support into the runtime graph
- hiding owner-layer concepts behind restricted visibility (revisit step 4)

## Design taste

Prefer completeness over shortcuts when the scope is boilable.
Prefer lawful design pressure over larger deny-lists.
Prefer precise ownership and witness models over clever local rewrites.
