// Trying rules out with a sample message: which case of which rule applies,
// and where the message ends up, across the scripts that run in order. This
// runs in the app, not on the server, so it covers what the rule editor
// writes (headers, addresses, envelope, size, body text, dates). Code blocks
// and anything else it can't evaluate are reported as unknown, never guessed.

import type { Action, Branch, Condition, Conditions, ConditionTest, Matcher, RuleItem, SetModifier } from "#tauri/types";
import { toRegExp } from "./regex.ts";
import { FOLDER_VARIABLE } from "./view.ts";

// ── The sample message ───────────────────────────────────────────────────────

export interface SampleMail {
  from: string;
  to: string;
  subject: string;
  /** Further header lines, "Name: value" each. */
  headers: string;
  body: string;
}

export const EMPTY_MAIL: SampleMail = { from: "", to: "", subject: "", headers: "", body: "" };

export interface Message {
  /** Header values by lower-case name, in order. */
  headers: Map<string, string[]>;
  envelopeFrom: string;
  envelopeTo: string;
  body: string;
  size: number;
}

/** "Name: value" lines, continuation lines folded in. */
function headerLines(text: string): [string, string][] {
  const out: [string, string][] = [];
  for (const line of text.split(/\r?\n/)) {
    if (/^[ \t]/.test(line) && out.length) {
      out[out.length - 1][1] += " " + line.trim();
      continue;
    }
    const m = /^([!-9;-~]+):\s*(.*)$/.exec(line);
    if (m) out.push([m[1], m[2].trim()]);
  }
  return out;
}

/** A whole message as pasted (headers, blank line, body) as the sample's fields. */
export function parseMail(raw: string): SampleMail {
  const text = raw.replace(/^\s+/, "");
  const split = /\r?\n\r?\n/.exec(text);
  const head = split ? text.slice(0, split.index) : text;
  const body = split ? text.slice(split.index + split[0].length) : "";
  const mail: SampleMail = { ...EMPTY_MAIL, body: body.trim() };
  const rest: string[] = [];
  for (const [name, value] of headerLines(head)) {
    const lower = name.toLowerCase();
    if (lower === "from" && !mail.from) mail.from = value;
    else if (lower === "to" && !mail.to) mail.to = value;
    else if (lower === "subject" && !mail.subject) mail.subject = value;
    else rest.push(`${name}: ${value}`);
  }
  mail.headers = rest.join("\n");
  return mail;
}

/** Whether a pasted text looks like a whole message rather than a value. */
export function looksLikeMail(text: string): boolean {
  return headerLines(text).length >= 2;
}

export function toMessage(mail: SampleMail): Message {
  const headers = new Map<string, string[]>();
  const add = (name: string, value: string) => {
    const key = name.toLowerCase();
    headers.set(key, [...(headers.get(key) ?? []), value]);
  };
  if (mail.from.trim()) add("from", mail.from.trim());
  if (mail.to.trim()) add("to", mail.to.trim());
  if (mail.subject.trim()) add("subject", mail.subject.trim());
  for (const [name, value] of headerLines(mail.headers)) add(name, value);
  const first = (name: string) => addresses(headers.get(name) ?? [])[0] ?? "";
  const envelopeTo = first("delivered-to") || first("x-original-to") || first("envelope-to") || first("to");
  const all = [...headers].map(([k, vs]) => vs.map((v) => `${k}: ${v}`).join("\r\n")).join("\r\n");
  return {
    headers,
    envelopeFrom: first("return-path") || first("from"),
    envelopeTo,
    body: mail.body,
    size: new TextEncoder().encode(`${all}\r\n\r\n${mail.body}`).length,
  };
}

/** Bare addresses in header values: `"Name" <a@b>, c@d` → a@b, c@d. */
export function addresses(values: string[]): string[] {
  const out: string[] = [];
  for (const value of values) {
    let quoted = false;
    let depth = 0;
    let part = "";
    const parts: string[] = [];
    for (const ch of value) {
      if (ch === '"') quoted = !quoted;
      else if (!quoted && ch === "<") depth++;
      else if (!quoted && ch === ">") depth = Math.max(0, depth - 1);
      if (!quoted && depth === 0 && (ch === "," || ch === ";")) {
        parts.push(part);
        part = "";
      } else part += ch;
    }
    parts.push(part);
    for (const p of parts) {
      const angle = /<([^>]*)>/.exec(p);
      const addr = (angle ? angle[1] : p.replace(/\(.*?\)/g, "").replace(/"[^"]*"/g, "")).trim();
      if (addr) out.push(addr);
    }
  }
  return out;
}

// ── Conditions ───────────────────────────────────────────────────────────────

/** true / false, or null when the app can't tell (e.g. an unknown comparator). */
type Tri = boolean | null;

function caseless(m: Matcher): boolean | null {
  if (m.comparator === null || m.comparator === "i;ascii-casemap") return true;
  if (m.comparator === "i;octet") return false;
  return null;
}

/**
 * Matches a `:matches` pattern; on a match, the whole text and what each
 * wildcard matched (RFC 5229 match variables). Each `*` matches as little as
 * the rest of the pattern allows, as in Pigeonhole: "*.*" on "a.b.c" gives
 * "a" and "b.c".
 */
function glob(pattern: string, text: string, fold: boolean): string[] | null {
  let re = "";
  for (let i = 0; i < pattern.length; i++) {
    const ch = pattern[i];
    if (ch === "\\" && i + 1 < pattern.length) re += pattern[++i].replace(/[.*+?^${}()|[\]\\/]/g, "\\$&");
    else if (ch === "*") re += "([\\s\\S]*?)";
    else if (ch === "?") re += "([\\s\\S])";
    else re += ch.replace(/[.*+?^${}()|[\]\\/]/g, "\\$&");
  }
  const m = new RegExp(`^${re}$`, fold ? "i" : "").exec(text);
  return m ? [...m] : null;
}

function relate(op: string, a: string | number, b: string | number): boolean {
  switch (op) {
    case "gt":
      return a > b;
    case "ge":
      return a >= b;
    case "lt":
      return a < b;
    case "le":
      return a <= b;
    case "eq":
      return a === b;
    default:
      return a !== b;
  }
}

// ── Variables ────────────────────────────────────────────────────────────────

/** Variables while a script runs (RFC 5229). */
export interface Scope {
  /** The script uses variables; otherwise "${…}" is plain text. */
  on: boolean;
  /** Variables set so far, by lower-case name. */
  vars: Map<string, string>;
  /** `${0}` … `${9}`: what the last successful `:matches` / `:regex` matched. */
  match: string[];
  /** Global variables this scope reads elsewhere, e.g. the folder recorded so far. */
  global?: (name: string) => string | undefined;
}

export function newScope(on = true): Scope {
  return { on, vars: new Map(), match: [] };
}

/**
 * `text` with its variables replaced. Unset variables are empty; text that
 * isn't a variable reference (`${a-b}`) stays. Variables of a namespace
 * (`${env.x}`) can't be known here: null.
 */
export function expand(text: string, scope: Scope | undefined): string | null {
  if (!scope?.on || !text.includes("${")) return text;
  let unknown = false;
  const out = text.replace(/\$\{([A-Za-z_][A-Za-z0-9_]*(?:\.[A-Za-z_][A-Za-z0-9_]*)*|[0-9]+)\}/g, (_, name: string) => {
    if (/^[0-9]+$/.test(name)) return scope.match[Number(name)] ?? "";
    if (name.includes(".")) {
      unknown = true;
      return "";
    }
    const lower = name.toLowerCase();
    return scope.global?.(lower) ?? scope.vars.get(lower) ?? "";
  });
  return unknown ? null : out;
}

function expandAll(list: string[], scope: Scope | undefined): string[] | null {
  const out: string[] = [];
  for (const s of list) {
    const v = expand(s, scope);
    if (v === null) return null;
    out.push(v);
  }
  return out;
}

/** The precedence of `set` modifiers (RFC 5229 §4.1): higher ones apply first. */
const MODIFIER_PRECEDENCE: Record<SetModifier, number> = {
  lower: 40,
  upper: 40,
  lowerFirst: 30,
  upperFirst: 30,
  quoteWildcard: 20,
  quoteRegex: 20,
  length: 10,
};

/** A value as `set` stores it with these modifiers. */
export function modify(value: string, modifiers: SetModifier[]): string {
  let v = value;
  const ordered = [...modifiers].sort((a, b) => MODIFIER_PRECEDENCE[b] - MODIFIER_PRECEDENCE[a]);
  for (const m of ordered) {
    switch (m) {
      case "lower":
        v = v.toLowerCase();
        break;
      case "upper":
        v = v.toUpperCase();
        break;
      case "lowerFirst":
        v = v.charAt(0).toLowerCase() + v.slice(1);
        break;
      case "upperFirst":
        v = v.charAt(0).toUpperCase() + v.slice(1);
        break;
      case "quoteWildcard":
        v = v.replace(/[*?\\]/g, "\\$&");
        break;
      case "quoteRegex":
        v = v.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
        break;
      case "length":
        v = String([...v].length);
        break;
    }
  }
  return v;
}

/** Whether any of `values` matches any of `keys`; a match sets the match variables of `scope`. */
function matchValues(m: Matcher, values: string[], keys: string[], scope?: Scope): Tri {
  const mt = m.matchType;
  if (mt.type === "count") {
    if (m.comparator !== "i;ascii-numeric") return null;
    return keys.some((k) => relate(mt.op, values.length, Number(k)));
  }
  if (mt.type === "value") {
    if (m.comparator === "i;ascii-numeric") {
      const num = (s: string) => (/^\d+/.test(s) ? Number(/^\d+/.exec(s)![0]) : Infinity);
      return values.some((v) => keys.some((k) => relate(mt.op, num(v), num(k))));
    }
    const fold = caseless(m);
    if (fold === null) return null;
    const f = (s: string) => (fold ? s.toLowerCase() : s);
    return values.some((v) => keys.some((k) => relate(mt.op, f(v), f(k))));
  }
  const fold = caseless(m);
  if (fold === null) return null;
  const f = (s: string) => (fold ? s.toLowerCase() : s);
  for (const v of values) {
    for (const k of keys) {
      if (mt.type === "is" && f(v) === f(k)) return true;
      if (mt.type === "contains" && f(v).includes(f(k))) return true;
      if (mt.type === "matches") {
        const groups = glob(k, v, fold);
        if (groups) {
          if (scope) scope.match = groups;
          return true;
        }
      }
      if (mt.type === "regex") {
        const re = toRegExp(k);
        if (!re) return null;
        const found = (fold ? re : new RegExp(re.source, re.flags.replace("i", ""))).exec(v);
        if (found) {
          if (scope) scope.match = [...found].map((g) => g ?? "");
          return true;
        }
      }
    }
  }
  return false;
}

function addressPart(addr: string, part: string): string | null {
  const at = addr.lastIndexOf("@");
  const local = at >= 0 ? addr.slice(0, at) : addr;
  const domain = at >= 0 ? addr.slice(at + 1) : "";
  const plus = local.indexOf("+");
  switch (part) {
    case "localPart":
      return local;
    case "domain":
      return domain;
    case "user":
      return plus >= 0 ? local.slice(0, plus) : local;
    case "detail":
      return plus >= 0 ? local.slice(plus + 1) : null;
    default:
      return addr;
  }
}

const pad = (n: number, w = 2) => String(n).padStart(w, "0");

/** A date part as Sieve's `date` extension writes it (local time). */
function datePart(d: Date, part: string): string | null {
  switch (part.toLowerCase()) {
    case "year":
      return pad(d.getFullYear(), 4);
    case "month":
      return pad(d.getMonth() + 1);
    case "day":
      return pad(d.getDate());
    case "date":
      return `${pad(d.getFullYear(), 4)}-${pad(d.getMonth() + 1)}-${pad(d.getDate())}`;
    case "hour":
      return pad(d.getHours());
    case "minute":
      return pad(d.getMinutes());
    case "second":
      return pad(d.getSeconds());
    case "time":
      return `${pad(d.getHours())}:${pad(d.getMinutes())}:${pad(d.getSeconds())}`;
    case "weekday":
      return String(d.getDay());
    default:
      return null;
  }
}

/** The values of headers, or of the given part of the addresses in them. */
function fieldValues(msg: Message, headers: string[], part: string | null): string[] {
  const values = headers.flatMap((h) => msg.headers.get(h.toLowerCase()) ?? []);
  if (part === null) return values;
  return addresses(values)
    .map((a) => addressPart(a, part))
    .filter((p): p is string => p !== null);
}

function testValue(test: ConditionTest, msg: Message, now: Date, scope?: Scope): Tri {
  // Variables in the keys are replaced before comparing.
  const keys = "keys" in test ? expandAll(test.keys, scope) : [];
  if (keys === null) return null;
  switch (test.type) {
    case "header":
      return matchValues(test.matcher, fieldValues(msg, test.headers, null), keys, scope);
    case "address":
      return matchValues(test.matcher, fieldValues(msg, test.headers, test.part), keys, scope);
    case "string": {
      const sources = expandAll(test.sources, scope);
      return sources === null ? null : matchValues(test.matcher, sources, keys, scope);
    }
    case "compare": {
      // The capture: the first value; then whether the other field has it too.
      const first = fieldValues(msg, test.headers, test.part)[0];
      if (first === undefined) return false;
      if (scope) scope.match = [first, first];
      const same = fieldValues(msg, test.other, test.part).some((v) => v.toLowerCase() === first.toLowerCase());
      return same !== test.differs;
    }
    case "envelope": {
      const list = test.parts
        .map((p) => (p.toLowerCase() === "from" ? msg.envelopeFrom : p.toLowerCase() === "to" ? msg.envelopeTo : ""))
        .filter(Boolean)
        .map((a) => addressPart(a, test.part))
        .filter((p): p is string => p !== null);
      return matchValues(test.matcher, list, keys, scope);
    }
    case "exists":
      return test.headers.every((h) => msg.headers.has(h.toLowerCase()));
    case "size":
      return test.over ? msg.size > test.limit : msg.size < test.limit;
    case "body":
      if (test.transform.type === "content") return null;
      return matchValues(test.matcher, [msg.body], keys, scope);
    case "currentDate": {
      const v = datePart(now, test.datePart);
      return v === null ? null : matchValues(test.matcher, [v], keys, scope);
    }
    case "date": {
      const raw = msg.headers.get(test.header.toLowerCase())?.[0];
      if (!raw) return false;
      const d = new Date(raw);
      if (Number.isNaN(d.getTime())) return null;
      const v = datePart(d, test.datePart);
      return v === null ? null : matchValues(test.matcher, [v], keys, scope);
    }
    case "group":
      return conditionsValue({ join: test.join, items: test.items }, msg, now, scope);
  }
}

function conditionValue(c: Condition, msg: Message, now: Date, scope?: Scope): Tri {
  const v = testValue(c.test, msg, now, scope);
  return v === null ? null : c.negate ? !v : v;
}

/**
 * Three-valued "all of" / "any of": unknown only if it decides the result.
 * Tests run in order, so a capture can feed a later test (`scope`).
 */
export function conditionsValue(cs: Conditions, msg: Message, now = new Date(), scope?: Scope): Tri {
  if (cs.items.length === 0) return true;
  let unknown = false;
  for (const c of cs.items) {
    const v = conditionValue(c, msg, now, scope);
    if (v === null) unknown = true;
    else if (cs.join === "any" && v) return true;
    else if (cs.join === "all" && !v) return false;
  }
  return unknown ? null : cs.join === "all";
}

// ── Running the rules ────────────────────────────────────────────────────────

/** What one applied case did, for the message's way through the scripts. */
export interface Step {
  script: string;
  /** Index of the rule among the script's items. */
  index: number;
  /** The case that applied (see `CasePath` in route.ts). */
  path: string;
  /** The case's own actions (checks inside it are steps of their own). */
  actions: Action[];
  /** Moves this case records that a later one replaced ("last move wins"). */
  overridden: boolean;
}

export interface ScriptRun {
  name: string;
  /** Applied case paths per rule index. */
  hits: Map<number, string[]>;
  /** Cases tried that didn't match, per rule index. */
  misses: Map<number, string[]>;
  /** Rules (by index) the app couldn't evaluate: code, unknown tests. */
  unknown: number[];
  /** Index of the rule whose "stop" ended the script, if any. */
  stoppedAt: number | null;
  /** The script never ran: an earlier one stopped all processing. */
  skipped: boolean;
}

export interface Delivery {
  folders: string[];
  inbox: boolean;
  flags: string[];
  forwards: string[];
  rejected: boolean;
  discarded: boolean;
  replied: boolean;
  /** Which script filed into a folder ("folder:<name>") or set a label ("flag:<name>"). */
  sources: Record<string, string>;
}

export interface Trial {
  scripts: ScriptRun[];
  steps: Step[];
  delivery: Delivery;
  /** Something on the way couldn't be evaluated: the result may differ. */
  uncertain: boolean;
}

export interface TrialScript {
  name: string;
  items: RuleItem[];
  /** Moves are recorded for "last move wins" and "stop" ends only this script. */
  singleFolder: boolean;
  /** The script's `require` list: with `variables`, "${…}" in values are variables. */
  requires?: string[];
}

/** Whether rules use variables (`set`, `string`, comparisons), which the exporter requires `variables` for. */
export function usesVariables(items: RuleItem[]): boolean {
  const inConditions = (cs: Condition[]): boolean =>
    cs.some((c) => c.test.type === "string" || c.test.type === "compare" || (c.test.type === "group" && inConditions(c.test.items)));
  const inBlock = (b: { conditions: Conditions; actions: Action[]; elsif: Branch[]; otherwise?: Action[] | null }): boolean =>
    [b.conditions, ...b.elsif.map((e) => e.conditions)].some((cs) => inConditions(cs.items)) ||
    [b.actions, ...b.elsif.map((e) => e.actions), b.otherwise ?? []].some((list) =>
      list.some((a) => a.type === "set" || (a.type === "if" && inBlock(a))),
    );
  return items.some((item) => item.type === "rule" && inBlock(item));
}

function caseList(b: { conditions: Conditions; actions: Action[]; elsif: Branch[]; otherwise?: Action[] | null }) {
  const list: { conditions: Conditions | null; actions: Action[] }[] = [
    { conditions: b.conditions, actions: b.actions },
    ...b.elsif.map((e) => ({ conditions: e.conditions, actions: e.actions })),
  ];
  if (b.otherwise) list.push({ conditions: null, actions: b.otherwise });
  return list;
}

/**
 * Runs `mail` through `scripts` in order, as the server would with the
 * root script of sieve-gui (`lastMoveWins`: the last recorded move files the
 * message at the end).
 */
export function runTrial(scripts: TrialScript[], mail: SampleMail, lastMoveWins: boolean, now = new Date()): Trial {
  const msg = toMessage(mail);
  const runs: ScriptRun[] = [];
  const steps: Step[] = [];
  const delivery: Delivery = { folders: [], inbox: false, flags: [], forwards: [], rejected: false, discarded: false, replied: false, sources: {} };
  let keepCancelled = false;
  let recorded: { folder: string; step: Step } | null = null;
  let stoppedAll = false;
  let uncertain = false;
  let flags: string[] = [];

  const setFlags = (op: string, list: string[], script: string) => {
    const words = list.flatMap((f) => f.split(/\s+/)).filter(Boolean);
    if (op !== "remove") for (const w of words) delivery.sources[`flag:${w}`] ??= script;
    const has = (f: string) => flags.some((x) => x.toLowerCase() === f.toLowerCase());
    if (op === "set") flags = [...new Set(words)];
    else if (op === "add") flags = [...flags, ...words.filter((w) => !has(w))];
    else flags = flags.filter((x) => !words.some((w) => w.toLowerCase() === x.toLowerCase()));
  };
  const file = (folder: string, script: string) => {
    if (!delivery.folders.includes(folder)) delivery.folders.push(folder);
    delivery.sources[`folder:${folder}`] ??= script;
  };

  for (const script of scripts) {
    const run: ScriptRun = { name: script.name, hits: new Map(), misses: new Map(), unknown: [], stoppedAt: null, skipped: stoppedAll };
    runs.push(run);
    if (stoppedAll) continue;

    // Variables are local to a script, except the folder recorded so far.
    const records = script.singleFolder && lastMoveWins;
    const scope = newScope(
      script.singleFolder || (script.requires ?? []).some((r) => r.toLowerCase() === "variables") || usesVariables(script.items),
    );
    if (records) scope.global = (name) => (name === FOLDER_VARIABLE ? (recorded?.folder ?? "") : undefined);
    /** A value with its variables replaced; one that can't be known makes the result uncertain. */
    const value = (text: string): string => {
      const v = expand(text, scope);
      if (v === null) uncertain = true;
      return v ?? text;
    };
    const record = (folder: string, step: Step) => {
      if (recorded) recorded.step.overridden = true;
      recorded = { folder, step };
    };

    /** Applies actions; returns true when a "stop" ends the script. */
    const apply = (index: number, path: string, actions: Action[]): boolean => {
      const step: Step = { script: script.name, index, path, actions: actions.filter((a) => a.type !== "if"), overridden: false };
      steps.push(step);
      run.hits.set(index, [...(run.hits.get(index) ?? []), path]);
      let nested = 0;
      for (const a of actions) {
        switch (a.type) {
          case "fileInto":
            if (records && !a.copy && !a.create && !a.flags) {
              record(value(a.mailbox), step);
            } else {
              file(value(a.mailbox), script.name);
              if (!a.copy) keepCancelled = true;
            }
            break;
          case "set": {
            const v = modify(value(a.value), a.modifiers);
            const name = a.name.toLowerCase();
            // Another way to record a folder: `set :lower "move_to_folder" …`.
            if (records && name === FOLDER_VARIABLE) record(v, step);
            else scope.vars.set(name, v);
            break;
          }
          case "redirect":
            delivery.forwards.push(value(a.address));
            if (!a.copy) keepCancelled = true;
            break;
          case "reject":
            delivery.rejected = true;
            keepCancelled = true;
            break;
          case "discard":
            delivery.discarded = true;
            keepCancelled = true;
            break;
          case "keep":
            delivery.inbox = true;
            break;
          case "flags":
            setFlags(a.op, a.flags.map(value), script.name);
            break;
          case "vacation":
            delivery.replied = true;
            break;
          case "stop":
            return true;
          case "if": {
            const at = nested++;
            if (branch(index, a, `${path}.${at}.`)) return true;
            break;
          }
        }
      }
      return false;
    };

    /** Runs the first matching case of a rule or check; true on "stop". */
    const branch = (index: number, b: Parameters<typeof caseList>[0], prefix: string): boolean => {
      for (const [i, c] of caseList(b).entries()) {
        const v = c.conditions ? conditionsValue(c.conditions, msg, now, scope) : true;
        if (v === null) {
          // Can't tell whether this case applies, so neither what follows.
          if (!run.unknown.includes(index)) run.unknown.push(index);
          uncertain = true;
          return false;
        }
        if (v) return apply(index, prefix + i, c.actions);
        run.misses.set(index, [...(run.misses.get(index) ?? []), prefix + i]);
      }
      return false;
    };

    for (const [index, item] of script.items.entries()) {
      if (item.type === "raw") {
        if (item.code.trim()) {
          run.unknown.push(index);
          uncertain = true;
        }
        continue;
      }
      if (!item.enabled) continue;
      if (branch(index, item, "")) {
        run.stoppedAt = index;
        if (!script.singleFolder || !lastMoveWins) stoppedAll = true;
        break;
      }
    }
  }

  if (recorded) {
    const last = recorded as { folder: string; step: Step };
    file(last.folder, last.step.script);
    keepCancelled = true;
  }
  if (!keepCancelled) delivery.inbox = true;
  delivery.flags = flags;
  return { scripts: runs, steps, delivery, uncertain };
}

// ── A sample message for a case ──────────────────────────────────────────────

/**
 * A message that likely matches the given conditions (a case and the cases
 * it sits in): the first value of each plain header or address test, as far
 * as one can be made up. Patterns, regexes and negations are left out; the
 * test then shows whether the message really matches.
 */
export function sampleFor(conditions: Conditions[]): SampleMail {
  const mail: SampleMail = { ...EMPTY_MAIL };
  const extra: string[] = [];
  const put = (header: string, value: string) => {
    const h = header.toLowerCase();
    if (h === "from" && !mail.from) mail.from = value;
    else if (["to", "cc", "bcc"].includes(h) && !mail.to) mail.to = value;
    else if (h === "subject" && !mail.subject) mail.subject = value;
    else if (!["to", "cc", "bcc", "from", "subject"].includes(h) && !extra.some((l) => l.toLowerCase().startsWith(h + ":")))
      extra.push(`${header}: ${value}`);
  };
  const visit = (cs: Conditions) => {
    // For "any of", one condition is enough.
    const items = cs.join === "any" ? cs.items.slice(0, 1) : cs.items;
    for (const c of items) {
      if (c.negate) continue;
      const test = c.test;
      if (test.type === "group") {
        visit({ join: test.join, items: test.items });
        continue;
      }
      if (test.type !== "header" && test.type !== "address") continue;
      const mt = test.matcher.matchType.type;
      if (mt !== "is" && mt !== "contains") continue;
      const key = test.keys[0];
      if (!key) continue;
      let value = key;
      if (test.type === "address" && test.part === "domain") value = `info@${key}`;
      else if (test.type === "address" && (test.part === "localPart" || test.part === "user")) value = `${key}@example.org`;
      put(test.headers[0] ?? "", value);
    }
  };
  for (const cs of conditions) visit(cs);
  mail.headers = extra.join("\n");
  return mail;
}
