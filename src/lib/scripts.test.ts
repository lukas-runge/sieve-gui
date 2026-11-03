import { test } from "node:test";
import assert from "node:assert/strict";
import { scriptTree } from "./scripts.ts";

const names = ["sort.Banken", "spam_rule", "sort", "mails", "synomc", "filter", "spam", "a", "b"];
const includes = { synomc: ["spam_rule", "filter"], sort: ["sort.Banken"], a: ["b"], b: ["a"] };
const flat = (nodes: { name: string; depth: number }[]) => nodes.map((n) => "  ".repeat(n.depth) + n.name);

test("running scripts in run order, included ones under them", () => {
  const tree = scriptTree(names, ["synomc", "mails"], includes);
  assert.deepEqual(flat(tree.running), ["synomc", "  spam_rule", "  filter", "mails"]);
  assert.equal(tree.running[1].via, "synomc");
  // Inactive: alphabetical, sort.Banken under sort, the cycle a ↔ b once.
  assert.deepEqual(flat(tree.inactive), ["sort", "  sort.Banken", "spam", "a", "  b"]);
});

test("a script that runs on its own isn't listed under its includer", () => {
  const tree = scriptTree(names, ["synomc", "sort.Banken", "sort"], includes);
  assert.deepEqual(flat(tree.running), ["synomc", "  spam_rule", "  filter", "sort.Banken", "sort"]);
});

test("unknown names in the run order are skipped", () => {
  assert.deepEqual(flat(scriptTree(["x"], ["gone", "x"], {}).running), ["x"]);
});
