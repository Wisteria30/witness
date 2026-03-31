export function readToolUseId(toolUse: Record<string, string>) {
  return "toolUseId" in toolUse ? toolUse["toolUseId"] : <span>tool</span>;
}
