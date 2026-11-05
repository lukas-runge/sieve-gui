import { test } from "node:test";
import assert from "node:assert/strict";
import { isNewer } from "./version.ts";

test("later versions", () => {
  assert.ok(isNewer("0.2.0", "0.1.0"));
  assert.ok(isNewer("v0.10.0", "0.9.3"));
  assert.ok(isNewer("1.0", "0.99.99"));
  assert.ok(isNewer("0.1.1", "0.1"));
});

test("same or earlier versions", () => {
  assert.ok(!isNewer("0.1.0", "0.1.0"));
  assert.ok(!isNewer("v0.1.0", "0.1"));
  assert.ok(!isNewer("0.1.0", "0.2.0"));
  assert.ok(!isNewer("0.2.0-rc.1", "0.2.0"));
});
