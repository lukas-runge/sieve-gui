import { t } from "./i18n.svelte";
import type { Diagnostic } from "#tauri/types";

// Mirrors `AppError` in src-tauri/src/error.rs (tauri-ts-generator does not
// export error types of command results).
export type ErrorKind =
  | "connection"
  | "tls"
  | "auth"
  | "server"
  | "conflict"
  | "parse"
  | "keychain"
  | "config"
  | "notFound";

export interface AppError {
  kind: ErrorKind;
  message: string;
  diagnostics: Diagnostic[];
}

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
    case "auth":
      return t("error.auth");
    default:
      return t("error.generic", { message: e.message });
  }
}
