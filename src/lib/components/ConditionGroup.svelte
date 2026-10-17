<script lang="ts">
  // The conditions of a rule, or a group nested in them (`allof`/`anyof`):
  // a join plus any number of conditions and further groups.
  import type { Condition } from "#tauri/types";
  import { t, type MessageKey } from "../i18n.svelte";
  import {
    addCondition,
    addGroup,
    GROUP_MODES,
    groupMode,
    newCondition,
    removeFromGroup,
    setGroupMode,
    type ConditionGroup,
    type GroupMode,
  } from "../rules/view";
  import ConditionRow from "./ConditionRow.svelte";
  import Self from "./ConditionGroup.svelte";

  let {
    group,
    holder = null,
    extensions,
    mutate,
    onremove,
  }: {
    group: ConditionGroup;
    /** The condition holding a nested group; null for the rule's own conditions. */
    holder?: Condition | null;
    extensions: string[];
    /** Applies an edit (with an undo step) to this group and its holder. */
    mutate: (fn: (group: ConditionGroup, holder: Condition | null) => void, merge?: boolean) => void;
    /** Removes a nested group. */
    onremove?: () => void;
  } = $props();

  const top = $derived(holder === null);

  /** Edits the nested group at `index` of this one. */
  function child(index: number) {
    return (fn: (group: ConditionGroup, holder: Condition | null) => void, merge?: boolean) =>
      mutate((g) => {
        const c = g.items[index];
        if (c?.test.type === "group") fn(c.test, c);
      }, merge);
  }

  /** Removes an item; a nested group that becomes empty goes too. */
  function remove(index: number) {
    if (!top && group.items.length === 1) return onremove?.();
    mutate((g) => removeFromGroup(g, index, top));
  }
</script>

{#snippet joinSelect()}
  {#if top}
    {#if group.items.length > 1}
      <select
        class="select select-sm w-auto"
        aria-label={t("conditions.join")}
        value={group.join}
        onchange={(e) => mutate((g) => (g.join = e.currentTarget.value as "all" | "any"))}
      >
        <option value="all">{t("conditions.all")}</option>
        <option value="any">{t("conditions.any")}</option>
      </select>
    {:else if group.items.length === 0}
      <span class="text-base-content/70">{t("conditions.always")}</span>
    {/if}
  {:else if holder}
    <select
      class="select select-sm w-auto"
      aria-label={t("conditions.join")}
      value={groupMode(holder)}
      onchange={(e) => mutate((_, h) => h && setGroupMode(h, e.currentTarget.value as GroupMode))}
    >
      {#each GROUP_MODES as mode}
        <option value={mode}>{t(`conditions.${mode}` as MessageKey)}</option>
      {/each}
    </select>
  {/if}
{/snippet}

{#snippet body()}
  {#each group.items as condition, i}
    {#if condition.test.type === "group"}
      <Self group={condition.test} holder={condition} {extensions} mutate={child(i)} onremove={() => remove(i)} />
    {:else}
      <ConditionRow
        {condition}
        {extensions}
        onchange={(c, merge) => mutate((g) => (g.items[i] = c), merge)}
        onremove={() => remove(i)}
      />
    {/if}
  {/each}
  <div class="flex flex-wrap gap-1">
    <button
      class="btn btn-ghost btn-sm w-fit"
      onclick={() => mutate((g) => (top ? addCondition(g, newCondition()) : g.items.push(newCondition())))}
    >
      + {t("conditions.add")}
    </button>
    <button class="btn btn-ghost btn-sm w-fit" onclick={() => mutate((g) => addGroup(g, top))}>
      + {t("conditions.addGroup")}
    </button>
  </div>
{/snippet}

{#if top}
  <section class="mt-1 grid gap-2" aria-label={t("conditions.heading")}>
    <div class="flex items-center gap-2">
      <h4 class="font-semibold">{t("conditions.heading")}</h4>
      {@render joinSelect()}
    </div>
    {@render body()}
  </section>
{:else}
  <div role="group" aria-label={t("conditions.group")} class="grid gap-2 rounded-box border border-base-300 p-2">
    <div class="flex items-center gap-2">
      {@render joinSelect()}
      <button
        class="btn btn-ghost btn-sm btn-square ml-auto"
        aria-label={t("conditions.removeGroup")}
        title={t("conditions.removeGroup")}
        onclick={() => onremove?.()}>✕</button
      >
    </div>
    {@render body()}
  </div>
{/if}
