import json
from pathlib import Path


SCHEMA_PATH = (
    Path(__file__).resolve().parents[3] / "schemas" / "events" / "order_placed.v1.json"
)


def test_order_placed_schema_requires_non_empty_line_items() -> None:
    schema = json.loads(SCHEMA_PATH.read_text())

    line_items = schema["properties"]["line_items"]
    assert schema["type"] == "object"
    assert "line_items" in schema["required"]
    assert line_items["type"] == "array"
    assert line_items["minItems"] == 1
