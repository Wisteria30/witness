# Witness Repair Doctrine

A fallback is an effect handler, not a convenience.
A production substitute is an adapter, not a fake.
A hidden owner-layer concept is a broken surface witness.
A new boundary parser without an explicit contract is a broken contract witness.

When a witness finding fires, never preserve the violating line.
Repair at the owner layer, under the repo constitution and the active charter.

## Step 0 — load constitution and charter
Before touching code, load:

- `policy/ownership.yml`
- `policy/defaults.yml`
- `policy/adapters.yml`
- `policy/surfaces.yml`
- `policy/contracts.yml` if present
- `policy/contexts.yml` if present
- the relevant active charter slice, if one was supplied

If a judgement is already fixed by the constitution or charter, obey it.
If the judgement is not fixed there and cannot be derived from code with high confidence, do **not** guess. Return `needs_charter_decision`.

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

## Step 1.25 — challenge the context
Every new public concept must belong to exactly one bounded context.

Ask:

- which context’s vocabulary names this concept?
- does the symbol mix nouns from multiple contexts?
- is the symbol better moved than repaired in place?

If the context assignment is unclear, return `needs_charter_decision` with two likely contexts.

## Step 1.5 — challenge the optionality
Before choosing a remedy, ask:

- is there a spec, schema, or contract that says this value can be absent?
- does any caller intentionally omit it?
- is there a registry-backed blessed eliminator for this case?

If the absent case has no specification: choose `eliminate_optionality`.
If you cannot determine whether absence is intended: do **not** guess. Return `needs_charter_decision`.

## Step 2 — choose exactly one legal remedy

### `eliminate_optionality`
Use when the absent case has no specification.
Remove the default, make the caller supply the value, or remove the field entirely.

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
Do not totalize it prematurely.
Keep the sum type visible and handle it explicitly.

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
Choose based on what the remedy introduced.

| Remedy | Preferred witness |
|--------|-------------------|
| `eliminate_optionality` | parser/schema validation (the required field itself is the proof) |
| `approved_policy_api` | registered approval id + blessed symbol |
| `boundary_parser` | parser/schema validation + contract witness if the boundary is new |
| `optional_exhaustive_handling` | exhaustiveness check |
| `typed_exception` | contract/property/stateful test |
| `resilience_adapter` | contract/property/stateful test + architecture/import rule |
| `move_double_to_tests` | architecture/import rule |
| `promote_to_first_class_adapter` | contract/property/stateful test + `policy/adapters.yml` registration |

If a repair introduces new public symbols, also add an explicit export manifest.
If a repair introduces or changes a boundary promise, also add or update a contract witness in `policy/contracts.yml`.
If multiple witnesses apply, choose the one closest to the repaired code and compile any durable policy changes in the same patch.

## Step 4 — challenge the interface
Every new top-level symbol introduced by a repair must be classified:

- **public concept** — an owner-layer noun (`Payload`, `Policy`, `Adapter`, `Error`, `Repository`, `Settings`, `Parser`). Public by default. No restricted visibility.
- **subclass / extension API** — designed for downstream override. Public.
- **internal mechanic** — a helper that serves exactly one public entry point. Prefer local scope or nesting over a top-level restricted-visibility symbol.

Diagnostic:

If the construct can be explained in one sentence as a principal role — for example, `ToolUsePayload is the passport control at the API boundary` — it is probably a public concept.
If it cannot, it may carry too much responsibility and should be split or moved.

**Public concepts, private mechanics.**
Owner-layer concepts are public.
One-off computation steps are private.

If publicity is unclear, return `needs_charter_decision` with:

- the symbol name
- its one-sentence role description
- the two most likely classifications

### Interface witness
After repair, the module’s explicit export manifest must reflect the decision:

- Python: update `__all__` or explicit `__init__.py` re-exports
- TypeScript: use named `export`
- Other languages: use the language’s idiomatic export/visibility mechanism

If a module gains new public symbols and has no export manifest, create one.

## Step 5 — challenge the contract
If the repair introduces or changes a boundary crossing or inter-context interaction, determine the contract kind:

- `shape` — payload / DTO / parser / settings shape
- `interaction` — HTTP / queue / event / consumer-provider interaction
- `law` — in-process behavioral law for a port

Then determine the compatibility mode:

- `exact`
- `backward_additive`
- `additive_fields_only`
- `no_breaking_change`
- `explicit_migration_required`

If contract kind or compatibility is unclear, return `needs_charter_decision`.

## Step 6 — compile persistent constitutional changes
If your repair introduced a durable architectural fact, compile it into the constitution in the same patch.
Examples:

- new public concept class → `policy/surfaces.yml` may need an added concept pattern only if it is a reusable family, not a one-off
- new boundary promise → `policy/contracts.yml`
- new bounded context or allowed dependency → `policy/contexts.yml`
- new lawful runtime adapter → `policy/adapters.yml`
- new blessed eliminator → `policy/defaults.yml`

Do not leave durable repo law only in a transient charter when the repo should remember it.

## Step 7 — re-scan and decide outcome
After repairing each file:

1. re-run the relevant scan for that file
2. if the scan returns clean and any durable policy changes are coherent, delete the corresponding pending report JSON
3. if a narrow constitutional judgement is still missing, keep the report pending and return `needs_charter_decision`

## Forbidden moves
Never do these:

- rename `mock` / `stub` / `fake`
- translate `.get(key, default)` into `if key in dict else default`
- add a new implicit default
- invent a new `policy-approved` id
- import test support into runtime code
- hide an owner-layer concept behind restricted visibility
- proliferate top-level restricted-visibility helpers without a clear public entry point
- add a boundary parser without a contract witness when the boundary is new or changed
- guess public/private, context, optionality, contract kind, or compatibility when underdetermined

## Fast examples

### Bad

```python
tool_use_id = event.tool_use.get("toolUseId", "tool")
```

### Good — eliminate optionality (when absence has no spec)

```python
__all__ = ["ToolUsePayload", "parse_tool_use"]

class ToolUsePayload(BaseModel):
    toolUseId: str


def parse_tool_use(raw: dict) -> ToolUsePayload:
    return ToolUsePayload.model_validate(raw)
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

### Good — public concept with export and contract witness

```python
__all__ = ["ToolUsePayload", "parse_tool_use"]

class ToolUsePayload(BaseModel):
    """Passport control at the API boundary — validates a raw tool-use dict."""
    toolUseId: str


def parse_tool_use(raw: dict) -> ToolUsePayload:
    return ToolUsePayload.model_validate(raw)
```

And compile the boundary promise into `policy/contracts.yml`.

### Bad

```ts
const repo = new FakeUserRepository()
```

### Good

- move the fake to tests, or
- promote it to `SandboxUserRepository`, register it in `policy/adapters.yml`, prove it with contract tests, and instantiate it only in the composition root
