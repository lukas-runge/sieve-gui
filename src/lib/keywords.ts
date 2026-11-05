// Tags: custom keywords (IMAP flags like `$label1`) with a name and a
// colour, per account.

/** Whether a flag is a custom keyword rather than a system flag (`\Seen`). */
export function isKeyword(flag: string): boolean {
  return flag !== "" && !flag.startsWith("\\");
}

/** A flag as shown: its name if it has one. */
export function flagLabel(flag: string, names: Record<string, string>): string {
  return names[flag] ?? flag;
}

/** A keyword with what the account keeps for it. */
export interface Tag {
  keyword: string;
  /** Empty: shown as the keyword. */
  name: string;
  /** `#rrggbb`, or `null` for a colour derived from the name. */
  color: string | null;
}

/** The account's tags, sorted by name (unnamed ones by keyword, last). */
export function tagList(names: Record<string, string>, colors: Record<string, string>): Tag[] {
  const keywords = new Set([...Object.keys(names), ...Object.keys(colors)]);
  return [...keywords]
    .map((keyword) => ({ keyword, name: names[keyword] ?? "", color: colors[keyword] ?? null }))
    .sort((a, b) => (!a.name === !b.name ? (a.name || a.keyword).localeCompare(b.name || b.keyword) : a.name ? -1 : 1));
}

/** Tags back as the account stores them; tags without a keyword are left out. */
export function tagMaps(tags: Tag[]): { names: Record<string, string>; colors: Record<string, string> } {
  const names: Record<string, string> = {};
  const colors: Record<string, string> = {};
  for (const tag of tags) {
    const keyword = tag.keyword.trim();
    if (!keyword) continue;
    if (tag.name.trim()) names[keyword] = tag.name.trim();
    if (tag.color) colors[keyword] = tag.color;
  }
  return { names, colors };
}

/** A tag as a provider (e.g. MailPlus) keeps it. */
export interface ProvidedTag {
  name: string;
  color: string | null;
  /** `null`: the provider's keyword for it couldn't be told. */
  keyword: string | null;
}

/**
 * Takes a provider's tags over: their names and colours replace those of the
 * same keywords, other tags stay. Returns the tags and what happened.
 */
export function mergeTags(
  tags: Tag[],
  provided: ProvidedTag[],
): { tags: Tag[]; added: number; changed: number; unmatched: string[] } {
  const out = tags.map((t) => ({ ...t }));
  let added = 0;
  let changed = 0;
  const unmatched: string[] = [];
  for (const p of provided) {
    if (!p.keyword) {
      unmatched.push(p.name);
      continue;
    }
    const existing = out.find((t) => t.keyword === p.keyword);
    if (!existing) {
      out.push({ keyword: p.keyword, name: p.name, color: p.color });
      added++;
    } else if (existing.name !== p.name || existing.color !== p.color) {
      existing.name = p.name;
      existing.color = p.color;
      changed++;
    }
  }
  const sorted = tagList(tagMaps(out).names, tagMaps(out).colors);
  // Rows still being typed (no keyword yet) stay at the end.
  return { tags: [...sorted, ...out.filter((t) => !t.keyword.trim())], added, changed, unmatched };
}

// ── Colours ──────────────────────────────────────────────────────────────────

/**
 * A stable, calm hue per label name, so the same label looks the same
 * everywhere. Green (matches, running) and blue (selection, actions) are
 * left out, so a label never looks like a state.
 */
const LABEL_HUES = [25, 60, 95, 180, 210, 290, 330, 0];
export function labelHue(name: string): number {
  let h = 0;
  for (const ch of name) h = (h * 31 + ch.charCodeAt(0)) % 997;
  return LABEL_HUES[h % LABEL_HUES.length];
}

/** The colour a label without its own colour gets, as `#rrggbb` (for a colour picker to start from). */
export function labelHex(name: string): string {
  return oklchToHex(0.62, 0.13, labelHue(name));
}

/** OKLCH to sRGB hex, clipped to the sRGB gamut. */
export function oklchToHex(l: number, c: number, hueDegrees: number): string {
  const h = (hueDegrees * Math.PI) / 180;
  const a = c * Math.cos(h);
  const b = c * Math.sin(h);
  const l_ = (l + 0.3963377774 * a + 0.2158037573 * b) ** 3;
  const m_ = (l - 0.1055613458 * a - 0.0638541728 * b) ** 3;
  const s_ = (l - 0.0894841775 * a - 1.291485548 * b) ** 3;
  const linear = [
    4.0767416621 * l_ - 3.3077115913 * m_ + 0.2309699292 * s_,
    -1.2684380046 * l_ + 2.6097574011 * m_ - 0.3413193965 * s_,
    -0.0041960863 * l_ - 0.7034186147 * m_ + 1.707614701 * s_,
  ];
  return (
    "#" +
    linear
      .map((x) => {
        const v = x <= 0.0031308 ? 12.92 * x : 1.055 * Math.max(x, 0) ** (1 / 2.4) - 0.055;
        return Math.round(Math.min(1, Math.max(0, v)) * 255)
          .toString(16)
          .padStart(2, "0");
      })
      .join("")
  );
}

/**
 * How a label looks: its own colour if it has one (`#rrggbb`), otherwise a
 * hue from its name. `chip`: border and background of the tag, `dot`: its
 * colour dot.
 */
export function labelStyle(name: string, color: string | null | undefined): { chip: string; dot: string } {
  if (color) {
    return {
      chip: `border-color: color-mix(in oklch, ${color} 60%, transparent); background: color-mix(in oklch, ${color} 12%, transparent)`,
      dot: `background: ${color}`,
    };
  }
  const h = labelHue(name);
  return {
    chip: `border-color: oklch(0.65 0.11 ${h} / 0.6); background: oklch(0.7 0.1 ${h} / 0.1)`,
    dot: `background: oklch(0.62 0.13 ${h})`,
  };
}
