<script lang="ts">
  // One case of a rule with several cases, to change on its own: the cases
  // above it as a line of context each, its conditions and actions, checks
  // inside it as summaries that open like any case. With no case picked, the
  // rule's cases as a list to pick from.
  import type { Action, Rule } from "#tauri/types";
  import { t } from "../i18n.svelte";
  import type { ScriptFolders } from "../rules/mailbox";
  import { caseChain, caseNumber, cases, conditionShape, outcomes, stops, type Case, type CasePath } from "../rules/route";
  import { buildAction, newBranch, type Branches, type ConditionGroup as Group } from "../rules/view";
  import ActionRow from "./ActionRow.svelte";
  import ConditionGroup from "./ConditionGroup.svelte";
  import Icon from "./Icon.svelte";
  import RouteConditions from "./route/Conditions.svelte";
  import RouteOutcomes from "./route/Outcomes.svelte";

  let {
    rule,
    path,
    extensions,
    folders,
    singleFolder,
    keywords,
    delimiter,
    mutate,
    onfocus,
  }: {
    rule: Rule;
    /** The case to change; "" lists the rule's cases. */
    path: CasePath;
    extensions: string[];
    folders: ScriptFolders;
    singleFolder: boolean;
    keywords: Record<string, string>;
    delimiter: string | null;
    /** Applies an edit (with an undo step) to the rule. */
    mutate: (fn: (r: Rule) => void, merge?: boolean) => void;
    onfocus: (path: CasePath) => void;
  } = $props();

  const chain = $derived(caseChain(rule, path));
  const step = $derived(chain?.at(-1) ?? null);
  const depth = $derived((chain?.length ?? 1) - 1);
  const rail = $derived(`var(--rail-${Math.min(depth, 3)})`);
  /** Number prefix of the block the case belongs to: "1." inside case 1. */
  const prefix = $derived(chain ? caseNumber(chain.slice(0, -1)) : "");

  /** The block holding the case, in the rule being edited. */
  function blockOf(r: Rule): Branches {
    const parts = path.split(".").map(Number);
    let block: Branches = r;
    for (let i = 0; i + 1 < parts.length; i += 2) block = cases(block)[parts[i]].nested[parts[i + 1]];
    return block;
  }
  /** The case's actions, in the rule being edited. */
  function actionsIn(b: Branches, index: number): Action[] {
    if (index === 0) return b.actions;
    if (index <= b.elsif.length) return b.elsif[index - 1].actions;
    return (b.otherwise ??= []);
  }
  function groupIn(b: Branches, index: number): Group {
    return index === 0 ? b.conditions : b.elsif[index - 1].conditions;
  }

  const label = (c: Case, list: Case[], pre: string) => (list.length > 1 && c.kind !== "else" ? (pre ? `${pre}.${c.number}` : String(c.number)) : null);
  const word = (c: Case, single: boolean) =>
    c.kind === "else" ? t("route.otherwise") : single ? t("route.if") : c.kind === "if" ? t("route.if") : t("route.elseIf");

  /** "1–3", "1.1–1.2": the cases tried before this one. */
  function before(index: number, pre: string): string {
    const n = (k: number) => (pre ? `${pre}.${k}` : String(k));
    return index === 1 ? n(1) : `${n(1)}–${n(index)}`;
  }

  /** When the case applies, all of it: the case it sits in, and the cases before it. */
  const hint = $derived.by(() => {
    if (!step) return "";
    const parent = depth > 0 ? caseNumber(chain!.slice(0, -1)) : "";
    const inside = depth > 0 ? (parent ? t("case.inside", { n: parent }) : t("case.insideAbove")) : "";
    if (step.list.length === 1) return depth > 0 ? t("case.extra", { inside }) : "";
    if (step.case.kind === "else") return t("case.otherwise", { list: before(step.list.length - 1, prefix), inside: inside ? ` ${inside}` : "" });
    if (step.index === 0) return depth > 0 ? t("case.firstInside", { inside }) : t("case.first");
    return t("case.after", { list: before(step.index, prefix), inside: inside ? `${inside} ${t("case.and")} ` : "" });
  });

  function removeCase() {
    if (!step) return;
    const index = step.index;
    const elsif = index > 0 && step.case.kind !== "else";
    mutate((r) => {
      const b = blockOf(r);
      if (elsif) b.elsif.splice(index - 1, 1);
      else b.otherwise = null;
    });
    const parent = path.split(".").slice(0, -1).join(".");
    onfocus(parent ? `${parent}.${index - 1}` : String(index - 1));
  }

  function addCase(otherwise: boolean) {
    const at = path === "" ? "" : path.split(".").slice(0, -1).join(".");
    let index = 0;
    mutate((r) => {
      const b = path === "" ? r : blockOf(r);
      if (otherwise) b.otherwise = [buildAction("move")];
      else b.elsif.push(newBranch());
      index = otherwise ? 1 + b.elsif.length : b.elsif.length;
    });
    onfocus(at ? `${at}.${index}` : String(index));
  }

  /** The nested check index of each action that is a check, for case paths. */
  function checkIndex(list: Action[], i: number): number {
    return list.slice(0, i).filter((a) => a.type === "if").length;
  }
</script>

<!-- A case as one line, like on the route; picks it. -->
{#snippet caseLine(c: Case, list: Case[], pre: string, to: CasePath, color: string, context = false)}
  {@const n = label(c, list, pre)}
  <button
    class="flex w-full min-w-0 items-start gap-2 rounded-field px-2 py-1.5 text-left hover:bg-base-200"
    onclick={() => onfocus(to)}
  >
    <span class="flex w-[6.2rem] shrink-0 items-center gap-1.5" style="color: {color}">
      {#if n}<span class="rounded-full border-2 px-1.5 text-[0.8rem] leading-[1.05rem] font-bold" style="border-color: {color}">{n}</span>{/if}
      <span class="text-[0.875rem] font-semibold whitespace-nowrap">{word(c, list.length === 1)}</span>
    </span>
    <span class="grid min-w-0 gap-1">
      {#if c.conditions}<RouteConditions shape={conditionShape(c.conditions, false, 2)} />{:else}<span class="text-base-content/70">{t("route.allOthers")}</span>{/if}
      <RouteOutcomes items={outcomes(c.actions, keywords, delimiter)} stop={stops(c.actions)} {singleFolder} />
      {#if c.nested.length && !context}
        <span class="flex items-center gap-1 text-[0.8rem] text-base-content/65"><Icon name="nested" size={13} />{c.nested.length === 1 ? t("case.hasCheck") : t("case.hasChecks", { count: c.nested.length })}</span>
      {/if}
    </span>
  </button>
{/snippet}

{#if path === "" || !step}
  <!-- All cases: pick one to change it. -->
  <p class="mb-2 text-sm text-base-content/70">{t("case.pick")}</p>
  {@const list = cases(rule)}
  <ul class="rails grid gap-1" style="--rail: var(--rail-0); --elbow: 1rem">
    {#each list as c, i}
      <li>{@render caseLine(c, list, "", String(i), "var(--rail-0)")}</li>
    {/each}
  </ul>
  <div class="mt-3 flex flex-wrap gap-1 border-t border-dashed border-base-300 pt-2">
    <button class="btn btn-ghost btn-sm gap-1 text-base-content/75" onclick={() => addCase(false)}><Icon name="branch" size={14} />{t("case.addElsif")}</button>
    {#if !rule.otherwise}
      <button class="btn btn-ghost btn-sm gap-1 text-base-content/75" onclick={() => addCase(true)}><Icon name="plus" size={14} />{t("case.addElse")}</button>
    {/if}
  </div>
{:else}
  <!-- Where the case sits: the cases above it, one line each. -->
  {#if chain!.length > 1 || cases(rule).length > 1}
  <nav class="mb-4 grid gap-0.5 rounded-box bg-base-200/60 p-1.5" aria-label={t("case.context")}>
    {#if cases(rule).length > 1}
      <button class="flex items-center gap-1.5 rounded-field px-2 py-1 text-left text-[0.8rem] font-medium text-base-content/70 hover:bg-base-200" onclick={() => onfocus("")}>
        <Icon name="arrowLeft" size={13} />{t("case.all", { count: cases(rule).length })}
      </button>
    {/if}
    {#each chain!.slice(0, -1) as s, i}
      <div style="padding-left: {i * 0.9}rem">
        {@render caseLine(s.case, s.list, caseNumber(chain!.slice(0, i)), s.path, `var(--rail-${Math.min(i, 3)})`, true)}
      </div>
    {/each}
  </nav>
  {/if}

  <!-- The case itself. -->
  {#if !(depth === 0 && step.list.length === 1)}
  <div class="mb-1 flex items-center gap-2" style="color: {rail}">
    {#if label(step.case, step.list, prefix)}
      <span class="rounded-full border-2 px-2 text-[0.9rem] leading-[1.35rem] font-bold" style="border-color: {rail}">{label(step.case, step.list, prefix)}</span>
    {/if}
    <span class="grow text-[1.05rem] font-semibold">{word(step.case, step.list.length === 1)}</span>
    {#if step.list.length > 1}
      <button
        class="btn btn-ghost btn-sm btn-square text-base-content/70"
        disabled={step.index === 0}
        aria-label={t("case.prev")}
        title={t("case.prev")}
        onclick={() => onfocus(path.replace(/\d+$/, String(step!.index - 1)))}><Icon name="chevronLeft" size={16} /></button
      >
      <button
        class="btn btn-ghost btn-sm btn-square text-base-content/70"
        disabled={step.index === step.list.length - 1}
        aria-label={t("case.next")}
        title={t("case.next")}
        onclick={() => onfocus(path.replace(/\d+$/, String(step!.index + 1)))}><Icon name="chevronRight" size={16} /></button
      >
    {/if}
    {#if step.index > 0}
      <button class="btn btn-ghost btn-sm gap-1 text-base-content/70" onclick={removeCase}><Icon name="trash" size={14} />{t("case.remove")}</button>
    {/if}
  </div>
  {/if}
  {#if hint}<p class="mb-4 text-sm text-base-content/70">{hint}</p>{/if}

  <div class="grid gap-4">
    {#if step.case.kind !== "else"}
      <ConditionGroup
        group={step.case.conditions!}
        {extensions}
        mutate={(fn, merge) => mutate((r) => fn(groupIn(blockOf(r), step!.index), null), merge)}
      />
    {/if}

    <section class="grid gap-1.5" aria-label={t("actions.heading")}>
      <h4 class="text-sm font-semibold text-base-content/75">{t("actions.heading")}</h4>
      {#each step.case.actions as action, i}
        {#if action.type === "if"}
          {@const j = checkIndex(step.case.actions, i)}
          {@const list = cases(action)}
          {@const inner = `var(--rail-${Math.min(depth + 1, 3)})`}
          <!-- A check inside this case: its cases open like any case. -->
          <div class="rounded-field border-l-2 py-1.5 pr-1.5 pl-2" style="border-color: {inner}; background: color-mix(in oklab, {inner} 6%, transparent)">
            <div class="mb-1 flex items-center gap-2 pl-1">
              <p class="grow text-[0.875rem] font-semibold" style="color: {inner}">
                {list.length > 1 ? t("case.checkMany") : t("case.checkOne")}
              </p>
              <button
                class="btn btn-ghost btn-xs btn-square text-base-content/65"
                aria-label={t("case.removeCheck")}
                title={t("case.removeCheck")}
                onclick={() => mutate((r) => actionsIn(blockOf(r), step!.index).splice(i, 1))}><Icon name="trash" size={14} /></button
              >
            </div>
            <ul class="rails grid gap-0.5" style="--rail: {inner}; --elbow: 1rem">
              {#each list as c, k}
                <li>{@render caseLine(c, list, caseNumber(chain!), `${path}.${j}.${k}`, inner)}</li>
              {/each}
            </ul>
          </div>
        {:else}
          <ActionRow
            {action}
            {extensions}
            {folders}
            {singleFolder}
            depth={depth + 1}
            onchange={(a, merge) => mutate((r) => (actionsIn(blockOf(r), step!.index)[i] = a), merge)}
            onremove={() => mutate((r) => actionsIn(blockOf(r), step!.index).splice(i, 1))}
          />
        {/if}
      {/each}
      <button class="btn btn-ghost btn-sm justify-self-start gap-1 text-base-content/75" onclick={() => mutate((r) => actionsIn(blockOf(r), step!.index).push(buildAction("move")))}>
        <Icon name="plus" size={14} />{t("actions.add")}
      </button>
    </section>
  </div>

  <div class="mt-4 flex flex-wrap gap-1 border-t border-dashed border-base-300 pt-2">
      <button class="btn btn-ghost btn-sm gap-1 text-base-content/75" onclick={() => addCase(false)}><Icon name="branch" size={14} />{t("case.addElsif")}</button>
      {#if !step.list.some((c) => c.kind === "else")}
        <button class="btn btn-ghost btn-sm gap-1 text-base-content/75" onclick={() => addCase(true)}><Icon name="plus" size={14} />{t("case.addElse")}</button>
      {/if}
  </div>
{/if}
