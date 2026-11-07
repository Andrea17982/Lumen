/** One action reachable from the command palette; menus and shortcuts can share the same run. */
export interface Command {
  id: string;
  label: string;
  group: string;
  keywords?: string;
  /** Display-only hint; the actual key handler lives where the shortcut is bound. */
  shortcut?: string;
  /** Unavailable commands are hidden from the palette; menus that list them show them disabled. */
  available?: boolean;
  run: () => void;
}

/** Runs a registered command by id, the one path shared by menus and shortcuts. */
export function runCommand(commands: Command[], id: string): boolean {
  const command = commands.find((c) => c.id === id);
  if (!command || command.available === false) return false;
  command.run();
  return true;
}

/** Subsequence score: lower is better, null when the query does not match. */
function score(text: string, query: string): number | null {
  let at = 0;
  let gaps = 0;
  let last = -1;
  for (const ch of query) {
    const next = text.indexOf(ch, at);
    if (next < 0) return null;
    if (last >= 0) gaps += next - last - 1;
    last = next;
    at = next + 1;
  }
  const start = text.indexOf(query);
  return start >= 0 ? start : 100 + gaps;
}

export function filterCommands(commands: Command[], query: string): Command[] {
  const visible = commands.filter((c) => c.available !== false);
  const q = query.trim().toLowerCase();
  if (!q) return visible;
  return visible
    .map((c) => {
      const label = score(c.label.toLowerCase(), q);
      const other = score(`${c.group} ${c.keywords ?? ''}`.toLowerCase(), q);
      const best = label ?? (other === null ? null : 1000 + other);
      return { c, best };
    })
    .filter((x): x is { c: Command; best: number } => x.best !== null)
    .sort((a, b) => a.best - b.best)
    .map((x) => x.c);
}
