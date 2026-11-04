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
  <!-- A condition group: an indented bracket with a heading saying how many
       must match, and "or"/"and" sitting on the bracket between its parts,
       like the connectors of a route. Groups inside groups nest the same way. -->
  {@const negative = shape.mode === "none" || shape.mode === "notAll"}
  {@const joiner = shape.mode === "any" || shape.mode === "none" ? t("route.or") : t("route.and")}
  <div class={["grid", nested && "mt-0.5"]}>
    <span class={["text-[0.75rem] font-bold tracking-wide uppercase", negative ? "text-error" : "text-base-content/85"]}>{t(heading[shape.mode])}</span>
    <ul class="ml-1 grid border-l-2 border-base-content/35 pl-3">
      {#each shape.items as item, i}
        {#if i > 0}
          <li class="-ml-[1.05rem] py-px leading-none" aria-hidden="true">
            <span class="inline-block rounded-full bg-base-200 px-1.5 text-[0.8125rem] leading-5 font-medium text-base-content/85 ring-1 ring-base-300">{joiner}</span>
          </li>
        {/if}
        <li class="min-w-0 py-0.5">
          {#if item.kind === "token"}<Token token={item.token} {words} />{:else}<Self shape={item.shape} {words} nested />{/if}
        </li>
      {/each}
    </ul>
  </div>
{/if}
