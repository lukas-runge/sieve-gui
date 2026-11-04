// Searching the rule list: which items contain all words of a query.

import type { Action, Branch, RuleItem } from "#tauri/types";
import { mailboxLabel } from "./mailbox.ts";

/** Model keys that only hold technical values (match types, units, …). */
const TECHNICAL = new Set(["type", "matchType", "op", "unit", "part", "join", "comparator", "transform", "zone", "datePart"]);

/** Every text a user may search for in an item, lowercased. */
export function searchText(item: RuleItem, keywordNames: Record<string, string> = {}): string {
  if (item.type === "raw") return item.code.toLowerCase();
  const texts: string[] = [item.name];
  const walk = (value: unknown, key = "") => {
    if (typeof value === "string") {
      if (TECHNICAL.has(key)) return;
      texts.push(value);
      // Folder names as people read them (modified UTF-7 decoded).
      if (key === "mailbox") texts.push(mailboxLabel(value));
      // Labels by the names the account gave them.
      if (key === "flags" && keywordNames[value]) texts.push(keywordNames[value]);
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

export function matches(item: RuleItem, words: string[], keywordNames: Record<string, string> = {}): boolean {
  if (words.length === 0) return true;
  const text = searchText(item, keywordNames);
  return words.every((w) => text.includes(w));
}

/** A folder or label, to find everything that goes there. */
export interface Target {
  kind: "folder" | "label";
  /** The folder as stored (e.g. modified UTF-7) or the label's keyword. */
  key: string;
  /** Folders inside the folder count too. */
  subfolders?: boolean;
}

function inFolder(mailbox: string, target: Target): boolean {
  if (mailbox === target.key) return true;
  return !!target.subfolders && [".", "/"].some((d) => mailbox.startsWith(target.key + d));
}

/** Whether the actions (checks inside them included) file into the folder or set the label. */
export function actionsGoTo(actions: Action[], target: Target): boolean {
  return actions.some((a) => {
    if (a.type === "fileInto") return target.kind === "folder" && inFolder(a.mailbox, target);
    if (a.type === "flags") return target.kind === "label" && a.op !== "remove" && a.flags.some((f) => f.split(/\s+/).includes(target.key));
    if (a.type === "if") return blockGoesTo(a, target);
    return false;
  });
}

function blockGoesTo(b: { actions: Action[]; elsif: Branch[]; otherwise?: Action[] | null }, target: Target): boolean {
  return actionsGoTo(b.actions, target) || (b.elsif ?? []).some((e) => actionsGoTo(e.actions, target)) || (!!b.otherwise && actionsGoTo(b.otherwise, target));
}

/** Whether a rule files into the folder or sets the label in any of its cases. */
export function goesTo(item: RuleItem, target: Target): boolean {
  return item.type === "rule" && blockGoesTo(item, target);
}
