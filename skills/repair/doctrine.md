# Guardrail Repair Doctrine

A fallback is an effect handler, not a convenience.
A production substitute is an adapter, not a fake.

When a guardrail fires, never preserve the violating line.

## Step 1 — classify the owner layer

Choose one:

- `boundary`
- `domain`
- `application`
- `infrastructure`
- `composition_root`
- `tests`

Heuristics:

- raw JSON, raw dicts, env vars, CLI flags, queue payloads, and HTTP request decoding belong to **boundary**
- invariants and always-valid objects belong to **domain**
- orchestration and use-case flow belong to **application**
- retries, caches, secondary systems, noop/sandbox behavior, and resilience belong to **infrastructure**
- concrete adapter selection belongs to **composition_root**
- doubles belong to **tests**

## Step 1.5 — challenge the optionality

Before choosing a remedy, ask:

- Is there a spec, schema, or contract that says this value can be absent?
- Does any caller intentionally omit it?

If the absent case has no specification: choose `eliminate_optionality`.

If you cannot determine whether absence is intended: do NOT guess.
Mark the violation as `needs_human_decision` in your output and move to the next violation.
Record what you found and what the two most likely remedies would be.
These will be presented to the user for decision after all decidable repairs are done.

## Step 2 — choose exactly one legal remedy

### `eliminate_optionality`

Use when the absent case has no specification.
The field should be required, not optional. Remove the default,
make the caller supply the value, or remove the field entirely.
This is not a fallback remedy — it eliminates the need for fallback.

### `approved_policy_api`

Use when the default is genuinely specified.
Prefer a context-specific policy API over an inline helper.

Examples:

- `LocalePolicy.default_locale(raw_locale)`
- `DemoLabelPolicy.resolve(api_value)`
- `SettingsPolicy.default_port(raw_env)`

### `boundary_parser`

Use when the problem is untrusted input.
Parse once, validate once, normalize once, then move only trusted values into the core.

Examples:

- Pydantic `BaseModel.model_validate(...)`
- Zod / valibot / io-ts parse at the edge
- a settings object that owns env defaults

### `optional_exhaustive_handling`

Use when the value is truly optional and the spec says so.
Do not totalize it prematurely. Keep the sum type visible and handle it explicitly.

### `typed_exception`

Use when the state is invalid or unreachable for the current layer.
Raise or propagate a typed error. Map transport concerns only at the outer edge.

### `resilience_adapter`

Use when the fallback belongs to infrastructure policy.
Retries, cache hits, secondaries, and degrade modes must live in an explicit adapter with metrics and tests.

### `move_double_to_tests`

Use when the runtime substitute exists only for test convenience.
Delete it from runtime code and keep it under tests.

### `promote_to_first_class_adapter`

Use when an alternate runtime implementation is legitimate.
Name it as a real adapter, wire it in the composition root, and add contract tests.

## Step 3 — add one witness

Choose at least one:

- parser/schema validation
- exhaustiveness check
- architecture/import rule
- contract/property/stateful test
- registered approval id
- explicit export manifest (`__all__`, named exports, etc.)

## Step 4 — challenge the interface

Every new top-level symbol introduced by a repair must be classified:

- **public concept** — an owner-layer noun (Payload, Policy, Adapter, Error, Repository, Settings, Parser). Public by default. No restricted visibility.
- **subclass / extension API** — designed for downstream override. Public.
- **internal mechanic** — a helper that serves exactly one public entry point. Prefer local scope or nesting over a top-level restricted-visibility symbol.

Diagnostic: if the construct can be explained in one sentence as a metaphor
(e.g. "ToolUsePayload is the passport control at the API boundary"),
it is a public concept. If it cannot, it carries too much responsibility — split or move it.

**Public concepts, private mechanics.**
Owner-layer concepts are public. One-off computation steps are private.
The problem is not "private exists" but "an owner-layer concept hiding behind restricted visibility"
or "top-level restricted-visibility helpers proliferating without a clear public entry point."

If publicity is unclear, return `needs_human_decision` with the symbol name,
its one-sentence description, and the two most likely classifications.

### Interface witness

After repair, the module's explicit export manifest must reflect the decision:

- Python: update `__all__` (or explicit `__init__.py` re-exports) to include every public symbol.
- TypeScript: use named `export` for every public symbol.
- Other languages: use the language's idiomatic export/visibility mechanism.

If a module gains new public symbols and has no export manifest, create one.

## Forbidden moves

Never do these:

- rename `mock` / `stub` / `fake`
- translate `.get(key, default)` into `if key in dict else default`
- add a new implicit default
- invent a new `policy-approved` id
- import test support into runtime code
- hide an owner-layer concept behind restricted visibility (interface uncertainty — revisit Step 4)
- proliferate top-level restricted-visibility helpers without a clear public entry point

## Fast examples

### Bad

```python
tool_use_id = event.tool_use.get("toolUseId", "tool")
```

### Good — eliminate optionality (when absence has no spec)

```python
class ToolUsePayload(BaseModel):
    toolUseId: str

payload = ToolUsePayload.model_validate(event.tool_use)
tool_use_id = payload.toolUseId
```

### Good — approved policy (when default is specified)

```python
# policy-approved: REQ-123 locale default is defined by spec
lang = LocalePolicy.default_locale(payload.get("lang"))
```

### Bad — owner-layer concept hidden behind restricted visibility

```python
class _ToolUsePayload(BaseModel):
    toolUseId: str

def _parse_tool_use(raw: dict) -> _ToolUsePayload:
    return _ToolUsePayload.model_validate(raw)
```

### Good — public concept with export manifest witness

```python
__all__ = ["ToolUsePayload", "parse_tool_use"]

class ToolUsePayload(BaseModel):
    """Passport control at the API boundary — validates a raw tool-use dict."""
    toolUseId: str

def parse_tool_use(raw: dict) -> ToolUsePayload:
    return ToolUsePayload.model_validate(raw)
```

### Bad

```ts
const repo = new FakeUserRepository()
```

### Good

- move the fake to tests, or
- promote it to `SandboxUserRepository`, register it in `policy/adapters.yml`, prove it with contract tests, and instantiate it only in the composition root
