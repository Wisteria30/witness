# Policy files

The verifier intentionally knows very little by itself. It relies on three policy files.

## `policy/ownership.yml`

Maps file globs to owner layers.

Use this file to teach the verifier where your:

- boundaries live
- domain model lives
- application orchestration lives
- infrastructure lives
- composition root lives
- tests live

## `policy/defaults.yml`

Registers approved default identifiers.

Each entry documents:

- the approval id (`REQ-*`, `ADR-*`, `SPEC-*`, or another project convention)
- the blessed policy symbol
- the layers where that symbol may be used
- the reason the default exists

Adjacent `policy-approved:` comments only suppress findings when the identifier is registered here and the file belongs to an allowed layer.

## `policy/adapters.yml`

Lists lawful runtime adapters per port and their contract suites.

Use it for two things:

- tell the verifier which concrete adapter symbols are legitimate runtime implementations
- document the tests that prove those adapters are lawful substitutes

This file is deliberately conservative. Anything not listed here should be treated with suspicion.
