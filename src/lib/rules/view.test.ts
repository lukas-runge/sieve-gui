import { test } from "node:test";
import assert from "node:assert/strict";
import type { Condition } from "#tauri/types";
import {
  addCondition,
  addGroup,
  groupMode,
  removeFromGroup,
  setGroupMode,
  buildCondition,
  newRule,
  operatorsFor,
  partsFor,
  removeCondition,
  viewCondition,
  withField,
  withPart,
  type ConditionView,
} from "./view.ts";

const from: ConditionView = { field: "from", header: "", part: "header", operator: "contains", values: ["x"] };

function address(part: string, matchType: string, keys: string[], negate = false): Condition {
  return {
    negate,
    test: { type: "address", headers: ["from"], part, matcher: { matchType: { type: matchType }, comparator: null }, keys },
  } as Condition;
}

test("the whole header stays a header test", () => {
  assert.equal(buildCondition(from).test.type, "header");
});

test("an address part makes an address test", () => {
  const c = buildCondition({ ...from, part: "domain", operator: "is", values: ["example.com"] });
  assert.deepEqual(c, address("domain", "is", ["example.com"]));
  assert.deepEqual(viewCondition(c), { ...from, part: "domain", operator: "is", values: ["example.com"] });
});

test("ends with on the address", () => {
  const c = buildCondition({ ...from, part: "all", operator: "endsWith", values: ["@example.com"] });
  assert.deepEqual(c, address("all", "matches", ["*@example.com"]));
  assert.equal(viewCondition(c)?.operator, "endsWith");
});

test("domain including subdomains", () => {
  const view: ConditionView = { ...from, part: "domain", operator: "isWithSubdomains", values: ["a.org", "b.org"] };
  const c = buildCondition(view);
  assert.deepEqual(c, address("domain", "matches", ["a.org", "*.a.org", "b.org", "*.b.org"]));
  assert.deepEqual(viewCondition(c), view);

  const negated = buildCondition({ ...view, operator: "notIsWithSubdomains" });
  assert.equal(negated.negate, true);
  assert.equal(viewCondition(negated)?.operator, "notIsWithSubdomains");
});

test("subdomain pairs are recognized in any order, and only as pairs", () => {
  const view = (keys: string[], part = "domain") => viewCondition(address(part, "matches", keys));
  assert.deepEqual(view(["*.a.org", "a.org"])?.values, ["a.org"]);
  // While the user is still typing.
  assert.equal(view(["", "*."])?.operator, "isWithSubdomains");
  assert.equal(view(["*.a.org"])?.operator, "endsWith");
  assert.equal(view(["a.org", "*.b.org"])?.operator, "matches");
  assert.equal(view(["a.org", "*.a.org", "a.org"])?.operator, "matches");
  // Only the domain part means domains.
  assert.equal(view(["a.org", "*.a.org"], "all")?.operator, "matches");
});

test("address tests on other headers", () => {
  const c = { ...address("localPart", "is", ["me"]), test: { ...address("localPart", "is", ["me"]).test, headers: ["Reply-To"] } };
  assert.deepEqual(viewCondition(c as Condition), { field: "header", header: "Reply-To", part: "localPart", operator: "is", values: ["me"] });
  const subject = { ...c, test: { ...c.test, headers: ["subject"] } };
  assert.equal(viewCondition(subject as Condition), null);
});

test("operators follow the part", () => {
  assert.ok(operatorsFor("from", "header", []).includes("exists"));
  assert.ok(!operatorsFor("from", "all", []).includes("exists"));
  assert.ok(!operatorsFor("from", "all", []).includes("isWithSubdomains"));
  assert.ok(operatorsFor("from", "domain", []).includes("isWithSubdomains"));
});

test("subaddress parts need the extension, unless already used", () => {
  assert.deepEqual(partsFor("from", [], "header"), ["header", "all", "domain", "localPart"]);
  assert.deepEqual(partsFor("from", [], "detail"), ["header", "all", "domain", "localPart", "detail"]);
  assert.equal(partsFor("from", ["subaddress"], "header").length, 6);
  assert.deepEqual(partsFor("subject", ["subaddress"], "header"), []);
});

test("switching part or field keeps what still fits", () => {
  const subdomains: ConditionView = { ...from, part: "domain", operator: "notIsWithSubdomains", values: ["a.org"] };
  assert.deepEqual(withPart(subdomains, "all"), { ...subdomains, part: "all", operator: "notIs" });
  assert.deepEqual(withPart({ ...from, operator: "exists", values: [] }, "domain"), {
    ...from,
    part: "domain",
    operator: "contains",
    values: [""],
  });
  assert.deepEqual(withField(subdomains, "to"), { ...subdomains, field: "to" });
  assert.deepEqual(withField(subdomains, "subject"), { ...subdomains, field: "subject", part: "header", operator: "notIs" });
});

test("a single condition is written bare, two start with all", () => {
  const conditions = newRule("x").conditions;
  assert.equal(conditions.join, "any");
  addCondition(conditions);
  assert.equal(conditions.join, "all");
  addCondition(conditions);
  conditions.join = "any";
  removeCondition(conditions, 0);
  assert.equal(conditions.join, "any", "the user's choice stays while there are two");
  conditions.join = "all";
  removeCondition(conditions, 0);
  assert.deepEqual([conditions.items.length, conditions.join], [1, "any"]);
  removeCondition(conditions, 0);
  addCondition(conditions);
  assert.deepEqual([conditions.items.length, conditions.join], [1, "any"]);
});

function partType(types: string[], negate = false, keys = [""], matchType = "contains"): Condition {
  return {
    negate,
    test: {
      type: "body",
      transform: { type: "content", value: types },
      matcher: { matchType: { type: matchType }, comparator: null },
      keys,
    },
  } as Condition;
}

test("MIME part types are a body :content test with an empty key", () => {
  const view: ConditionView = { field: "parts", header: "", part: "header", operator: "hasType", values: ["application/x-sogo-notification"] };
  const c = buildCondition(view);
  assert.deepEqual(c, partType(["application/x-sogo-notification"]));
  assert.deepEqual(viewCondition(c), view);

  const none = buildCondition({ ...view, operator: "notHasType", values: ["application/pdf", "image"] });
  assert.deepEqual(none, partType(["application/pdf", "image"], true));
  assert.equal(viewCondition(none)?.operator, "notHasType");
});

test("other body :content tests stay read-only", () => {
  assert.equal(viewCondition(partType(["text/plain"], false, ["invoice"])), null);
  assert.equal(viewCondition(partType(["text/plain"], false, [""], "is")), null);
});

test("switching to and from MIME parts", () => {
  const parts = withField(from, "parts");
  assert.deepEqual(parts, { field: "parts", header: "", part: "header", operator: "hasType", values: [""] });
  assert.deepEqual(operatorsFor("parts", "header", ["body"]), ["hasType", "notHasType"]);
  assert.deepEqual(partsFor("parts", [], "header"), []);
  assert.deepEqual(withField(parts, "subject"), { field: "subject", header: "", part: "header", operator: "contains", values: [""] });
});

test("a new group starts with one condition and the other join", () => {
  const conditions = newRule("x").conditions;
  addGroup(conditions, true);
  assert.equal(conditions.join, "all");
  const group = conditions.items[1];
  assert.equal(group.test.type, "group");
  if (group.test.type !== "group") return;
  assert.deepEqual([group.test.join, group.test.items.length], ["any", 1]);

  addGroup(group.test, false);
  const inner = group.test.items[1];
  assert.ok(inner.test.type === "group" && inner.test.join === "all");
  assert.equal(group.test.join, "any", "a nested group keeps its join");
});

test("group modes cover negation", () => {
  const group: Condition = { negate: false, test: { type: "group", join: "all", items: [] } };
  for (const mode of ["any", "none", "notAll", "all"] as const) {
    setGroupMode(group, mode);
    assert.ok(group.test.type === "group");
    assert.equal(groupMode(group), mode);
  }
  setGroupMode(group, "none");
  assert.deepEqual([group.negate, group.test.type === "group" && group.test.join], [true, "any"]);
});

test("removing the last condition of a nested group removes the group", () => {
  const conditions = newRule("x").conditions;
  addGroup(conditions, true);
  const holder = conditions.items[1];
  if (holder.test.type !== "group") throw new Error("no group");
  addGroup(holder.test, false);
  removeFromGroup(holder.test, 0, false);
  assert.equal(holder.test.items.length, 1);
  const inner = holder.test.items[0];
  if (inner.test.type !== "group") throw new Error("no inner group");
  assert.equal(removeFromGroup(inner.test, 0, false), true, "now empty");
  assert.equal(removeFromGroup(conditions, 0, true), false, "the rule's own conditions may be empty");
});
