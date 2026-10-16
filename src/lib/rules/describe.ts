// Display-only Sieve rendering of conditions the editor can't edit.

import type { Condition, Matcher } from "#tauri/types";

function str(s: string): string {
  return `"${s.replace(/[\\"]/g, (c) => "\\" + c)}"`;
}

function list(values: string[]): string {
  return values.length === 1 ? str(values[0]) : `[${values.map(str).join(", ")}]`;
}

function matcher(m: Matcher): string {
  const mt = m.matchType;
  const out = mt.type === "value" || mt.type === "count" ? `:${mt.type} ${str(mt.op)}` : `:${mt.type}`;
  return m.comparator ? `${out} :comparator ${str(m.comparator)}` : out;
}

export function print(c: Condition): string {
  const t = c.test;
  let out: string;
  switch (t.type) {
    case "header":
      out = `header ${matcher(t.matcher)} ${list(t.headers)} ${list(t.keys)}`;
      break;
    case "address":
    case "envelope": {
      const part = t.part === "all" ? "" : `:${t.part.toLowerCase()} `;
      const fields = t.type === "address" ? t.headers : t.parts;
      out = `${t.type} ${part}${matcher(t.matcher)} ${list(fields)} ${list(t.keys)}`;
      break;
    }
    case "exists":
      out = `exists ${list(t.headers)}`;
      break;
    case "size":
      out = `size ${t.over ? ":over" : ":under"} ${t.limit}`;
      break;
    case "body": {
      const tr = t.transform.type === "content" ? `:content ${list(t.transform.value)}` : `:${t.transform.type}`;
      out = `body ${tr} ${matcher(t.matcher)} ${list(t.keys)}`;
      break;
    }
    case "currentDate":
      out = `currentdate ${t.zone ? `:zone ${str(t.zone)} ` : ""}${matcher(t.matcher)} ${str(t.datePart)} ${list(t.keys)}`;
      break;
    case "date": {
      const zone = t.zone.type === "zone" ? `:zone ${str(t.zone.value)} ` : t.zone.type === "original" ? ":originalzone " : "";
      out = `date ${zone}${matcher(t.matcher)} ${str(t.header)} ${str(t.datePart)} ${list(t.keys)}`;
      break;
    }
  }
  return c.negate ? `not ${out}` : out;
}
