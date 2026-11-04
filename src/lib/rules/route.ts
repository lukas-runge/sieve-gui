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
import { actionsGoTo, type Target } from "./search";
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
  /** How the values are compared ("contains", "ends with", "exists", ">" …); none for "@domain". */
  word: string | null;
  /** Values are patterns or regular expressions: shown in the code font. */
  pattern: boolean;
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

/** How a value is compared, as a word before it; none for addresses written as "@domain". */
function compareWord(op: string, part: string): string | null {
  switch (op) {
    case "is":
      return part === "domain" ? null : t("route.op.is");
    case "isWithSubdomains":
      return null;
    case "startsWith":
      return t("route.op.startsWith");
    case "endsWith":
      return t("route.op.endsWith");
    case "matches":
      return t("route.op.matches");
    case "regex":
      return t("route.op.regex");
    case "contains":
      return t("route.op.contains");
    default:
      return null;
  }
}

function valueToken(op: string, part: string, value: string): ValueToken {
  let before = "";
  let after = "";
  if (op === "isWithSubdomains") after = " " + t("route.withSubdomains");
  // The address part shows as part of the address: @domain, name@, +detail.
  if (part === "domain") before = "@";
  else if (part === "localPart" || part === "user") after += "@";
  else if (part === "detail") before = "+";
  return { before, text: value || "…", after };
}

function sentence(v: ConditionView, chip: string): string {
  const values = v.values.map((x) => `„${x}“`).join(` ${t("sentence.or")} `);
  const part = v.part !== "header" && v.part !== "all" ? ` (${t(`part.${v.part}` as MessageKey)})` : "";
  return `${chip}${part} ${t(`op.${v.operator}` as MessageKey)}${hasValues(v.operator) ? " " + values : ""}`;
}

export function conditionToken(c: Condition, max = MAX_VALUES): ConditionToken {
  const v = viewCondition(c);
  if (!v) {
    return {
      field: t("field.other"),
      fieldTitle: null,
      negated: false,
      word: null,
      pattern: false,
      values: [],
      more: [],
      sentence: t("field.other"),
    };
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
    word = compareWord(op, v.part);
    values = v.values.map((x) => valueToken(op, v.part, x));
  }
  return {
    field: chip.label,
    fieldTitle: chip.title,
    negated,
    word,
    pattern: op === "matches" || op === "regex",
    values: values.slice(0, max),
    more: v.values.slice(max),
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

export function conditionShape(conditions: Conditions, negate = false, max = MAX_VALUES): ConditionShape {
  const { items, join } = conditions;
  if (items.length === 0) return { kind: "always" };
  const flat = items.every((c) => c.test.type !== "group");
  if (!negate && items.length === 1 && flat) return { kind: "line", token: conditionToken(items[0], max) };
  // "From contains a" or "From contains b": one token with both values.
  if (!negate && flat && join === "any" && sameComparison(items)) {
    const tokens = items.map((c) => conditionToken(c, Infinity));
    const all = tokens.flatMap((tk) => [...tk.values]);
    const first = tokens[0];
    const raw = items.flatMap((c) => viewCondition(c)!.values);
    return {
      kind: "line",
      token: {
        ...first,
        values: all.slice(0, max),
        more: raw.slice(max),
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
        ? { kind: "group", shape: conditionShape({ join: c.test.join, items: c.test.items }, c.negate, max) }
        : { kind: "token", token: conditionToken(c, max) },
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
  /** What "everything that goes here" filters by: the folder or label as stored. */
  target?: Target;
}

const SYSTEM_FLAGS: Record<string, MessageKey> = {
  "\\seen": "route.flag.seen",
  "\\flagged": "route.flag.flagged",
  "\\answered": "route.flag.answered",
  "\\deleted": "route.flag.deleted",
  "\\draft": "route.flag.draft",
};

/** The IMAP system flags a rule can set, in the order they are offered. */
export const SYSTEM_FLAG_LIST = ["\\Seen", "\\Flagged", "\\Answered", "\\Draft", "\\Deleted"];

/**
 * The flag a typed text stands for: a system flag or label by its name, a
 * flag written as such (`\Seen`, `$label1`), else a new keyword (spaces
 * aren't allowed in keywords, so they become underscores).
 */
export function flagFromText(text: string, keywords: Record<string, string>): string | null {
  const typed = text.trim();
  if (!typed) return null;
  const lower = typed.toLocaleLowerCase();
  const system = SYSTEM_FLAG_LIST.find((f) => f.toLowerCase() === lower || flagName(f, {}).toLocaleLowerCase() === lower);
  if (system) return system;
  const named = Object.entries(keywords).find(([, name]) => name.toLocaleLowerCase() === lower);
  if (named) return named[0];
  return typed.replace(/\s+/g, "_");
}

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
        out.push({ kind: a.copy ? "copy" : "move", path, text: path.join(" / "), target: { kind: "folder", key: a.mailbox } });
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
        // `setflag` sets these and drops all others: said once, after them.
        const prefix = a.op === "remove" ? t("route.remove") : "";
        for (const f of system) {
          const name = flagName(f, keywords);
          const kindOf: OutcomeKind = a.op === "remove" && f.toLowerCase() === "\\seen" ? "unread" : f.toLowerCase() === "\\seen" ? "read" : f.toLowerCase() === "\\flagged" ? "flag" : "system";
          out.push({ kind: kindOf, path: [], text: prefix ? prefix + name.toLocaleLowerCase(i18n.locale) : name });
        }
        for (const f of labels)
          out.push({
            kind: a.op === "remove" ? "unlabel" : "label",
            path: [],
            text: flagName(f, keywords),
            target: a.op === "remove" ? undefined : { kind: "label", key: f },
          });
        if (a.op === "set" && labels.length) out.push({ kind: "system", path: [], text: t("route.othersClearedNote") });
        else if (a.op === "set" && system.length) out[out.length - 1].text += t("route.othersCleared");
        else if (a.op === "set") out.push({ kind: "system", path: [], text: t("route.allCleared") });
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

/**
 * A stable, calm hue per label name, so the same label looks the same
 * everywhere. Green (matches, running) and blue (selection, actions) are
 * left out, so a label never looks like a state.
 */
const LABEL_HUES = [15, 40, 65, 195, 285, 310, 335, 355];
export function labelHue(name: string): number {
  let h = 0;
  for (const ch of name) h = (h * 31 + ch.charCodeAt(0)) % 997;
  return LABEL_HUES[h % LABEL_HUES.length];
}

/** What the checks inside a case do, for a case without actions of its own. */
export function nestedOutcomes(c: Case, keywords: Record<string, string>, delimiter: string | null): Outcome[] {
  const seen = new Set<string>();
  const out: Outcome[] = [];
  const visit = (x: Case) => {
    for (const o of outcomes(x.actions, keywords, delimiter)) {
      const key = `${o.kind}:${o.text}`;
      if (seen.has(key)) continue;
      seen.add(key);
      out.push(o);
    }
    for (const n of x.nested) for (const inner of cases(n)) visit(inner);
  };
  for (const n of c.nested) for (const inner of cases(n)) visit(inner);
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

/**
 * Cases that can never apply because a case before them always matches
 * first: one without conditions, or one testing exactly the same.
 */
export function unreachable(list: Case[]): Set<number> {
  const out = new Set<number>();
  const key = (c: Case) => (c.conditions ? JSON.stringify(c.conditions) : null);
  for (let i = 1; i < list.length; i++) {
    const own = key(list[i]);
    if (list.slice(0, i).some((p) => p.conditions && (p.conditions.items.length === 0 || (own !== null && key(p) === own)))) out.add(i);
  }
  return out;
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

// ── Search and picking ───────────────────────────────────────────────────────

/** `text` split into parts that match one of the search words and parts that don't. */
export function marks(text: string, words: string[]): { text: string; hit: boolean }[] {
  if (words.length === 0 || text === "") return [{ text, hit: false }];
  const lower = text.toLocaleLowerCase();
  const hits: boolean[] = new Array(text.length).fill(false);
  for (const w of words) {
    if (!w) continue;
    for (let i = lower.indexOf(w); i >= 0; i = lower.indexOf(w, i + 1)) hits.fill(true, i, i + w.length);
  }
  const out: { text: string; hit: boolean }[] = [];
  for (let i = 0; i < text.length; i++) {
    const last = out.at(-1);
    if (last && last.hit === hits[i]) last.text += text[i];
    else out.push({ text: text[i], hit: hits[i] });
  }
  return out;
}

function shapeText(shape: ConditionShape): string {
  if (shape.kind === "always") return "";
  if (shape.kind === "line") return tokenText(shape.token);
  return shape.items.map((i) => (i.kind === "token" ? tokenText(i.token) : shapeText(i.shape))).join(" ");
}

function tokenText(token: ConditionToken): string {
  return [token.field, token.fieldTitle ?? "", ...token.values.map((v) => v.text), ...token.more].join(" ");
}

/** Whether a case (with its nested checks) files into the folder or sets the label. */
export function caseGoesTo(c: Case, target: Target): boolean {
  return actionsGoTo(c.actions, target);
}

/** Whether a case (with its nested checks) contains one of the search words. */
export function caseMatches(c: Case, words: string[], keywords: Record<string, string>, delimiter: string | null): boolean {
  if (words.length === 0) return true;
  const text = [
    c.conditions ? shapeText(conditionShape(c.conditions)) : "",
    ...outcomes(c.actions, keywords, delimiter).map((o) => o.text),
  ]
    .join(" ")
    .toLocaleLowerCase();
  return words.some((w) => text.includes(w)) || c.nested.some((n) => cases(n).some((x) => caseMatches(x, words, keywords, delimiter)));
}

/**
 * Where a case sits in a rule: top-level case indexes and, for checks inside
 * a case, "case.check.case", e.g. "0.0.2" for the third case of the first
 * check in the first case. "" is the rule itself.
 */
export type CasePath = string;

/** One case on the way to a case path: the block it belongs to and its place. */
export interface CaseStep {
  block: Branches;
  list: Case[];
  index: number;
  case: Case;
  /** The case's path. */
  path: CasePath;
}

/** The cases from the rule down to the case at `path`; null if there is none. */
export function caseChain(rule: Branches, path: CasePath): CaseStep[] | null {
  if (path === "") return null;
  const parts = path.split(".").map(Number);
  const out: CaseStep[] = [];
  let block: Branches = rule;
  for (let i = 0; i < parts.length; i += 2) {
    const list = cases(block);
    const c = list[parts[i]];
    if (!c) return null;
    out.push({ block, list, index: parts[i], case: c, path: parts.slice(0, i + 1).join(".") });
    if (parts[i + 1] === undefined) break;
    const nested = c.nested[parts[i + 1]];
    if (!nested) return null;
    block = nested;
  }
  return out;
}

/**
 * The number a case shows: "3" for the third case of a rule, "1.2" for the
 * second case of a check inside case 1. Checks with a single case add no
 * number of their own.
 */
export function caseNumber(chain: CaseStep[]): string {
  return chain.filter((s) => s.list.length > 1).map((s) => s.case.number).join(".");
}

/** Readable name of a case: "Case 1.2", "Case 3 › extra check", "Otherwise". */
export function casePathLabel(rule: Branches, path: CasePath): string {
  const chain = caseChain(rule, path);
  if (!chain) return "";
  const last = chain[chain.length - 1];
  const number = caseNumber(chain);
  // The only case of a rule is the rule itself.
  if (chain.length === 1 && last.list.length === 1) return "";
  if (last.case.kind === "else") {
    const parent = caseNumber(chain.slice(0, -1));
    return parent ? `${t("route.caseN", { n: parent })} › ${t("route.otherwiseCase")}` : t("route.otherwiseCase");
  }
  if (last.list.length === 1 && chain.length > 1) return number ? `${t("route.caseN", { n: number })} › ${t("route.checkCase")}` : t("route.checkCase");
  return number ? t("route.caseN", { n: number }) : t("route.checkCase");
}
