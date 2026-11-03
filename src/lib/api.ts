import { t } from "./i18n.svelte";
import type { AppError, UntrustedCertificate } from "#tauri/types";

export function isAppError(e: unknown): e is AppError {
  return typeof e === "object" && e !== null && "kind" in e && "message" in e;
}

/** A sentence for the user describing an error from a command. */
export function describeError(e: unknown): string {
  if (!isAppError(e)) return String(e);
  switch (e.kind) {
    case "connection":
      return t("error.connection", { message: e.message });
    case "tls":
      return t("error.tls", { message: e.message });
    case "untrustedCertificate":
      return t(e.certificate?.pinned ? "error.certificateChanged" : "error.untrustedCertificate");
    case "auth":
      return t("error.auth");
    default:
      return t("error.generic", { message: e.message });
  }
}

/** The certificate to offer trusting, if the server's certificate was rejected. */
export function untrustedCertificate(e: unknown): UntrustedCertificate | null {
  return isAppError(e) ? e.certificate : null;
}
