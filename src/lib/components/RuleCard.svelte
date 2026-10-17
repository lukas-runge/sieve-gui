<script lang="ts">
  import type { Action, Rule } from "#tauri/types";
  import type { ScriptEditor } from "../editor.svelte";
  import { t } from "../i18n.svelte";
  import { closesMenu } from "../menu";
  import type { ScriptFolders } from "../rules/mailbox";
  import { buildAction } from "../rules/view";
  import ActionRow from "./ActionRow.svelte";
  import ConditionGroup from "./ConditionGroup.svelte";
  import RuleSentence from "./RuleSentence.svelte";

  let {
    rule,
    index,
    editor,
    extensions,
    folders,
    editing,
    onedit,
    onmove,
    onduplicate,
    ondelete,
    dragHandle,
  }: {
    rule: Rule;
    index: number;
    editor: ScriptEditor;
    extensions: string[];
    folders: ScriptFolders;
    editing: boolean;
    onedit: (editing: boolean) => void;
    onmove: (delta: number) => void;
    onduplicate: () => void;
    ondelete: () => void;
    dragHandle: import("svelte").Snippet<[number]>;
  } = $props();

  /** Mutates this rule with an undo step. */
  function change(fn: (r: Rule) => void, merge = false) {
    editor.edit((rules) => {
      const item = rules.items[index];
      if (item?.type === "rule") fn(item);
    }, merge);
  }

  function setAction(i: number, a: Action, merge: boolean) {
    change((r) => (r.actions[i] = a), merge);
  }

  const headingId = `rule-${Math.random().toString(36).slice(2)}`;
  const lostInRoundcube = $derived(editor.roundcubeRisks?.rules.includes(index) ?? false);
</script>

<article
  class="rounded-box border border-base-300 bg-base-100 {rule.enabled ? '' : 'rule-disabled'}"
  aria-labelledby={headingId}
>
  <header class="flex items-center gap-2 px-3 pt-2.5">
    {@render dragHandle(index)}
    {#if editing}
      <input
        class="input input-sm grow font-semibold"
        aria-label={t("rule.name")}
        placeholder={t("rule.untitled")}
        value={rule.name}
        oninput={(e) => change((r) => (r.name = e.currentTarget.value), true)}
      />
    {:else}
      <h3 id={headingId} class="grow truncate font-semibold {rule.name ? '' : 'text-base-content/50 italic'}">
        {rule.name || t("rule.untitled")}
      </h3>
    {/if}
    {#if lostInRoundcube}
      <!-- Roundcube's form has one level of "all of" / "any of". -->
      <span class="badge badge-sm badge-soft badge-warning whitespace-nowrap">{t("roundcube.badge")}</span>
    {/if}
    <label class="flex items-center gap-1.5 text-sm text-base-content/70">
      <input
        type="checkbox"
        class="toggle toggle-sm toggle-primary"
        checked={rule.enabled}
        onchange={(e) => change((r) => (r.enabled = e.currentTarget.checked))}
      />
      <span class="w-14">{rule.enabled ? t("rule.enabled") : t("rule.disabled")}</span>
    </label>
    <button class="btn btn-sm {editing ? 'btn-primary' : 'btn-ghost'}" onclick={() => onedit(!editing)}>
      {editing ? t("rule.done") : t("rule.edit")}
    </button>
    <div class="dropdown dropdown-end">
      <div tabindex="0" role="button" class="btn btn-ghost btn-sm btn-square" aria-label={t("menu.more")}>⋯</div>
      <ul tabindex="-1" {@attach closesMenu} class="dropdown-content menu z-10 w-48 rounded-box border border-base-300 bg-base-100 p-1">
        <li><button onclick={() => onmove(-1)}>{t("rule.moveUp")}</button></li>
        <li><button onclick={() => onmove(1)}>{t("rule.moveDown")}</button></li>
        <li><button onclick={onduplicate}>{t("rule.duplicate")}</button></li>
        <li><button class="text-error" onclick={ondelete}>{t("rule.delete")}</button></li>
      </ul>
    </div>
  </header>

  <div class="px-3 pb-3 pt-1.5">
    {#if !editing}
      <button class="block w-full text-left cursor-text" onclick={() => onedit(true)}>
        <RuleSentence {rule} singleFolder={editor.rules.singleFolder} />
      </button>
    {:else}
      <ConditionGroup
        group={rule.conditions}
        {extensions}
        mutate={(fn, merge) => change((r) => fn(r.conditions, null), merge)}
      />

      <section class="mt-3 grid gap-2" aria-label={t("actions.heading")}>
        <h4 class="font-semibold">{t("actions.heading")}</h4>
        {#each rule.actions as action, i}
          <ActionRow
            {action}
            {extensions}
            {folders}
            singleFolder={editor.rules.singleFolder}
            onchange={(a, merge) => setAction(i, a, merge)}
            onremove={() => change((r) => r.actions.splice(i, 1))}
          />
        {/each}
        <button
          class="btn btn-ghost btn-sm justify-self-start w-fit"
          onclick={() => change((r) => r.actions.push(buildAction("move")))}
        >
          + {t("actions.add")}
        </button>
      </section>
    {/if}
  </div>
</article>
