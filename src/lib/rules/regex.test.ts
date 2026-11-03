import { test } from "node:test";
import assert from "node:assert/strict";
import { checkRegex, regexMatches, toRegExp } from "./regex.ts";

const SPAM = "^(info|support|kontakt|suite)[0-9]{4,}@([a-z0-9-]+\\.)+[a-z0-9-]+\\.shop$";

test("the spam campaign's addresses", () => {
  assert.equal(checkRegex(SPAM), null);
  for (const from of ["info1234@mail.xyzfashion.shop", "SUPPORT0000000123@a.b.xyzbeauty.shop", "suite99999@x-y.xyzfashion.shop"]) {
    assert.equal(regexMatches([SPAM], from), true, from);
  }
  for (const from of ["info123@mail.xyzfashion.shop", "info1234@xyzfashion.shop", "hello1234@mail.xyzfashion.shop", "info1234@mail.xyzfashion.shop.de"]) {
    assert.equal(regexMatches([SPAM], from), false, from);
  }
});

test("POSIX classes and glibc escapes", () => {
  assert.ok(toRegExp("[[:graph:]]")!.test("x"));
  assert.ok(!toRegExp("^[[:space:]]*$")!.test("a"));
  assert.ok(toRegExp("^[[:digit:]]{2}$")!.test("42"));
  assert.ok(toRegExp("\\bfoo\\b")!.test("a foo b"));
  assert.ok(toRegExp("^a{,2}$")!.test("aa"), "{,n}");
  assert.ok(toRegExp("[a\\]")!.test("\\"), "backslash is literal in brackets");
  assert.ok(toRegExp("[]a]")!.test("]"));
  assert.ok(toRegExp("^(ab)\\1$")!.test("abab"), "back reference");
});

test("what POSIX doesn't have", () => {
  assert.deepEqual(checkRegex("^\\d+$"), { kind: "perlClass", escape: "\\d", instead: "[0-9]" });
  assert.deepEqual(checkRegex("(?=x)"), { kind: "groupSyntax" });
  assert.deepEqual(checkRegex("(?:x)"), { kind: "groupSyntax" });
  assert.deepEqual(checkRegex("a+?"), { kind: "lazy" });
  assert.deepEqual(checkRegex("\\t"), { kind: "escape", escape: "\\t" });
  assert.deepEqual(checkRegex("[[:word:]]"), { kind: "unknownClass", name: "word" });
  assert.equal(checkRegex("(a")?.kind, "syntax");
  assert.equal(checkRegex("[a")?.kind, "syntax");
  assert.equal(regexMatches(["(a"], "a"), null);
});
