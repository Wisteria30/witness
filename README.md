# witness

**Catch unsafe defaults, swallowed failures, leaked test doubles, and missing interface promises before they ship.**

`witness` is a Claude Code plugin for teams that want AI-generated code to stay aligned with the architecture of the repository.
It does not try to replace normal planning or design review. It focuses on a narrower job:

- stop production code from silently turning missing data or errors into defaults
- stop test-only substitutes from leaking into runtime code
- make public vs internal interfaces explicit
- make boundary promises and context boundaries visible

Current language support:

- Python: `.py`
- TypeScript: `.ts`, `.cts`, `.mts`
- TSX: `.tsx`
- Go: `.go`
- Rust: `.rs`

If you want the deeper model behind `constitution`, `charter`, finding kinds, and policy files, start with [docs/core-concepts.md](docs/core-concepts.md).

---

## Install

Run inside Claude Code:

```text
/plugin marketplace add Wisteria30/witness
/plugin install witness@witness-marketplace
```

Restart Claude Code, then verify the plugin is active:

```text
/witness:scan
```

Dependencies such as `ast-grep` and `ripgrep` are installed by the setup script. You do not need Rust to use the released plugin.
The plugin also ships with bundled default policy files, so you can start scanning before defining repo-specific policy overrides.

### Share with your team

```bash
cp -Rf ~/.claude/plugins/witness .claude/plugins/witness
rm -rf .claude/plugins/witness/.git
git add .claude/plugins/witness && git commit -m "chore: add witness plugin"
```

### Local development

```bash
claude --plugin-dir ./path-to-witness
```

After local changes, run `/reload-plugins` in the Claude Code session.

---

## Quickstart

Use this if you just want to know how to work with the plugin day to day.

### 1. Install the plugin

Use the install commands above and confirm `/witness:scan` is available.

### 2. You can start without creating `policy/`

By default, `witness` uses the bundled policy files that ship with the plugin.
That means you can install it and immediately run `/witness:scan` without first designing your own repo policy.

Create a `policy/` directory in your repo only when you want to override the bundled defaults for your project.
Overrides are file-based:

- if your repo has `policy/ownership.yml`, that file overrides the bundled `ownership.yml`
- if your repo does not have `policy/contracts.yml`, the bundled `contracts.yml` is still used

If you are adopting witness in an existing codebase and want repo-specific overrides, see [docs/policies.md](docs/policies.md).

### 3. For ordinary code changes, just work normally

Edit code as usual. The plugin hook checks changed files automatically.

When you want a full check, run:

```text
/witness:scan
```

If the scan reports `violation`, `drift`, or `obligation`, run:

```text
/witness:repair
```

If the scan reports `hole`, answer that missing design decision first. `witness` is telling you it cannot safely guess.

### 4. For changes that add new public API or new architecture rules, create a charter first

If your change introduces things like:

- a new public type or public function
- a new contract at a boundary
- a new adapter
- a new bounded-context assignment

first get the broad plan approved in your normal workflow, then run:

```text
/witness:charter
```

That records only the change-specific architecture decisions that `witness` needs.

### 5. Stop only when the repo is clean

Before you stop, make sure there are no unresolved witness reports. The stop gate will block if there are still unresolved problems or missing decisions.

---

## When To Use Which Skill

### `/witness:scan`

Use this when you want to know the current state of the repo or a change.

Use it for:

- "Did my change introduce any witness issues?"
- "Show me the current violations, holes, drift, or obligations."
- "I changed several files and want one report before I continue."

### `/witness:repair`

Use this when `scan` already found concrete problems and you want witness to apply fixes.

Use it for:

- unsafe defaults and swallowed errors
- leaked test doubles in production code
- missing export witnesses for public concepts
- contract/policy updates that should happen in the same patch

Do not start here. Run `/witness:scan` first so the repair work has a clear target.

### `/witness:charter`

Use this when the change itself extends the architecture rules of the repo.

Use it for:

- adding a new public surface
- adding a new contract
- adding a new runtime adapter
- deciding which bounded context a new concept belongs to

Do not use it for routine bug fixes that stay inside the current architecture.

### `/witness:shape`

Use this when the code is hard to reason about and you want structural diagnosis before changing it.

Use it for:

- legacy modules that mix too many responsibilities
- code where public vs internal API is unclear
- modules that seem to blend multiple bounded contexts

`shape` is read-only. It helps you understand the problem before you decide how to change the code.

### `/witness:add-rule`

Use this when you want to teach witness a new cheap syntactic detection rule.

---

## What The Results Mean

`witness` reports four kinds of findings:

- `violation`: the code is clearly breaking an existing rule
- `hole`: a design decision is missing, so witness refuses to guess
- `drift`: the code and the declared repo rules disagree
- `obligation`: the change declared work that has not been completed yet

The normal loop is:

1. Run `/witness:scan`.
2. If you get `hole`, answer the missing design question.
3. If you get `violation`, `drift`, or `obligation`, run `/witness:repair`.
4. Run `/witness:scan` again until it is clean.

For concrete examples of each finding type, see [docs/core-concepts.md](docs/core-concepts.md).

---

## What It Catches

Examples of problems `witness` is built to catch:

- `config.get("timeout", 30)` in the wrong layer
- `user_name or "unknown"`
- `catch {}` or `.catch(() => null)`
- `mock`, `stub`, or `fake` names in production code
- hidden owner-layer concepts such as `_ToolUsePayload`
- public symbols that were added without an explicit export witness
- boundary parsing code with no declared contract witness

More examples and rule details are in [docs/core-concepts.md](docs/core-concepts.md).

---

## Policy Files

These files describe the repo rules that `witness` checks against.
They are optional in your repo because `witness` ships with bundled defaults.
Add a file under your repo's `policy/` directory only when you want to override the bundled file of the same name.

| File | Purpose |
|------|---------|
| `policy/ownership.yml` | Which files belong to which owner layer |
| `policy/defaults.yml` | Which defaults are explicitly approved |
| `policy/adapters.yml` | Which runtime adapters are allowed |
| `policy/surfaces.yml` | Which symbols are public, internal, or extension-facing |
| `policy/contracts.yml` | Which boundary and inter-context contracts exist |
| `policy/contexts.yml` | Which bounded contexts exist and how files/symbols map to them |

Start with [docs/policies.md](docs/policies.md) if you need to set these up for a real repository.

---

## Add To Your CLAUDE.md

```markdown
## AI Code Policy
witness hook is active. Every Edit/Write is scanned for violations.

- NEVER write `except: pass`, empty `catch {}`, or `.catch(() => null)`
- NEVER use `mock`, `stub`, `fake` identifiers in production code
- NEVER add silent defaults without spec approval
- NEVER hide owner-layer concepts behind restricted visibility
- Unspecified fallbacks are bugs. If the spec does not say "default to X", do not default to X
```

---

## Further Reading

- [docs/core-concepts.md](docs/core-concepts.md)
- [docs/policies.md](docs/policies.md)
- [docs/architecture.md](docs/architecture.md)
- [docs/migration-guide.md](docs/migration-guide.md)
- [docs/v3-design.md](docs/v3-design.md)

---

## Development

```bash
cargo build --release
cargo test --all-targets
cargo test --test metadata_validation
cargo fmt --check
cargo clippy -- -D warnings
```

The following skeleton stays intentionally unchanged from the current witness repository:

- CI jobs and shell validation in `.github/workflows/ci.yml`
- Rust package name/version wiring in `Cargo.toml`
- engine entrypoint in `src/main.rs`
- hook script locations under `hooks/*.sh`
- skill placement under `skills/*/SKILL.md`
- plugin packaging under `.claude-plugin/`

## Releasing

See [docs/releasing.md](docs/releasing.md).

## License

MIT
