# Changelog

## 1.0.0

- introduced verifier/doctrine/repairer split
- added registry-backed approval model via `policy/defaults.yml`
- added owner-layer mapping via `policy/ownership.yml`
- added lawful adapter registry via `policy/adapters.yml`
- added persisted pending reports and append-only report history
- added `Stop` and `SubagentStop` gates for unresolved guardrail work
- added repair subagent and forked skills for scan + repair
- added rules for equivalent fallback rewrites and runtime test-support imports
- added integration fixtures and CLI tests
