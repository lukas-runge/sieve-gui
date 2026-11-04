// Accounts, server connections and the open script.

import {
  activateScript,
  connectAccount,
  copyStatus,
  resolveCopy,
  deleteAccount,
  deleteScript,
  deployScript,
  exportRules,
  getScript,
  importRules,
  importRulesSingleFolder,
  listAccounts,
  listFolders,
  listScripts,
  parseRoot,
  renameScript,
  rootScript,
  saveAccount,
  scriptLinks,
  setKeywordNames,
  trustCertificate,
} from "#tauri/commands";
import type {
  Account,
  AccountInput,
  CopyStatus,
  Folder,
  Root,
  ScriptLinks,
  ServerInfo,
  Side,
  UntrustedCertificate,
} from "#tauri/types";
import { describeError, untrustedCertificate } from "./api";
import { scriptTree, type ScriptNode } from "./scripts";
import { ask, tell } from "./dialogs.svelte";
import { t } from "./i18n.svelte";
import { ScriptEditor } from "./editor.svelte";

export interface ServerState {
  status: "connecting" | "ready" | "error";
  info: ServerInfo | null;
  error: string | null;
  /** The server's certificate, if it was rejected and can be trusted. */
  certificate: UntrustedCertificate | null;
}

/**
 * Name of the root script sieve-gui creates (`sieve_core::root`). It
 * includes the active scripts if several may be active or the last move
 * wins (account settings).
 */
export const ROOT_SCRIPT = "sieve-gui";

/** The account settings that need a root script, as far as the server supports them. */
export interface RootSettings {
  multipleScripts: boolean;
  lastMoveWins: boolean;
}


export type { ScriptNode };

export interface FolderState {
  status: "loading" | "ready" | "error";
  folders: Folder[];
  error: string | null;
  /** The IMAP server's certificate, if it was rejected and can be trusted. */
  certificate: UntrustedCertificate | null;
}

class Session {
  accounts = $state<Account[]>([]);
  servers = $state<Record<string, ServerState>>({});
  /** IMAP folder lists, only for accounts with IMAP set up. */
  folders = $state<Record<string, FolderState>>({});
  /** Per account: the root script, if it is the active script. */
  roots = $state<Record<string, Root | null>>({});
  /** Per account: the root script's text on the server, if there is a root. */
  rootSources = $state<Record<string, string>>({});
  /**
   * Per account: the root script exists but another script (or none) was
   * made active elsewhere, e.g. by saving filters in SOGo. Holds that script.
   */
  displaced = $state<Record<string, { active: string | null }>>({});
  /**
   * Per account: a script sieve-gui didn't create is active, and the root
   * script would take over. Waits for the user to decide whether that script
   * runs unchanged inside the root or is taken over (see `adopt`).
   */
  adoption = $state<Record<string, { active: string }>>({});
  /** Per account: differences between the server and the local copy, until resolved. */
  copies = $state<Record<string, CopyStatus>>({});
  /** The account whose copy differences are shown. */
  copyPrompt = $state<string | null>(null);
  /** Rejected server certificates to show, one after the other. */
  certificatePrompts = $state<{ accountId: string; untrusted: UntrustedCertificate }[]>([]);
  /** Accounts set up or changed just now: their rejected certificates are shown right away. */
  #promptCertificates = new Set<string>();
  /** Per account and script: what it includes and whether it files messages. */
  links = $state<Record<string, Record<string, ScriptLinks>>>({});
  /** The account whose root script is shown (read-only) instead of an editor. */
  rootView = $state<string | null>(null);
  editor = $state<ScriptEditor | null>(null);
  loadingScript = $state(false);
  error = $state<string | null>(null);

  /** Sieve extensions of the open script's server. */
  extensions = $derived(
    this.editor ? (this.servers[this.editor.accountId]?.info?.capabilities.sieve ?? []) : [],
  );

  async loadAccounts() {
    try {
      this.accounts = await listAccounts();
    } catch (e) {
      this.error = describeError(e);
    }
  }

  async connect(id: string): Promise<void> {
    this.servers[id] = { status: "connecting", info: this.servers[id]?.info ?? null, error: null, certificate: null };
    try {
      const info = await connectAccount(id);
      await this.#loadRoot(id, info);
      this.servers[id] = { status: "ready", info, error: null, certificate: null };
    } catch (e) {
      const certificate = untrustedCertificate(e);
      if (certificate && (await this.#trustKnown(id, certificate))) return this.connect(id);
      this.servers[id] = { status: "error", info: null, error: describeError(e), certificate };
      if (certificate && this.#promptCertificates.has(id)) this.showCertificate(id, certificate);
      return;
    }
    void this.loadFolders(id);
    void this.#loadLinks(id);
    // Before the root is brought in line with the settings, which would
    // otherwise cover up what changed on the server (e.g. a webmail taking
    // over the active script).
    if (await this.#checkCopy(id)) return;
    await this.#applySettings(id);
  }

  async loadFolders(id: string): Promise<void> {
    if (!this.accounts.find((a) => a.id === id)?.imap) {
      delete this.folders[id];
      return;
    }
    this.folders[id] = { status: "loading", folders: this.folders[id]?.folders ?? [], error: null, certificate: null };
    try {
      this.folders[id] = { status: "ready", folders: await listFolders(id), error: null, certificate: null };
    } catch (e) {
      const certificate = untrustedCertificate(e);
      if (certificate && (await this.#trustKnown(id, certificate))) return this.loadFolders(id);
      this.folders[id] = { status: "error", folders: [], error: describeError(e), certificate };
      if (certificate && this.#promptCertificates.has(id)) this.showCertificate(id, certificate);
    }
  }

  /**
   * Compares the server with the local copy. Returns whether they differ;
   * the settings are then applied once the user resolved the differences.
   */
  async #checkCopy(id: string): Promise<boolean> {
    try {
      const status = await copyStatus(id);
      if (status.differences.length === 0 && !status.active) {
        delete this.copies[id];
        return false;
      }
      this.copies[id] = status;
      this.copyPrompt ??= id;
      return true;
    } catch (e) {
      this.error = describeError(e);
      return false;
    }
  }

  /** Applies the user's decisions on the copy differences of an account. */
  async resolveCopy(id: string, choices: { name: string; keep: Side }[], active: Side | null) {
    try {
      await resolveCopy(id, choices, active);
    } catch (e) {
      this.error = describeError(e);
      return;
    }
    delete this.copies[id];
    if (this.copyPrompt === id) this.copyPrompt = null;
    this.links[id] = {};
    await this.refreshScripts(id);
    // An open script may have been replaced.
    const editor = this.editor;
    if (editor?.accountId === id && !editor.dirty && choices.some((c) => c.name === editor.name)) {
      await this.openScript(id, editor.name);
    }
    await this.#applySettings(id);
  }

  /** Names of the account's custom keywords. */
  keywordNames(id: string): Record<string, string> {
    return this.accounts.find((a) => a.id === id)?.keywordNames ?? {};
  }

  /** Names a keyword of the account (`null` removes the name). */
  async setKeywordName(id: string, keyword: string, name: string | null) {
    const names = { ...this.keywordNames(id) };
    if (name) names[keyword] = name;
    else delete names[keyword];
    try {
      const account = await setKeywordNames(id, names);
      this.accounts = this.accounts.map((a) => (a.id === id ? account : a));
    } catch (e) {
      this.error = describeError(e);
    }
  }

  /** Shows a rejected certificate (after those already waiting). */
  showCertificate(accountId: string, untrusted: UntrustedCertificate) {
    const waiting = this.certificatePrompts.some(
      (p) => p.accountId === accountId && p.untrusted.service === untrusted.service,
    );
    if (!waiting) this.certificatePrompts = [...this.certificatePrompts, { accountId, untrusted }];
  }

  /** Done with the certificate shown first. */
  closeCertificate() {
    this.certificatePrompts = this.certificatePrompts.slice(1);
  }

  /**
   * Trusts a certificate for one of the account's servers if the user already
   * trusts exactly this certificate for the other one (often the same server).
   * A certificate replacing a trusted one is always shown instead.
   */
  async #trustKnown(id: string, untrusted: UntrustedCertificate): Promise<boolean> {
    const account = this.accounts.find((a) => a.id === id);
    if (!account || untrusted.pinned) return false;
    const other = untrusted.service === "manageSieve" ? account.imap?.certificate : account.certificate;
    if (other !== untrusted.certificate.sha256) return false;
    try {
      await trustCertificate(id, untrusted.service, other);
      await this.loadAccounts();
      return true;
    } catch {
      return false;
    }
  }

  /** Trusts a rejected server certificate of the account and connects again. */
  async trustCertificate(id: string, untrusted: UntrustedCertificate) {
    try {
      await trustCertificate(id, untrusted.service, untrusted.certificate.sha256);
      await this.loadAccounts();
    } catch (e) {
      this.error = describeError(e);
      return;
    }
    if (untrusted.service === "manageSieve") await this.connect(id);
    else await this.loadFolders(id);
  }

  async refreshScripts(id: string) {
    const server = this.servers[id];
    if (!server?.info) return this.connect(id);
    try {
      server.info.scripts = await listScripts(id);
      await this.#loadRoot(id, server.info);
    } catch (e) {
      this.error = describeError(e);
    }
    void this.#loadLinks(id);
  }

  /**
   * Reads what the account's scripts include and whether they file messages
   * themselves. Scripts already read are kept until `scriptChanged`.
   */
  async #loadLinks(id: string) {
    const names = (this.servers[id]?.info?.scripts ?? []).map((s) => s.name).filter((n) => n !== ROOT_SCRIPT);
    const known = this.links[id] ?? {};
    const next: Record<string, ScriptLinks> = {};
    for (const name of names) {
      try {
        next[name] = known[name] ?? (await scriptLinks(await getScript(id, name)));
      } catch {
        // Not readable right now; shown without links.
      }
    }
    this.links[id] = next;
  }

  /** Forgets what is known about a script after it was written. */
  scriptChanged(id: string, name: string) {
    if (this.links[id]) delete this.links[id][name];
  }

  /** The scripts that include `name`. */
  includedBy(id: string, name: string): string[] {
    return Object.entries(this.links[id] ?? {})
      .filter(([other, links]) => other !== name && links.includes.includes(name))
      .map(([other]) => other);
  }

  async #loadRoot(id: string, info: ServerInfo) {
    let root: Root | null = null;
    let source = "";
    try {
      if (info.scripts.some((s) => s.active && s.name === ROOT_SCRIPT)) {
        source = await getScript(id, ROOT_SCRIPT);
        root = await parseRoot(source);
      }
    } catch {
      // Then it's shown as a plain active script.
    }
    this.roots[id] = root;
    if (root) delete this.displaced[id];
    if (root) this.rootSources[id] = source;
    else delete this.rootSources[id];
  }

  /** Which root settings the account's server supports (null until connected). */
  supported(id: string): RootSettings | null {
    const sieve = this.servers[id]?.info?.capabilities.sieve;
    if (!sieve) return null;
    const include = sieve.includes("include");
    return { multipleScripts: include, lastMoveWins: include && sieve.includes("variables") && sieve.includes("fileinto") };
  }

  /** The account's root settings, as far as its server supports them. */
  rootSettings(id: string): RootSettings {
    const account = this.accounts.find((a) => a.id === id);
    const supported = this.supported(id);
    return {
      multipleScripts: !!account?.multipleScripts && !!supported?.multipleScripts,
      // Roundcube drops the `global` and `return` this needs whenever it
      // saves a script, and then no message is moved at all.
      lastMoveWins: !!account?.lastMoveWins && !!supported?.lastMoveWins && !account?.roundcube,
    };
  }

  /** Whether several scripts can be active, so their order matters. */
  multipleActive(id: string): boolean {
    return !!this.roots[id] && this.rootSettings(id).multipleScripts;
  }

  /**
   * Brings the server in line with the account's root settings: creates,
   * rewrites or removes the root script and switches the included scripts
   * in or out of single-folder mode. In the order used, every step leaves a
   * working setup, so a failure halfway does no harm.
   */
  async #applySettings(id: string) {
    const info = this.servers[id]?.info;
    if (!info) return;
    const settings = this.rootSettings(id);
    const needed = settings.multipleScripts || settings.lastMoveWins;
    const root = this.roots[id];
    delete this.displaced[id];
    delete this.adoption[id];
    if (!needed && !root) return;
    const affected = root ? root.scripts : info.scripts.filter((s) => s.active).map((s) => s.name);
    if (!(await this.#canRewrite(id, affected))) return;
    try {
      if (needed && !root) {
        if (info.scripts.some((s) => s.name === ROOT_SCRIPT)) {
          // A script of that name that is no root is the user's.
          if (!(await parseRoot(await getScript(id, ROOT_SCRIPT)))) {
            this.error = t("root.nameTaken", { name: ROOT_SCRIPT });
            return;
          }
          // Ours, but something else was activated meanwhile. Leave it to the
          // user: taking over again here would undo what they just did there.
          this.displaced[id] = { active: info.scripts.find((s) => s.active)?.name ?? null };
          return;
        }
        const current = info.scripts.find((s) => s.active)?.name;
        if (current) {
          // A script sieve-gui didn't create runs today. Ask before the root
          // takes over (see adopt).
          this.adoption[id] = { active: current };
          return;
        }
        await deployScript(id, ROOT_SCRIPT, await rootScript([], [], settings.lastMoveWins), null);
        await activateScript(id, ROOT_SCRIPT);
      } else if (needed && root) {
        // Only one script may stay active: the first.
        const scripts = settings.multipleScripts ? root.scripts : root.scripts.slice(0, 1);
        // Scripts included unchanged are never rewritten.
        const managed = (names: string[]) => names.filter((n) => !root.unchanged.includes(n));
        if (settings.lastMoveWins && (!root.lastMoveWins || root.damaged)) {
          // A damaged root lost its `global` to another editor (Roundcube
          // drops it on save), and its scripts probably did too.
          await this.#writeRoot(id, scripts, true);
          for (const name of managed(scripts)) await this.#setSingleFolder(id, name, true, root.lastMoveWins);
        } else if (!settings.lastMoveWins && root.lastMoveWins) {
          for (const name of managed(root.scripts)) await this.#setSingleFolder(id, name, false, true);
          await this.#writeRoot(id, scripts, false);
        } else if (scripts.length !== root.scripts.length) {
          await this.#writeRoot(id, scripts, root.lastMoveWins);
        } else if (settings.lastMoveWins) {
          // A script saved by an editor that drops `global` (Roundcube does)
          // records its moves where the root doesn't see them; set it up again.
          let changed = false;
          for (const name of managed(scripts)) changed = (await this.#setSingleFolder(id, name, true, true)) || changed;
          if (!changed) return;
        } else {
          return;
        }
      } else if (root) {
        // Back to a single active script, the first of the root.
        for (const name of root.scripts.filter((n) => !root.unchanged.includes(n))) {
          await this.#setSingleFolder(id, name, false, root.lastMoveWins);
        }
        await activateScript(id, root.scripts[0] ?? null);
        await deleteScript(id, ROOT_SCRIPT);
      }
    } catch (e) {
      this.error = describeError(e);
    }
    await this.refreshScripts(id);
  }

  #roundcube(id: string): boolean {
    return this.accounts.find((a) => a.id === id)?.roundcube ?? false;
  }

  async saveAccount(input: AccountInput): Promise<Account> {
    const account = await saveAccount(input);
    await this.loadAccounts();
    if (this.editor?.accountId === account.id) {
      this.editor.roundcube = account.roundcube;
      void this.editor.regenerate();
    }
    // Certificates of a server set up just now are asked about right away.
    this.#promptCertificates.add(account.id);
    void this.connect(account.id);
    return account;
  }

  async deleteAccount(id: string) {
    await deleteAccount(id);
    if (this.editor?.accountId === id) this.editor = null;
    delete this.servers[id];
    delete this.folders[id];
    delete this.roots[id];
    delete this.rootSources[id];
    delete this.adoption[id];
    delete this.links[id];
    delete this.copies[id];
    if (this.rootView === id) this.rootView = null;
    await this.loadAccounts();
  }

  /** Whether switching away from the open script is fine (asks if dirty). */
  async confirmLeave(): Promise<boolean> {
    if (!this.editor?.dirty) return true;
    return ask(t("editor.discardConfirm", { name: this.editor.name }), t("editor.discard"), true);
  }

  /**
   * Lets the root script take over from the script that is active today
   * (see `adoption`): `unchanged` runs it inside the root as it is, never
   * rewriting it; `takeOver` lets sieve-gui adjust it like its own scripts
   * (single-folder mode with "last move wins").
   */
  async adopt(id: string, mode: "unchanged" | "takeOver") {
    const pending = this.adoption[id];
    if (!pending) return;
    const name = pending.active;
    if (mode === "takeOver" && !(await this.#canRewrite(id, [name]))) return;
    const settings = this.rootSettings(id);
    delete this.adoption[id];
    try {
      const unchanged = mode === "unchanged" ? [name] : [];
      await deployScript(id, ROOT_SCRIPT, await rootScript([name], unchanged, settings.lastMoveWins), null);
      await activateScript(id, ROOT_SCRIPT);
      if (settings.lastMoveWins && mode === "takeOver") await this.#setSingleFolder(id, name, true);
    } catch (e) {
      this.error = describeError(e);
    }
    await this.refreshScripts(id);
  }

  /** Lets sieve-gui adjust a script the root includes unchanged. */
  async takeOver(id: string, name: string) {
    const root = this.roots[id];
    if (!root?.unchanged.includes(name) || !(await this.#canRewrite(id, [name]))) return;
    try {
      if (root.lastMoveWins) await this.#setSingleFolder(id, name, true);
      await this.#writeRoot(id, root.scripts, root.lastMoveWins, root.unchanged.filter((n) => n !== name));
    } catch (e) {
      this.error = describeError(e);
    }
    await this.refreshScripts(id);
  }

  /** Includes a script unchanged again, undoing sieve-gui's adjustments. */
  async leaveUnchanged(id: string, name: string) {
    const root = this.roots[id];
    if (!root?.scripts.includes(name) || root.unchanged.includes(name) || !(await this.#canRewrite(id, [name]))) return;
    try {
      if (root.lastMoveWins) await this.#setSingleFolder(id, name, false, true);
      await this.#writeRoot(id, root.scripts, root.lastMoveWins, [...root.unchanged, name]);
    } catch (e) {
      this.error = describeError(e);
    }
    await this.refreshScripts(id);
  }

  /** Makes the root script active again after something else replaced it. */
  async reactivateRoot(id: string) {
    try {
      await activateScript(id, ROOT_SCRIPT);
      await this.refreshScripts(id);
      await this.#applySettings(id);
    } catch (e) {
      this.error = describeError(e);
    }
  }

  /** Shows the account's root script (read-only) in place of the editor. */
  async viewRoot(id: string) {
    if (this.rootView === id || !(await this.confirmLeave())) return;
    this.editor = null;
    this.rootView = id;
  }

  async openScript(id: string, name: string) {
    this.rootView = null;
    this.loadingScript = true;
    this.error = null;
    try {
      const source = await getScript(id, name);
      this.editor = await ScriptEditor.open(id, name, source, this.#roundcube(id));
    } catch (e) {
      this.error = describeError(e);
    } finally {
      this.loadingScript = false;
    }
  }

  async newScript(id: string, name: string) {
    this.rootView = null;
    this.editor = await ScriptEditor.open(id, name, null, this.#roundcube(id));
  }

  /**
   * Makes a script active. If several may be active, it runs where it stands
   * in the list; otherwise it replaces the active one. `null` deactivates
   * all scripts (without a root script).
   */
  async activate(id: string, name: string | null) {
    if (name !== null && !(await this.#canRewrite(id, [name]))) return;
    // Activating a script means running the root again.
    if (name !== null && this.displaced[id]) await this.reactivateRoot(id);
    // The script that runs today keeps running, unchanged.
    if (name !== null && this.adoption[id]) await this.adopt(id, "unchanged");
    const root = this.roots[id];
    try {
      // Every step leaves a working setup: a script not yet in single-folder
      // mode just files messages directly.
      if (root && name !== null) {
        if (!root.scripts.includes(name)) {
          // A script activated now runs last; the order can be changed in
          // the sidebar.
          const scripts = this.rootSettings(id).multipleScripts ? [...root.scripts, name] : [name];
          await this.#writeRoot(id, scripts, root.lastMoveWins);
          if (root.lastMoveWins) await this.#setSingleFolder(id, name, true);
        }
      } else {
        // A script set up for the root files nothing on its own.
        if (name !== null) await this.#setSingleFolder(id, name, false);
        await activateScript(id, name);
      }
      await this.refreshScripts(id);
    } catch (e) {
      this.error = describeError(e);
    }
  }

  /** Stops running a script. */
  async deactivate(id: string, name: string) {
    const root = this.roots[id];
    if (!root) return this.activate(id, null);
    try {
      await this.#writeRoot(id, root.scripts.filter((s) => s !== name), root.lastMoveWins);
    } catch (e) {
      this.error = describeError(e);
    }
  }

  /**
   * The account's scripts as the sidebar shows them: first the running ones
   * in the order they run, then the others alphabetically. Scripts included
   * by another one follow it, indented (`depth`), unless they run on their
   * own. The root script itself isn't listed.
   */
  scriptTree(id: string): { running: ScriptNode[]; inactive: ScriptNode[] } {
    const root = this.roots[id];
    const managed = !!root || !!this.displaced[id];
    const names = (this.servers[id]?.info?.scripts ?? []).map((s) => s.name).filter((n) => !managed || n !== ROOT_SCRIPT);
    const top = root ? root.scripts : names.filter((n) => this.isActive(id, n));
    const includes = Object.fromEntries(Object.entries(this.links[id] ?? {}).map(([n, l]) => [n, l.includes]));
    return scriptTree(names, top, includes);
  }

  /** Sets the order in which the running scripts run. */
  async reorderRunning(id: string, names: string[]) {
    await this.#reorderActive(id, names);
  }

  /**
   * Sets the order in which the active scripts run. Included scripts not in
   * `names` (e.g. missing on the server) keep running after them.
   */
  async #reorderActive(id: string, names: string[]) {
    const root = this.roots[id];
    if (!root) return;
    const scripts = [...names, ...root.scripts.filter((s) => !names.includes(s))];
    if (scripts.every((s, i) => s === root.scripts[i])) return;
    // Show the new order right away, while the root is being rewritten.
    this.roots[id] = { ...root, scripts };
    try {
      await this.#writeRoot(id, scripts, root.lastMoveWins);
    } catch (e) {
      this.roots[id] = root;
      this.error = describeError(e);
    }
  }

  /** Writes the root; `unchanged` defaults to what the current root has. */
  async #writeRoot(id: string, scripts: string[], lastMoveWins: boolean, unchanged?: string[]) {
    const keep = (unchanged ?? this.roots[id]?.unchanged ?? []).filter((n) => scripts.includes(n));
    const source = await rootScript(scripts, keep, lastMoveWins);
    await deployScript(id, ROOT_SCRIPT, source, null);
    this.roots[id] = { scripts, unchanged: keep, lastMoveWins, damaged: false };
    this.rootSources[id] = source;
  }

  /** Scripts are rewritten on the server; refuse if one has unsaved edits here. */
  async #canRewrite(id: string, names: string[]): Promise<boolean> {
    const editor = this.editor;
    if (editor?.accountId === id && names.includes(editor.name) && editor.dirty) {
      await tell(t("root.saveFirst", { name: editor.name }));
      return false;
    }
    return true;
  }

  /**
   * Rewrites a script on the server in or out of single-folder mode. `wasOn`:
   * the root runs it in single-folder mode, so its moves and stops are read
   * as such even if an editor that drops `global` (Roundcube does) saved it
   * since. Returns whether the script changed.
   */
  async #setSingleFolder(id: string, name: string, on: boolean, wasOn = false): Promise<boolean> {
    const source = await getScript(id, name);
    let rules = await importRules(source);
    if (wasOn && !rules.singleFolder) rules = await importRulesSingleFolder(source);
    else if (rules.singleFolder === on) return false;
    rules.singleFolder = on;
    const code = await exportRules(rules, this.#roundcube(id));
    await deployScript(id, name, code, source);
    this.scriptChanged(id, name);
    if (this.editor?.accountId === id && this.editor.name === name) {
      this.editor = await ScriptEditor.open(id, name, code, this.#roundcube(id));
    }
    return true;
  }

  async rename(id: string, name: string, newName: string) {
    try {
      await renameScript(id, name, newName);
      if (this.editor?.accountId === id && this.editor.name === name) this.editor.name = newName;
      const root = this.roots[id];
      if (root?.scripts.includes(name)) {
        const renamed = (names: string[]) => names.map((s) => (s === name ? newName : s));
        await this.#writeRoot(id, renamed(root.scripts), root.lastMoveWins, renamed(root.unchanged));
      }
      await this.refreshScripts(id);
    } catch (e) {
      this.error = describeError(e);
    }
  }

  async deleteScript(id: string, name: string) {
    try {
      await deleteScript(id, name);
      if (this.editor?.accountId === id && this.editor.name === name) this.editor = null;
      await this.refreshScripts(id);
    } catch (e) {
      this.error = describeError(e);
    }
  }

  /** Whether a script runs: it is the active one, or the root includes it. */
  isActive(id: string, name: string): boolean {
    const root = this.roots[id];
    if (root) return root.scripts.includes(name);
    return this.servers[id]?.info?.scripts.some((s) => s.name === name && s.active) ?? false;
  }

  /** Whether a script's rules run: it is active, or a running script includes it. */
  runs(id: string, name: string): boolean {
    return this.isActive(id, name) || this.scriptTree(id).running.some((n) => n.name === name);
  }
}

export const session = new Session();
