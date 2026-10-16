// Folder names in scripts. Roundcube writes them either as UTF-8 ("Bücher")
// or, by default, as IMAP modified UTF-7 ("B&APw-cher", RFC 3501 §5.1.3).
// Names are kept exactly as the script has them; the UI shows them readable
// and writes new names in the encoding the script already uses.

const BASE64 = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+,";

export type MailboxEncoding = "utf8" | "utf7";

/** The folders a script uses, for folder inputs. */
export interface ScriptFolders {
  /** Readable names, as suggestions. */
  labels: string[];
  /** How new names are written. */
  encoding: MailboxEncoding;
}

/**
 * Decodes modified UTF-7. Returns null unless `name` is strictly valid and
 * contains at least one encoded character, so plain names such as "Q&A"
 * stay as they are.
 */
export function decodeModifiedUtf7(name: string): string | null {
  let out = "";
  let encoded = false;
  let i = 0;
  while (i < name.length) {
    const c = name[i];
    const code = c.charCodeAt(0);
    if (code < 0x20 || code > 0x7e) return null;
    if (c !== "&") {
      out += c;
      i++;
      continue;
    }
    const end = name.indexOf("-", i + 1);
    if (end < 0) return null;
    const chunk = name.slice(i + 1, end);
    i = end + 1;
    if (chunk === "") {
      out += "&";
      encoded = true;
      continue;
    }
    const units = decodeBase64Utf16(chunk);
    if (!units) return null;
    out += String.fromCharCode(...units);
    encoded = true;
  }
  return encoded && out.isWellFormed() ? out : null;
}

function decodeBase64Utf16(chunk: string): number[] | null {
  let bits = 0;
  let bitCount = 0;
  const units: number[] = [];
  for (const c of chunk) {
    const v = BASE64.indexOf(c);
    if (v < 0) return null;
    bits = (bits << 6) | v;
    bitCount += 6;
    if (bitCount >= 16) {
      bitCount -= 16;
      const unit = (bits >> bitCount) & 0xffff;
      bits &= (1 << bitCount) - 1;
      // Printable ASCII must not be encoded.
      if (unit >= 0x20 && unit <= 0x7e) return null;
      units.push(unit);
    }
  }
  // Leftover bits are padding: fewer than one base64 character, all zero.
  if (bitCount >= 6 || bits !== 0 || units.length === 0) return null;
  return units;
}

export function encodeModifiedUtf7(name: string): string {
  let out = "";
  let pending: number[] = [];
  const flush = () => {
    if (pending.length === 0) return;
    let bits = 0;
    let bitCount = 0;
    out += "&";
    for (const unit of pending) {
      bits = (bits << 16) | unit;
      bitCount += 16;
      while (bitCount >= 6) {
        bitCount -= 6;
        out += BASE64[(bits >> bitCount) & 0x3f];
      }
      bits &= (1 << bitCount) - 1;
    }
    if (bitCount > 0) out += BASE64[(bits << (6 - bitCount)) & 0x3f];
    out += "-";
    pending = [];
  };
  for (let i = 0; i < name.length; i++) {
    const code = name.charCodeAt(i);
    if (code >= 0x20 && code <= 0x7e) {
      flush();
      out += code === 0x26 ? "&-" : name[i];
    } else {
      pending.push(code);
    }
  }
  flush();
  return out;
}

/** The folder name as the user should read it. */
export function mailboxLabel(raw: string): string {
  return decodeModifiedUtf7(raw) ?? raw;
}

/** UTF-7 if any of the script's folder names is written that way. */
export function detectMailboxEncoding(rawNames: Iterable<string>): MailboxEncoding {
  for (const raw of rawNames) {
    if (decodeModifiedUtf7(raw) !== null) return "utf7";
  }
  return "utf8";
}

/** The name to write into the script for a name the user typed. */
export function mailboxRaw(label: string, encoding: MailboxEncoding): string {
  return encoding === "utf7" ? encodeModifiedUtf7(label) : label;
}
