<script lang="ts">
  // A rule on the route. A simple rule is one line: what it tests, where the
  // message goes. A rule with several cases is a switch: its cases hang off a
  // rail in order, and the first that matches applies. A check inside a case
  // sits in a box of its own, labeled with its case, on a rail one level
  // deeper and in the next color. Every case can be picked to edit it.
  import type { Rule } from "#tauri/types";
  import { t } from "../../i18n.svelte";
  import type { Branches } from "../../rules/view";
  import { folds, toggleFold } from "../../folds.svelte";
  import { blockOutcomes, caseMatches, cases, nestedOutcomes, unreachable, conditionShape, isSimple, outcomes, stops, type Case, type CasePath, type Outcome } from "../../rules/route";
  import Conditions from "./Conditions.svelte";
  import { SvelteSet } from "svelte/reactivity";
  import Icon from "../Icon.svelte";
  import Mark from "./Mark.svelte";
  import Outcomes from "./Outcomes.svelte";
  import Possible from "./Possible.svelte";

  let {
    rule,
    delimiter,
    keywords,
    singleFolder,
    next = null,
    words = [],
    picked = null,
    hideParent = false,
    trial = null,
    filter = null,
    compact = false,
    id = "",
    ontarget,
    onpick,
  }: {
    rule: Rule;
    delimiter: string | null;
    keywords: Record<string, string>;
    singleFolder: boolean;
    /** The script that runs after this one: where "no further rules" continues. */
    next?: string | null;
    /** Search words: matches are marked, cases without any are dimmed. */
    words?: string[];
    /** The case open in the inspector, if this rule is. */
    picked?: CasePath | null;
    /** The folder path is shown once above a run of rules filing into the same parent. */
    hideParent?: boolean;
    /** A sample message's way: the cases that applied to it. */
    trial?: { hits: CasePath[]; misses: CasePath[]; reached: boolean } | null;
    /** Only cases this accepts are shown at full strength (e.g. those filing into a folder). */
    filter?: ((c: Case) => boolean) | null;
    /** In a list of matches: cases that don't match fold into one line. */
    compact?: boolean;
    /** Identifies the rule for folding it; none: it can't be folded. */
    id?: string;
    ontarget?: (o: Outcome) => void;
    onpick: (path: CasePath) => void;
  } = $props();

  /** A case that neither the search nor the filter is about. */
  const unrelated = (c: Case) => (words.length > 0 && !caseMatches(c, words, keywords, delimiter)) || (filter !== null && !filter(c));

  /** Runs of hidden cases the user asked to see, by block prefix. */
  const unfolded = new SvelteSet<string>();

  /** The cases in order, runs of unrelated ones folded into one entry when compact. */
  function segments(list: Case[], prefix: string) {
    const out: ({ kind: "case"; c: Case; i: number } | { kind: "folded"; count: number; i: number })[] = [];
    for (const [i, c] of list.entries()) {
      const last = out.at(-1);
      if (compact && !unfolded.has(prefix) && unrelated(c)) {
        if (last?.kind === "folded") last.count++;
        else out.push({ kind: "folded", count: 1, i });
      } else out.push({ kind: "case", c, i });
    }
    return out;
  }

  /** Applied to the sample message: true; tried and not applied: false; no test: null. */
  const applies = (path: CasePath) => (trial ? trial.hits.includes(path) : null);

  const simple = $derived(isSimple(rule));
  const top = $derived(cases(rule));
</script>

<!-- The case label: "1 if", "2 else if", "otherwise"; nested cases count on
     from their case: "1.2 else if". Fixed width, so conditions line up. -->
{#snippet label(c: Case, num: string | null, hit: boolean, missed: boolean)}
  <span class="case-label flex w-[7rem] shrink-0 items-center gap-1.5 pt-px" style="color: var(--rail)">
    {#if hit}
      <span class="grid size-[1.25rem] shrink-0 place-items-center rounded-full bg-success text-success-content" title={t("trial.applies")}
        ><Icon name="check" size={13} /></span
      >
    {:else if num}
      <span class="grid h-[1.25rem] min-w-[1.25rem] shrink-0 place-items-center rounded-full border-2 px-[0.3rem] text-[0.8125rem] font-bold" style="border-color: var(--rail)"
        >{num}</span
      >
    {/if}
    <span class="text-[0.875rem] font-semibold whitespace-nowrap">
      {#if c.kind === "else"}{t("route.otherwise")}{:else if c.kind === "if"}{t("route.if")}{:else}{t("route.elseIf")}{/if}
    </span>
  </span>
{/snippet}

<!-- What a case tests (left) and does (right, in the column shared by all rows). -->
{#snippet caseRow(c: Case, num: string | null, path: CasePath, dead = false)}
  {@const hit = applies(path)}
  {@const dim = (words.length > 0 && !caseMatches(c, words, keywords, delimiter)) || hit === false || (filter !== null && !filter(c))}
  <button
    class={[
      "route-row case-row w-full rounded-field py-1 pr-1 text-left transition-opacity hover:bg-base-content/[0.04]",
      picked === path && "bg-primary/10 ring-2 ring-primary/60",
      hit && picked !== path && "bg-success/10 ring-1 ring-success/35",
      dim && "opacity-70",
    ]}
    style="--row-indent: calc(7rem + 0.375rem)"
    data-nav
    aria-current={picked === path ? "true" : undefined}
    onclick={(e) => {
      e.stopPropagation();
      onpick(path);
    }}
  >
    <span class="case-head flex min-w-0 items-start gap-1.5">
      {@render label(c, num, !!hit, !!trial?.misses.includes(path))}
      <span class="min-w-0">
        {#if trial?.misses.includes(path)}
          <span class="mr-1.5 inline-flex items-center gap-1 rounded-full bg-base-content/10 px-2 text-[0.75rem] leading-5 font-medium text-base-content/80" title={t("trial.missed")}
            >{t("trial.missedShort")}</span
          >
        {/if}
        {#if c.conditions}
          <Conditions shape={conditionShape(c.conditions, false, picked === path ? Infinity : 3)} {words} />
          {#if dead}
            <span class="mt-0.5 flex items-center gap-1 text-[0.8125rem] font-medium text-base-content">
              <span class="inline-flex items-center gap-1 rounded-full bg-warning/25 px-2"><Icon name="alert" size={13} />{t("route.unreachable")}</span>
            </span>
          {/if}
        {:else}
          <span class="text-[0.94rem] text-base-content/70">{t("route.allOthers")}</span>
        {/if}
      </span>
    </span>
    {#if outcomes(c.actions, keywords, delimiter).length === 0 && !stops(c.actions) && c.nested.length}
      <!-- Nothing of its own: what happens is in the check below. -->
      {@const innerCases = c.nested.length === 1 ? cases(c.nested[0]) : []}
      {@const numbered = innerCases.filter((x) => x.kind !== "else").length}
      <Possible
        compact
        items={nestedOutcomes(c, keywords, delimiter)}
        heading={innerCases.length > 1 && num
          ? t(innerCases.length > numbered ? "route.dependsOnElse" : "route.dependsOn", { from: `${num}.1`, to: `${num}.${numbered}` })
          : t("route.dependsOnCheck")}
        {words}
      />
    {:else}
      <Outcomes items={outcomes(c.actions, keywords, delimiter)} stop={stops(c.actions)} {singleFolder} {next} {words} {ontarget} />
    {/if}
  </button>
{/snippet}

<!-- `pre`: the number of the case this block sits in ("1"), "" for a rule. -->
{#snippet switchCases(b: Branches, depth: number, prefix: string, pre: string)}
  {@const list = cases(b)}
  {@const single = list.length === 1}
  {@const dead = unreachable(list)}
  <ul class="rails" style="--rail: var(--rail-{Math.min(depth, 3)}); --elbow: 0.95rem">
    {#each segments(list, prefix) as seg}
      {#if seg.kind === "folded"}
        <li>
          <button
            class="flex w-full items-center gap-1 rounded-field px-1 py-0.5 text-left text-[0.875rem] text-base-content/75 hover:bg-base-content/[0.04]"
            onclick={(e) => {
              e.stopPropagation();
              unfolded.add(prefix);
            }}
          >
            <Icon name="chevronRight" size={14} />{seg.count === 1 ? t("route.otherCase") : t("route.otherCases", { count: seg.count })}
          </button>
        </li>
      {:else}
      {@const c = seg.c}
      {@const i = seg.i}
      {@const path = prefix + i}
      {@const num = single || c.kind === "else" ? null : pre ? `${pre}.${c.number}` : String(c.number)}
      {@const own = single ? pre : pre ? `${pre}.${c.number}` : String(c.number)}
      <li>
        <!-- A case with checks inside: row and checks in one frame, so they read as one. -->
        <div class={c.nested.length > 0 ? "case-frame" : undefined}>
        {@render caseRow(c, num, path, dead.has(i))}
        {#each c.nested as n, j}
          {@const multi = cases(n).length > 1}
          {@const color = `var(--rail-${Math.min(depth + 1, 3)})`}
          {@const fold = id ? `${id}|${path}.${j}` : ""}
          {@const folded = !!fold && folds.has(fold)}
          <!-- A check inside this case: a box of its own, one rail deeper, saying whose it is. -->
          <!-- Hangs off its case's number by an elbow, indented: a child, not a sibling. -->
          <div
            class="nested-box relative mx-1 mt-0.5 mb-1 ml-[1.6rem] rounded-field border-l-2 py-1.5 pr-1.5 pl-2"
            style="--box: {color}; border-color: {color}; background: color-mix(in oklab, {color} var(--nest-tint), transparent)"
          >
            <p class={["flex items-start gap-1 pl-0.5 text-[0.875rem] text-base-content/75", !folded && "mb-1", applies(path) === false && "opacity-70"]}>
              {#if fold}
                <button
                  class="mt-0 grid size-6 shrink-0 place-items-center rounded hover:bg-base-content/10"
                  aria-expanded={!folded}
                  aria-label={folded ? t("route.unfold") : t("route.fold")}
                  title={folded ? t("route.unfold") : t("route.fold")}
                  onclick={(e) => {
                    e.stopPropagation();
                    toggleFold(fold);
                  }}><Icon name={folded ? "chevronRight" : "chevronDown"} size={14} /></button
                >
              {/if}
              <span>
                <span class="font-semibold" style="color: {color}">
                  {#if c.kind === "else"}{t("route.nestedInOtherwise")}{:else if own}{t("route.nestedIn", { n: own })}{:else}{t("route.nestedHeadingSingle")}{/if}</span
                >
                {#if multi}<span class="text-base-content/70">· {t("route.casesFirst", { count: cases(n).length })}</span>{/if}
              </span>
            </p>
            {#if folded}
              <div class="pt-1 pb-0.5 pl-6"><Possible items={blockOutcomes(n, keywords, delimiter)} heading={t("route.foldedSummary")} {words} /></div>
            {:else}
              {@render switchCases(n, depth + 1, `${path}.${j}.`, c.kind === "else" ? "" : own)}
            {/if}
          </div>
        {/each}
        </div>
      </li>
      {/if}
    {/each}
  </ul>
{/snippet}

{#if simple}
  {@const hit = applies("0")}
  <button
    class={["w-full rounded-field text-left transition-opacity", hit && "trial-hit-row", hit === false && "opacity-70"]}
    aria-current={picked === "" ? "true" : undefined}
    data-nav
    onclick={() => onpick("")}
  >
    <span class="route-row">
      <span class="flex min-w-0 flex-wrap items-baseline gap-x-2.5 gap-y-0.5">
        {#if rule.name}<span class="font-semibold"><Mark text={rule.name} {words} /></span>{/if}
        <Conditions shape={conditionShape(rule.conditions, false, picked === "" ? Infinity : 3)} {words} />
      </span>
      <Outcomes items={outcomes(rule.actions, keywords, delimiter)} stop={stops(rule.actions)} {singleFolder} {next} {words} {hideParent} {ontarget} />
    </span>
  </button>
{:else}
  {@const folded = !!id && folds.has(id)}
  <div class={["flex items-start gap-1", trial && !trial.hits.length && "opacity-70"]}>
    {#if id}
      <button
        class="-ml-1 mt-0.5 grid size-6 shrink-0 place-items-center rounded hover:bg-base-content/10"
        aria-expanded={!folded}
        aria-label={folded ? t("route.unfold") : t("route.fold")}
        title={folded ? t("route.unfold") : t("route.fold")}
        onclick={() => toggleFold(id)}><Icon name={folded ? "chevronRight" : "chevronDown"} size={15} /></button
      >
    {/if}
    <button class="flex min-w-0 grow flex-wrap items-baseline gap-x-2 text-left" data-nav onclick={() => onpick("")}>
      <span class="font-semibold"><Mark text={rule.name || t("route.switch")} {words} /></span>
      {#if top.length > 1}
        <span class="text-[0.875rem] text-base-content/70">{t("route.casesFirst", { count: top.length })}</span>
      {/if}
    </button>
  </div>
  {#if folded}
    <!-- Folded: where its mail can go, in one line. -->
    <button class="route-row mt-1 w-full rounded-field py-1 pl-6 text-left hover:bg-base-content/[0.04]" onclick={() => onpick("")}>
      <span></span>
      <Possible items={blockOutcomes(rule, keywords, delimiter)} heading={t("route.foldedSummary")} {words} />
    </button>
  {:else}
    <div class="mt-1">
      {@render switchCases(rule, 0, "", "")}
    </div>
  {/if}
{/if}
