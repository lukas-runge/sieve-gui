// Whether the route sheet shows account-wide results (search across
// scripts, destinations) instead of the open script, so the page header can
// say so.

export const accountView = $state({ active: false, kind: "" as "" | "search" | "dest" });
