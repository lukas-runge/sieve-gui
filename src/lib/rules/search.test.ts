import { test } from "node:test";
import assert from "node:assert/strict";
import type { RuleItem } from "#tauri/types";
import { goesTo, matches, queryWords } from "./search.ts";

const rule = {
  type: "rule",
  name: "Online shops",
  enabled: true,
  conditions: {
    join: "any",
    items: [
      {
        negate: false,
        test: { type: "address", headers: ["from"], part: "domain", matcher: { matchType: { type: "matches" }, comparator: null }, keys: ["*thomann.de"] },
      },
    ],
  },
  actions: [{ type: "fileInto", mailbox: "Accounts.Versand.B&APw-cher", copy: false, create: false, flags: null }],
} as unknown as RuleItem;

test("finds rules by name, values and readable folder names", () => {
  assert.ok(matches(rule, queryWords("thomann")));
  assert.ok(matches(rule, queryWords("ONLINE")));
  assert.ok(matches(rule, queryWords("bücher")), "decoded folder");
  assert.ok(matches(rule, queryWords("versand shops")), "all words, any order");
  assert.ok(!matches(rule, queryWords("thomann amazon")));
  assert.ok(matches(rule, queryWords("  ")), "empty query");
});

test("technical values don't match", () => {
  assert.ok(!matches(rule, queryWords("fileInto")));
  assert.ok(!matches(rule, queryWords("domain")));
});

test("code cards match their code", () => {
  const raw = { type: "raw", code: 'include :personal "sort.Banken";' } as unknown as RuleItem;
  assert.ok(matches(raw, queryWords("banken")));
  assert.ok(!matches(raw, queryWords("thomann")));
});

test("branches and nested ifs are searched", () => {
  const branched = {
    ...rule,
    elsif: [{ conditions: { join: "any", items: [] }, actions: [{ type: "fileInto", mailbox: "Rechnungen", copy: false, create: false, flags: null }] }],
    otherwise: [{ type: "if", conditions: { join: "any", items: [] }, actions: [{ type: "redirect", address: "boss@example.org", copy: false }], elsif: [], otherwise: null }],
  } as unknown as RuleItem;
  assert.ok(matches(branched, queryWords("rechnungen")));
  assert.ok(matches(branched, queryWords("boss")));
});

test("finds rules by where they file messages", () => {
  assert.equal(goesTo(rule, { kind: "folder", key: "Accounts.Versand.B&APw-cher" }), true);
  assert.equal(goesTo(rule, { kind: "folder", key: "Accounts" }), false);
  assert.equal(goesTo(rule, { kind: "label", key: "$label1" }), false);
});
