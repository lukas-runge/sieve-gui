// Names for custom keywords (IMAP flags like `$label1`), per account.

/** Whether a flag is a custom keyword rather than a system flag (`\Seen`). */
export function isKeyword(flag: string): boolean {
  return flag !== "" && !flag.startsWith("\\");
}

/** A flag as shown: its name if it has one. */
export function flagLabel(flag: string, names: Record<string, string>): string {
  return names[flag] ?? flag;
}

/** Names as editable text: one `keyword = Name` per line, sorted by name. */
export function formatKeywordNames(names: Record<string, string>): string {
  return Object.entries(names)
    .sort(([, a], [, b]) => a.localeCompare(b))
    .map(([keyword, name]) => `${keyword} = ${name}`)
    .join("\n");
}

/** Reads `keyword = Name` lines; other lines are ignored. */
export function parseKeywordNames(text: string): Record<string, string> {
  const names: Record<string, string> = {};
  for (const line of text.split("\n")) {
    const at = line.indexOf("=");
    if (at < 0) continue;
    const keyword = line.slice(0, at).trim();
    const name = line.slice(at + 1).trim();
    if (keyword && name) names[keyword] = name;
  }
  return names;
}
