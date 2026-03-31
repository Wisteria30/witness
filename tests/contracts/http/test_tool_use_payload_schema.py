import json
from pathlib import Path


SCHEMA_PATH = (
    Path(__file__).resolve().parents[3] / "schemas" / "http" / "tool_use_payload.v1.json"
)


def test_tool_use_payload_schema_declares_required_boundary_fields() -> None:
    schema = json.loads(SCHEMA_PATH.read_text())

    assert schema["type"] == "object"
    assert schema["required"] == ["tool_name", "arguments"]
    assert schema["additionalProperties"] is False
