# Rule Metadata Reference

## Violation classes

| Class | Description | Typical policy group |
|-------|-------------|---------------------|
| `fallback_unowned_default` | Implicit default without owner | `fallback` |
| `fallback_unowned_handler` | Swallowed error or silent catch | `fallback` |
| `runtime_double_in_graph` | Test double in non-test code | `test-double` |
| `adapter_choice_outside_composition_root` | Adapter wiring in wrong layer | `test-double` |
| `hidden_owner_concept` | Owner-layer symbol behind restricted visibility | `surface` |
| `missing_surface_witness` | Public concept without export manifest | `surface` |
| `missing_contract_witness` | Boundary crossing without contract | `contract` |

## Policy groups

| Group | What it catches |
|-------|----------------|
| `fallback` | Implicit defaults, silent error handlers, unowned eliminators |
| `test-double` | Runtime test doubles, adapter wiring outside composition root |
| `surface` | Hidden owner concepts, missing export manifests |
| `contract` | Missing boundary/inter-context contract witnesses |

## Owner hints

| Layer | Typical code |
|-------|-------------|
| `boundary` | API, HTTP, events inbound, settings parsing |
| `domain` | Invariants, always-valid objects |
| `application` | Use cases, orchestration |
| `infrastructure` | Resilience, caching, secondary systems |
| `composition_root` | Adapter selection, main/bootstrap |
| `tests` | Test code |

## Approval modes

| Mode | When to use |
|------|------------|
| `registry_policy_comment` | Approvable via `policy-approved: REQ-xxx` (typical for fallback rules) |
| `none` | Never approvable (test-double, surface, and contract rules) |

## Rule YAML metadata template

```yaml
metadata:
  policy_group: fallback|test-double|surface|contract
  violation_class: <see table above>
  owner_hint: boundary|domain|application|infrastructure|composition_root|tests
  approval_mode: registry_policy_comment|none
```

## Fixture directory structure

```
fixtures/{language}/{policy_group}/should_fail/   -- must trigger
fixtures/{language}/{policy_group}/should_pass/   -- must NOT trigger
fixtures/{language}/{policy_group}/approved/       -- suppressed by policy-approved
```

## Language directories

Rule files go in `rules/{language_dir}/`:
- `go/`
- `python/`
- `rust/`
- `typescript/`

Naming convention: `{lang}-no-{policy_group}-{pattern-name}.yml`
