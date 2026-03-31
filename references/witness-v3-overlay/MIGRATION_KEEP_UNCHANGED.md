# Keep these parts unchanged from the current witness repository

This v3 overlay intentionally reuses the current witness repository skeleton.
Unless you have a compelling reason, keep these files and conventions **verbatim** from the existing repo and layer the v3 changes on top.

## Keep as-is

- `.github/workflows/ci.yml`
- `Cargo.toml`
- `Cargo.lock`
- `src/main.rs`
- `.claude-plugin/*`
- `setup`
- `scripts/release.sh`
- `hooks/session-start.sh`
- `hooks/post-edit-classify.sh`
- `hooks/post-edit-audit.sh`
- `hooks/stop-gate.sh`
- `rules/*`
- `fixtures/*`
- `tests/*` (extend, don’t rewrite)

## Why

v3 is a constitutional extension, not a repo-layout rewrite.
The following should remain stable:

- Rust package name and binary identity (`witness-engine`)
- engine entrypoint in `src/main.rs`
- CI structure and shell validation jobs
- hook script placement and plugin packaging
- skill directory placement under `skills/*/SKILL.md`

## What v3 actually adds

- new policy files: `policy/contracts.yml`, `policy/contexts.yml`
- stronger `policy/surfaces.yml`
- new skills: `skills/charter/SKILL.md`, `skills/shape/SKILL.md`
- charter-aware updates to `skills/scan/SKILL.md`, `skills/repair/SKILL.md`
- updated doctrine and repair agent
- updated report schema and design docs

## Migration order

1. Replace `README.md` and `CLAUDE.md`.
2. Add `policy/contracts.yml` and `policy/contexts.yml`.
3. Replace `policy/surfaces.yml`.
4. Add `skills/charter/SKILL.md` and `skills/shape/SKILL.md`.
5. Replace `skills/scan/SKILL.md`, `skills/repair/SKILL.md`, `skills/repair/doctrine.md`, `agents/guardrail-repairer.md`.
6. Replace `hooks/hooks.json`.
7. Add `docs/report-schema-v3.json` and `docs/charter-schema-v1.json`.
8. Extend engine/tests/metadata validation to understand charter/contracts/contexts when you implement the code changes.
