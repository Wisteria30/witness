export async function loadName(fetcher: () => Promise<string>): Promise<string | null> {
  return fetcher().catch(() => null)
}
