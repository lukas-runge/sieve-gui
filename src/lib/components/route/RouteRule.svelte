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
  import { blockOutcomes, caseMatches, cases, nestedOutcomes, possibleHeading, unreachable, conditionShape, isSimple, outcomes, stops, type Case, type CasePath, type Outcome } from "../../rules/route";
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

  /** A short name for screen readers: "Fall 1.2, sonst wenn: X enthält y → Hochschule". */
  function caseName(c: Case, num: string | null): string {
    const word = c.kind === "else" ? t("route.otherwise") : c.kind === "if" ? t("route.if") : t("route.elseIf");
    const what = c.conditions ? shapeSentence(conditionShape(c.conditions, false, Infinity)) : t("route.allOthers");
    const does = outcomes(c.actions, keywords, delimiter).map((o) => o.text).join(", ");
    return `${num ? t("route.caseN", { n: num }) + ", " : ""}${word}: ${what}${does ? " → " + does : ""}`;
  }
  function shapeSentence(shape: ReturnType<typeof conditionShape>): string {
    if (shape.kind === "always") return t("route.always");
    if (shape.kind === "line") return shape.token.sentence;
    const joiner = shape.mode === "any" || shape.mode === "none" ? t("route.or") : t("route.and");
    return `${t(groupHeading[shape.mode])} ${shape.items.map((i) => (i.kind === "token" ? i.token.sentence : `(${shapeSentence(i.shape)})`)).join(` ${joiner} `)}`;
  }
  const groupHeading = { any: "route.groupAny", all: "route.groupAll", none: "route.groupNone", notAll: "route.groupNotAll" } as const;

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
{#snippet label(c: Case, num: string | null, hit: boolean, missed: boolean, untried = false)}
  <span class="case-label grid w-[7rem] shrink-0 gap-0.5 pt-px">
  <span class="flex items-center gap-1.5" style="color: var(--rail)">
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
  <!-- In the test: tried and no match, or never tried. -->
  {#if missed}
    <span class="justify-self-start rounded-full bg-base-content/10 px-2 text-[0.75rem] leading-5 font-medium text-base-content/85" title={t("trial.missed")}>{t("trial.missedShort")}</span>
  {:else if untried}
    <span class="text-[0.75rem] text-base-content/85 italic" title={t("trial.notTriedHint")}>{t("trial.notTried")}</span>
  {/if}
  </span>
{/snippet}

<!-- What a case tests (left) and does (right, in the column shared by all rows). -->
{#snippet caseRow(c: Case, num: string | null, path: CasePath, dead = false, pre = "", depth = 0, bare = false)}
  {@const hit = applies(path)}
  {@const dim = (words.length > 0 && !caseMatches(c, words, keywords, delimiter)) || hit === false || (filter !== null && !filter(c))}
  <button
    class={[
      "route-row case-row w-full rounded-field py-1 pr-1 text-left transition-opacity hover:bg-base-content/[0.04]",
      bare && "pl-1.5",
      depth > 0 && "case-deep",
      picked === path && "ring-2 ring-primary/70",
      hit && picked !== path && "bg-success/10 ring-1 ring-success/35",
      dim && "opacity-70",
    ]}
    style="--row-indent: {bare ? '0px' : 'calc(7rem + 0.375rem)'}"
    title={trial && trial.reached && hit === false && !trial.misses.includes(path) ? t("trial.notTriedHint") : undefined}
    data-nav
    aria-label={caseName(c, num)}
    aria-current={picked === path ? "true" : undefined}
    onclick={(e) => {
      e.stopPropagation();
      onpick(path);
    }}
  >
    <span class="case-head flex min-w-0 items-start gap-1.5">
      {#if !bare}
      {@render label(
        c,
        num,
        !!hit,
        !!trial?.misses.includes(path),
        !!trial && (trial.hits.length > 0 || trial.misses.length > 0) && !trial.hits.includes(path) && !trial.misses.includes(path),
      )}
      {/if}
      <span class="min-w-0">
        {#if c.conditions}
          <Conditions shape={conditionShape(c.conditions, false, picked === path ? Infinity : 3)} {words} />
          {#if dead}
            <span class="mt-0.5 flex items-center gap-1 text-[0.8125rem] font-medium text-base-content">
              <span class="inline-flex items-center gap-1 rounded-full bg-warning/25 px-2"><Icon name="alert" size={13} />{t("route.unreachable")}</span>
            </span>
          {/if}
        {:else}
          <!-- Inside a check, "the rest" is the rest of that case's mail. -->
          <span class="text-[0.94rem] text-base-content/75"
            >{pre ? t("route.allOthersIn", { n: pre }) : depth > 0 ? t("route.allOthersAbove") : t("route.allOthers")}</span
          >
        {/if}
      </span>
    </span>
    {#if outcomes(c.actions, keywords, delimiter).length === 0 && !stops(c.actions) && c.nested.length}
      <!-- Nothing of its own: what happens is in the check below. -->
      <span class="flex items-start gap-1 text-[0.8125rem] text-base-content/75"
        ><Icon name="chevronDown" size={13} class="mt-0.5 shrink-0" />{possibleHeading(c, num).replace(/:$/, "")}</span
      >
    {:else}
      <Outcomes items={outcomes(c.actions, keywords, delimiter)} stop={stops(c.actions)} {singleFolder} {next} {words} {ontarget} />
    {/if}
  </button>
{/snippet}

<!-- A check inside a case: a box of its own, one rail deeper, saying whose it is. -->
{#snippet nestedBox(c: Case, n: Branches, j: number, path: CasePath, depth: number, own: string)}
          {@const multi = cases(n).length > 1}
          {@const color = `var(--rail-${Math.min(depth + 1, 3)})`}
          {@const fold = id ? `${id}|${path}.${j}` : ""}
          {@const folded = !!fold && folds.has(fold)}
          <!-- Two kinds of nesting look different before reading: a case that
               decides further (no actions of its own) continues its rail,
               without a box; a check that adds to what a case does is an
               attached section with a "+". -->
          {@const decides = outcomes(c.actions, keywords, delimiter).length === 0 && !stops(c.actions)}
          <div
            class={[
              "relative mt-0.5 mb-1 ml-[1.6rem] py-1 pr-1.5",
              decides ? "pl-1" : "nested-box mx-1 rounded-field border-l-2 border-dashed pl-2",
            ]}
            style="--box: {color}; border-color: {color}; {decides ? '' : `background: color-mix(in oklab, ${color} var(--nest-tint), transparent)`}"
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
                {#if !decides}<span class="mr-1 inline-grid size-4 place-items-center rounded-full text-[0.75rem] font-bold text-base-100" style="background: {color}" aria-hidden="true">+</span>{/if}
                <span class="font-semibold" style="color: {color}">
                  {#if outcomes(c.actions, keywords, delimiter).length === 0 && !stops(c.actions) && !multi}{t(
                    "route.onlyIfAlso",
                  )}{:else if outcomes(c.actions, keywords, delimiter).length === 0 && !stops(c.actions)}{own
                    ? t("route.decidesIn", { n: own })
                    : t("route.decidesHere")}{:else if c.kind === "else"}{t("route.nestedInOtherwise")}{:else if own}{t("route.nestedIn", { n: own })}{:else}{t(
                    "route.nestedHeadingSingle",
                  )}{/if}</span
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
{/snippet}

<!-- `pre`: the number of the case this block sits in ("1"), "" for a rule. -->
{#snippet switchCases(b: Branches, depth: number, prefix: string, pre: string)}
  {@const list = cases(b)}
  {@const single = list.length === 1}
  {@const dead = unreachable(list)}
  {#if single}
    <!-- A check with one case: one row under its heading, no tree. -->
    {@const c = list[0]}
    {@const path = prefix + "0"}
    <div class="pl-1">
      {@render caseRow(c, null, path, false, pre, depth, true)}
      {#each c.nested as n, j}
        {@render nestedBox(c, n, j, path, depth, pre)}
      {/each}
    </div>
  {:else}
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
        {@render caseRow(c, num, path, dead.has(i), pre, depth)}
        {#each c.nested as n, j}
          {@render nestedBox(c, n, j, path, depth, own)}
        {/each}
        </div>
      </li>
      {/if}
    {/each}
  </ul>
  {/if}
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
      <!-- The name sits above the condition, so conditions start at one edge. -->
      <span class="grid min-w-0 gap-y-0.5">
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
    <!-- The title opens the first case; the inspector lists the others from there. -->
    <button class="flex min-w-0 grow flex-wrap items-baseline gap-x-2 text-left" data-nav onclick={() => onpick("0")}>
      <span class="font-semibold"><Mark text={rule.name || t("route.switch")} {words} /></span>
      {#if top.length > 1}
        <span class="text-[0.875rem] text-base-content/70">{t("route.casesFirst", { count: top.length })}</span>
      {/if}
    </button>
  </div>
  {#if folded}
    <!-- Folded: where its mail can go, in one line. -->
    <button class="mt-1 ml-6 block max-w-[40rem] rounded-field py-1 text-left hover:bg-base-content/[0.04]" onclick={() => onpick("")}>
      <Possible items={blockOutcomes(rule, keywords, delimiter)} heading={t("route.foldedSummary")} {words} />
    </button>
  {:else}
    <div class="mt-1">
      {@render switchCases(rule, 0, "", "")}
    </div>
  {/if}
{/if}
