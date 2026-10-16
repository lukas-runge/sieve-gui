// Accounts, server connections and the open script.

import {
  activateScript,
  connectAccount,
  deleteAccount,
  deleteScript,
  getScript,
  listAccounts,
  listScripts,
  renameScript,
  saveAccount,
} from "#tauri/commands";
import type { Account, AccountInput, ServerInfo } from "#tauri/types";
import { describeError } from "./api";
import { ScriptEditor } from "./editor.svelte";

export interface ServerState {
  status: "connecting" | "ready" | "error";
  info: ServerInfo | null;
  error: string | null;
}

class Session {
  accounts = $state<Account[]>([]);
  servers = $state<Record<string, ServerState>>({});
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
      this.servers[id] = { status: "ready", info, error: null };
    } catch (e) {
      this.servers[id] = { status: "error", info: null, error: describeError(e) };
    }
  }

  async refreshScripts(id: string) {
    const server = this.servers[id];
    if (!server?.info) return this.connect(id);
    try {
      server.info.scripts = await listScripts(id);
    } catch (e) {
      this.error = describeError(e);
    }
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
    await this.loadAccounts();
  }

  /** Whether switching away from the open script is fine (asks if dirty). */
  confirmLeave(message: string): boolean {
    return !this.editor?.dirty || confirm(message);
  }

  async openScript(id: string, name: string) {
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
    this.editor = await ScriptEditor.open(id, name, null);
  }

  async activate(id: string, name: string | null) {
    try {
      await activateScript(id, name);
      await this.refreshScripts(id);
    } catch (e) {
      this.error = describeError(e);
    }
  }

  async rename(id: string, name: string, newName: string) {
    try {
      await renameScript(id, name, newName);
      if (this.editor?.accountId === id && this.editor.name === name) this.editor.name = newName;
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

  isActive(id: string, name: string): boolean {
    return this.servers[id]?.info?.scripts.some((s) => s.name === name && s.active) ?? false;
  }
}

export const session = new Session();
