# code-guardrails vNext

code-guardrails is a Claude Code plugin for a narrower and more valuable target than “bad patterns” in the abstract:

- unowned elimination of absence or failure into a value
- unproved substitution of runtime implementations

In practice, that means two families of bugs that AI coding tools keep introducing into production code:

- implicit fallbacks such as `.get(key, default)`, `??`, `||`, empty `catch`, or equivalent rewrites
- runtime test doubles or test-only semantics leaking into the production dependency graph

The current public version blocks obvious syntax. This repository packages the next version as a repair system. It still uses a fast Rust verifier plus ast-grep and ripgrep, but it no longer optimizes for “find a string and block.” It optimizes for “push the code back to the owning layer, with a machine-checkable witness.”

## What changed in vNext

The design is split into three layers:

- **Verifier**: a deterministic Rust engine shells out to `ripgrep` for candidate discovery and `ast-grep` for syntax rules, then enriches findings with owner guesses, legal remedies, forbidden moves, registry-backed approval checks, and pending report files.
- **Doctrine**: skills and project instructions teach the repair playbook instead of a longer denylist.
- **Repairer**: a dedicated subagent is optimized for multi-file owner-layer refactors and proof-carrying repairs.

The biggest behavioral changes are:

1. **No more full findings in main context.** The hot-path hook stores a detailed JSON report under `${CLAUDE_PLUGIN_DATA}/reports/` and returns only a short capsule to Claude.
2. **Registry-backed approvals.** `policy-approved:` comments are only honored when the identifier exists in `policy/defaults.yml` and the current file belongs to an allowed layer.
3. **Stop gate.** `PostToolUse` still blocks immediately for local feedback, but unresolved reports also block `Stop` and `SubagentStop`, which makes deeper multi-file repairs possible without spending the main conversation on giant findings blobs.
4. **Owner-layer doctrine.** Every violation is classified into an owner layer and a small set of legal remedies. The default answer is never “rename the variable and move on.”

## Core doctrine

A fallback is an effect handler, not a convenience.
A production substitute is an adapter, not a fake.

When a guardrail fires:

1. Find the owner layer: `boundary`, `domain`, `application`, `infrastructure`, `composition_root`, or `tests`.
2. Choose exactly one legal remedy:
   - approved policy API
   - boundary parser or settings model
   - `Optional`/union + exhaustive handling
   - typed exception / contract violation
   - explicit resilience adapter
   - move double to tests
   - promote substitute to a first-class adapter + contract tests
3. Add one witness:
   - schema/parser validation
   - exhaustiveness check
   - architecture/import rule
   - contract/property/stateful test
   - registered approval id
4. Never “fix” by rename-only or syntax-equivalent rewrites.

## Repository layout

```text
code-guardrails/
├── .claude-plugin/
├── agents/
├── docs/
├── fixtures/
├── hooks/
├── policy/
├── rules/
├── scripts/
├── skills/
├── src/
├── tests/
├── CHANGELOG.md
├── CLAUDE.md
├── Cargo.toml
├── marketplace.json
├── README.md
├── setup
└── sgconfig.yml
```

## Hook lifecycle

- `SessionStart` ensures the binary is built and version-synced.
- `PostToolUse` on `Edit|Write` runs a synchronous classifier that writes report files and returns a short block response.
- `PostToolUse` also runs an async audit that refreshes pending reports in the background.
- `Stop` and `SubagentStop` refuse to finish while unresolved pending reports remain.

Detailed hook behavior is documented in [`docs/architecture.md`](docs/architecture.md), [`docs/report-format.md`](docs/report-format.md), and [`docs/migration-guide.md`](docs/migration-guide.md).

## Policy files

Three policy files drive ownership and lawful escape hatches:

- [`policy/ownership.yml`](policy/ownership.yml) maps file globs to owner layers.
- [`policy/defaults.yml`](policy/defaults.yml) registers approved default IDs and their blessed symbols.
- [`policy/adapters.yml`](policy/adapters.yml) declares lawful runtime adapters and their contract suites.

Treat these as project-specific configuration. The defaults included here are intentionally opinionated examples.

## Commands

```bash
# Build the Rust engine
cargo build --release

# Run tests
cargo test --all-targets

# Lint
cargo fmt --check
cargo clippy -- -D warnings
python scripts/check-metadata.py

# Full project scan
./bin/code-guardrails-engine scan-tree --root . --config-dir .

# Single file scan
./bin/code-guardrails-engine scan-file --file path/to/file.py --config-dir .

# Hook-mode scan (reads Claude hook JSON from stdin)
cat hook-input.json | ./bin/code-guardrails-engine scan-hook --config-dir . --report-dir /tmp/cg-reports

# Stop gate
./bin/code-guardrails-engine scan-stop --report-dir /tmp/cg-reports --config-dir .
```

The engine exits `0` when clean, `1` when violations or pending reports exist, and `2` on tool/setup errors.

## Installation

Prerequisites:

- Claude Code
- `ast-grep` 0.14+
- `ripgrep` 14+
- Rust 1.85+
- `jq` 1.6+ (hooks and release script)

```bash
brew install ast-grep ripgrep
curl https://sh.rustup.rs -sSf | sh
```

Inside Claude Code:

```bash
/plugin marketplace add Wisteria30/code-guardrails
/plugin install code-guardrails@code-guardrails-marketplace
```

Restart Claude Code, then run `/scan`.

## Suggested project CLAUDE.md snippet

```markdown
## AI Code Policy

code-guardrails hook is active. Every Edit/Write is scanned for unowned fallbacks and runtime test doubles.

A fallback is an effect handler, not a convenience.
A production substitute is an adapter, not a fake.

When a guardrail fires, do not preserve the violating line.
Repair at the owner layer and add one witness.

Forbidden moves:
- rename mock/stub/fake
- syntax-equivalent fallback rewrites
- adding a new inline default
- inventing a new approval id
- importing test support into the runtime graph
```

## Example: good repair versus escape

Bad escape:

```python
tool_use_id = tool_use["toolUseId"] if "toolUseId" in tool_use else "tool"
```

Owner-layer repair:

```python
from pydantic import BaseModel, ConfigDict, ValidationError

class ToolUsePayload(BaseModel):
    model_config = ConfigDict(extra="forbid")
    toolUseId: str

def parse_tool_use(raw: dict) -> ToolUsePayload:
    try:
        return ToolUsePayload.model_validate(raw)
    except ValidationError as exc:
        raise BadRequest("toolUseId is required") from exc

payload = parse_tool_use(event.tool_use)
tool_use_id = payload.toolUseId
```

## Development notes

This repository keeps the hot path lean on purpose.

- `ripgrep` handles candidate discovery.
- `ast-grep` handles syntax-aware matching.
- Rust glues those tools together, validates approvals against the registry, classifies findings, and persists pending reports.

Heavy reasoning is intentionally pushed into skills and the repair subagent, not into synchronous hooks.

## Release

```bash
scripts/release.sh 1.2.3
```

This syncs `Cargo.toml`, `.claude-plugin/plugin.json`, and `marketplace.json`.

## License

MIT
