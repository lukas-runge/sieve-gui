// Comparing release versions (tags like `v0.2.0`).

/** `v1.2.3` → [1, 2, 3]; anything after a `-` or `+` is ignored. */
function parts(version: string): number[] {
  return version.replace(/^v/, "").split(/[-+]/)[0].split(".").map((n) => Number.parseInt(n, 10) || 0);
}

/** Whether `a` is a later version than `b`. */
export function isNewer(a: string, b: string): boolean {
  const x = parts(a);
  const y = parts(b);
  for (let i = 0; i < Math.max(x.length, y.length); i++) {
    const d = (x[i] ?? 0) - (y[i] ?? 0);
    if (d !== 0) return d > 0;
  }
  return false;
}
