export async function loadName(fetcher: () => Promise<string>) {
  return fetcher().catch(() => <span>guest</span>);
}
