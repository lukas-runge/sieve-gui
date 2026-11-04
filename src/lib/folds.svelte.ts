// Rules and checks folded to a one-line summary on the route, kept while
// switching scripts and views (not across restarts).

import { SvelteSet } from "svelte/reactivity";

export const folds = new SvelteSet<string>();

export function toggleFold(key: string) {
  if (folds.has(key)) folds.delete(key);
  else folds.add(key);
}
