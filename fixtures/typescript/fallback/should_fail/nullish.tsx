export function ToolUsePanel(props: { toolUseId?: string }) {
  const toolUseId = props.toolUseId ?? "default";
  return <div>{toolUseId}</div>;
}
