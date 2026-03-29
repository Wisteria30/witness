def read_tool_use_id(tool_use: dict) -> str:
    return tool_use["toolUseId"] if "toolUseId" in tool_use else "tool"
