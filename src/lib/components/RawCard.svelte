<script lang="ts">
  import { closesMenu } from "../menu";
  // A part of the script the rule model can't represent, kept verbatim.
  import { importRules } from "#tauri/commands";
  import type { ScriptEditor } from "../editor.svelte";
  import { describeError } from "../api";
  import { t } from "../i18n.svelte";
  import CodeEditor from "./CodeEditor.svelte";

  let {
    code,
    index,
    editor,
    editing,
    onedit,
    onmove,
    ondelete,
    dragHandle,
  }: {
    code: string;
    index: number;
    editor: ScriptEditor;
    editing: boolean;
    onedit: (editing: boolean) => void;
    onmove: (delta: number) => void;
    ondelete: () => void;
    dragHandle: import("svelte").Snippet<[number]>;
  } = $props();

  let syntaxError = $state<string | null>(null);

  async function update(next: string) {
    editor.edit((rules) => {
      const item = rules.items[index];
      if (item?.type === "raw") item.code = next;
    }, true);
    try {
      await importRules(next);
      syntaxError = null;
    } catch (e) {
      syntaxError = describeError(e);
    }
  }
</script>

<article class="rounded-box border border-dashed border-base-content/30 bg-base-100">
  <header class="flex items-center gap-2 px-3 pt-2.5">
    {@render dragHandle(index)}
    <h3 class="grow font-semibold">{t("raw.heading")}</h3>
    <button class="btn btn-sm {editing ? 'btn-primary' : 'btn-ghost'}" onclick={() => onedit(!editing)}>
      {editing ? t("raw.done") : t("raw.edit")}
    </button>
    <div class="dropdown dropdown-end">
      <div tabindex="0" role="button" class="btn btn-ghost btn-sm btn-square" aria-label={t("menu.more")}>⋯</div>
      <ul tabindex="-1" {@attach closesMenu} class="dropdown-content menu z-10 w-48 rounded-box border border-base-300 bg-base-100 p-1">
        <li><button onclick={() => onmove(-1)}>{t("rule.moveUp")}</button></li>
        <li><button onclick={() => onmove(1)}>{t("rule.moveDown")}</button></li>
        <li><button class="text-error" onclick={ondelete}>{t("rule.delete")}</button></li>
      </ul>
    </div>
  </header>
  <div class="grid gap-2 px-3 pb-3 pt-1.5">
    <p class="text-sm text-base-content/70">{t("raw.explain")}</p>
    {#if editing}
      <CodeEditor value={code} label={t("raw.heading")} onchange={update} />
      {#if syntaxError}
        <p role="alert" class="text-sm text-error">{t("raw.invalid", { message: syntaxError })}</p>
      {/if}
    {:else}
      <pre class="overflow-x-auto rounded-field bg-base-200 p-2 font-mono text-sm">{code}</pre>
    {/if}
  </div>
</article>
