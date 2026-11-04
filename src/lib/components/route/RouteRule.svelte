<script lang="ts">
  // A rule on the route. A simple rule is one line: what it tests, where the
  // message goes. A rule with several cases is a switch: its cases hang off a
  // rail in order, and the first that matches applies. A check inside a case
  // sits in a box of its own, labeled with its case, on a rail one level
  // deeper and in the next color. Every case can be picked to edit it.
  import type { Rule } from "#tauri/types";
  import { t } from "../../i18n.svelte";
  import type { Branches } from "../../rules/view";
  import { caseMatches, cases, conditionShape, isSimple, outcomes, stops, type Case, type CasePath } from "../../rules/route";
  import Conditions from "./Conditions.svelte";
  import Icon from "../Icon.svelte";
  import Mark from "./Mark.svelte";
  import Outcomes from "./Outcomes.svelte";

  let {
    rule,
    delimiter,
    keywords,
    singleFolder,
    words = [],
    picked = null,
    hideParent = false,
    trial = null,
    onpick,
  }: {
    rule: Rule;
    delimiter: string | null;
    keywords: Record<string, string>;
    singleFolder: boolean;
    /** Search words: matches are marked, cases without any are dimmed. */
    words?: string[];
    /** The case open in the inspector, if this rule is. */
    picked?: CasePath | null;
    /** The folder path is shown once above a run of rules filing into the same parent. */
    hideParent?: boolean;
    /** A sample message's way: the cases that applied to it. */
    trial?: { hits: CasePath[]; reached: boolean } | null;
    onpick: (path: CasePath) => void;
  } = $props();

  /** Applied to the sample message: true; tried and not applied: false; no test: null. */
  const applies = (path: CasePath) => (trial ? trial.hits.includes(path) : null);

  const simple = $derived(isSimple(rule));
  const top = $derived(cases(rule));
</script>

<!-- The case label: "1 if", "2 else if", "otherwise"; "if" / "and if" for a
     check with a single case. Fixed width, so conditions line up. -->
{#snippet label(c: Case, single: boolean, depth: number, hit: boolean)}
  <span class="flex w-[6.6rem] shrink-0 items-center gap-1.5 pt-px" style="color: var(--rail)">
    {#if hit}
      <span class="grid size-[1.25rem] shrink-0 place-items-center rounded-full bg-success text-success-content" title={t("trial.applies")}
        ><Icon name="check" size={13} /></span
      >
    {:else if c.kind !== "else" && !single}
      <span class="grid size-[1.25rem] shrink-0 place-items-center rounded-full border-2 text-[0.8rem] font-bold" style="border-color: var(--rail)"
        >{c.number}</span
      >
    {/if}
    <span class="text-[0.875rem] font-semibold whitespace-nowrap">
      {#if c.kind === "else"}{t("route.otherwise")}{:else if single}{depth > 0 ? t("route.andIf") : t("route.if")}{:else if c.kind === "if"}{t(
          "route.if",
        )}{:else}{t("route.elseIf")}{/if}
    </span>
  </span>
{/snippet}

<!-- What a case tests (left) and does (right, in the column shared by all rows). -->
{#snippet caseRow(c: Case, single: boolean, depth: number, path: CasePath)}
  {@const hit = applies(path)}
  {@const dim = (words.length > 0 && !caseMatches(c, words, keywords, delimiter)) || hit === false}
  <button
    class={[
      "route-row case-row w-full rounded-field py-1 pr-1 text-left transition-opacity hover:bg-base-content/[0.04]",
      picked === path && "bg-primary/10 ring-1 ring-primary/35",
      hit && picked !== path && "bg-success/10 ring-1 ring-success/35",
      dim && "opacity-35",
    ]}
    style="--row-indent: calc(6.6rem + 0.375rem)"
    aria-current={picked === path ? "true" : undefined}
    onclick={(e) => {
      e.stopPropagation();
      onpick(path);
    }}
  >
    <span class="flex min-w-0 items-start gap-1.5">
      {@render label(c, single, depth, !!hit)}
      <span class="min-w-0">
        {#if c.conditions}
          <Conditions shape={conditionShape(c.conditions, false, picked === path ? Infinity : 3)} {words} />
        {:else}
          <span class="text-[0.94rem] text-base-content/70">{t("route.allOthers")}</span>
        {/if}
      </span>
    </span>
    <Outcomes items={outcomes(c.actions, keywords, delimiter)} stop={stops(c.actions)} {singleFolder} {words} />
  </button>
{/snippet}

{#snippet switchCases(b: Branches, depth: number, prefix: string)}
  {@const list = cases(b)}
  {@const single = list.length === 1}
  <ul class="rails" style="--rail: var(--rail-{Math.min(depth, 3)}); --elbow: 0.95rem">
    {#each list as c, i}
      {@const path = prefix + i}
      <li>
        {@render caseRow(c, single, depth, path)}
        {#each c.nested as n, j}
          {@const multi = cases(n).length > 1}
          <!-- A check inside this case: a box of its own, one rail deeper. -->
          <div
            class="my-1 ml-1 rounded-field border-l-2 py-1.5 pr-1.5 pl-2"
            style="border-color: var(--rail-{Math.min(depth + 1, 3)}); background: color-mix(in oklab, var(--rail-{Math.min(depth + 1, 3)}) 6%, transparent)"
          >
            {#if multi}
              <p class={["mb-1 pl-1 text-[0.875rem] text-base-content/70", applies(path) === false && "opacity-40"]}>
                <span class="font-semibold" style="color: var(--rail-{Math.min(depth + 1, 3)})"
                  >{single && depth === 0 ? t("route.nestedHeadingSingle") : t("route.nestedHeading", { n: c.number })}</span
                >{t("route.firstMatchTail")}
              </p>
            {/if}
            {@render switchCases(n, depth + 1, `${path}.${j}.`)}
          </div>
        {/each}
      </li>
    {/each}
  </ul>
{/snippet}

{#if simple}
  {@const hit = applies("0")}
  <button
    class={["w-full rounded-field text-left transition-opacity", hit && "trial-hit-row", hit === false && "opacity-40"]}
    aria-current={picked === "" ? "true" : undefined}
    onclick={() => onpick("")}
  >
    <span class="route-row">
      <span class="flex min-w-0 flex-wrap items-baseline gap-x-2.5 gap-y-0.5">
        {#if rule.name}<span class="text-[0.875rem] font-medium text-base-content/70"><Mark text={rule.name} {words} /></span>{/if}
        <Conditions shape={conditionShape(rule.conditions, false, picked === "" ? Infinity : 3)} {words} />
      </span>
      <Outcomes items={outcomes(rule.actions, keywords, delimiter)} stop={stops(rule.actions)} {singleFolder} {words} {hideParent} />
    </span>
  </button>
{:else}
  <button class={["flex w-full flex-wrap items-baseline gap-x-2 text-left", trial && !trial.hits.length && "opacity-40"]} onclick={() => onpick("")}>
    <span class="font-semibold"><Mark text={rule.name || t("route.switch")} {words} /></span>
    {#if top.length > 1}
      <span class="text-[0.875rem] text-base-content/65">{t("route.casesFirst", { count: top.length })}</span>
    {/if}
  </button>
  <div class="mt-1">
    {@render switchCases(rule, 0, "")}
  </div>
{/if}
