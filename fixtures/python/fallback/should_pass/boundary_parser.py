from pydantic import BaseModel, ConfigDict


class ToolUsePayload(BaseModel):
    model_config = ConfigDict(extra="forbid")
    toolUseId: str


def parse_tool_use(raw: dict) -> ToolUsePayload:
    return ToolUsePayload.model_validate(raw)
