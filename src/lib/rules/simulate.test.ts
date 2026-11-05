import { test } from "node:test";
import assert from "node:assert/strict";
import type { Action, Condition, RuleItem } from "#tauri/types";
import { addresses, EMPTY_MAIL, expand, modify, newScope, parseMail, runTrial } from "./simulate.ts";

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
  assert.equal(trial.scripts[0].misses.get(0), undefined);
  assert.deepEqual(trial.delivery.folders, ["Rechnungen"]);
  assert.equal(trial.delivery.sources["folder:Rechnungen"], "s");
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
  assert.deepEqual(trial.scripts[0].misses.get(0), ["0"]);
  assert.equal(trial.uncertain, true);
  assert.deepEqual(trial.scripts[0].unknown, [1]);
});

test("makes up a message for a case", async () => {
  const { sampleFor } = await import("./simulate.ts");
  const mail = sampleFor([{ join: "all", items: [from("shop.example"), subject("Rechnung")] }]);
  assert.equal(mail.from, "info@shop.example");
  assert.equal(mail.subject, "Rechnung");
  const trial = runTrial(
    [{ name: "s", items: [rule([from("shop.example"), subject("rechnung")], [move("R")])], singleFolder: true }],
    mail,
    true,
  );
  assert.deepEqual(trial.delivery.folders, ["R"]);
});

const compare = (headers: string[], other: string[], differs: boolean, part: string | null = "all"): Condition =>
  ({ negate: false, test: { type: "compare", headers, part, other, differs } }) as Condition;
const str = (source: string, key: string, type = "is"): Condition =>
  ({ negate: false, test: { type: "string", sources: [source], matcher: m(type), keys: [key] } }) as Condition;
const set = (name: string, value: string, modifiers: string[] = []): Action => ({ type: "set", name, value, modifiers }) as Action;
const addflag = (flag: string): Action => ({ type: "flags", op: "add", flags: [flag] });

test("comparisons of two fields", () => {
  const items = [rule([compare(["reply-to"], ["from"], true)], [move("Junk")])];
  const run = (headers: string) => runTrial([{ name: "s", items, singleFolder: false }], { ...EMPTY_MAIL, from: "A <a@x.example>", headers }, false);
  assert.deepEqual(run("Reply-To: b@y.example").delivery.folders, ["Junk"]);
  // Addresses compare without case; without Reply-To nothing differs.
  assert.deepEqual(run("Reply-To: A@X.example").delivery.folders, []);
  assert.deepEqual(run("").delivery.folders, []);
  assert.equal(run("").uncertain, false);
});

test("the folder chosen so far, across scripts", () => {
  const spam = rule([subject("viagra")], [move("Junk")]);
  const mark = rule([str("${move_to_folder}", "junk")], [addflag("$Junk")]);
  const mail = { ...EMPTY_MAIL, subject: "Viagra!" };
  const scripts = [
    { name: "spam", items: [spam], singleFolder: true },
    { name: "mark", items: [mark], singleFolder: true },
  ];
  assert.deepEqual(runTrial(scripts, mail, true).delivery.flags, ["$Junk"]);
  assert.deepEqual(runTrial(scripts, { ...mail, subject: "Hi" }, true).delivery.flags, []);
});

test("set, match variables and modifiers", () => {
  const items = [
    rule([{ negate: false, test: { type: "header", headers: ["subject"], matcher: m("matches"), keys: ["[*] *"] } } as Condition], [
      set("tag", "${1}", ["upperFirst", "lower"]),
    ]),
    rule([str("${tag}", "Spam")], [move("${tag}-${2}"), addflag("${tag}")]),
  ];
  const trial = runTrial([{ name: "s", items, singleFolder: false, requires: ["variables"] }], { ...EMPTY_MAIL, subject: "[SPAM] buy now" }, false);
  // `${2}` still holds the last match: `:is` doesn't capture.
  assert.deepEqual(trial.delivery.folders, ["Spam-buy now"]);
  assert.deepEqual(trial.delivery.flags, ["Spam"]);
  // Without `variables`, "${…}" is plain text.
  const plain = runTrial([{ name: "s", items: [rule([subject("${x}")], [move("A")])], singleFolder: false }], { ...EMPTY_MAIL, subject: "a ${x} b" }, false);
  assert.deepEqual(plain.delivery.folders, ["A"]);
});

test("expanding variables", () => {
  const scope = newScope();
  scope.vars.set("a", "1");
  scope.match = ["whole", "first"];
  assert.equal(expand("${A}-${1}-${2}-${b}-${a-b}", scope), "1-first--" + "-${a-b}");
  assert.equal(expand("${env.x}", scope), null);
  assert.equal(modify("hELLO", ["upperFirst", "lower"]), "Hello");
  assert.equal(modify("abc", ["length", "upper"]), "3");
  assert.equal(modify("a*b?", ["quoteWildcard"]), "a\\*b\\?");
});
