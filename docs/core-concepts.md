# Core Concepts

This document explains the model behind `witness` without assuming you already know the internal v3 design documents.

If you only want installation and the normal workflow, go back to [../README.md](../README.md).

---

## What witness is trying to protect

`witness` exists to keep AI-generated changes aligned with the architecture of a repository.

In practice, it focuses on a few failure modes that are easy for coding agents to introduce:

- silently replacing missing data with a default
- swallowing errors instead of making them explicit
- leaking test-only code into runtime paths
- creating public concepts without making the interface explicit
- adding boundary parsing without declaring the contract it is supposed to enforce

The point is not "ban convenience." The point is: if the code is making a business or architectural decision, that decision should live in the place that owns it and should leave a machine-checkable trace.

---

## The two layers of rules

`witness` works with two kinds of rules.

### Constitution

The **constitution** is the stable, repo-level rule set. It lives in `policy/*.yml` and explains things like:

- which files belong to which owner layer
- which defaults are explicitly approved
- which adapters are allowed
- which symbols are public
- which contracts exist
- which bounded contexts exist

This is the long-lived structure of the repo.
`witness` ships with bundled policy files, so your repo does not need to provide every file on day one. Add repo-local `policy/*.yml` only when you want to override the bundled defaults.

### Charter

The **charter** is the short-lived, change-specific rule set.

Use it when a single change introduces new architecture decisions, for example:

- a new public symbol
- a new contract
- a new adapter
- a new bounded-context assignment

The charter is not a full project plan. It is only the minimal change-specific information that `witness` needs to evaluate the patch correctly.

---

## The four kinds of findings

When `witness` scans a change, it reports findings in four buckets.

### Violation

The code is clearly breaking an existing rule.

Examples:

- a silent fallback in the wrong layer
- an empty `catch {}`
- a runtime `FakeRepository`

### Hole

The code needs a design decision that has not been declared yet, and `witness` refuses to guess.

Examples:

- a new public concept appears, but there is no context assignment
- boundary parsing exists, but no contract is declared

### Drift

The code and the declared rules disagree.

Examples:

- the charter says a symbol is public, but the export witness is missing
- a file is assigned to one context in policy but behaves like another

### Obligation

The change declared work that has not been completed yet.

Examples:

- the charter declares a contract, but `policy/contracts.yml` was not updated
- a contract requires a schema or witness file that does not exist yet

---

## How witness fits into a normal workflow

`witness` is not a replacement for planning, design discussion, or code review.

A good mental model is:

- your normal workflow decides **what** to build
- `witness` checks **whether the code and repo rules still agree**

That means:

- use your usual plan or design review first
- use `/witness:charter` only when the change adds new architecture decisions
- use `/witness:scan` to see the current state
- use `/witness:repair` to fix concrete findings

---

## How the system is organized

`witness` v3 is split into four layers:

| Layer | What it does | Where it runs |
|-------|--------------|---------------|
| Constitution | Holds durable repo rules | `policy/*.yml`, `CLAUDE.md` |
| Verifier | Detects and reports problems | Rust engine + `ast-grep` + `ripgrep` |
| Charter compiler | Records change-specific architecture decisions | `/witness:charter` |
| Repairer | Applies fixes and related policy updates | `/witness:repair` |

The hot path stays cheap. Expensive repair work is pushed into explicit workflows instead of running on every edit.

---

## What witness catches

### Unapproved fallbacks

Examples:

```python
timeout = config.get("timeout", 30)
name = user_name or "unknown"
```

```typescript
const port = config.port ?? 3000;
const name = input || "default";
```

### Swallowed failures

Examples:

```python
try:
    connect()
except ConnectionError:
    pass
```

```typescript
try {
  await fetch(url);
} catch {}
```

### Test doubles in runtime code

Examples:

```python
mock_client = MockHttpClient()
from unittest.mock import patch
```

```typescript
const fakeRepo = new FakeUserRepository();
```

### Hidden owner-layer concepts

Example:

```python
class _ToolUsePayload(BaseModel):
    toolUseId: str
```

### Missing boundary/interface witnesses

Examples:

- public symbol added without explicit export witness
- boundary parser added without contract witness

---

## What a lawful fix usually looks like

`witness` is opinionated about the shape of a fix.

Typical lawful fixes include:

| Remedy | Meaning |
|--------|---------|
| `approved_policy_api` | Use a named, explicitly approved policy API |
| `boundary_parser` | Parse and validate once at the edge |
| `typed_exception` | Raise a clear typed error instead of hiding the failure |
| `optional_exhaustive_handling` | Keep the value optional and handle all branches explicitly |
| `move_double_to_tests` | Remove test doubles from runtime paths |
| `promote_to_first_class_adapter` | Turn the alternate implementation into a real runtime adapter |

The general rule is: do not hide the decision inline if the architecture expects it to be explicit.

---

## Approval comments

Some defaults are intentional and allowed, but they must be registered.

Example:

```python
# policy-approved: REQ-123 explicit locale default
lang = LocalePolicy.default_locale(payload.get("lang"))
```

For that approval to be valid:

- the ID must exist in `policy/defaults.yml`
- the file must belong to an allowed owner layer
- the symbol at the call site must match the blessed API

If any of those checks fail, the code is still reported.

---

## Policy files

These files are the main inputs that `witness` reads:

| File | Purpose |
|------|---------|
| `policy/ownership.yml` | Map file globs to owner layers |
| `policy/defaults.yml` | Register approved default IDs and blessed symbols |
| `policy/adapters.yml` | Declare legal runtime adapters |
| `policy/surfaces.yml` | Define public/internal symbol rules |
| `policy/contracts.yml` | Declare boundary and inter-context contracts |
| `policy/contexts.yml` | Define bounded contexts and vocabulary |

Your repo does not have to define all of these files immediately. If a repo-local file is missing, `witness` falls back to the bundled default file that ships with the plugin. If you add a repo-local file with the same name, that file overrides the bundled one.

If you need more detail, continue with [policies.md](policies.md).
