// Update notice on Linux (see src-tauri/src/update.rs). macOS and Windows get
// their updates from Homebrew and winget, so the app says nothing there.

import { getAppInfo, installUpdate, updateChannel } from "#tauri/commands";
import type { UpdateChannel } from "#tauri/types";
import { isNewer } from "./version";

const LATEST = "https://api.github.com/repos/lukas-runge/sieve-gui/releases/latest";

export const update = $state<{
  channel: UpdateChannel | null;
  /** The newer version, e.g. "0.2.0", once one is out. */
  available: string | null;
  installing: boolean;
}>({ channel: null, available: null, installing: false });

/** Asks GitHub for the latest release, once at start. Quiet when offline. */
export async function checkForUpdate() {
  update.channel = await updateChannel();
  if (!update.channel) return;
  try {
    const [{ version }, response] = await Promise.all([getAppInfo(), fetch(LATEST, { headers: { Accept: "application/vnd.github+json" } })]);
    if (!response.ok) return;
    const latest = String((await response.json()).tag_name ?? "").replace(/^v/, "");
    if (latest && isNewer(latest, version)) update.available = latest;
  } catch {
    // No network, rate limit: the next start asks again.
  }
}

/** Quits the app; the install script updates it and starts it again. */
export async function startUpdate() {
  update.installing = true;
  try {
    await installUpdate();
  } finally {
    update.installing = false;
  }
}
