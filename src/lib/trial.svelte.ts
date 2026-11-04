// Trying the rules with a sample message: the message stays while you switch
// scripts or edit rules, so the route can show its way at once.

import type { RuleSet } from "#tauri/types";
import { EMPTY_MAIL, runTrial, type SampleMail, type Trial, type TrialScript } from "./rules/simulate";
import { session } from "./session.svelte";

class TrialState {
  /** The panel with the sample message is open. */
  open = $state(false);
  mail = $state<SampleMail>({ ...EMPTY_MAIL });
  /** The saved rules of the account's scripts, for those not open. */
  saved = $state<{ account: string; scripts: { name: string; rules: RuleSet }[] } | null>(null);

  /** Something was entered, so the route shows the message's way. */
  readonly active = $derived(Object.values(this.mail).some((v) => v.trim() !== ""));

  /** The scripts the message passes, in order: the running ones, or just the open one if it doesn't run. */
  readonly order = $derived.by(() => {
    const editor = session.editor;
    if (!editor) return [];
    const id = editor.accountId;
    if (!session.runs(id, editor.name)) return [editor.name];
    const root = session.roots[id];
    return root ? root.scripts : session.scriptTree(id).running.filter((n) => n.depth === 0).map((n) => n.name);
  });

  readonly result = $derived.by((): Trial | null => {
    const editor = session.editor;
    if (!editor || !this.active) return null;
    const saved = this.saved?.account === editor.accountId ? this.saved.scripts : [];
    const scripts: TrialScript[] = [];
    for (const name of this.order) {
      const rules = name === editor.name ? editor.rules : saved.find((s) => s.name === name)?.rules;
      if (rules) scripts.push({ name, items: rules.items, singleFolder: rules.singleFolder });
    }
    const root = session.roots[editor.accountId];
    return runTrial(scripts, this.mail, !!root?.lastMoveWins);
  });

  /**
   * Where the message went with the rules as saved, while the open script
   * has unsaved changes: what the edit changes for it.
   */
  readonly before = $derived.by((): Trial | null => {
    const editor = session.editor;
    if (!editor || !this.active || !editor.dirty || this.saved?.account !== editor.accountId) return null;
    const scripts: TrialScript[] = [];
    for (const name of this.order) {
      const rules = this.saved.scripts.find((s) => s.name === name)?.rules;
      if (rules) scripts.push({ name, items: rules.items, singleFolder: rules.singleFolder });
    }
    const root = session.roots[editor.accountId];
    return runTrial(scripts, this.mail, !!root?.lastMoveWins);
  });

  /** Reads the other scripts of the account (again after saving). */
  async load(account: string) {
    const scripts = await session.allRules(account);
    if (session.editor?.accountId === account) this.saved = { account, scripts };
  }

  clear() {
    this.mail = { ...EMPTY_MAIL };
  }
}

export const trial = new TrialState();
