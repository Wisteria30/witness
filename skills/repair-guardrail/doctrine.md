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

## Step 2 — choose exactly one legal remedy

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

Use when the value is truly optional.
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

## Forbidden moves

Never do these:

- rename `mock` / `stub` / `fake`
- translate `.get(key, default)` into `if key in dict else default`
- add a new implicit default
- invent a new `policy-approved` id
- import test support into runtime code

## Fast examples

### Bad

```python
tool_use_id = event.tool_use.get("toolUseId", "tool")
```

### Good

```python
class ToolUsePayload(BaseModel):
    toolUseId: str

payload = ToolUsePayload.model_validate(event.tool_use)
tool_use_id = payload.toolUseId
```

### Bad

```ts
const repo = new FakeUserRepository()
```

### Good

- move the fake to tests, or
- promote it to `SandboxUserRepository`, register it in `policy/adapters.yml`, prove it with contract tests, and instantiate it only in the composition root
