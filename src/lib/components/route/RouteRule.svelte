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
  import Outcomes from "./Outcomes.svelte";

  let {
    rule,
    delimiter,
    keywords,
    singleFolder,
    words = [],
    picked = null,
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
    onpick: (path: CasePath) => void;
  } = $props();

  const simple = $derived(isSimple(rule));
  const top = $derived(cases(rule));
</script>

<!-- The case badge: its number, "otherwise", or "if" / "and if" for a single case. -->
{#snippet badge(c: Case, single: boolean, depth: number)}
  {#if c.kind === "else"}
    <span
      class="mt-px shrink-0 rounded-full border border-dashed px-1.5 text-[0.74rem] leading-[1.15rem] font-semibold whitespace-nowrap"
      style="border-color: var(--rail); color: var(--rail)">{t("route.otherwise")}</span
    >
  {:else if single}
    <span
      class="mt-px shrink-0 rounded-full border px-1.5 text-[0.74rem] leading-[1.15rem] font-semibold whitespace-nowrap"
      style="border-color: var(--rail); color: var(--rail)">{depth > 0 ? t("route.andIf") : t("route.if")}</span
    >
  {:else}
    <span
      class="mt-px grid size-[1.25rem] shrink-0 place-items-center rounded-full border-2 text-[0.72rem] font-bold"
      style="border-color: var(--rail); color: var(--rail)">{c.number}</span
    >
  {/if}
{/snippet}

<!-- What a case tests (left) and does (right, in the column shared by all rows). -->
{#snippet caseRow(c: Case, single: boolean, depth: number, path: CasePath)}
  {@const dim = words.length > 0 && !caseMatches(c, words, keywords, delimiter)}
  <button
    class={[
      "route-row w-full rounded-field py-1 pr-1 text-left transition-opacity hover:bg-base-content/[0.04]",
      picked === path && "bg-primary/10 ring-1 ring-primary/35",
      dim && "opacity-35",
    ]}
    aria-current={picked === path ? "true" : undefined}
    onclick={(e) => {
      e.stopPropagation();
      onpick(path);
    }}
  >
    <span class="flex min-w-0 items-start gap-2">
      {@render badge(c, single, depth)}
      <span class="min-w-0">
        {#if c.conditions}<Conditions shape={conditionShape(c.conditions)} {words} />{/if}
      </span>
    </span>
    <Outcomes items={outcomes(c.actions, keywords, delimiter)} stop={stops(c.actions)} {singleFolder} {words} />
  </button>
{/snippet}

{#snippet switchCases(b: Branches, depth: number, prefix: string)}
  {@const list = cases(b)}
  {@const single = list.length === 1}
  <ul class="rails" style="--rail: var(--rail-{Math.min(depth, 3)}); --elbow: 0.9rem">
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
              <p class="mb-1 pl-1 text-[0.82rem] text-base-content/70">
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
  <button class="w-full rounded-field text-left" aria-current={picked === "" ? "true" : undefined} onclick={() => onpick("")}>
    <span class="route-row">
      <span class="min-w-0">
        <Conditions shape={conditionShape(rule.conditions)} {words} />
      </span>
      <Outcomes items={outcomes(rule.actions, keywords, delimiter)} stop={stops(rule.actions)} {singleFolder} {words} />
    </span>
  </button>
{:else}
  <button class="flex w-full flex-wrap items-baseline gap-x-2 text-left" onclick={() => onpick("")}>
    <span class="font-semibold">{rule.name || t("route.switch")}</span>
    {#if top.length > 1}
      <span class="text-[0.84rem] text-base-content/65">{t("route.casesFirst", { count: top.length })}</span>
    {/if}
  </button>
  <div class="mt-1">
    {@render switchCases(rule, 0, "")}
  </div>
{/if}
