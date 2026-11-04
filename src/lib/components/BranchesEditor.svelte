<script lang="ts">
  // The cases of a rule or of a nested check, drawn like on the route: with
  // one case it's "When … then …"; with several, each case hangs off a rail
  // with its number (the first that matches applies), "otherwise" last.
  // Checks inside a case use this component again, one rail deeper.
  import type { Action } from "#tauri/types";
  import { t } from "../i18n.svelte";
  import type { ScriptFolders } from "../rules/mailbox";
  import { buildAction, newBranch, type Branches, type ConditionGroup as Group } from "../rules/view";
  import ActionRow from "./ActionRow.svelte";
  import ConditionGroup from "./ConditionGroup.svelte";
  import Icon from "./Icon.svelte";

  let {
    block,
    extensions,
    folders,
    singleFolder,
    depth = 0,
    mutate,
  }: {
    block: Branches;
    extensions: string[];
    folders: ScriptFolders;
    singleFolder: boolean;
    /** Nesting level: 0 for a rule, 1 for a check inside it, … */
    depth?: number;
    /** Applies an edit (with an undo step) to this block. */
    mutate: (fn: (b: Branches) => void, merge?: boolean) => void;
  } = $props();

  type Where = { branch: "if" } | { branch: "elsif"; index: number } | { branch: "else" };

  function actionsOf(b: Branches, where: Where): Action[] {
    if (where.branch === "if") return b.actions;
    if (where.branch === "elsif") return b.elsif[where.index].actions;
    return (b.otherwise ??= []);
  }

  function conditionsMutate(get: (b: Branches) => Group) {
    return (fn: (group: Group, holder: null) => void, merge?: boolean) => mutate((b) => fn(get(b), null), merge);
  }

  const several = $derived(block.elsif.length > 0 || !!block.otherwise);
  const rail = $derived(`var(--rail-${Math.min(depth, 3)})`);
</script>

{#snippet badge(text: string | number, dashed = false)}
  {#if typeof text === "number"}
    <span class="grid size-5 shrink-0 place-items-center rounded-full border-2 text-[0.72rem] font-bold" style="border-color: {rail}; color: {rail}">{text}</span>
  {:else}
    <span class={["shrink-0 rounded-full border px-1.5 text-[0.72rem] leading-[1.15rem] font-semibold", dashed && "border-dashed"]} style="border-color: {rail}; color: {rail}">{text}</span>
  {/if}
{/snippet}

{#snippet actions(list: Action[], where: Where)}
  <section class="grid gap-1.5" aria-label={t("actions.heading")}>
    <h4 class="text-sm font-semibold text-base-content/70">{t("actions.heading")}</h4>
    {#each list as action, i}
      <ActionRow
        {action}
        {extensions}
        {folders}
        {singleFolder}
        depth={depth + 1}
        onchange={(a, merge) => mutate((b) => (actionsOf(b, where)[i] = a), merge)}
        onremove={() => mutate((b) => actionsOf(b, where).splice(i, 1))}
      />
    {/each}
    <button class="btn btn-ghost btn-xs justify-self-start gap-1 text-base-content/70" onclick={() => mutate((b) => actionsOf(b, where).push(buildAction("move")))}>
      <Icon name="plus" size={13} />{t("actions.add")}
    </button>
  </section>
{/snippet}

{#snippet removeCase(fn: (b: Branches) => void)}
  <button class="btn btn-ghost btn-xs btn-square ml-auto text-base-content/50" aria-label={t("branch.remove")} title={t("branch.remove")} onclick={() => mutate(fn)}>
    <Icon name="x" size={14} />
  </button>
{/snippet}

{#if !several}
  <div class="grid gap-4">
    <ConditionGroup group={block.conditions} heading={depth > 0 ? t("branch.andIf") : undefined} {extensions} mutate={conditionsMutate((b) => b.conditions)} />
    {@render actions(block.actions, { branch: "if" })}
  </div>
{:else}
  <ol class="rails grid gap-3" style="--rail: {rail}; --elbow: 0.7rem" aria-label={t("branch.cases")}>
    <li class="grid gap-3">
      <div class="flex items-center gap-2">{@render badge(1)}<span class="text-sm text-base-content/60">{t("branch.firstMatch")}</span></div>
      <ConditionGroup group={block.conditions} {extensions} mutate={conditionsMutate((b) => b.conditions)} />
      {@render actions(block.actions, { branch: "if" })}
    </li>
    {#each block.elsif as branch, index}
      <li class="grid gap-3">
        <div class="flex items-center gap-2">
          {@render badge(index + 2)}
          {@render removeCase((b) => b.elsif.splice(index, 1))}
        </div>
        <ConditionGroup group={branch.conditions} heading={t("branch.elsif")} {extensions} mutate={conditionsMutate((b) => b.elsif[index].conditions)} />
        {@render actions(branch.actions, { branch: "elsif", index })}
      </li>
    {/each}
    {#if block.otherwise}
      <li class="otherwise grid gap-3">
        <div class="flex items-center gap-2">
          {@render badge(t("route.otherwise"), true)}
          <span class="text-sm text-base-content/60">{t("branch.otherwiseHint")}</span>
          {@render removeCase((b) => (b.otherwise = null))}
        </div>
        {@render actions(block.otherwise, { branch: "else" })}
      </li>
    {/if}
  </ol>
{/if}

<div class="mt-3 flex flex-wrap gap-1 border-t border-dashed border-base-300 pt-2">
  <button class="btn btn-ghost btn-xs gap-1 text-base-content/70" onclick={() => mutate((b) => b.elsif.push(newBranch()))}>
    <Icon name="branch" size={13} />{t("branch.addElsif")}
  </button>
  {#if !block.otherwise}
    <button class="btn btn-ghost btn-xs gap-1 text-base-content/70" onclick={() => mutate((b) => (b.otherwise = [buildAction("move")]))}>
      <Icon name="plus" size={13} />{t("branch.addElse")}
    </button>
  {/if}
</div>
