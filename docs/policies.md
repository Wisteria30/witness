# Policy files

The verifier intentionally knows very little by itself. In v3 it relies on a repo constitution made of six policy files.

## `policy/ownership.yml`

Maps file globs to owner layers.

Use it to tell witness where your:

- boundaries live
- domain model lives
- application orchestration lives
- infrastructure lives
- composition root lives
- tests live

## `policy/defaults.yml`

Registers blessed default identifiers.

Each entry documents:

- the approval id (`REQ-*`, `ADR-*`, `SPEC-*`, or another project convention)
- the blessed policy symbol
- the owner layers where that symbol may appear
- the reason the default exists

Adjacent `policy-approved:` comments only suppress a finding when the identifier is registered here, the file belongs to an allowed layer, and the blessed symbol matches the call site.

## `policy/adapters.yml`

Lists lawful runtime adapters per port.

Use it to tell the verifier which concrete adapter symbols are legitimate runtime implementations. Anything not listed there should be treated as suspect runtime substitution.

## `policy/surfaces.yml`

Defines public/internal symbol policy and export witnesses.

Use it to declare:

- public-by-default concept families such as `*Payload`, `*Policy`, and `*Adapter`
- extension API families such as `*Base`
- whether public concepts may hide behind restricted visibility
- whether new public symbols must have an explicit export manifest

## `policy/contracts.yml`

Declares boundary and inter-context contracts.

Each contract may define:

- `kind` (`shape`, `interaction`, `law`)
- the owning bounded context
- compatibility mode
- schema path
- witness files such as contract tests

## `policy/contexts.yml`

Declares bounded contexts, vocabulary, and permitted dependencies.

Use it to give public concepts a unique semantic home through:

- owned paths
- context vocabulary
- allowed dependencies
- public entrypoints when they matter
