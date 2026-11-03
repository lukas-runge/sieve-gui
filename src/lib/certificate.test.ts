import { test } from "node:test";
import assert from "node:assert/strict";
import { formatFingerprint, outsideValidity } from "./certificate.ts";

test("fingerprints are grouped in uppercase pairs", () => {
  assert.equal(formatFingerprint("ab01ff"), "AB:01:FF");
  assert.equal(formatFingerprint(""), "");
});

test("validity period", () => {
  assert.equal(outsideValidity(100, 200, 150), false);
  assert.equal(outsideValidity(100, 200, 50), true);
  assert.equal(outsideValidity(100, 200, 250), true);
  assert.equal(outsideValidity(null, null, 0), false);
});
