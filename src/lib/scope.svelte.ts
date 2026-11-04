// Whether the route sheet shows account-wide results (search across
// scripts, destinations) instead of the open script, so the page header can
// say so.

import type { Target } from "./rules/search";

export const accountView = $state({ active: false, kind: "" as "" | "search" | "dest" | "flow" });

/**
 * The account whose running scripts are shown together ("Alle laufenden
 * Skripte" in the sidebar), on the route and in destinations; null: the open
 * script alone. The editor still holds one script, the one a picked rule is in.
 */
export const flowView = $state({ account: null as string | null });

/** A folder or label whose rules are listed. */
export type ShownTarget = Target & { text: string; folder: boolean };

/**
 * What the route sheet shows, kept outside it: in narrow windows the
 * inspector takes its place, and a result in another script opens that
 * script. Either way the list stays where it was.
 */
export const routeNav = $state({
  accountId: "",
  view: "route" as "route" | "dest",
  target: null as ShownTarget | null,
  query: "",
  scope: "running" as "running" | "all" | "script",
  withInactive: false,
});
