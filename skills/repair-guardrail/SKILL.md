---
name: repair-guardrail
description: Repair unresolved guardrail reports at the owner layer with one lawful remedy and one witness.
context: fork
disable-model-invocation: true
allowed-tools: Bash, Read, Grep, Glob, Edit, Write
argument-hint: [report-path-or-file]
---

Repair the unresolved guardrail target in `$ARGUMENTS`.

Before editing anything:

1. Read [`doctrine.md`](doctrine.md).
2. Read the pending report JSON if a report path was given.
3. Identify the owner layer and the violation class.
4. Choose exactly one legal remedy.
5. Add one witness.
6. Never preserve the violating line.

When you are done:

- re-run the relevant guardrail scan
- mention which witness was added
- explain why rename-only and equivalent-rewrite escapes were not used
