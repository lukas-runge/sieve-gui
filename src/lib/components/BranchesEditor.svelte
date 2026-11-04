<script lang="ts" module>
  /** What the case summaries need to read like the route. */
  export interface SummaryContext {
    keywords: Record<string, string>;
    delimiter: string | null;
  }
  export const SUMMARY = Symbol("summary");
</script>

<script lang="ts">
  // The cases of a rule or of a nested check, drawn like on the route: with
  // one case it's "When … then …"; with several, each case hangs off a rail
  // with its number (the first that matches applies), "otherwise" last, and
  // folds to the same one-line summary the route shows. Checks inside a case
  // use this component again, one rail deeper.
  import { getContext } from "svelte";
  import { SvelteSet } from "svelte/reactivity";
  import type { Action, Conditions as ConditionsModel } from "#tauri/types";
  import { t } from "../i18n.svelte";
  import type { ScriptFolders } from "../rules/mailbox";
  import { conditionShape, outcomes, stops } from "../rules/route";
  import { buildAction, newBranch, type Branches, type ConditionGroup as Group } from "../rules/view";
  import ActionRow from "./ActionRow.svelte";
  import ConditionGroup from "./ConditionGroup.svelte";
  import Icon from "./Icon.svelte";
  import RouteConditions from "./route/Conditions.svelte";
  import RouteOutcomes from "./route/Outcomes.svelte";

  let {
    block,
    extensions,
    folders,
    singleFolder,
    depth = 0,
    path = "",
    focus = "",
    mutate,
  }: {
    block: Branches;
    extensions: string[];
    folders: ScriptFolders;
    singleFolder: boolean;
    /** Nesting level: 0 for a rule, 1 for a check inside it, … */
    depth?: number;
    /** Case path prefix of this block (see `CasePath`): "" for a rule. */
    path?: string;
    /** The case to show unfolded and scroll to. */
    focus?: string;
    /** Applies an edit (with an undo step) to this block. */
    mutate: (fn: (b: Branches) => void, merge?: boolean) => void;
  } = $props();

  const summary = getContext<SummaryContext | undefined>(SUMMARY);

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
  const count = $derived(1 + block.elsif.length + (block.otherwise ? 1 : 0));

  /** Unfolded cases (indexes). The focused one unfolds on its own. */
  const open = new SvelteSet<number>();
  $effect(() => {
    for (let i = 0; i < count; i++) {
      const p = `${path}${i}`;
      if (focus === p || focus.startsWith(p + ".")) open.add(i);
    }
  });

  function toggle(i: number) {
    if (open.has(i)) open.delete(i);
    else open.add(i);
  }

  /** The nested check index of each action that is a check, for case paths. */
  function checkIndex(list: Action[], i: number): number {
    return list.slice(0, i).filter((a) => a.type === "if").length;
  }
</script>

{#snippet badge(text: string | number, dashed = false)}
  {#if typeof text === "number"}
    <span class="grid size-[1.3rem] shrink-0 place-items-center rounded-full border-2 text-[0.8rem] font-bold" style="border-color: {rail}; color: {rail}">{text}</span>
  {:else}
    <span class="shrink-0 text-[0.875rem] leading-[1.3rem] font-semibold" style="color: {rail}">{text}</span>
  {/if}
{/snippet}

{#snippet actions(list: Action[], where: Where, casePath: string)}
  <section class="grid gap-1.5" aria-label={t("actions.heading")}>
    <h4 class="text-sm font-semibold text-base-content/75">{t("actions.heading")}</h4>
    {#each list as action, i}
      <ActionRow
        {action}
        {extensions}
        {folders}
        {singleFolder}
        depth={depth + 1}
        path="{casePath}.{checkIndex(list, i)}."
        {focus}
        onchange={(a, merge) => mutate((b) => (actionsOf(b, where)[i] = a), merge)}
        onremove={() => mutate((b) => actionsOf(b, where).splice(i, 1))}
      />
    {/each}
    <button class="btn btn-ghost btn-xs justify-self-start gap-1 text-base-content/75" onclick={() => mutate((b) => actionsOf(b, where).push(buildAction("move")))}>
      <Icon name="plus" size={13} />{t("actions.add")}
    </button>
  </section>
{/snippet}

<!-- A folded case: the same summary as on the route. -->
{#snippet folded(conditions: ConditionsModel | null, list: Action[])}
  {#if summary}
    <span class="grid min-w-0 gap-1">
      {#if conditions}<RouteConditions shape={conditionShape(conditions)} />{/if}
      <RouteOutcomes items={outcomes(list, summary.keywords, summary.delimiter)} stop={stops(list)} {singleFolder} />
    </span>
  {/if}
{/snippet}

{#snippet caseHead(i: number, label: string | number, dashed: boolean, conditions: ConditionsModel | null, list: Action[], remove: ((b: Branches) => void) | null)}
  <div class="flex items-start gap-2">
    <button
      class="flex min-w-0 grow items-start gap-2 rounded-field py-0.5 text-left hover:bg-base-200/70"
      aria-expanded={open.has(i)}
      onclick={() => toggle(i)}
    >
      <Icon name={open.has(i) ? "chevronDown" : "chevronRight"} size={16} class="mt-0.5 text-base-content/65" />
      {@render badge(label, dashed)}
      {#if open.has(i)}
        <span class="text-sm text-base-content/70">{i === 0 ? t("branch.firstMatch") : label === t("route.otherwise") ? t("branch.otherwiseHint") : ""}</span>
      {:else}
        {@render folded(conditions, list)}
      {/if}
    </button>
    {#if remove}
      <button class="btn btn-ghost btn-xs btn-square text-base-content/65" aria-label={t("branch.remove")} title={t("branch.remove")} onclick={() => mutate(remove)}>
        <Icon name="x" size={14} />
      </button>
    {/if}
  </div>
{/snippet}

{#if !several}
  <div class="grid gap-4" data-case="{path}0">
    <ConditionGroup group={block.conditions} heading={depth > 0 ? t("branch.andIf") : undefined} {extensions} mutate={conditionsMutate((b) => b.conditions)} />
    {@render actions(block.actions, { branch: "if" }, `${path}0`)}
  </div>
{:else}
  <ol class="rails grid gap-2.5" style="--rail: {rail}; --elbow: 0.85rem" aria-label={t("branch.cases")}>
    <li class="grid gap-3" data-case="{path}0">
      {@render caseHead(0, 1, false, block.conditions, block.actions, null)}
      {#if open.has(0)}
        <ConditionGroup group={block.conditions} {extensions} mutate={conditionsMutate((b) => b.conditions)} />
        {@render actions(block.actions, { branch: "if" }, `${path}0`)}
      {/if}
    </li>
    {#each block.elsif as branch, index}
      <li class="grid gap-3" data-case="{path}{index + 1}">
        {@render caseHead(index + 1, index + 2, false, branch.conditions, branch.actions, (b) => b.elsif.splice(index, 1))}
        {#if open.has(index + 1)}
          <ConditionGroup group={branch.conditions} heading={t("branch.elsif")} {extensions} mutate={conditionsMutate((b) => b.elsif[index].conditions)} />
          {@render actions(branch.actions, { branch: "elsif", index }, `${path}${index + 1}`)}
        {/if}
      </li>
    {/each}
    {#if block.otherwise}
      <li class="grid gap-3" data-case="{path}{count - 1}">
        {@render caseHead(count - 1, t("route.otherwise"), true, null, block.otherwise, (b) => (b.otherwise = null))}
        {#if open.has(count - 1)}
          {@render actions(block.otherwise, { branch: "else" }, `${path}${count - 1}`)}
        {/if}
      </li>
    {/if}
  </ol>
{/if}

<div class="mt-3 flex flex-wrap gap-1 border-t border-dashed border-base-300 pt-2">
  <button
    class="btn btn-ghost btn-xs gap-1 text-base-content/75"
    onclick={() => {
      mutate((b) => b.elsif.push(newBranch()));
      open.add(block.elsif.length + 1);
    }}
  >
    <Icon name="branch" size={13} />{t("case.addElsif")}
  </button>
  {#if !block.otherwise}
    <button
      class="btn btn-ghost btn-xs gap-1 text-base-content/75"
      onclick={() => {
        mutate((b) => (b.otherwise = [buildAction("move")]));
        open.add(count);
      }}
    >
      <Icon name="plus" size={13} />{t("case.addElse")}
    </button>
  {/if}
</div>
