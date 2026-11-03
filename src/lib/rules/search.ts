// Searching the rule list: which items contain all words of a query.

import type { RuleItem } from "#tauri/types";
import { mailboxLabel } from "./mailbox.ts";

/** Model keys that only hold technical values (match types, units, …). */
const TECHNICAL = new Set(["type", "matchType", "op", "unit", "part", "join", "comparator", "transform", "zone", "datePart"]);

/** Every text a user may search for in an item, lowercased. */
export function searchText(item: RuleItem): string {
  if (item.type === "raw") return item.code.toLowerCase();
  const texts: string[] = [item.name];
  const walk = (value: unknown, key = "") => {
    if (typeof value === "string") {
      if (TECHNICAL.has(key)) return;
      texts.push(value);
      // Folder names as people read them (modified UTF-7 decoded).
      if (key === "mailbox") texts.push(mailboxLabel(value));
    } else if (typeof value === "number") {
      texts.push(String(value));
    } else if (Array.isArray(value)) {
      for (const v of value) walk(v, key);
    } else if (value && typeof value === "object") {
      for (const [k, v] of Object.entries(value)) walk(v, k);
    }
  };
  // Branches and nested ifs ("also check") are searched too.
  walk(item.conditions);
  walk(item.actions);
  walk(item.elsif);
  walk(item.otherwise);
  return texts.join("\n").toLowerCase();
}

/** The words of a query; an item matches if it contains all of them. */
export function queryWords(query: string): string[] {
  return query.toLowerCase().split(/\s+/).filter((w) => w !== "");
}

export function matches(item: RuleItem, words: string[]): boolean {
  if (words.length === 0) return true;
  const text = searchText(item);
  return words.every((w) => text.includes(w));
}
