// Maps the rule model onto what the editor shows: a field, an operator and
// values per condition, and a kind plus parameters per action. Anything the
// editor can't express is shown read-only and left untouched.

import type { Action, Condition, ConditionTest, Matcher, Rule } from "#tauri/types";

// ── Conditions ────────────────────────────────────────────────────────────────

export const FIELDS = ["from", "to", "cc", "toOrCc", "subject", "header", "body", "size", "date"] as const;
export type Field = (typeof FIELDS)[number];

export const TEXT_OPERATORS = [
  "contains",
  "notContains",
  "is",
  "notIs",
  "startsWith",
  "notStartsWith",
  "endsWith",
  "notEndsWith",
  "matches",
  "notMatches",
  "regex",
  "notRegex",
  "exists",
  "notExists",
] as const;
export const SIZE_OPERATORS = ["over", "under"] as const;
/** Delivery date (`currentdate :value "ge"/"le"`), as Roundcube's out-of-office uses. */
export const DATE_OPERATORS = ["onOrAfter", "onOrBefore"] as const;
export type Operator =
  | (typeof TEXT_OPERATORS)[number]
  | (typeof SIZE_OPERATORS)[number]
  | (typeof DATE_OPERATORS)[number];

export interface ConditionView {
  field: Field;
  /** Header name for the `header` field. */
  header: string;
  operator: Operator;
  /** Values as the user typed them (wildcards of starts/ends with removed).
   * For dates: `YYYY-MM-DD` or `YYYY-MM-DDTHH:MM`. */
  values: string[];
  /** Time zone of a date condition (`+0100`), kept when editing. */
  zone?: string | null;
}

const FIELD_HEADERS: Partial<Record<Field, string[]>> = {
  from: ["from"],
  to: ["to"],
  cc: ["cc"],
  toOrCc: ["to", "cc"],
  subject: ["subject"],
};

function fieldOf(headers: string[]): { field: Field; header: string } | null {
  const lower = headers.map((h) => h.toLowerCase()).sort();
  for (const [field, names] of Object.entries(FIELD_HEADERS)) {
    if (names && names.length === lower.length && [...names].sort().every((n, i) => n === lower[i])) {
      return { field: field as Field, header: "" };
    }
  }
  return headers.length === 1 ? { field: "header", header: headers[0] } : null;
}

/** Escapes `*`, `?` and `\` for a literal part of a `:matches` pattern. */
function escapePattern(s: string): string {
  return s.replace(/[\\*?]/g, (c) => "\\" + c);
}

/** The literal text of a pattern without unescaped wildcards, else null. */
function unescapePattern(s: string): string | null {
  let out = "";
  for (let i = 0; i < s.length; i++) {
    const c = s[i];
    if (c === "\\" && i + 1 < s.length) {
      out += s[++i];
    } else if (c === "*" || c === "?") {
      return null;
    } else {
      out += c;
    }
  }
  return out;
}

/** "starts with" / "ends with" values, if every key has that shape. */
function affixValues(keys: string[], kind: "start" | "end"): string[] | null {
  const out: string[] = [];
  for (const key of keys) {
    const ok = kind === "start" ? key.endsWith("*") && !key.endsWith("\\*") : key.startsWith("*");
    if (!ok || key.length < 2) return null;
    const literal = unescapePattern(kind === "start" ? key.slice(0, -1) : key.slice(1));
    if (literal === null) return null;
    out.push(literal);
  }
  return out;
}

function negated(op: string, negate: boolean): Operator {
  return (negate ? "not" + op[0].toUpperCase() + op.slice(1) : op) as Operator;
}

/** The editor view of a condition, or null if it can't be edited. */
export function viewCondition(c: Condition): ConditionView | null {
  const test = c.test;
  switch (test.type) {
    case "header": {
      const f = fieldOf(test.headers);
      if (!f) return null;
      const m = textOperator(test.matcher, test.keys, c.negate);
      return m && { ...f, ...m };
    }
    case "exists": {
      const f = fieldOf(test.headers);
      return f && { ...f, operator: c.negate ? "notExists" : "exists", values: [] };
    }
    case "body": {
      if (test.transform.type !== "text") return null;
      const m = textOperator(test.matcher, test.keys, c.negate);
      return m && { field: "body", header: "", ...m };
    }
    case "size":
      if (c.negate) return null;
      return { field: "size", header: "", operator: test.over ? "over" : "under", values: [String(test.limit)] };
    case "currentDate": {
      const mt = test.matcher.matchType;
      if (c.negate || test.matcher.comparator !== null || mt.type !== "value" || test.keys.length !== 1) return null;
      if (mt.op !== "ge" && mt.op !== "le") return null;
      const key = test.keys[0];
      let value: string;
      if (test.datePart === "date" && /^\d{4}-\d{2}-\d{2}$/.test(key)) value = key;
      else if (test.datePart === "iso8601" && /^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}/.test(key)) value = key.slice(0, 16);
      else return null;
      return { field: "date", header: "", operator: mt.op === "ge" ? "onOrAfter" : "onOrBefore", values: [value], zone: test.zone };
    }
    default:
      return null;
  }
}

/** The UTC offset of the local time zone at a date, as `+0100`. */
function localZone(date: string): string {
  const d = new Date(date.length === 10 ? `${date}T12:00` : date);
  const minutes = -(Number.isNaN(d.getTime()) ? new Date() : d).getTimezoneOffset();
  const abs = Math.abs(minutes);
  return `${minutes < 0 ? "-" : "+"}${String(Math.floor(abs / 60)).padStart(2, "0")}${String(abs % 60).padStart(2, "0")}`;
}

function textOperator(
  matcher: Matcher,
  keys: string[],
  negate: boolean,
): { operator: Operator; values: string[] } | null {
  if (matcher.comparator !== null) return null;
  switch (matcher.matchType.type) {
    case "contains":
    case "is":
    case "regex":
      return { operator: negated(matcher.matchType.type, negate), values: keys };
    case "matches": {
      const start = affixValues(keys, "start");
      if (start) return { operator: negated("startsWith", negate), values: start };
      const end = affixValues(keys, "end");
      if (end) return { operator: negated("endsWith", negate), values: end };
      return { operator: negated("matches", negate), values: keys };
    }
    default:
      return null;
  }
}

/** Builds the model condition for an editor view. */
export function buildCondition(v: ConditionView): Condition {
  const headers = v.field === "header" ? [v.header || "x-header"] : (FIELD_HEADERS[v.field] ?? ["subject"]);
  if (v.field === "size") {
    const limit = Math.max(0, Math.round(Number(v.values[0]) || 0));
    return { negate: false, test: { type: "size", over: v.operator !== "under", limit } };
  }
  if (v.field === "date") {
    const value = v.values[0] || new Date().toISOString().slice(0, 10);
    const zone = v.zone ?? localZone(value);
    const withTime = value.includes("T");
    const key = withTime ? `${value}:00${zone.slice(0, 3)}:${zone.slice(3)}` : value;
    return {
      negate: false,
      test: {
        type: "currentDate",
        zone,
        matcher: { matchType: { type: "value", op: v.operator === "onOrBefore" ? "le" : "ge" }, comparator: null },
        datePart: withTime ? "iso8601" : "date",
        keys: [key],
      },
    };
  }
  const negate = v.operator.startsWith("not");
  const base = negate ? v.operator[3].toLowerCase() + v.operator.slice(4) : v.operator;
  if (base === "exists") {
    return { negate, test: { type: "exists", headers } };
  }
  let keys = v.values.length ? v.values : [""];
  let matchType: Matcher["matchType"];
  switch (base) {
    case "startsWith":
      keys = keys.map((k) => escapePattern(k) + "*");
      matchType = { type: "matches" };
      break;
    case "endsWith":
      keys = keys.map((k) => "*" + escapePattern(k));
      matchType = { type: "matches" };
      break;
    case "is":
    case "matches":
    case "regex":
      matchType = { type: base };
      break;
    default:
      matchType = { type: "contains" };
  }
  const matcher: Matcher = { matchType, comparator: null };
  const test: ConditionTest =
    v.field === "body"
      ? { type: "body", transform: { type: "text" }, matcher, keys }
      : { type: "header", headers, matcher, keys };
  return { negate, test };
}

export function operatorsFor(field: Field, extensions: string[]): readonly Operator[] {
  if (field === "size") return SIZE_OPERATORS;
  if (field === "date") return DATE_OPERATORS;
  let ops: readonly Operator[] = TEXT_OPERATORS;
  if (field === "body") ops = ops.filter((o) => !o.endsWith("xists"));
  if (!extensions.includes("regex")) ops = ops.filter((o) => !/regex/i.test(o));
  return ops;
}

export function fieldsFor(extensions: string[]): readonly Field[] {
  return FIELDS.filter(
    (f) =>
      (f !== "body" || extensions.includes("body")) &&
      (f !== "date" || (extensions.includes("date") && extensions.includes("relational"))),
  );
}

export function hasValues(op: Operator): boolean {
  return op !== "exists" && op !== "notExists";
}

/** Changing the field keeps operator and values where they still make sense. */
export function withField(v: ConditionView, field: Field): ConditionView {
  if (field === "size") return { field, header: "", operator: "over", values: ["1048576"] };
  if (field === "date") return { field, header: "", operator: "onOrAfter", values: [new Date().toISOString().slice(0, 10)] };
  if (v.field === "size" || v.field === "date") return { field, header: "", operator: "contains", values: [""] };
  const operator = field === "body" && !hasValues(v.operator) ? "contains" : v.operator;
  return { ...v, field, operator };
}

export function newCondition(): Condition {
  return buildCondition({ field: "from", header: "", operator: "contains", values: [""] });
}

// ── Actions ───────────────────────────────────────────────────────────────────

export const ACTION_KINDS = [
  "move",
  "copy",
  "redirect",
  "redirectCopy",
  "markRead",
  "flag",
  "addFlags",
  "setFlags",
  "removeFlags",
  "vacation",
  "reject",
  "keep",
  "discard",
  "stop",
] as const;
export type ActionKind = (typeof ACTION_KINDS)[number];

export function actionKind(a: Action): ActionKind {
  switch (a.type) {
    case "fileInto":
      return a.copy ? "copy" : "move";
    case "redirect":
      return a.copy ? "redirectCopy" : "redirect";
    case "flags":
      if (a.op === "add" && a.flags.length === 1 && a.flags[0] === "\\Seen") return "markRead";
      if (a.op === "add" && a.flags.length === 1 && a.flags[0] === "\\Flagged") return "flag";
      return a.op === "add" ? "addFlags" : a.op === "set" ? "setFlags" : "removeFlags";
    case "reject":
      return "reject";
    case "keep":
      return "keep";
    case "discard":
      return "discard";
    case "stop":
      return "stop";
    case "vacation":
      return "vacation";
  }
}

/** Extensions an action kind needs on the server. */
export function actionExtension(kind: ActionKind): string | null {
  switch (kind) {
    case "move":
      return "fileinto";
    case "copy":
      return "fileinto";
    case "redirectCopy":
      return "copy";
    case "markRead":
    case "flag":
    case "addFlags":
    case "setFlags":
    case "removeFlags":
      return "imap4flags";
    case "vacation":
      return "vacation";
    case "reject":
      return "reject";
    default:
      return null;
  }
}

/** A new action of `kind`, taking over the parameter of `from` if it fits. */
export function buildAction(kind: ActionKind, from?: Action): Action {
  const folder = from?.type === "fileInto" ? from.mailbox : "";
  const address = from?.type === "redirect" ? from.address : "";
  const flags = from?.type === "flags" ? from.flags : [];
  switch (kind) {
    case "move":
    case "copy":
      return {
        type: "fileInto",
        mailbox: folder,
        copy: kind === "copy",
        create: from?.type === "fileInto" ? from.create : false,
        flags: from?.type === "fileInto" ? from.flags : null,
      };
    case "redirect":
    case "redirectCopy":
      return { type: "redirect", address, copy: kind === "redirectCopy" };
    case "markRead":
      return { type: "flags", op: "add", flags: ["\\Seen"] };
    case "flag":
      return { type: "flags", op: "add", flags: ["\\Flagged"] };
    case "addFlags":
      return { type: "flags", op: "add", flags };
    case "setFlags":
      return { type: "flags", op: "set", flags };
    case "removeFlags":
      return { type: "flags", op: "remove", flags };
    case "reject":
      return { type: "reject", reason: from?.type === "reject" ? from.reason : "", extended: false };
    case "keep":
      return { type: "keep", flags: null };
    case "discard":
      return { type: "discard" };
    case "stop":
      return { type: "stop" };
    case "vacation":
      return from?.type === "vacation"
        ? from
        : {
            type: "vacation",
            period: { unit: "days", value: 7 },
            subject: null,
            from: null,
            addresses: [],
            mime: false,
            handle: null,
            reason: "",
          };
  }
}

export function newRule(name: string): Rule {
  return { name, enabled: true, conditions: { join: "all", items: [newCondition()] }, actions: [buildAction("move")] };
}
