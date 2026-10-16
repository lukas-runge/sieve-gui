const UNITS = [
  { factor: 1 << 30, label: "GB" },
  { factor: 1 << 20, label: "MB" },
  { factor: 1 << 10, label: "KB" },
];

/** A byte count as shown to the user, e.g. "10 MB". */
export function formatSize(bytes: number): string {
  for (const { factor, label } of UNITS) {
    if (bytes >= factor && bytes % factor === 0) return `${bytes / factor} ${label}`;
  }
  return `${bytes} B`;
}

export const SIZE_UNITS = [
  { factor: 1, label: "B" },
  { factor: 1 << 10, label: "KB" },
  { factor: 1 << 20, label: "MB" },
  { factor: 1 << 30, label: "GB" },
] as const;

/** Splits a byte count into the largest exact unit. */
export function splitSize(bytes: number): { amount: number; factor: number } {
  for (const { factor } of [...SIZE_UNITS].reverse()) {
    if (bytes >= factor && bytes % factor === 0) return { amount: bytes / factor, factor };
  }
  return { amount: bytes, factor: 1 };
}

/** A date condition value (`YYYY-MM-DD` or `YYYY-MM-DDTHH:MM`) for display. */
export function formatDate(value: string, locale: string): string {
  const withTime = value.includes("T");
  const d = new Date(withTime ? value : `${value}T00:00`);
  if (Number.isNaN(d.getTime())) return value;
  return d.toLocaleString(locale, withTime ? { dateStyle: "medium", timeStyle: "short" } : { dateStyle: "medium" });
}
