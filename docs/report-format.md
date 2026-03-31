# Report format

Pending reports are JSON documents written under `${CLAUDE_PLUGIN_DATA}/reports/pending/`.

Each report is keyed by the canonical file path that currently owns the unresolved findings. That can be a source file or an active charter file.

## Example

```json
{
  "version": 3,
  "report_id": "wg-9c1dbf10f7e0a0b2-0001",
  "created_at": "2026-03-31T12:34:56Z",
  "status": "pending",
  "charter_ref": "CHG-1",
  "file": "src/api/tool_use.py",
  "canonical_file": "/abs/path/src/api/tool_use.py",
  "summary": {
    "files_scanned": 1,
    "violations": 1,
    "holes": 0,
    "drift": 0,
    "obligations": 0,
    "by_kind": {
      "violation": 1
    },
    "by_file": {
      "src/api/tool_use.py": 1
    }
  },
  "findings": [
    {
      "kind": "violation",
      "file": "src/api/tool_use.py",
      "canonical_file": "/abs/path/src/api/tool_use.py",
      "line": 7,
      "rule_id": "py-no-fallback-get-default",
      "violation_class": "fallback_unowned_default",
      "owner_layer": "boundary",
      "snippet": "tool_use_id = event.tool_use.get(\"toolUseId\", \"tool\")",
      "message": "Dictionary get default owns policy at the wrong layer (missing registry-backed approval comment)",
      "required_judgements": [
        "owner",
        "default_or_optionality"
      ],
      "remedy_candidates": [
        "approved_policy_api",
        "boundary_parser",
        "typed_exception",
        "optional_exhaustive_handling"
      ],
      "proof_options": [
        "registered approval id",
        "parser/schema validation",
        "typed exception test"
      ]
    }
  ]
}
```

## Semantics

- `pending/` is the authoritative unresolved set.
- `history/` is append-only audit history.
- `status` is `pending` for machine-decidable work and `needs_charter_decision` when a narrow constitutional judgement remains unresolved.
- `charter_ref` is populated when active charter files were consumed during the scan.
- `findings.kind` is one of `violation`, `hole`, `drift`, or `obligation`.
- `canonical_file` may point at a charter file when the unresolved work belongs to the charter rather than source code.
