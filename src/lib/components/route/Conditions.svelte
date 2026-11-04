<script lang="ts">
  // A case's conditions: one line, or a group in a bracket under a label that
  // says how many must match, with "or" / "and" between its items. Groups
  // inside groups get a bracket of their own.
  import type { ConditionShape } from "../../rules/route";
  import { t, type MessageKey } from "../../i18n.svelte";
  import Self from "./Conditions.svelte";
  import Token from "./Token.svelte";

  let { shape, words = [], nested = false }: { shape: ConditionShape; words?: string[]; nested?: boolean } = $props();

  const heading: Record<string, MessageKey> = {
    any: "route.groupAny",
    all: "route.groupAll",
    none: "route.groupNone",
    notAll: "route.groupNotAll",
  };
</script>

{#if shape.kind === "always"}
  <span class="text-[0.94rem] text-base-content/75">{t("route.always")}</span>
{:else if shape.kind === "line"}
  <Token token={shape.token} {words} />
{:else}
  <!-- A condition group: a thin neutral bracket with a short lead-in, unlike
       the colored rails of cases, so "which conditions" never looks like
       "which case". -->
  {@const negative = shape.mode === "none" || shape.mode === "notAll"}
  {@const joiner = shape.mode === "any" || shape.mode === "none" ? t("route.or") : t("route.and")}
  {#if nested}
    <!-- A group inside a group reads as one line in a light bracket: its
         joiner says how its parts combine; "none"/"not all" say so first. -->
    <span class="inline-flex max-w-full flex-wrap items-baseline gap-x-1.5 gap-y-0.5 rounded-md bg-base-content/[0.045] px-1.5 py-0.5 ring-1 ring-base-content/10">
      {#if negative}<span class="text-[0.75rem] font-bold tracking-wide text-error uppercase">{t(heading[shape.mode])}</span>{/if}
      {#each shape.items as item, i}
        {#if i > 0}<span class="rounded bg-base-content/[0.07] px-1.5 text-[0.75rem] font-bold tracking-wide text-base-content/70 uppercase">{joiner}</span>{/if}
        {#if item.kind === "token"}<Token token={item.token} {words} />{:else}<Self shape={item.shape} {words} nested />{/if}
      {/each}
    </span>
  {:else}
  <div class="grid gap-0.5">
    <span class={["text-[0.75rem] font-bold tracking-wide uppercase", negative ? "text-error" : "text-base-content/75"]}>{t(heading[shape.mode])}</span>
    <ul class="ml-0.5 grid border-l-2 border-base-content/25 pl-2.5">
      <!-- The lead-in says how they combine, so no "or" between them. -->
      {#each shape.items as item}
        <li class="min-w-0 py-0.5" aria-label={joiner}>
          {#if item.kind === "token"}<Token token={item.token} {words} />{:else}<Self shape={item.shape} {words} nested />{/if}
        </li>
      {/each}
    </ul>
  </div>
  {/if}
{/if}
