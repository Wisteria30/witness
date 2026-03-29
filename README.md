# witness

**Push AI-generated fallbacks and test doubles back to their rightful owner layer, with proof.**

AI coding tools are fast. But they silently paper over failures with `?? "default"`, swallow exceptions with `pass`, and leave `FakeRepository` in production. Miss it in review, and it ships.

witness doesn't just block bad syntax. It classifies each violation by owner layer, demands a lawful remedy, and repairs at the architecture level — not the line level.

---

## Install

Run inside Claude Code:

```
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
# OK — exception handled properly
try:
    connect()
except ConnectionError as e:
    logger.error(f"Connection failed: {e}")
    raise ServiceUnavailable("DB unreachable") from e

# OK — boundary parser (the witness way)
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

---

## How It Works

witness is split into three layers:

| Layer | What it does | Where it runs |
|-------|-------------|---------------|
| **Verifier** | Detect, classify, persist reports | Rust engine + ast-grep + ripgrep (sync hook, < 100ms) |
| **Doctrine** | Teach owner-layer repair playbook | Skills + CLAUDE.md |
| **Repairer** | Multi-file architectural repair | 5 parallel worktree-isolated agents |

The hot-path hook returns a short capsule to Claude — never full findings. Detailed reports are persisted to disk. Heavy repair runs in isolated subagents, not in main context.

---

## Core Doctrine

A fallback is an effect handler, not a convenience.
A production substitute is an adapter, not a fake.

When a violation fires:

1. **Classify** the owner layer: `boundary` / `domain` / `application` / `infrastructure` / `composition_root` / `tests`
2. **Challenge** the optionality: is the absent case specified? If not, eliminate it.
3. **Choose** exactly one legal remedy
4. **Add** one machine-checkable witness
5. **Never** rename, rewrite to equivalent syntax, or add a new inline default

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
|--------|------------|
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

Approval is **registry-backed**: the ID must exist in `policy/defaults.yml` and the file must belong to an allowed owner layer. Invalid or unregistered IDs are reported as violations.

---

## Policy Files

| File | Purpose |
|------|---------|
| `policy/ownership.yml` | Maps file globs to owner layers |
| `policy/defaults.yml` | Registers approved default IDs and blessed symbols |
| `policy/adapters.yml` | Declares lawful runtime adapters and contract test paths |

These are project-specific. Configure them for your codebase.

---

## Skills

| Skill | What it does |
|-------|-------------|
| `/witness:scan` | Full project scan. Reports violations by owner layer. Read-only. |
| `/witness:repair` | Dispatches 5 parallel repair agents in isolated worktrees. Challenges optionality, applies lawful remedies, asks user for ambiguous cases. |
| `/witness:add-rule` | Guided workflow for adding a new ast-grep detection rule. |

---

## Add to your CLAUDE.md

```markdown
## AI Code Policy

witness hook is active. Every Edit/Write is scanned for violations.

- NEVER write `except: pass`, empty `catch {}`, or `.catch(() => null)`
- NEVER use `mock`, `stub`, `fake` identifiers in production code
- NEVER add silent defaults without spec approval
- Unspecified fallbacks are bugs. If the spec doesn't say "default to X", don't default to X
```

---

## Development

```bash
cargo build --release
cargo test --all-targets
cargo fmt --check
cargo clippy -- -D warnings
```

## Releasing

See [docs/releasing.md](docs/releasing.md).

## License

MIT
