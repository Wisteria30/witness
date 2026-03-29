# Report format

Pending reports are JSON documents written under `${CLAUDE_PLUGIN_DATA}/reports/pending/`.

Each report is keyed by source file, and each history snapshot gets a unique `report_id`.

## Example

```json
{
  "schema_version": 1,
  "report_id": "cg-1764456123456-0001",
  "created_at_ms": 1764456123456,
  "file": "src/api/tool_use.py",
  "canonical_file": "/abs/path/src/api/tool_use.py",
  "capsule": "guardrail count=1 classes=fallback_unowned_default owners=boundary remedies=boundary_parser|typed_exception|approved_policy_api forbidden=rename|equivalent_rewrite|new_inline_default",
  "summary": {
    "files_scanned": 1,
    "violation_count": 1,
    "classes": {
      "fallback_unowned_default": 1
    },
    "owners": {
      "boundary": 1
    },
    "by_file": {
      "src/api/tool_use.py": 1
    }
  },
  "violations": [
    {
      "file": "src/api/tool_use.py",
      "canonical_file": "/abs/path/src/api/tool_use.py",
      "line": 7,
      "rule_id": "py-no-fallback-get-default",
      "policy_group": "fallback",
      "violation_class": "fallback_unowned_default",
      "owner_guess": "boundary",
      "owner_hint": "boundary",
      "message": "Inline default owns policy at the wrong layer.",
      "code": "tool_use_id = event.tool_use.get(\"toolUseId\", \"tool\")",
      "legal_remedies": [
        "approved_policy_api",
        "boundary_parser",
        "typed_exception",
        "optional_exhaustive_handling"
      ],
      "forbidden_moves": [
        "rename",
        "equivalent_rewrite",
        "new_inline_default"
      ],
      "approval_status": "missing"
    }
  ]
}
```

## Semantics

- `pending/` is the authoritative unresolved set
- `history/` is append-only audit history
- `capsule` is the short string that may be passed into `additionalContext`
- `approval_status` can be `missing`, `approved`, or `invalid`
