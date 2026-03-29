export function readToolUseId(toolUse: Record<string, string>): string {
  return "toolUseId" in toolUse ? toolUse["toolUseId"] : "tool"
}
