// Accounts, server connections and the open script.

import {
  activateScript,
  connectAccount,
  deleteAccount,
  deleteScript,
  deployScript,
  exportRules,
  getScript,
  importRules,
  listAccounts,
  listFolders,
  listScripts,
  parseRoot,
  renameScript,
  rootScript,
  saveAccount,
} from "#tauri/commands";
import type { Account, AccountInput, Folder, Root, ServerInfo } from "#tauri/types";
import { describeError } from "./api";
import { ask, tell } from "./dialogs.svelte";
import { t } from "./i18n.svelte";
import { ScriptEditor } from "./editor.svelte";

export interface ServerState {
  status: "connecting" | "ready" | "error";
  info: ServerInfo | null;
  error: string | null;
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

/** localStorage key of the sidebar order of scripts, per account. */
const ORDER_KEY = "scriptOrder";

export interface FolderState {
  status: "loading" | "ready" | "error";
  folders: Folder[];
  error: string | null;
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
  /** The account whose root script is shown (read-only) instead of an editor. */
  rootView = $state<string | null>(null);
  /** Per account: script names in the order the user arranged them. */
  #order = $state<Record<string, string[]>>(JSON.parse(localStorage.getItem(ORDER_KEY) ?? "{}"));
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

  async connect(id: string) {
    this.servers[id] = { status: "connecting", info: this.servers[id]?.info ?? null, error: null };
    try {
      const info = await connectAccount(id);
      await this.#loadRoot(id, info);
      this.servers[id] = { status: "ready", info, error: null };
    } catch (e) {
      this.servers[id] = { status: "error", info: null, error: describeError(e) };
      return;
    }
    void this.loadFolders(id);
    await this.#applySettings(id);
  }

  async loadFolders(id: string) {
    if (!this.accounts.find((a) => a.id === id)?.imap) {
      delete this.folders[id];
      return;
    }
    this.folders[id] = { status: "loading", folders: this.folders[id]?.folders ?? [], error: null };
    try {
      this.folders[id] = { status: "ready", folders: await listFolders(id), error: null };
    } catch (e) {
      this.folders[id] = { status: "error", folders: [], error: describeError(e) };
    }
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
      lastMoveWins: !!account?.lastMoveWins && !!supported?.lastMoveWins,
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
    if (!needed && !root) return;
    const affected = root ? root.scripts : info.scripts.filter((s) => s.active).map((s) => s.name);
    if (!(await this.#canRewrite(id, affected))) return;
    try {
      if (needed && !root) {
        // A script of that name that is not a root (or not active) is the user's.
        if (info.scripts.some((s) => s.name === ROOT_SCRIPT) && !(await parseRoot(await getScript(id, ROOT_SCRIPT)))) {
          this.error = t("root.nameTaken", { name: ROOT_SCRIPT });
          return;
        }
        const current = info.scripts.find((s) => s.active)?.name;
        const scripts = current ? [current] : [];
        await deployScript(id, ROOT_SCRIPT, await rootScript(scripts, settings.lastMoveWins), null);
        await activateScript(id, ROOT_SCRIPT);
        if (settings.lastMoveWins) for (const name of scripts) await this.#setSingleFolder(id, name, true);
      } else if (needed && root) {
        // Only one script may stay active: the first.
        const scripts = settings.multipleScripts ? root.scripts : root.scripts.slice(0, 1);
        if (settings.lastMoveWins && !root.lastMoveWins) {
          await this.#writeRoot(id, scripts, true);
          for (const name of scripts) await this.#setSingleFolder(id, name, true);
        } else if (!settings.lastMoveWins && root.lastMoveWins) {
          for (const name of root.scripts) await this.#setSingleFolder(id, name, false);
          await this.#writeRoot(id, scripts, false);
        } else if (scripts.length !== root.scripts.length) {
          await this.#writeRoot(id, scripts, root.lastMoveWins);
        } else {
          return;
        }
      } else if (root) {
        // Back to a single active script, the first of the root.
        for (const name of root.scripts) await this.#setSingleFolder(id, name, false);
        await activateScript(id, root.scripts[0] ?? null);
        await deleteScript(id, ROOT_SCRIPT);
      }
    } catch (e) {
      this.error = describeError(e);
    }
    await this.refreshScripts(id);
  }

  async saveAccount(input: AccountInput): Promise<Account> {
    const account = await saveAccount(input);
    await this.loadAccounts();
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
    if (this.rootView === id) this.rootView = null;
    await this.loadAccounts();
  }

  /** Whether switching away from the open script is fine (asks if dirty). */
  async confirmLeave(): Promise<boolean> {
    if (!this.editor?.dirty) return true;
    return ask(t("editor.discardConfirm", { name: this.editor.name }), t("editor.discard"), true);
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
      this.editor = await ScriptEditor.open(id, name, source);
    } catch (e) {
      this.error = describeError(e);
    } finally {
      this.loadingScript = false;
    }
  }

  async newScript(id: string, name: string) {
    this.rootView = null;
    this.editor = await ScriptEditor.open(id, name, null);
  }

  /**
   * Makes a script active. If several may be active, it runs where it stands
   * in the list; otherwise it replaces the active one. `null` deactivates
   * all scripts (without a root script).
   */
  async activate(id: string, name: string | null) {
    const root = this.roots[id];
    if (name !== null && !(await this.#canRewrite(id, [name]))) return;
    try {
      // Every step leaves a working setup: a script not yet in single-folder
      // mode just files messages directly.
      if (root && name !== null) {
        if (!root.scripts.includes(name)) {
          let scripts = [name];
          if (this.rootSettings(id).multipleScripts) {
            const order = this.scriptOrder(id);
            if (!order.includes(name)) order.push(name);
            const running = order.filter((s) => s === name || root.scripts.includes(s));
            scripts = [...running, ...root.scripts.filter((s) => !running.includes(s))];
          }
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
   * The account's scripts (without the root script) in the order the user
   * arranged them. Active scripts of a root appear in the order they run;
   * inactive ones keep their place in between.
   */
  scriptOrder(id: string): string[] {
    const root = this.roots[id];
    const names = (this.servers[id]?.info?.scripts ?? []).map((s) => s.name).filter((n) => !root || n !== ROOT_SCRIPT);
    const saved = (this.#order[id] ?? []).filter((n) => names.includes(n));
    const list = [...saved, ...names.filter((n) => !saved.includes(n))];
    if (!root) return list;
    // The root decides the run order (it may have changed elsewhere), so the
    // active scripts fill the places of active scripts in that order.
    const running = root.scripts.filter((n) => list.includes(n));
    let next = 0;
    return list.map((n) => (root.scripts.includes(n) ? running[next++] : n));
  }

  /** Arranges the scripts; with a root, the active ones then run in this order. */
  async reorderScripts(id: string, names: string[]) {
    this.#order[id] = names;
    localStorage.setItem(ORDER_KEY, JSON.stringify(this.#order));
    const root = this.roots[id];
    if (root) await this.#reorderActive(id, names.filter((n) => root.scripts.includes(n)));
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

  async #writeRoot(id: string, scripts: string[], lastMoveWins: boolean) {
    const source = await rootScript(scripts, lastMoveWins);
    await deployScript(id, ROOT_SCRIPT, source, null);
    this.roots[id] = { scripts, lastMoveWins };
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

  /** Rewrites a script on the server in or out of single-folder mode. */
  async #setSingleFolder(id: string, name: string, on: boolean) {
    const source = await getScript(id, name);
    const rules = await importRules(source);
    if (rules.singleFolder === on) return;
    rules.singleFolder = on;
    const code = await exportRules(rules);
    await deployScript(id, name, code, source);
    if (this.editor?.accountId === id && this.editor.name === name) {
      this.editor = await ScriptEditor.open(id, name, code);
    }
  }

  async rename(id: string, name: string, newName: string) {
    try {
      await renameScript(id, name, newName);
      const order = this.#order[id];
      if (order?.includes(name)) {
        this.#order[id] = order.map((s) => (s === name ? newName : s));
        localStorage.setItem(ORDER_KEY, JSON.stringify(this.#order));
      }
      if (this.editor?.accountId === id && this.editor.name === name) this.editor.name = newName;
      const root = this.roots[id];
      if (root?.scripts.includes(name)) {
        await this.#writeRoot(id, root.scripts.map((s) => (s === name ? newName : s)), root.lastMoveWins);
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
}

export const session = new Session();
