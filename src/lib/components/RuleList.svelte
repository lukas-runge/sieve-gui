<script lang="ts">
  import { flip } from "svelte/animate";
  import type { Folder } from "#tauri/types";
  import type { ScriptEditor } from "../editor.svelte";
  import { uid } from "../editor.svelte";
  import { t } from "../i18n.svelte";
  import { scriptFolders, type ScriptFolders } from "../rules/mailbox";
  import { newRule } from "../rules/view";
  import RawCard from "./RawCard.svelte";
  import RuleCard from "./RuleCard.svelte";

  let {
    editor,
    extensions,
    serverFolders,
  }: { editor: ScriptEditor; extensions: string[]; serverFolders: Folder[] | null } = $props();

  let editingUid = $state<string | null>(null);
  let dragFrom = $state<number | null>(null);
  let dropAt = $state<number | null>(null);

  /** Folders on the server and used anywhere in the script, for folder inputs. */
  const folders: ScriptFolders = $derived.by(() => {
    const raw = editor.rules.items.flatMap((item) =>
      item.type === "rule" ? item.actions.flatMap((a) => (a.type === "fileInto" && a.mailbox ? [a.mailbox] : [])) : [],
    );
    return scriptFolders(raw, serverFolders);
  });

  function move(from: number, to: number) {
    const count = editor.rules.items.length;
    if (to < 0 || to >= count || from === to) return;
    editor.edit((rules) => {
      const [item] = rules.items.splice(from, 1);
      rules.items.splice(to, 0, item);
    });
  }

  function duplicate(index: number) {
    editor.edit((rules) => {
      const item = $state.snapshot(rules.items[index]);
      if (item.type === "rule") item.name = item.name ? t("rule.copyOf", { name: item.name }) : "";
      rules.items.splice(index + 1, 0, { ...item, uid: uid() });
    });
  }

  function remove(index: number) {
    editor.edit((rules) => {
      rules.items.splice(index, 1);
    });
  }

  function addRule() {
    const item = { type: "rule" as const, ...newRule(t("rule.new")), uid: uid() };
    editor.edit((rules) => {
      rules.items.push(item);
    });
    editingUid = item.uid;
  }

  function addCode() {
    const item = { type: "raw" as const, code: "", uid: uid() };
    editor.edit((rules) => {
      rules.items.push(item);
    });
    editingUid = item.uid;
  }

  function onkeydown(e: KeyboardEvent, index: number) {
    if (!e.altKey || (e.key !== "ArrowUp" && e.key !== "ArrowDown")) return;
    e.preventDefault();
    const to = index + (e.key === "ArrowUp" ? -1 : 1);
    move(index, to);
    // Keep focus on the moved handle.
    requestAnimationFrame(() => document.querySelector<HTMLElement>(`[data-handle="${to}"]`)?.focus());
  }

  function ondrop(e: DragEvent, index: number) {
    e.preventDefault();
    if (dragFrom !== null) move(dragFrom, dragFrom < index ? index - 1 : index);
    dragFrom = dropAt = null;
  }
</script>

{#snippet handle(index: number)}
  <button
    class="btn btn-ghost btn-xs btn-square cursor-grab text-base-content/50"
    draggable="true"
    data-handle={index}
    aria-label="{t('rule.drag')} (Alt+↑/↓)"
    title="{t('rule.drag')} (Alt+↑/↓)"
    ondragstart={(e) => {
      dragFrom = index;
      e.dataTransfer?.setData("text/plain", String(index));
    }}
    ondragend={() => (dragFrom = dropAt = null)}
    onkeydown={(e) => onkeydown(e, index)}
  >
    ⠿
  </button>
{/snippet}

<div class="grid grid-cols-[minmax(0,1fr)] gap-3" role="list">
  {#each editor.rules.items as item, index (item.uid)}
    <div
      role="listitem"
      animate:flip={{ duration: 150 }}
      class="relative {dragFrom === index ? 'opacity-50' : ''}"
      ondragover={(e) => {
        if (dragFrom === null) return;
        e.preventDefault();
        const rect = e.currentTarget.getBoundingClientRect();
        dropAt = e.clientY < rect.top + rect.height / 2 ? index : index + 1;
      }}
      ondrop={(e) => ondrop(e, dropAt ?? index)}
    >
      {#if dropAt === index && dragFrom !== null}
        <div class="absolute -top-2 left-0 right-0 h-1 rounded bg-primary"></div>
      {/if}
      {#if item.type === "rule"}
        <RuleCard
          rule={item}
          {index}
          {editor}
          {extensions}
          {folders}
          editing={editingUid === item.uid}
          onedit={(on) => (editingUid = on ? item.uid : null)}
          onmove={(d) => move(index, index + d)}
          onduplicate={() => duplicate(index)}
          ondelete={() => remove(index)}
          dragHandle={handle}
        />
      {:else}
        <RawCard
          code={item.code}
          {index}
          {editor}
          editing={editingUid === item.uid}
          onedit={(on) => (editingUid = on ? item.uid : null)}
          onmove={(d) => move(index, index + d)}
          ondelete={() => remove(index)}
          dragHandle={handle}
        />
      {/if}
      {#if dropAt === index + 1 && index === editor.rules.items.length - 1 && dragFrom !== null}
        <div class="absolute -bottom-2 left-0 right-0 h-1 rounded bg-primary"></div>
      {/if}
    </div>
  {:else}
    <p class="rounded-box border border-dashed border-base-300 p-6 text-center text-base-content/70">
      {t("editor.noRules")}
    </p>
  {/each}

  <div class="flex flex-wrap gap-2">
    <button class="btn btn-primary btn-sm" onclick={addRule}>+ {t("editor.addRule")}</button>
    <button class="btn btn-ghost btn-sm" onclick={addCode}>+ {t("editor.addCode")}</button>
  </div>
</div>
