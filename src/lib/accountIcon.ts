// How an account looks in the account switcher: a picture the user picked,
// or a short text (their initials by default) on a colour of its own.

import type { AccountIcon } from "#tauri/types";
import { labelHue } from "./rules/route";

/** Colours to pick from; the same hues the labels use. */
export const ICON_HUES = [25, 60, 95, 180, 210, 250, 290, 330];

/** "Acme GmbH" → "AG", "HomeServer" → "HS", "nas" → "NA". */
export function initials(label: string): string {
  const words = label.trim().split(/\s+/).filter(Boolean);
  if (words.length > 1) return (words[0][0] + words[1][0]).toUpperCase();
  const w = words[0] ?? "?";
  const caps = w.slice(1).match(/[A-ZÄÖÜ]/);
  return (w[0] + (caps ? caps[0] : (w[1] ?? ""))).toUpperCase();
}

/** The first two characters as one sees them (an emoji counts as one). */
export function shortText(text: string): string {
  const parts = [...new Intl.Segmenter(undefined, { granularity: "grapheme" }).segment(text.trim())].map((s) => s.segment);
  return parts.slice(0, 2).join("");
}

export function iconText(label: string, icon: AccountIcon | null | undefined): string {
  return icon?.text ? shortText(icon.text) : initials(label);
}

export function iconHue(label: string, icon: AccountIcon | null | undefined): number {
  return icon?.hue ?? labelHue(label);
}

/** A picture as a square 128px PNG (cropped to its middle), small enough to keep with the account. */
export async function iconImage(file: File): Promise<string> {
  const url = URL.createObjectURL(file);
  try {
    const img = new Image();
    img.src = url;
    await img.decode();
    const side = Math.min(img.naturalWidth, img.naturalHeight);
    const canvas = document.createElement("canvas");
    canvas.width = canvas.height = 128;
    const ctx = canvas.getContext("2d");
    if (!ctx) throw new Error("no canvas");
    ctx.imageSmoothingQuality = "high";
    ctx.drawImage(img, (img.naturalWidth - side) / 2, (img.naturalHeight - side) / 2, side, side, 0, 0, 128, 128);
    return canvas.toDataURL("image/png");
  } finally {
    URL.revokeObjectURL(url);
  }
}
