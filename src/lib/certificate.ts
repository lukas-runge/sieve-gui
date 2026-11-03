// Server certificates the OS doesn't trust (see managesieve::tls::PinningVerifier).

/** A SHA-256 fingerprint as people compare it: "AB:CD:…". */
export function formatFingerprint(sha256: string): string {
  return (sha256.toUpperCase().match(/[0-9A-F]{2}/g) ?? []).join(":");
}

/** Whether `now` (seconds since the epoch) is outside the validity period. */
export function outsideValidity(notBefore: number | null, notAfter: number | null, now = Date.now() / 1000): boolean {
  return (notBefore !== null && now < notBefore) || (notAfter !== null && now > notAfter);
}
