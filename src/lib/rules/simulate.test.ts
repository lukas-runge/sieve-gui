import { test } from "node:test";
import assert from "node:assert/strict";
import type { Action, Condition, RuleItem } from "#tauri/types";
import { addresses, EMPTY_MAIL, parseMail, runTrial } from "./simulate.ts";

const m = (type: string) => ({ matchType: { type }, comparator: null }) as never;
const from = (key: string, type = "is", part = "domain"): Condition =>
  ({ negate: false, test: { type: "address", headers: ["from"], part, matcher: m(type), keys: [key] } }) as Condition;
const subject = (key: string): Condition => ({ negate: false, test: { type: "header", headers: ["subject"], matcher: m("contains"), keys: [key] } }) as Condition;
const move = (mailbox: string): Action => ({ type: "fileInto", mailbox, copy: false, create: false, flags: null });
const rule = (conditions: Condition[], actions: Action[], extra: object = {}): RuleItem =>
  ({ type: "rule", name: "", enabled: true, conditions: { join: "all", items: conditions }, actions, elsif: [], otherwise: null, uid: "x", ...extra }) as RuleItem;

test("reads addresses from header values", () => {
  assert.deepEqual(addresses(['"Doe, Jane" <jane@a.example>, bob@b.example']), ["jane@a.example", "bob@b.example"]);
});

test("parses a pasted message", () => {
  const mail = parseMail("From: a@b.example\nSubject: Hi\nX-Spam-Flag: YES\n\nHello");
  assert.equal(mail.from, "a@b.example");
  assert.equal(mail.subject, "Hi");
  assert.equal(mail.headers, "X-Spam-Flag: YES");
  assert.equal(mail.body, "Hello");
});

test("the first matching case applies, checks inside it run too", () => {
  const sw = rule([from("shop.example")], [move("Shop"), { type: "if", conditions: { join: "all", items: [subject("rechnung")] }, actions: [move("Rechnungen")], elsif: [], otherwise: null }], {
    elsif: [{ conditions: { join: "all", items: [from("shop.example")] }, actions: [move("Never")] }],
    otherwise: [move("Rest")],
  });
  const trial = runTrial([{ name: "s", items: [sw], singleFolder: true }], { ...EMPTY_MAIL, from: "Shop <news@shop.example>", subject: "Ihre Rechnung" }, true);
  assert.deepEqual(trial.scripts[0].hits.get(0), ["0", "0.0.0"]);
  assert.deepEqual(trial.delivery.folders, ["Rechnungen"]);
  assert.equal(trial.steps[0].overridden, true);
  assert.equal(trial.delivery.inbox, false);
});

test("without last move wins every move files, stop ends all scripts", () => {
  const a = rule([from("x.example")], [move("A")]);
  const b = rule([], [move("B"), { type: "stop" }]);
  const trial = runTrial(
    [
      { name: "one", items: [a, b], singleFolder: false },
      { name: "two", items: [rule([], [move("C")])], singleFolder: false },
    ],
    { ...EMPTY_MAIL, from: "a@x.example" },
    false,
  );
  assert.deepEqual(trial.delivery.folders, ["A", "B"]);
  assert.equal(trial.scripts[1].skipped, true);
});

test("nothing matches: the message stays in the inbox; code is unknown", () => {
  const trial = runTrial(
    [{ name: "s", items: [rule([from("x.example")], [move("A")]), { type: "raw", code: "redirect \"a@b\";", uid: "r" } as RuleItem], singleFolder: true }],
    { ...EMPTY_MAIL, from: "a@y.example" },
    true,
  );
  assert.equal(trial.delivery.inbox, true);
  assert.equal(trial.uncertain, true);
  assert.deepEqual(trial.scripts[0].unknown, [1]);
});
