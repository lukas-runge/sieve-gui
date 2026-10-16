// State of the script being edited: the rule cards, undo history and the
// generated Sieve code.

import { exportRules, importRules, isRoundcubeFormat, roundcubeRisks } from "#tauri/commands";
import type { RoundcubeRisks, RuleItem, RuleSet } from "#tauri/types";

/** A rule item with a stable id for keyed lists (ignored by the backend). */
export type EditorItem = RuleItem & { uid: string };

export interface EditorRuleSet extends Omit<RuleSet, "items"> {
  items: EditorItem[];
}

let nextUid = 0;
export function uid(): string {
  return `i${++nextUid}`;
}

function withUids(set: RuleSet): EditorRuleSet {
  return { ...set, items: set.items.map((item) => ({ ...item, uid: uid() })) };
}

const HISTORY_LIMIT = 200;
/** Edits within this time (e.g. typing) form one undo step. */
const MERGE_MS = 800;

export class ScriptEditor {
  readonly accountId: string;
  name = $state("");
  /** The script text on the server, or null for a script not saved yet. */
  original = $state<string | null>(null);
  rules = $state<EditorRuleSet>({ preamble: null, requires: [], singleFolder: false, items: [] });
  /** The generated script for the current rules. */
  code = $state("");
  codeError = $state<string | null>(null);
  /** Whether the server version is in Roundcube's format. */
  roundcube = $state(false);
  /** What Roundcube would drop of the current rules (only if `roundcube`). */
  roundcubeRisks = $state<RoundcubeRisks | null>(null);

  #past: string[] = [];
  #future: string[] = [];
  #lastEdit = 0;
  canUndo = $state(false);
  canRedo = $state(false);

  dirty = $derived(this.original === null || this.code !== this.original);

  private constructor(accountId: string, name: string, original: string | null, rules: RuleSet) {
    this.accountId = accountId;
    this.name = name;
    this.original = original;
    this.rules = withUids(rules);
  }

  static async open(accountId: string, name: string, source: string | null): Promise<ScriptEditor> {
    const rules = await importRules(source ?? "");
    const editor = new ScriptEditor(accountId, name, source, rules);
    editor.roundcube = source !== null && (await isRoundcubeFormat(source));
    await editor.regenerate();
    return editor;
  }

  /** Records the state before an edit; call it before mutating `rules`. */
  checkpoint(merge = false) {
    const now = Date.now();
    if (!(merge && now - this.#lastEdit < MERGE_MS)) {
      this.#past.push(JSON.stringify($state.snapshot(this.rules)));
      if (this.#past.length > HISTORY_LIMIT) this.#past.shift();
      this.#future = [];
    }
    this.#lastEdit = now;
    this.#updateFlags();
  }

  /** Applies an edit with an undo step and regenerates the code. */
  edit(fn: (rules: EditorRuleSet) => void, merge = false) {
    this.checkpoint(merge);
    fn(this.rules);
    void this.regenerate();
  }

  undo() {
    const prev = this.#past.pop();
    if (prev === undefined) return;
    this.#future.push(JSON.stringify($state.snapshot(this.rules)));
    this.rules = JSON.parse(prev);
    this.#lastEdit = 0;
    this.#updateFlags();
    void this.regenerate();
  }

  redo() {
    const next = this.#future.pop();
    if (next === undefined) return;
    this.#past.push(JSON.stringify($state.snapshot(this.rules)));
    this.rules = JSON.parse(next);
    this.#lastEdit = 0;
    this.#updateFlags();
    void this.regenerate();
  }

  #updateFlags() {
    this.canUndo = this.#past.length > 0;
    this.canRedo = this.#future.length > 0;
  }

  #generation = 0;
  async regenerate() {
    const generation = ++this.#generation;
    try {
      const rules = $state.snapshot(this.rules) as RuleSet;
      const [code, risks] = await Promise.all([exportRules(rules), this.roundcube ? roundcubeRisks(rules) : null]);
      if (generation === this.#generation) {
        this.code = code;
        this.codeError = null;
        this.roundcubeRisks = risks;
      }
    } catch (e) {
      if (generation === this.#generation) this.codeError = String(e);
    }
  }

  /** After a successful save, the current code is what the server has. */
  markSaved() {
    this.original = this.code;
  }
}
