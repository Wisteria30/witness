# witness

**Push AI-generated fallbacks and test doubles back to their rightful owner layer, with proof.**

AI coding tools are fast. But they silently paper over failures with `?? "default"`, swallow exceptions with `pass`, leave `FakeRepository` in production, and blur public concepts into hidden `_helpers`. Miss it in review, and it ships.

witness does not try to be another general-purpose planning system. Claude Code already has broad planning workflows. witness v3 focuses only on the constitutional kernel that those workflows do not guarantee by themselves:

- who owns a reduction of absence or failure into a value
- what is public versus internal
- what each boundary promises
- which runtime substitutes are lawful
- which bounded context a public concept belongs to

witness takes the minimal witness-relevant intent from an approved plan, compiles it into a sparse **charter** (`ΔK_w`), and then uses `/witness:scan`, `/witness:repair`, and stop gates to prove that the implementation matches the repo constitution plus that charter.

---

## Install

Run inside Claude Code:

```text
/plugin marketplace add Wisteria30/witness
/plugin install witness@witness-marketplace
```

Restart Claude Code. Verify with `/witness:scan`.

Dependencies (`ast-grep`, `ripgrep`) are auto-installed by the setup script. No Rust required — pre-built binaries are downloaded from GitHub Releases.

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

After changes, run `/reload-plugins` in-session.

---

## What It Catches

### Test doubles in production code

Flags `mock` / `stub` / `fake` identifiers and test-support imports in non-test files.

```python
# NG
mock_client = MockHttpClient()
from unittest.mock import patch
```

```python
# OK — inside test files (test_*.py, **/tests/**, etc.)
mock_client = MockHttpClient()
```

### Unapproved fallbacks

Flags patterns that silently eliminate absence or swallow errors.

#### Python

```python
# NG — silent defaults
timeout = config.get("timeout", 30)
name = user_name or "unknown"
port = os.getenv("PORT", "8080")

# NG — swallowed exception
try:
    connect()
except ConnectionError:
    pass
```

```python
# OK — typed error
try:
    connect()
except ConnectionError as e:
    logger.error(f"Connection failed: {e}")
    raise ServiceUnavailable("DB unreachable") from e

# OK — boundary parser
class Config(BaseModel):
    timeout: int
    port: int

config = Config.model_validate(raw_settings)
```

#### TypeScript

```typescript
// NG — silent defaults
const port = config.port ?? 3000;
const name = input || "default";

// NG — swallowed errors
try { await fetch(url); } catch {}
fetch(url).catch(() => null);
```

```typescript
// OK — typed error
try {
  await fetch(url);
} catch (e) {
  throw new FetchError("request failed", { cause: e });
}
```

### Equivalent rewrites (also caught)

```python
# NG — same fallback, different syntax
tool_use_id = tool_use["toolUseId"] if "toolUseId" in tool_use else "tool"
```

```python
# The real fix: boundary parser
class ToolUsePayload(BaseModel):
    toolUseId: str

payload = ToolUsePayload.model_validate(event.tool_use)
tool_use_id = payload.toolUseId
```

### Hidden owner-layer concepts

```python
# NG — owner-layer concept hidden behind restricted visibility
class _ToolUsePayload(BaseModel):
    toolUseId: str


def _parse_tool_use(raw: dict) -> _ToolUsePayload:
    return _ToolUsePayload.model_validate(raw)
```

```python
# OK — public concept with explicit export manifest
__all__ = ["ToolUsePayload", "parse_tool_use"]

class ToolUsePayload(BaseModel):
    toolUseId: str


def parse_tool_use(raw: dict) -> ToolUsePayload:
    return ToolUsePayload.model_validate(raw)
```

---

## What witness v3 Actually Proves

witness v3 is built around a repo **constitution** and a per-change **charter**.

- **Constitution (`K₀`)** lives in `policy/*.yml` and defines durable rules for ownership, defaults, adapters, surfaces, contracts, and bounded contexts.
- **Charter (`ΔK_w`)** is a sparse, change-local projection of an approved broad plan. It records only the witness-relevant intent for the current change.

`witness` never tries to replace Claude Code’s general planning workflows. It extracts only the narrow normative fragment that scan/repair need to know.

The effective environment for evaluation is:

```text
K = K₀ ⊕ ΔK_w
```

A change is accepted only when there are no unresolved:

- **violations** — clear constitutional breaches
- **holes** — underdetermined decisions that must not be guessed
- **drift** — constitution and code-level witnesses disagree
- **obligations** — charter-declared work not yet discharged

---

## How It Works

witness v3 is split into four layers:

| Layer | What it does | Where it runs |
|-------|--------------|---------------|
| **Constitution** | Holds durable repo law | `policy/*.yml`, `CLAUDE.md` |
| **Verifier** | Detects, classifies, persists reports | Rust engine + ast-grep + ripgrep (sync hook, < 100ms) |
| **Charter compiler** | Projects broad plan into sparse witness intent (`ΔK_w`) | `/witness:charter` |
| **Repairer** | Applies lawful, owner-layer repair and compiles persistent constitutional changes | `/witness:repair` + 5 worktree-isolated agents |

The hot-path hook returns a short capsule to Claude — never full findings. Detailed reports are persisted to disk. Heavy repair runs in isolated subagents, not in main context.

---

## Core Doctrine

A fallback is an effect handler, not a convenience.
A production substitute is an adapter, not a fake.
A hidden owner-layer concept is a broken interface witness.
A missing contract is a broken boundary witness.

When a witness violation fires:

1. **Load** the repo constitution and the active charter, if one exists.
2. **Classify** the owner layer.
3. **Challenge** the bounded context and optionality.
4. **Choose** exactly one legal remedy.
5. **Add** one machine-checkable witness.
6. **Challenge** the interface.
7. **Compile** any persistent constitutional change.
8. **Never** rename, rewrite to equivalent syntax, or guess underdetermined intent.

---

## Detection Rules

### Python (12 rules)

| Rule | Pattern | Example |
|------|---------|---------|
| `py-no-fallback-get-default` | `.get(key, default)` | `d.get("k", 0)` |
| `py-no-fallback-bool-or` | `x = a or b` | `name = val or "default"` |
| `py-no-fallback-getattr-default` | `getattr(o, n, default)` | `getattr(o, "x", None)` |
| `py-no-fallback-next-default` | `next(iter, default)` | `next(gen, None)` |
| `py-no-fallback-os-getenv-default` | `os.getenv(k, default)` | `os.getenv("PORT", "8080")` |
| `py-no-fallback-contextlib-suppress` | `contextlib.suppress(...)` | `with suppress(KeyError):` |
| `py-no-fallback-except-return-default` | `except: return default` | `except E: return []` |
| `py-no-fallback-conditional-*` | `x if cond else default` | `x if x in d else "fallback"` |
| `py-no-swallowing-except-pass` | `except ...: pass` | `except ValueError: pass` |
| `py-no-test-double-identifier` | `mock\|stub\|fake` identifier | `mock_client` |
| `py-no-test-double-unittest-mock` | `import unittest.mock` | `from unittest.mock import patch` |
| `py-no-hidden-owner-concept` | `_Payload`, `_Policy`, `_Adapter`, etc. | `_ToolUsePayload` |

### TypeScript (11 rules)

| Rule | Pattern | Example |
|------|---------|---------|
| `ts-no-fallback-nullish` | `a ?? b` | `port ?? 3000` |
| `ts-no-fallback-or` | `a \|\| b` | `val \|\| "default"` |
| `ts-no-fallback-nullish-assign` | `a ??= b` | `cache ??= new Map()` |
| `ts-no-fallback-or-assign` | `a \|\|= b` | `opt.x \|\|= 5` |
| `ts-no-fallback-ternary-default` | `x !== undefined ? x : d` | `v != null ? v : 0` |
| `ts-no-fallback-lookup-else-default` | `x ? x[k] : default` | `obj ? obj.id : "none"` |
| `ts-no-catch-return-default` | `catch { return default }` | `catch(e) { return [] }` |
| `ts-no-empty-catch` | `catch {}` | `catch(e) {}` |
| `ts-no-promise-catch-default` | `.catch(() => default)` | `.catch(() => null)` |
| `ts-no-test-double-identifier` | `mock\|stub\|fake` identifier | `mockFetch` |
| `ts-no-test-double-import` | test-support import | `import from "sinon"` |

---

## Legal Remedies

| Remedy | When to use |
|--------|-------------|
| `eliminate_optionality` | Absent case has no spec. Make it required. |
| `approved_policy_api` | Default is specified. Use a blessed policy API. |
| `boundary_parser` | Untrusted input. Parse once at the edge. |
| `optional_exhaustive_handling` | Value is truly optional per spec. Handle all branches. |
| `typed_exception` | State is invalid. Raise typed error. |
| `resilience_adapter` | Infra policy. Retry/cache/secondary with metrics. |
| `move_double_to_tests` | Test double leaked into production. Delete from runtime. |
| `promote_to_first_class_adapter` | Alternate implementation is legitimate. Name it, contract-test it. |

---

## Approval Model

Intentional fallbacks can be approved with an adjacent comment:

```python
# policy-approved: REQ-123 explicit locale default
lang = LocalePolicy.default_locale(payload.get("lang"))
```

Approval is **registry-backed**:

- the ID must exist in `policy/defaults.yml`
- the file must belong to an allowed owner layer
- the symbol must match the blessed policy API

Invalid or unregistered IDs are reported as violations.

---

## Policy Files

| File | Purpose |
|------|---------|
| `policy/ownership.yml` | Maps file globs to owner layers |
| `policy/defaults.yml` | Registers approved default IDs and blessed eliminator symbols |
| `policy/adapters.yml` | Declares lawful runtime adapters |
| `policy/surfaces.yml` | Defines public/internal symbol policy and export witnesses |
| `policy/contracts.yml` | Declares boundary and inter-context contracts (`shape`, `interaction`, `law`) |
| `policy/contexts.yml` | Declares bounded contexts, vocabulary, and permitted dependencies |

These are project-specific. Configure them for your codebase.

---

## Skills

All operational witness skills are **explicit-only**. They are not meant to auto-fire and compete with Claude Code’s built-in planning machinery.

| Skill | What it does |
|-------|--------------|
| `/witness:charter` | Projects only the witness-relevant part of an approved broad plan into a sparse change charter (`ΔK_w`). |
| `/witness:scan` | Full constitutional scan. Reports violations, holes, drift, and obligations. Read-only. |
| `/witness:repair` | Dispatches 5 parallel repair agents in isolated worktrees. Consumes the active charter when present. |
| `/witness:shape` | Read-only structural diagnosis. Extracts principal roles, context blur, and missing surface/contract witnesses. |
| `/witness:add-rule` | Guided workflow for adding a new ast-grep detection rule. |

### How witness coexists with broad planning

1. Use Claude Code Plan Mode or your preferred planning workflow to generate and approve the broad implementation plan.
2. Run `/witness:charter` to extract only the witness-relevant decisions.
3. Implement as usual.
4. Run `/witness:scan`.
5. If there are violations, drift, or obligations, run `/witness:repair`.
6. If there are holes, answer only the narrow charter questions witness asks.

witness does **not** create another full plan. It compiles only the minimum normative fragment scan and repair need.

---

## Add to your CLAUDE.md

```markdown
## AI Code Policy
witness hook is active. Every Edit/Write is scanned for violations.

- NEVER write `except: pass`, empty `catch {}`, or `.catch(() => null)`
- NEVER use `mock`, `stub`, `fake` identifiers in production code
- NEVER add silent defaults without spec approval
- NEVER hide owner-layer concepts behind restricted visibility
- Unspecified fallbacks are bugs. If the spec doesn't say "default to X", don't default to X
```

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
