class _ToolUsePayload {
  toolUseId!: string;
}

export function renderToolUse(raw: _ToolUsePayload) {
  return <div>{raw.toolUseId}</div>;
}
