import { test } from "node:test";
import assert from "node:assert/strict";
import { flagLabel, formatKeywordNames, isKeyword, parseKeywordNames } from "./keywords.ts";

test("keyword names as text and back", () => {
  const names = { $synolabel_tu0exccz: "Rechnung", $label1: "Wichtig" };
  const text = formatKeywordNames(names);
  assert.equal(text, "$synolabel_tu0exccz = Rechnung\n$label1 = Wichtig");
  assert.deepEqual(parseKeywordNames(text), names);
  assert.deepEqual(parseKeywordNames("  $a =  A b \nno equals\n= x\n$b =\n"), { $a: "A b" });
});

test("labels and keywords", () => {
  assert.equal(flagLabel("$label1", { $label1: "Wichtig" }), "Wichtig");
  assert.equal(flagLabel("\\Seen", {}), "\\Seen");
  assert.ok(isKeyword("$label1"));
  assert.ok(!isKeyword("\\Flagged"));
});
