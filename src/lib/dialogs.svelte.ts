// In-app replacements for alert/confirm/prompt, which silently do nothing in
// Tauri's macOS webview (WKWebView). Rendered by AppDialog.svelte.

export type DialogRequest =
  | { kind: "confirm"; message: string; confirmLabel: string; danger: boolean; resolve: (ok: boolean) => void }
  | { kind: "prompt"; message: string; value: string; confirmLabel: string; resolve: (value: string | null) => void }
  | { kind: "alert"; message: string; resolve: () => void };

export const dialogs = $state<{ current: DialogRequest | null }>({ current: null });

/** Shows a request, cancelling one that is still open. */
function show(request: DialogRequest) {
  if (dialogs.current) cancel(dialogs.current);
  dialogs.current = request;
}

export function cancel(request: DialogRequest) {
  if (dialogs.current === request) dialogs.current = null;
  if (request.kind === "confirm") request.resolve(false);
  else if (request.kind === "prompt") request.resolve(null);
  else request.resolve();
}

/** Asks a yes/no question; resolves to whether the user confirmed. */
export function ask(message: string, confirmLabel: string, danger = false): Promise<boolean> {
  return new Promise((resolve) => show({ kind: "confirm", message, confirmLabel, danger, resolve }));
}

/** Asks for a line of text; resolves to the trimmed text, or null if cancelled or empty. */
export function askText(message: string, confirmLabel: string, value = ""): Promise<string | null> {
  return new Promise((resolve) =>
    show({ kind: "prompt", message, value, confirmLabel, resolve: (v) => resolve(v?.trim() || null) }),
  );
}

/** Shows a message. */
export function tell(message: string): Promise<void> {
  return new Promise((resolve) => show({ kind: "alert", message, resolve }));
}
