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
  <!-- A condition group: a neutral bracket, unlike the colored rails of cases,
       so "which conditions" never looks like "which case". -->
  {@const negative = shape.mode === "none" || shape.mode === "notAll"}
  {@const joiner = shape.mode === "any" || shape.mode === "none" ? t("route.or") : t("route.and")}
  <div class="grid gap-1 rounded-md border border-base-content/15 bg-base-content/[0.025] py-1.5 pr-2 pl-2">
    <span
      class={[
        "justify-self-start rounded px-1.5 text-[0.75rem] leading-5 font-bold tracking-wide uppercase",
        negative ? "bg-error/12 text-error" : "bg-base-content/8 text-base-content/75",
      ]}>{t(heading[shape.mode])}</span
    >
    <ul class="grid">
      {#each shape.items as item, i}
        {#if i > 0}
          <li class="py-0.5 text-[0.75rem] font-bold tracking-wide text-base-content/55 uppercase" aria-hidden="true">{joiner}</li>
        {/if}
        <li class="min-w-0">
          {#if item.kind === "token"}<Token token={item.token} {words} />{:else}<Self shape={item.shape} {words} nested />{/if}
        </li>
      {/each}
    </ul>
  </div>
{/if}
