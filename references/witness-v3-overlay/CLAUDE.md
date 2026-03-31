# CLAUDE.md

This file guides Claude Code while working in this repository.

## What is this project?

witness is a Claude Code plugin that detects and repairs the constitutional failures most common in AI-generated production code:

- **unowned elimination** of absence or failure into a value
- **unproved substitution** of runtime implementations
- **hidden owner-layer concepts** behind restricted visibility
- **missing boundary or inter-context contracts**

In concrete terms, the project targets:

- implicit fallbacks such as `.get(key, default)`, `??`, `||`, `catch { return default }`, and equivalent rewrites
- runtime test doubles or test-only semantics leaking into non-test code
- public concepts hidden as `_Payload`, `_Policy`, `_Adapter`, etc.
- boundary parsing and new public concepts that lack contract or context witnesses

The plugin is intentionally repair-oriented.
The verifier should classify violations, point to the owner layer, surface charter holes when intent is underdetermined, and demand a lawful remedy plus one witness. It should never incentivize rename-only or syntax-equivalent escapes.

## Commands

```bash
cargo build --release
cargo test --all-targets
cargo fmt --check
cargo clippy -- -D warnings
cargo test --test metadata_validation

./bin/witness-engine scan-tree --root . --config-dir .
./bin/witness-engine scan-file --file path/to/file.py --config-dir .
cat hook-input.json | ./bin/witness-engine scan-hook --config-dir . --report-dir /tmp/witness-reports
./bin/witness-engine scan-stop --config-dir . --report-dir /tmp/witness-reports

scripts/release.sh
```

V3 design adds optional charter-aware flags to the engine contract:

```bash
./bin/witness-engine scan-tree --root . --config-dir . --charter-dir ${CLAUDE_PLUGIN_DATA}/charters/active --report-dir ${CLAUDE_PLUGIN_DATA}/reports
./bin/witness-engine scan-stop --config-dir . --charter-dir ${CLAUDE_PLUGIN_DATA}/charters/active --report-dir ${CLAUDE_PLUGIN_DATA}/reports
```

The CI/CD skeleton, Rust toolchain policy, binary name, engine entrypoint, hook script locations, and skill placement should remain unchanged from the current repository unless there is a compelling reason.

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
- `1` = violations, holes, drift, obligations, or unresolved pending reports
- `2` = verifier/tool failure

The hook wrappers are responsible for fail-open behavior on engine errors.

### Rules

`rules/` holds ast-grep rules. Rules should stay focused on cheap syntactic surfaces. They are not the place for deep semantics or project-scale reasoning.

Every rule must declare metadata for:

- `policy_group`
- `violation_class`
- `owner_hint`
- `approval_mode`

### Constitution policies

The following files are the source of truth:

- `policy/ownership.yml` — path-level owner layer assignment
- `policy/defaults.yml` — blessed eliminators / approved defaults
- `policy/adapters.yml` — lawful runtime adapter registry
- `policy/surfaces.yml` — public/internal symbol policy and export witnesses
- `policy/contracts.yml` — boundary/inter-context contracts (`shape`, `interaction`, `law`)
- `policy/contexts.yml` — bounded context vocabulary and permitted dependencies

The approval model is **registry-backed**:

- adjacent `policy-approved: REQ-123` comments are only valid when the ID exists in `policy/defaults.yml`
- the current file must belong to an allowed owner layer for that ID
- invalid approval comments must never suppress findings

### Charter

Broad planning belongs to Claude Code Plan Mode or other planning systems.
`witness` does not replace them.

`/witness:charter` compiles only the minimal witness-relevant intent (`ΔK_w`) from an approved plan and stores it under `${CLAUDE_PLUGIN_DATA}/charters/active/`.

A charter may specify only the narrow constitutional delta witness needs:

- public surface decisions
- bounded-context assignments
- boundary/inter-context contracts and compatibility mode
- blessed default/optionality decisions
- lawful adapter additions

Never use charter files to duplicate task ordering, milestone breakdown, rollout plans, or broad acceptance criteria.

### Hooks

- `hooks/post-edit-classify.sh` is the synchronous hot path
- `hooks/post-edit-audit.sh` is async and only refreshes pending reports / emits a short system message
- `hooks/stop-gate.sh` is authoritative for unresolved reports, holes, drift, and obligations
- `hooks/session-start.sh` ensures the binary exists and matches the plugin version

Never dump full scan output into `additionalContext`.
The main session should only receive a compact capsule and a path to a persisted report.

### Skills and agent

- `/witness:charter` compiles a sparse charter from an approved broad plan
- `/witness:scan` runs a full constitutional scan in a forked context and summarizes violations, holes, drift, and obligations
- `/witness:repair` dispatches 5 parallel `guardrail-repairer` agents (worktree-isolated) to fix all pending reports at once
- `/witness:shape` performs read-only structural diagnosis (principal role, context blur, missing witnesses)
- `guardrail-repairer` is the dedicated repair subagent for isolated, high-volume owner-layer refactors

**After `/witness:scan` returns, never auto-start repairs.** Present the report and wait for the user to choose whether to run `/witness:repair`.

All operational witness skills should set `disable-model-invocation: true` to avoid competing with broad planning workflows.

## Hot-path constraints

`scan-file` and `scan-hook` are latency-sensitive. Protect them.

- Prefer ripgrep for file discovery and keyword prefiltering
- Prefer ast-grep for syntax-aware matching
- Keep Rust as orchestration, policy validation, report writing, and light semantic enrichment
- Do not move heavy reasoning into the synchronous hook path
- Do not introduce network calls anywhere in the engine or hooks

## AI repair doctrine

The complete doctrine lives in [`skills/repair/doctrine.md`](skills/repair/doctrine.md).
Quick reference:

0. Load repo constitution `K₀` and active charter `ΔK_w`
1. Classify the owner layer
2. Challenge context and optionality — if unclear, `needs_charter_decision`
3. Choose exactly one legal remedy
4. Add one witness
5. Challenge the interface — public concepts, private mechanics
6. Challenge the contract — shape, interaction, or law
7. Compile persistent constitutional changes

Forbidden moves: rename mock/stub/fake, syntax-equivalent rewrites, new inline defaults, invented approval ids, test support in runtime, hiding owner-layer concepts behind restricted visibility, introducing public concepts without export manifests, and guessing underdetermined charter decisions.

## Design taste

Prefer completeness over shortcuts when the scope is boilable.
Prefer lawful design pressure over larger deny-lists.
Prefer precise ownership and witness models over clever local rewrites.
Prefer constitutional compilation over prose plans duplicated across tools.
