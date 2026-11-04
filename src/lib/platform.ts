// Where the app runs, for what the window looks like.

/**
 * macOS in the app: the title bar is laid over the window (see
 * tauri.conf.json), so the window's controls sit in its top-left corner and
 * the page draws the bar itself. The browser preview shows it with `?mac`.
 */
export const macTitleBar =
  typeof window !== "undefined" &&
  (("__TAURI_INTERNALS__" in window && /Macintosh|Mac OS X/.test(navigator.userAgent)) ||
    (import.meta.env.DEV && new URLSearchParams(location.search).has("mac")));
