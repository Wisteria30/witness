---
name: shape
description: Read-only structural diagnosis. Extract principal semantic roles, detect context blur, and propose constitution deltas. Never edits code.
context: fork
disable-model-invocation: true
allowed-tools: Read, Grep, Glob, Bash
argument-hint: [path-or-module]
---

Perform a **read-only structural diagnosis** for `$ARGUMENTS` if provided, otherwise the current project.

This skill is not a refactoring tool.
It does not change code, policy files, or reports.
Its job is to find structural problems that weaken witness guarantees:

- overloaded public symbols
- public concepts with no explicit export witness
- context blur (mixed vocabulary from multiple bounded contexts)
- boundary work with no contract witness
- hidden owner-layer concepts behind restricted visibility

## Diagnostic model

For each public symbol, attempt to extract a principal role:

```text
ρ(symbol) = (context, owner-layer, surface-class, verb, noun)
```

Render it as one sentence:

> "<symbol> is the authoritative place that <verb> <noun> for <context>."

If this sentence cannot be written without conjunction, multiple verbs, or cross-context vocabulary, flag the symbol as overloaded.

## Workflow

### 1. Load constitution
Read:

- `${CLAUDE_PLUGIN_ROOT}/policy/ownership.yml`
- `${CLAUDE_PLUGIN_ROOT}/policy/surfaces.yml`
- `${CLAUDE_PLUGIN_ROOT}/policy/contracts.yml` if it exists
- `${CLAUDE_PLUGIN_ROOT}/policy/contexts.yml` if it exists

### 2. Discover candidate public symbols
Use export manifests first:

- Python `__all__`
- `__init__.py` re-exports
- TypeScript named exports / barrel exports
- Rust `pub` / `pub use`

If explicit manifests are missing, fall back to top-level concept nouns and note that surface witness is missing.

### 3. Diagnose each candidate
For each candidate public symbol:

- infer owner layer from `ownership.yml`
- infer likely context from `contexts.yml`
- infer surface class (`public_concept`, `subclass_api`, `internal_mechanic`)
- attempt principal role extraction
- note missing export witness or missing contract witness

### 4. Detect structural anti-patterns
Flag at least these:

- one symbol appears to belong to multiple contexts
- one symbol has multiple principal verbs
- one module exports many unrelated concept families
- a boundary parser or DTO exists with no contract witness
- an owner-layer concept is hidden behind restricted visibility
- many top-level restricted-visibility helpers exist with no single public entry point

### 5. Produce a constitutional diagnosis
Organize the output as:

- `surface debt`
- `contract debt`
- `context debt`
- `principal-role conflicts`
- `recommended charter delta` (if constitutional changes are needed)

If the result implies a new charter, recommend `/witness:charter` and provide a minimal draft `witness-delta` block.

### 6. Stop
Never modify code. Never run repair. Never update policy files.
