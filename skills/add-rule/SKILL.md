---
name: add-rule
description: "Add or fix an ast-grep detection rule. Use when adding a rule, fixing a false positive/negative, or improving detection coverage."
disable-model-invocation: true
allowed-tools: Bash, Read, Grep, Glob, Edit, Write
argument-hint: [rule-name-or-pattern]
---

# Add Rule

From pattern discovery to tested rule in 7 steps.

## Step 1: Understand what was found

Identify which situation applies:

### A. New rule — "I found a pattern that should be caught"

Collect:
1. **The actual code** (the NG example)
2. **Language** — Python / TypeScript / both
3. **Policy group** — `test-double`, `fallback`, `surface`, or `contract`
4. **Violation class** — see [references/rule-metadata.md](references/rule-metadata.md) for the full table
5. **Owner hint** — which layer typically owns this
6. **Approval mode** — `registry_policy_comment` or `none`
7. **OK examples** — similar syntax that should NOT be flagged

Read [references/rule-metadata.md](references/rule-metadata.md) for the complete violation class, owner hint, and approval mode reference.

### B. False negative — "This code should be caught but isn't"

1. The code that slipped through
2. Which rule was expected to catch it
3. Why it should be caught

### C. False positive — "This code is flagged but shouldn't be"

1. The code wrongly flagged
2. Which rule flagged it
3. Why it's actually OK

### False positive prevention

Always check:
- "Match only in assignments? Or also in conditions / return statements?"
- "Are there library/framework APIs that use the same syntax innocently?"
- "Exclude generated code and test files as usual?"

## Step 2: Write the rule YAML

Create `rules/{language_dir}/{lang}-no-{policy_group}-{pattern-name}.yml`.

Language directories: `go`, `python`, `rust`, `typescript`.

Every rule MUST have metadata — see [references/rule-metadata.md](references/rule-metadata.md) for the template and allowed values.

Rules should stay focused on cheap syntactic surfaces. They are not the place for deep semantics or project-scale reasoning. Use `policy/surfaces.yml`, `policy/contracts.yml`, and `policy/contexts.yml` for semantic policy.

## Step 3: Write fixtures

```
fixtures/{language}/{policy_group}/should_fail/   — must trigger
fixtures/{language}/{policy_group}/should_pass/   — must NOT trigger
fixtures/{language}/{policy_group}/approved/       — must be suppressed by policy-approved
```

## Step 4: Update Rust candidate selection (if needed)

Check `detect_rule_ids()` in `src/main.rs`. Add a keyword check if existing keywords don't cover the new rule:

```rust
if lower.contains("your_keyword") {
    ids.insert("your-rule-id".to_string());
}
```

## Step 5: Run tests

```bash
cargo test --all-targets
```

Every `should_fail` fixture must be detected. Every `should_pass` must have zero findings.

Debug a specific file:
```bash
ast-grep scan --json=stream --inline-rules "$(cat rules/{language_dir}/your-rule.yml)" path/to/file.py
```

## Step 6: Verify with a real scan

```bash
./bin/witness-engine scan-tree --root /path/to/project --config-dir .
```

Check for unexpected false positives in real code.

## Step 7: Commit

```bash
git add rules/ fixtures/ src/main.rs
git commit -m "feat: add {rule-id} — detect {what it catches}"
```
