// Regular expressions of the Sieve `regex` extension as Dovecot/Pigeonhole
// evaluates them: POSIX extended regular expressions (regcomp with
// REG_EXTENDED), case-insensitive with the default comparator
// `i;ascii-casemap`. glibc adds `\w \W \s \S \b \B \< \>`.
//
// `checkRegex` reports what POSIX doesn't have (e.g. `\d`, lookaheads) with
// an understandable message; `toRegExp` translates an expression to a
// JavaScript RegExp for trying it out.

export type RegexProblem =
  | { kind: "perlClass"; escape: string; instead: string }
  | { kind: "groupSyntax" }
  | { kind: "lazy" }
  | { kind: "escape"; escape: string }
  | { kind: "unknownClass"; name: string }
  | { kind: "syntax"; message: string };

const POSIX_CLASSES: Record<string, string> = {
  alpha: "a-zA-Z",
  digit: "0-9",
  alnum: "a-zA-Z0-9",
  upper: "A-Z",
  lower: "a-z",
  space: " \\t\\n\\r\\f\\v",
  blank: " \\t",
  punct: "!-\\/:-@\\[-`{-~",
  xdigit: "0-9A-Fa-f",
  graph: "!-~",
  print: " -~",
  cntrl: "\\x00-\\x1f\\x7f",
};

/** Perl classes POSIX lacks, with what to write instead. */
const PERL_CLASSES: Record<string, string> = { d: "[0-9]", D: "[^0-9]", h: "[[:blank:]]", H: "[^[:blank:]]" };
/** Escapes glibc understands outside brackets, as JavaScript writes them. */
const GNU_ESCAPES: Record<string, string> = { w: "\\w", W: "\\W", s: "\\s", S: "\\S", b: "\\b", B: "\\B", "<": "\\b", ">": "\\b" };

type Translation = { regexp: RegExp } | { problem: RegexProblem };

function translate(pattern: string): Translation {
  let out = "";
  let i = 0;
  const n = pattern.length;
  while (i < n) {
    const c = pattern[i];
    if (c === "\\") {
      const next = pattern[i + 1];
      if (next === undefined) return { problem: { kind: "syntax", message: "\\" } };
      if (next in PERL_CLASSES) return { problem: { kind: "perlClass", escape: `\\${next}`, instead: PERL_CLASSES[next] } };
      if (next in GNU_ESCAPES) out += GNU_ESCAPES[next];
      else if (/[0-9]/.test(next)) out += `\\${next}`; // back reference
      else if (/[a-zA-Z]/.test(next)) return { problem: { kind: "escape", escape: `\\${next}` } };
      else out += `\\${next}`; // a literal character
      i += 2;
    } else if (c === "(" && pattern[i + 1] === "?") {
      return { problem: { kind: "groupSyntax" } };
    } else if ((c === "*" || c === "+" || c === "?" || c === "}") && pattern[i + 1] === "?") {
      return { problem: { kind: "lazy" } };
    } else if (c === "{" && pattern[i + 1] === ",") {
      out += "{0"; // `{,n}` (glibc) is `{0,n}`
      i += 1;
    } else if (c === "[") {
      // A bracket expression: backslashes are literal in POSIX.
      let j = i + 1;
      let cls = "[";
      if (pattern[j] === "^") {
        cls += "^";
        j++;
      }
      if (pattern[j] === "]") {
        cls += "\\]";
        j++;
      }
      let closed = false;
      while (j < n) {
        const d = pattern[j];
        if (d === "]") {
          closed = true;
          j++;
          break;
        }
        if (d === "[" && pattern[j + 1] === ":") {
          const end = pattern.indexOf(":]", j + 2);
          if (end < 0) return { problem: { kind: "syntax", message: "[:" } };
          const name = pattern.slice(j + 2, end);
          if (!(name in POSIX_CLASSES)) return { problem: { kind: "unknownClass", name } };
          cls += POSIX_CLASSES[name];
          j = end + 2;
        } else if (d === "[" && (pattern[j + 1] === "=" || pattern[j + 1] === ".")) {
          return { problem: { kind: "unknownClass", name: pattern.slice(j, j + 2) } };
        } else {
          cls += d === "\\" || d === "[" ? `\\${d}` : d;
          j++;
        }
      }
      if (!closed) return { problem: { kind: "syntax", message: "[" } };
      out += cls + "]";
      i = j;
    } else {
      out += c;
      i++;
    }
  }
  try {
    return { regexp: new RegExp(out, "i") };
  } catch (e) {
    return { problem: { kind: "syntax", message: e instanceof Error ? e.message : String(e) } };
  }
}

/** What's wrong with an expression, or null if the server accepts it. */
export function checkRegex(pattern: string): RegexProblem | null {
  const result = translate(pattern);
  return "problem" in result ? result.problem : null;
}

/** The expression as a JavaScript RegExp (case-insensitive), or null if invalid. */
export function toRegExp(pattern: string): RegExp | null {
  const result = translate(pattern);
  return "regexp" in result ? result.regexp : null;
}

/** Whether any of the expressions matches `text`; null if one is invalid. */
export function regexMatches(patterns: string[], text: string): boolean | null {
  let any = false;
  for (const p of patterns) {
    if (p === "") continue;
    const re = toRegExp(p);
    if (!re) return null;
    if (re.test(text)) any = true;
  }
  return any;
}
