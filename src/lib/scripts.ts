// The account's scripts as the sidebar lists them.

/** A script in the sidebar; `depth` > 0 if it is listed under `via`, which includes it. */
export interface ScriptNode {
  name: string;
  depth: number;
  via: string | null;
}

/**
 * First the running scripts (`top`, in the order they run), then the others
 * alphabetically. Scripts included by another one follow it, indented,
 * unless they run on their own. Scripts left over (e.g. including each
 * other in a cycle) come last.
 */
export function scriptTree(
  names: string[],
  top: string[],
  includes: Record<string, string[]>,
): { running: ScriptNode[]; inactive: ScriptNode[] } {
  const exists = new Set(names);
  const running_ = top.filter((n) => exists.has(n));
  const placed = new Set<string>();
  const place = (name: string, depth: number, via: string | null, out: ScriptNode[]) => {
    if (placed.has(name)) return;
    placed.add(name);
    out.push({ name, depth, via });
    for (const child of includes[name] ?? []) {
      if (exists.has(child) && !running_.includes(child)) place(child, depth + 1, name, out);
    }
  };
  const running: ScriptNode[] = [];
  for (const name of running_) place(name, 0, null, running);
  const rest = names.filter((n) => !placed.has(n)).sort((a, b) => a.localeCompare(b));
  const includedInRest = new Set(rest.flatMap((n) => includes[n] ?? []));
  const inactive: ScriptNode[] = [];
  for (const name of rest.filter((n) => !includedInRest.has(n))) place(name, 0, null, inactive);
  for (const name of rest) place(name, 0, null, inactive);
  return { running, inactive };
}
