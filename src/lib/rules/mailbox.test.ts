import { test } from "node:test";
import assert from "node:assert/strict";
import {
  decodeModifiedUtf7,
  detectMailboxEncoding,
  encodeModifiedUtf7,
  mailboxLabel,
  mailboxRaw,
} from "./mailbox.ts";

// Examples from RFC 3501 §5.1.3 and the Roundcube fixtures.
const PAIRS: [string, string][] = [
  ["B&APw-cher", "Bücher"],
  ["~peter/mail/&U,BTFw-/&ZeVnLIqe-", "~peter/mail/台北/日本語"],
  ["Entw&APw-rfe", "Entwürfe"],
  ["Tom &- Jerry &AMQ-", "Tom & Jerry Ä"],
  ["&2D3eAA-", "😀"],
];

test("decodes and encodes modified UTF-7", () => {
  for (const [raw, label] of PAIRS) {
    assert.equal(decodeModifiedUtf7(raw), label, raw);
    assert.equal(encodeModifiedUtf7(label), raw, label);
  }
});

test("plain names are not decoded", () => {
  for (const name of ["INBOX", "Lists/Rust", "Bücher", "Q&A", "a&b-c", "&-x&", "", "&Jjo"]) {
    assert.equal(decodeModifiedUtf7(name), null, name);
    assert.equal(mailboxLabel(name), name);
  }
});

test("rejects non-canonical encodings", () => {
  // Encoded printable ASCII ("A"), non-zero padding bits, lone surrogate.
  for (const name of ["&AEE-", "&APx-", "&2D0-"]) {
    assert.equal(decodeModifiedUtf7(name), null, name);
  }
});

test("new names follow the script's encoding", () => {
  assert.equal(detectMailboxEncoding(["INBOX", "B&APw-cher"]), "utf7");
  assert.equal(detectMailboxEncoding(["INBOX", "Bücher", "Q&A"]), "utf8");
  assert.equal(mailboxRaw("Entwürfe", "utf7"), "Entw&APw-rfe");
  assert.equal(mailboxRaw("Entwürfe", "utf8"), "Entwürfe");
  assert.equal(mailboxRaw("Q&A", "utf7"), "Q&-A");
});
