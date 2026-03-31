# witness v3 — constitutional proof kernel

## 1. Problem statement

Claude Code and other AI coding agents increasingly work in three broad phases:

1. plan
2. implement
3. evaluate

`witness` should not compete with broad planning systems. Claude Code already has high-quality planning primitives, including Plan Mode and agent teams. The role of `witness` is narrower and more precise:

> extract only the witness-relevant intent from an approved plan,
> combine it with durable repo law,
> and prove that the implementation respects both.

This makes `witness` a **constitutional proof kernel**, not another planner.

## 2. Core objects

### 2.1 Constitution

The durable repo constitution is:

```text
K₀ = (Ω, D, A, Σ, Χ, Γ)
```

- `Ω` — owner-layer assignment
- `D` — blessed eliminators / approved defaults
- `A` — lawful runtime adapter registry
- `Σ` — public/internal surface policy
- `Χ` — contracts (`shape`, `interaction`, `law`)
- `Γ` — bounded contexts and vocabulary

### 2.2 Charter

The change-local witness delta is:

```text
ΔK_w
```

This is a sparse projection of the approved broad plan. It stores only witness-relevant intent.

### 2.3 Effective evaluation environment

```text
K = K₀ ⊕ ΔK_w
```

`witness` evaluates code against `K`, never against the broad plan directly.

## 3. Axioms

### Axiom 0 — Constitutional duality
Evaluation is performed against the durable constitution plus a sparse per-change charter.

### Axiom 1 — Owned reduction
Any irreversible reduction of distinctions must have an owner.

Target reductions include:

- absence elimination: `1 + A -> A`
- failure elimination: `E + A -> A`
- substitution: `Mod(T) -> m`
- surface classification: `Symbols(M) -> {public, internal, subclass API}`

### Axiom 2 — Cheap witness
Every irreversible reduction must have a machine-checkable witness whose verification cost is much lower than rediscovering the intent from scratch.

### Axiom 3 — No guessing
If code plus constitution plus charter do not determine a judgement uniquely, `witness` must not guess. It must emit a **hole**.

### Axiom 4 — Surface explicitness
Every public concept must have an explicit surface witness.

### Axiom 5 — Contract explicitness
Every boundary crossing and inter-context interaction must have an explicit contract witness.

### Axiom 6 — Context uniqueness
Every public concept belongs to exactly one bounded context and should be explainable using that context’s vocabulary.

### Axiom 7 — Projection invariance
If two broad plans project to the same `ΔK_w`, then witness scan/repair outcomes must be identical.

### Axiom 8 — Compile then forget
Transient charter decisions may be discarded after they are compiled into durable policy files and code-level witnesses.

## 4. Findings model

`scan` produces four disjoint classes of findings:

- **violation** — clear breach of constitution or charter
- **hole** — underdetermined intent requiring a charter decision
- **drift** — constitution and code-level witnesses disagree
- **obligation** — charter-declared work not yet discharged

Acceptance condition:

```text
Accept(C) iff V(C, K)=∅ ∧ H(C, K)=∅ ∧ D(C, K)=∅ ∧ O(C, K)=∅
```

## 5. Why this does not duplicate broad planning

The broad plan may contain:

- task decomposition
- milestone sequencing
- migration strategy
- rollout strategy
- implementation order
- testing sequence
- team assignment

`witness` intentionally ignores all of that.

It only projects the following finite judgement set:

```text
Q_w = {
  owner,
  surface,
  context,
  default/optionality,
  adapter,
  contract/compatibility
}
```

These are the only plan facts scan/repair need to act lawfully.

## 6. Constitution files

`policy/ownership.yml`
: path → owner-layer assignment

`policy/defaults.yml`
: blessed eliminators and approved default IDs

`policy/adapters.yml`
: lawful runtime adapter registry

`policy/surfaces.yml`
: public/internal symbol policy and export witness modes

`policy/contracts.yml`
: contract registry (`shape`, `interaction`, `law`)

`policy/contexts.yml`
: bounded contexts, vocabulary, and allowed dependencies

## 7. Skills

### `/witness:charter`
Compiles `ΔK_w` from the approved broad plan or from an explicit `witness-delta` block.

### `/witness:scan`
Performs report-only constitutional scanning against `K₀ ⊕ ΔK_w`.

### `/witness:repair`
Uses 5 parallel worktree-isolated agents to repair all decidable findings and compile persistent constitutional changes.

### `/witness:shape`
Read-only structural diagnosis.
Extracts principal semantic roles, detects context blur, and proposes constitution deltas.

### `/witness:add-rule`
Extends cheap syntactic surface detection only.

## 8. Principal role extraction

Each public symbol should have a principal role:

```text
ρ(s) = (context, owner-layer, surface-class, verb, noun)
```

Human-readable rendering:

> “s is the authoritative place that verb noun for context.”

If this sentence cannot be written without conjunction, cross-context vocabulary, or multiple verbs, the symbol is overloaded.

`shape` is not the proof. It is the boundary finder that helps produce surface/context/contract witnesses.

## 9. Migration principle

Keep the current repository skeleton unchanged wherever possible:

- CI/CD workflow shape
- Rust versioning and package layout
- engine entrypoint (`src/main.rs`)
- hook script file locations
- skill directory layout
- plugin packaging

v3 is a constitutional extension, not a structural rewrite of the repo.
