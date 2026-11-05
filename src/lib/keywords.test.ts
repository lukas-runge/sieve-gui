import { test } from "node:test";
import assert from "node:assert/strict";
import { flagLabel, isKeyword, labelHex, labelStyle, mergeTags, oklchToHex, tagList, tagMaps } from "./keywords.ts";

test("labels and keywords", () => {
  assert.equal(flagLabel("$label1", { $label1: "Wichtig" }), "Wichtig");
  assert.equal(flagLabel("\\Seen", {}), "\\Seen");
  assert.ok(isKeyword("$label1"));
  assert.ok(!isKeyword("\\Flagged"));
});

test("tags from names and colours and back", () => {
  const names = { $synolabel_tu0exccz: "Rechnung", $label1: "Wichtig" };
  const colors = { $label1: "#ff0000", $label2: "#00ff00" };
  const tags = tagList(names, colors);
  assert.deepEqual(tags, [
    { keyword: "$synolabel_tu0exccz", name: "Rechnung", color: null },
    { keyword: "$label1", name: "Wichtig", color: "#ff0000" },
    // Unnamed ones last.
    { keyword: "$label2", name: "", color: "#00ff00" },
  ]);
  assert.deepEqual(tagMaps(tags), { names, colors });
  assert.deepEqual(tagMaps([{ keyword: " ", name: "x", color: "#000000" }, { keyword: " $a ", name: " A ", color: null }]), {
    names: { $a: "A" },
    colors: {},
  });
});

test("a provider's tags replace those of the same keyword", () => {
  const tags = [
    { keyword: "$a", name: "Old", color: null },
    { keyword: "$mine", name: "Mine", color: "#123456" },
    { keyword: "", name: "typing", color: null },
  ];
  const merged = mergeTags(tags, [
    { name: "xis/Rechnung", color: "#997f00", keyword: "$a" },
    { name: "LRV", color: "#cc3d9c", keyword: "$b" },
    { name: "xis", color: "#64696e", keyword: null },
    { name: "Mine", color: "#123456", keyword: "$mine" },
  ]);
  assert.equal(merged.added, 1);
  assert.equal(merged.changed, 1);
  assert.deepEqual(merged.unmatched, ["xis"]);
  assert.deepEqual(merged.tags, [
    { keyword: "$b", name: "LRV", color: "#cc3d9c" },
    { keyword: "$mine", name: "Mine", color: "#123456" },
    { keyword: "$a", name: "xis/Rechnung", color: "#997f00" },
    { keyword: "", name: "typing", color: null },
  ]);
});

test("OKLCH to hex", () => {
  assert.equal(oklchToHex(1, 0, 0), "#ffffff");
  assert.equal(oklchToHex(0, 0, 0), "#000000");
  assert.equal(oklchToHex(0.628, 0.2577, 29.23), "#ff0000");
  assert.match(labelHex("Rechnung"), /^#[0-9a-f]{6}$/);
});

test("labels in their own colour or one from the name", () => {
  const own = labelStyle("Rechnung", "#997f00");
  assert.equal(own.dot, "background: #997f00");
  assert.match(own.chip, /color-mix\(in oklch, #997f00 60%, transparent\)/);
  const derived = labelStyle("Rechnung", null);
  assert.match(derived.dot, /^background: oklch\(0\.62 0\.13 \d+\)$/);
  assert.deepEqual(labelStyle("Rechnung", undefined), derived);
});
