// Trying rules out with a sample message: which case of which rule applies,
// and where the message ends up, across the scripts that run in order. This
// runs in the app, not on the server, so it covers what the rule editor
// writes (headers, addresses, envelope, size, body text, dates). Code blocks
// and anything else it can't evaluate are reported as unknown, never guessed.

import type { Action, Branch, Condition, Conditions, ConditionTest, Matcher, RuleItem } from "#tauri/types";
import { toRegExp } from "./regex.ts";

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

function glob(pattern: string, text: string, fold: boolean): boolean {
  let re = "";
  for (let i = 0; i < pattern.length; i++) {
    const ch = pattern[i];
    if (ch === "\\" && i + 1 < pattern.length) re += pattern[++i].replace(/[.*+?^${}()|[\]\\/]/g, "\\$&");
    else if (ch === "*") re += "[\\s\\S]*";
    else if (ch === "?") re += "[\\s\\S]";
    else re += ch.replace(/[.*+?^${}()|[\]\\/]/g, "\\$&");
  }
  return new RegExp(`^${re}$`, fold ? "i" : "").test(text);
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

/** Whether any of `values` matches any of `keys`. */
function matchValues(m: Matcher, values: string[], keys: string[]): Tri {
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
      if (mt.type === "matches" && glob(k, v, fold)) return true;
      if (mt.type === "regex") {
        const re = toRegExp(k);
        if (!re) return null;
        if ((fold ? re : new RegExp(re.source, re.flags.replace("i", ""))).test(v)) return true;
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

function testValue(test: ConditionTest, msg: Message, now: Date): Tri {
  switch (test.type) {
    case "header":
      return matchValues(
        test.matcher,
        test.headers.flatMap((h) => msg.headers.get(h.toLowerCase()) ?? []),
        test.keys,
      );
    case "address": {
      const parts = addresses(test.headers.flatMap((h) => msg.headers.get(h.toLowerCase()) ?? []))
        .map((a) => addressPart(a, test.part))
        .filter((p): p is string => p !== null);
      return matchValues(test.matcher, parts, test.keys);
    }
    case "envelope": {
      const list = test.parts
        .map((p) => (p.toLowerCase() === "from" ? msg.envelopeFrom : p.toLowerCase() === "to" ? msg.envelopeTo : ""))
        .filter(Boolean)
        .map((a) => addressPart(a, test.part))
        .filter((p): p is string => p !== null);
      return matchValues(test.matcher, list, test.keys);
    }
    case "exists":
      return test.headers.every((h) => msg.headers.has(h.toLowerCase()));
    case "size":
      return test.over ? msg.size > test.limit : msg.size < test.limit;
    case "body":
      if (test.transform.type === "content") return null;
      return matchValues(test.matcher, [msg.body], test.keys);
    case "currentDate": {
      const v = datePart(now, test.datePart);
      return v === null ? null : matchValues(test.matcher, [v], test.keys);
    }
    case "date": {
      const raw = msg.headers.get(test.header.toLowerCase())?.[0];
      if (!raw) return false;
      const d = new Date(raw);
      if (Number.isNaN(d.getTime())) return null;
      const v = datePart(d, test.datePart);
      return v === null ? null : matchValues(test.matcher, [v], test.keys);
    }
    case "group":
      return conditionsValue({ join: test.join, items: test.items }, msg, now);
  }
}

function conditionValue(c: Condition, msg: Message, now: Date): Tri {
  const v = testValue(c.test, msg, now);
  return v === null ? null : c.negate ? !v : v;
}

/** Three-valued "all of" / "any of": unknown only if it decides the result. */
export function conditionsValue(cs: Conditions, msg: Message, now = new Date()): Tri {
  if (cs.items.length === 0) return true;
  let unknown = false;
  for (const c of cs.items) {
    const v = conditionValue(c, msg, now);
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
  const delivery: Delivery = { folders: [], inbox: false, flags: [], forwards: [], rejected: false, discarded: false, replied: false };
  let keepCancelled = false;
  let recorded: { folder: string; step: Step } | null = null;
  let stoppedAll = false;
  let uncertain = false;
  let flags: string[] = [];

  const setFlags = (op: string, list: string[]) => {
    const words = list.flatMap((f) => f.split(/\s+/)).filter(Boolean);
    const has = (f: string) => flags.some((x) => x.toLowerCase() === f.toLowerCase());
    if (op === "set") flags = [...new Set(words)];
    else if (op === "add") flags = [...flags, ...words.filter((w) => !has(w))];
    else flags = flags.filter((x) => !words.some((w) => w.toLowerCase() === x.toLowerCase()));
  };
  const file = (folder: string) => {
    if (!delivery.folders.includes(folder)) delivery.folders.push(folder);
  };

  for (const script of scripts) {
    const run: ScriptRun = { name: script.name, hits: new Map(), unknown: [], stoppedAt: null, skipped: stoppedAll };
    runs.push(run);
    if (stoppedAll) continue;

    /** Applies actions; returns true when a "stop" ends the script. */
    const apply = (index: number, path: string, actions: Action[]): boolean => {
      const step: Step = { script: script.name, index, path, actions: actions.filter((a) => a.type !== "if"), overridden: false };
      steps.push(step);
      run.hits.set(index, [...(run.hits.get(index) ?? []), path]);
      let nested = 0;
      for (const a of actions) {
        switch (a.type) {
          case "fileInto":
            if (script.singleFolder && lastMoveWins && !a.copy && !a.create && !a.flags) {
              if (recorded) recorded.step.overridden = true;
              recorded = { folder: a.mailbox, step };
            } else {
              file(a.mailbox);
              if (!a.copy) keepCancelled = true;
            }
            break;
          case "redirect":
            delivery.forwards.push(a.address);
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
            setFlags(a.op, a.flags);
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
        const v = c.conditions ? conditionsValue(c.conditions, msg, now) : true;
        if (v === null) {
          // Can't tell whether this case applies, so neither what follows.
          if (!run.unknown.includes(index)) run.unknown.push(index);
          uncertain = true;
          return false;
        }
        if (v) return apply(index, prefix + i, c.actions);
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
    file((recorded as { folder: string }).folder);
    keepCancelled = true;
  }
  if (!keepCancelled) delivery.inbox = true;
  delivery.flags = flags;
  return { scripts: runs, steps, delivery, uncertain };
}
