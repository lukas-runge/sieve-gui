<script lang="ts">
  // A rule on the route. A simple rule is one line: what it tests, where the
  // message goes. A rule with branches is a switch: its cases hang off a rail
  // in order (the first that matches applies), and a check inside a case
  // hangs off a rail of its own, one level deeper and in the next color.
  import type { Rule } from "#tauri/types";
  import { t } from "../../i18n.svelte";
  import type { Branches } from "../../rules/view";
  import { cases, conditionShape, isSimple, outcomes, stops, type Case } from "../../rules/route";
  import Conditions from "./Conditions.svelte";
  import Outcomes from "./Outcomes.svelte";

  let {
    rule,
    delimiter,
    keywords,
    singleFolder,
  }: { rule: Rule; delimiter: string | null; keywords: Record<string, string>; singleFolder: boolean } = $props();

  const simple = $derived(isSimple(rule));
</script>

<!-- The case badge: its number (first match applies), "otherwise", or "and if"
     for a check with a single case. -->
{#snippet badge(c: Case, single: boolean, depth: number)}
  {#if c.kind === "else"}
    <span
      class="mt-px shrink-0 rounded-full border border-dashed px-1.5 text-[0.72rem] leading-[1.1rem] font-semibold whitespace-nowrap"
      style="border-color: var(--rail); color: var(--rail)">{t("route.otherwise")}</span
    >
  {:else if single}
    <span
      class="mt-px shrink-0 rounded-full border px-1.5 text-[0.72rem] leading-[1.1rem] font-semibold whitespace-nowrap"
      style="border-color: var(--rail); color: var(--rail)">{depth > 0 ? t("route.andIf") : t("route.if")}</span
    >
  {:else}
    <span
      class="mt-px grid size-[1.2rem] shrink-0 place-items-center rounded-full border-2 text-[0.7rem] font-bold"
      style="border-color: var(--rail); color: var(--rail)">{c.number}</span
    >
  {/if}
{/snippet}

<!-- What a case tests (left) and does (right, in the column shared by all rows). -->
{#snippet caseRow(c: Case, single: boolean, depth: number)}
  <div class="route-row py-1">
    <div class="flex min-w-0 items-start gap-2">
      {@render badge(c, single, depth)}
      <div class="min-w-0">
        {#if c.conditions}<Conditions shape={conditionShape(c.conditions)} />{/if}
      </div>
    </div>
    <Outcomes items={outcomes(c.actions, keywords, delimiter)} stop={stops(c.actions)} {singleFolder} />
  </div>
{/snippet}

{#snippet switchCases(b: Branches, depth: number)}
  {@const list = cases(b)}
  {@const single = list.length === 1}
  <ul class="rails" style="--rail: var(--rail-{Math.min(depth, 3)})">
    {#each list as c}
      <li class={c.kind === "else" ? "otherwise" : ""}>
        {@render caseRow(c, single, depth)}
        {#each c.nested as n}{@render switchCases(n, depth + 1)}{/each}
      </li>
    {/each}
  </ul>
{/snippet}

{#if simple}
  <div class="route-row">
    <div class="min-w-0">
      <Conditions shape={conditionShape(rule.conditions)} />
    </div>
    <Outcomes items={outcomes(rule.actions, keywords, delimiter)} stop={stops(rule.actions)} {singleFolder} />
  </div>
{:else}
  <div class="flex items-baseline gap-2">
    <span class="font-semibold">{rule.name || t("route.switch")}</span>
    {#if cases(rule).length > 1}
      <span class="text-[0.8rem] text-base-content/55" title={t("route.firstMatch")}>
        {t("route.cases", { count: cases(rule).length })}
      </span>
    {/if}
  </div>
  <div class="mt-1">
    {@render switchCases(rule, 0)}
  </div>
{/if}
