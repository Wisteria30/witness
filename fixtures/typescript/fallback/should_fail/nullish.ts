export function readPort(config: { port?: number }): number {
  return config.port ?? 3000
}
