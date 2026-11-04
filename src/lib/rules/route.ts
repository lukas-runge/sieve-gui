// The route view of a script: what each rule tests and where a message goes,
// in short tokens instead of sentences. "Sender contains news@x" becomes the
// field chip "From" and the value "news@x"; how the value is compared shows in
// its notation ("= x" is, "…x" ends with, "x…" starts with, "/x/" regex).
// The full sentence stays available as a tooltip.

import type { Action, Condition, Conditions } from "#tauri/types";
import { flagLabel } from "../keywords";
import { i18n, t, type MessageKey } from "../i18n.svelte";
import { formatDate, formatSize } from "./format";
import { mailboxLabel } from "./mailbox";
import { actionKind, hasValues, headerNames, viewCondition, type Branches, type ConditionView } from "./view";

// ── Conditions ────────────────────────────────────────────────────────────────

/** One value as shown: the literal text plus the marks of its comparison. */
export interface ValueToken {
  before: string;
  text: string;
  after: string;
}

export interface ConditionToken {
  /** The field chip: "From", "Subject", "Recipients", a header name, … */
  field: string;
  /** Full header list or field name, when `field` abbreviates it. */
  fieldTitle: string | null;
  /** "not" etc.: the comparison is negated. */
  negated: boolean;
  /** Comparisons without a value ("exists") or with words ("larger than"). */
  word: string | null;
  values: ValueToken[];
  /** Values beyond those shown. */
  more: string[];
  /** The whole condition as a sentence, for tooltips and screen readers. */
  sentence: string;
}

const RECIPIENTS = ["to", "cc", "bcc"];
const DELIVERY = ["x-original-to", "delivered-to", "envelope-to"];
const MAX_VALUES = 3;

/** Short name for a list of headers: to/cc/bcc are "recipients". */
export function headerChip(names: string[]): { label: string; title: string | null } {
  const lower = names.map((n) => n.toLowerCase());
  const parts: string[] = [];
  const recipients = lower.filter((n) => RECIPIENTS.includes(n));
  const groupRecipients = recipients.length >= 2;
  const delivery = lower.filter((n) => DELIVERY.includes(n));
  const groupDelivery = delivery.length >= 2;
  if (groupRecipients) parts.push(t("route.recipients"));
  if (groupDelivery) parts.push(t("route.delivery"));
  for (const [i, n] of lower.entries()) {
    if (groupRecipients && RECIPIENTS.includes(n)) continue;
    if (groupDelivery && DELIVERY.includes(n)) continue;
    parts.push(names[i]);
  }
  const label = parts.join(" / ");
  return { label, title: label === names.join(", ") ? null : names.join(", ") };
}

function fieldChip(v: ConditionView): { label: string; title: string | null } {
  let chip: { label: string; title: string | null };
  if (v.field === "header") chip = headerChip(headerNames(v.header));
  else chip = { label: t(`route.field.${v.field}` as MessageKey), title: null };
  return chip;
}

const NEGATED: Record<string, string> = {
  notContains: "contains",
  notIs: "is",
  notIsWithSubdomains: "isWithSubdomains",
  notStartsWith: "startsWith",
  notEndsWith: "endsWith",
  notMatches: "matches",
  notRegex: "regex",
  notExists: "exists",
  notHasType: "hasType",
};

function valueToken(op: string, part: string, value: string): ValueToken {
  let before = "";
  let after = "";
  switch (op) {
    case "is":
      before = "= ";
      break;
    case "isWithSubdomains":
      before = "= ";
      after = " " + t("route.withSubdomains");
      break;
    case "startsWith":
      after = "…";
      break;
    case "endsWith":
      before = "…";
      break;
    case "regex":
      before = "/";
      after = "/";
      break;
  }
  // The address part shows as part of the address: @domain, name@, +detail.
  // A domain that "is" something needs no "=": @example.org says it.
  if (part === "domain") before = (op === "is" || op === "isWithSubdomains" ? "" : before) + "@";
  else if (part === "localPart" || part === "user") after = after + "@";
  else if (part === "detail") before += "+";
  return { before, text: value || "…", after };
}

function sentence(v: ConditionView, chip: string): string {
  const values = v.values.map((x) => `„${x}“`).join(` ${t("sentence.or")} `);
  const part = v.part !== "header" && v.part !== "all" ? ` (${t(`part.${v.part}` as MessageKey)})` : "";
  return `${chip}${part} ${t(`op.${v.operator}` as MessageKey)}${hasValues(v.operator) ? " " + values : ""}`;
}

export function conditionToken(c: Condition): ConditionToken {
  const v = viewCondition(c);
  if (!v) {
    return { field: t("field.other"), fieldTitle: null, negated: false, word: null, values: [], more: [], sentence: t("field.other") };
  }
  const chip = fieldChip(v);
  const negated = v.operator in NEGATED;
  const op = NEGATED[v.operator] ?? v.operator;
  let word: string | null = null;
  let values: ValueToken[] = [];
  if (op === "exists") word = t("route.exists");
  else if (v.field === "size") {
    word = op === "over" ? ">" : "<";
    values = v.values.map((x) => ({ before: "", text: formatSize(Number(x)), after: "" }));
  } else if (v.field === "date") {
    word = t(op === "onOrAfter" ? "route.from" : "route.until");
    values = v.values.map((x) => ({ before: "", text: formatDate(x, i18n.locale), after: "" }));
  } else if (op === "hasType") {
    values = v.values.map((x) => ({ before: "", text: x, after: "" }));
  } else {
    values = v.values.map((x) => valueToken(op, v.part, x));
  }
  return {
    field: chip.label,
    fieldTitle: chip.title,
    negated,
    word,
    values: values.slice(0, MAX_VALUES),
    more: v.values.slice(MAX_VALUES),
    sentence: sentence(v, chip.title ?? chip.label),
  };
}

/** How a case's conditions are shown. */
export type ConditionShape =
  | { kind: "always" }
  /** One token; several conditions on the same field with "or" merge. */
  | { kind: "line"; token: ConditionToken }
  /** A list of conditions, one per line, under "any of" / "all of" … */
  | { kind: "list"; mode: "any" | "all" | "none" | "notAll"; items: ConditionItem[] };

export type ConditionItem = { kind: "token"; token: ConditionToken } | { kind: "group"; shape: ConditionShape };

function sameComparison(items: Condition[]): boolean {
  const views = items.map(viewCondition);
  if (views.some((v) => !v || !hasValues(v.operator))) return false;
  const key = (v: ConditionView) => `${v.field}|${v.header.toLowerCase()}|${v.part}|${v.operator}`;
  return new Set(views.map((v) => key(v!))).size === 1;
}

export function conditionShape(conditions: Conditions, negate = false): ConditionShape {
  const { items, join } = conditions;
  if (items.length === 0) return { kind: "always" };
  const flat = items.every((c) => c.test.type !== "group");
  if (!negate && items.length === 1 && flat) return { kind: "line", token: conditionToken(items[0]) };
  // "From contains a" or "From contains b": one token with both values.
  if (!negate && flat && join === "any" && sameComparison(items)) {
    const tokens = items.map(conditionToken);
    const all = tokens.flatMap((tk) => [...tk.values]);
    const first = tokens[0];
    const raw = items.flatMap((c) => viewCondition(c)!.values);
    return {
      kind: "line",
      token: {
        ...first,
        values: all.slice(0, MAX_VALUES),
        more: raw.slice(MAX_VALUES),
        sentence: tokens.map((tk) => tk.sentence).join(` ${t("sentence.or")} `),
      },
    };
  }
  const mode = join === "any" ? (negate ? "none" : "any") : negate ? "notAll" : "all";
  return {
    kind: "list",
    mode,
    items: items.map((c) =>
      c.test.type === "group"
        ? { kind: "group", shape: conditionShape({ join: c.test.join, items: c.test.items }, c.negate) }
        : { kind: "token", token: conditionToken(c) },
    ),
  };
}

// ── Outcomes ──────────────────────────────────────────────────────────────────

export type OutcomeKind =
  | "move"
  | "copy"
  | "label"
  | "unlabel"
  | "setLabels"
  | "read"
  | "unread"
  | "flag"
  | "system"
  | "forward"
  | "reply"
  | "reject"
  | "delete"
  | "keep";

export interface Outcome {
  kind: OutcomeKind;
  /** Folder path segments for moves and copies; the text otherwise. */
  path: string[];
  text: string;
}

const SYSTEM_FLAGS: Record<string, MessageKey> = {
  "\\seen": "route.flag.seen",
  "\\flagged": "route.flag.flagged",
  "\\answered": "route.flag.answered",
  "\\deleted": "route.flag.deleted",
  "\\draft": "route.flag.draft",
};

/** A flag by its name: system flags in words, keywords by the account's names. */
export function flagName(flag: string, keywords: Record<string, string>): string {
  const key = SYSTEM_FLAGS[flag.toLowerCase()];
  return key ? t(key) : flagLabel(flag, keywords);
}

/** Folder path segments, split at the server's hierarchy delimiter. */
export function folderPath(raw: string, delimiter: string | null): string[] {
  const label = mailboxLabel(raw);
  return delimiter ? label.split(delimiter).filter((s) => s !== "") : [label];
}

/** What the actions do, in order; nested checks are left to the caller. */
export function outcomes(actions: Action[], keywords: Record<string, string>, delimiter: string | null): Outcome[] {
  const out: Outcome[] = [];
  for (const a of actions) {
    const kind = actionKind(a);
    switch (a.type) {
      case "fileInto": {
        const path = folderPath(a.mailbox, delimiter);
        out.push({ kind: a.copy ? "copy" : "move", path, text: path.join(" / ") });
        break;
      }
      case "flags": {
        if (kind === "markRead") {
          out.push({ kind: "read", path: [], text: t("route.flag.seen") });
          break;
        }
        if (kind === "flag") {
          out.push({ kind: "flag", path: [], text: t("route.flag.flagged") });
          break;
        }
        // System flags read as words with an icon, keywords as labels.
        // `setflag` replaces all flags: "Only read".
        const system = a.flags.filter((f) => SYSTEM_FLAGS[f.toLowerCase()]);
        const labels = a.flags.filter((f) => !SYSTEM_FLAGS[f.toLowerCase()]);
        const only = a.op === "set" ? t("route.only") : a.op === "remove" ? t("route.remove") : "";
        for (const f of system) {
          const name = flagName(f, keywords);
          const kindOf: OutcomeKind = a.op === "remove" && f.toLowerCase() === "\\seen" ? "unread" : f.toLowerCase() === "\\seen" ? "read" : f.toLowerCase() === "\\flagged" ? "flag" : "system";
          out.push({ kind: kindOf, path: [], text: only ? only + name.toLocaleLowerCase(i18n.locale) : name });
        }
        for (const f of labels)
          out.push({ kind: a.op === "remove" ? "unlabel" : a.op === "set" ? "setLabels" : "label", path: [], text: flagName(f, keywords) });
        break;
      }
      case "redirect":
        out.push({ kind: "forward", path: [], text: a.address });
        break;
      case "vacation":
        out.push({ kind: "reply", path: [], text: a.subject ?? a.reason.split("\n")[0] });
        break;
      case "reject":
        out.push({ kind: "reject", path: [], text: a.reason.split("\n")[0] });
        break;
      case "discard":
        out.push({ kind: "delete", path: [], text: t("route.delete") });
        break;
      case "keep":
        out.push({ kind: "keep", path: [], text: t("route.keep") });
        break;
      case "stop":
      case "if":
        break;
    }
  }
  return out;
}

/** Whether the actions end the script for the message ("stop"). */
export function stops(actions: Action[]): boolean {
  return actions.some((a) => a.type === "stop");
}

// ── Cases ─────────────────────────────────────────────────────────────────────

/** One way through a rule: its conditions and what happens. */
export interface Case {
  kind: "if" | "elsif" | "else";
  /** Position among the cases of its rule or check (1-based). */
  number: number;
  conditions: Conditions | null;
  actions: Action[];
  /** Checks inside this case, each with cases of its own. */
  nested: Branches[];
}

export function cases(b: Branches): Case[] {
  const nested = (actions: Action[]) => actions.filter((a): a is Extract<Action, { type: "if" }> => a.type === "if");
  const list: Case[] = [
    { kind: "if", number: 1, conditions: b.conditions, actions: b.actions, nested: nested(b.actions) },
    ...b.elsif.map((e, i) => ({
      kind: "elsif" as const,
      number: i + 2,
      conditions: e.conditions,
      actions: e.actions,
      nested: nested(e.actions),
    })),
  ];
  if (b.otherwise) list.push({ kind: "else", number: list.length + 1, conditions: null, actions: b.otherwise, nested: nested(b.otherwise) });
  return list;
}

/** A rule that reads as one line: a single case without nested checks. */
export function isSimple(b: Branches): boolean {
  return b.elsif.length === 0 && !b.otherwise && !b.actions.some((a) => a.type === "if");
}

/** All cases of a rule, those of nested checks included. */
export function countCases(b: Branches): number {
  return cases(b).reduce((n, c) => n + 1 + c.nested.reduce((m, x) => m + countCases(x), 0), 0);
}

/** Folders a rule can move messages to, last segment only, without repeats. */
export function destinations(b: Branches, delimiter: string | null): string[] {
  const all = cases(b).flatMap((c) => [
    ...outcomes(c.actions, {}, delimiter).filter((o) => o.kind === "move").map((o) => o.path.at(-1) ?? o.text),
    ...c.nested.flatMap((n) => destinations(n, delimiter)),
  ]);
  return [...new Set(all)];
}
