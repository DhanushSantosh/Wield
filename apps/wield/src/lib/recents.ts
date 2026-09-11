const KEY = "wield.recents";
const MAX = 8;

export function getRecentIds(): string[] {
  try {
    const stored = window.localStorage.getItem(KEY);
    if (stored === null) {
      return [];
    }
    const parsed: unknown = JSON.parse(stored);
    return Array.isArray(parsed) && parsed.every((id) => typeof id === "string")
      ? parsed
      : [];
  } catch {
    return [];
  }
}

export function bumpRecent(id: string): void {
  try {
    const recents = [id, ...getRecentIds().filter((recent) => recent !== id)].slice(0, MAX);
    window.localStorage.setItem(KEY, JSON.stringify(recents));
  } catch {
    // Recency is an optional enhancement; storage failures must not block a tool run.
  }
}
