// Syntax highlighting for Sieve in CodeMirror: a small stream tokenizer
// (RFC 5228 lexical rules) and the editor's color theme.

import { HighlightStyle, StreamLanguage, syntaxHighlighting, type StreamParser } from "@codemirror/language";
import { EditorView } from "@codemirror/view";
import { tags as t } from "@lezer/highlight";

const CONTROL = new Set(["if", "elsif", "else", "require"]);
const TEST_OPERATORS = new Set(["allof", "anyof", "not", "true", "false"]);
/** Tests from RFC 5228 and common extensions; any other identifier is an action. */
const TESTS = new Set([
  "address", "envelope", "exists", "header", "size", "body", "date", "currentdate", "string", "hasflag",
  "mailboxexists", "metadata", "metadataexists", "servermetadata", "servermetadataexists", "duplicate",
  "environment", "valid_notify_method", "notify_method_capability", "spamtest", "virustest", "ihave",
  "specialuse_exists", "valid_ext_list",
]);

interface State {
  /** Inside a `/* … *\/` comment or a `text:` multi-line string. */
  inside: "comment" | "text" | null;
  /** Inside a quoted string (strings may span lines). */
  quoted: boolean;
}

const parser: StreamParser<State> = {
  name: "sieve",
  startState: () => ({ inside: null, quoted: false }),
  token(stream, state) {
    if (state.inside === "comment") {
      if (stream.skipTo("*/")) {
        stream.pos += 2;
        state.inside = null;
      } else stream.skipToEnd();
      return "comment";
    }
    if (state.inside === "text") {
      // Ends with a line holding only ".".
      if (stream.sol() && stream.match(/^\.\r?$/)) state.inside = null;
      else stream.skipToEnd();
      return "string";
    }
    if (state.quoted) {
      while (!stream.eol()) {
        const ch = stream.next();
        if (ch === "\\") stream.next();
        else if (ch === '"') {
          state.quoted = false;
          break;
        }
      }
      return "string";
    }

    if (stream.eatSpace()) return null;
    if (stream.match("#")) {
      stream.skipToEnd();
      return "comment";
    }
    if (stream.match("/*")) {
      state.inside = "comment";
      return "comment";
    }
    if (stream.eat('"')) {
      state.quoted = true;
      return "string";
    }
    if (stream.match(/^text:/i)) {
      // The rest of this line may only hold a comment.
      state.inside = "text";
      stream.skipToEnd();
      return "string";
    }
    if (stream.match(/^:[a-z_][a-z0-9_]*/i)) return "attributeName";
    if (stream.match(/^\d+[kmg]?/i)) return "number";
    if (stream.match(/^[[\](){}]/)) return "bracket";
    if (stream.match(/^[;,]/)) return "punctuation";
    if (stream.match(/^[a-z_][a-z0-9_.]*/i)) {
      const word = stream.current().toLowerCase();
      if (CONTROL.has(word)) return "keyword";
      if (TEST_OPERATORS.has(word)) return "operatorKeyword";
      return TESTS.has(word) ? "variableName.function" : "variableName.standard";
    }
    stream.next();
    return null;
  },
  languageData: { commentTokens: { line: "#", block: { open: "/*", close: "*/" } } },
};

export const sieveLanguage = StreamLanguage.define(parser);

// Colors measured from the reference screenshot (a dark VS Code theme).
const palette = {
  background: "#252434",
  text: "#d4d3d7",
  lineNumber: "#878790",
  lineNumberActive: "#d4d3d7",
  activeLine: "#413b62",
  selection: "#413b6299",
  keyword: "#fade6c",
  punctuation: "#fbd73b",
  string: "#f39147",
  comment: "#858cbb",
  test: "#ec8676",
  action: "#ee5855",
};

const highlightStyle = HighlightStyle.define([
  { tag: [t.keyword, t.operatorKeyword, t.attributeName], color: palette.keyword },
  { tag: [t.bracket, t.punctuation], color: palette.punctuation },
  { tag: t.string, color: palette.string },
  { tag: t.number, color: palette.test },
  { tag: t.comment, color: palette.comment, fontStyle: "italic" },
  { tag: t.function(t.variableName), color: palette.test },
  { tag: t.standard(t.variableName), color: palette.action },
]);

const theme = EditorView.theme(
  {
    "&": {
      fontSize: "0.85rem",
      backgroundColor: palette.background,
      color: palette.text,
      borderRadius: "var(--radius-field)",
    },
    ".cm-content": { fontFamily: "var(--font-mono)", caretColor: palette.text },
    ".cm-cursor, .cm-dropCursor": { borderLeftColor: palette.text },
    ".cm-gutters": { backgroundColor: palette.background, color: palette.lineNumber, border: "none" },
    ".cm-lineNumbers .cm-gutterElement": { padding: "0 1.25em 0 0.75em" },
    ".cm-activeLineGutter": { backgroundColor: "transparent", color: palette.lineNumberActive },
    // The current line is outlined, not filled.
    ".cm-activeLine": { backgroundColor: "transparent", boxShadow: `inset 0 0 0 2px ${palette.activeLine}` },
    "&.cm-focused .cm-selectionBackground, .cm-selectionBackground, .cm-content ::selection": {
      backgroundColor: palette.selection,
    },
    ".cm-matchingBracket, &.cm-focused .cm-matchingBracket": {
      backgroundColor: "transparent",
      outline: `1px solid ${palette.lineNumber}`,
    },
    "&.cm-focused": { outline: "2px solid var(--color-primary)", outlineOffset: "1px" },
  },
  { dark: true },
);

/** Language, colors and theme for a Sieve editor. */
export const sieveHighlighting = [sieveLanguage, syntaxHighlighting(highlightStyle), theme];
