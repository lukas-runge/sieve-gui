<script lang="ts">
  // A case's conditions: one line, or a group under a heading that says how
  // many must match, with "or" / "and" between its items. The group's line
  // on the left has the color of its kind: blue for "any", violet for "all",
  // red for "none" / "not all"; groups inside groups get a box of their own.
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
  const color: Record<string, string> = {
    any: "var(--color-primary)",
    all: "var(--color-accent)",
    none: "var(--color-error)",
    notAll: "var(--color-error)",
  };
</script>

{#if shape.kind === "always"}
  <span class="text-[0.94rem] text-base-content/75">{t("route.always")}</span>
{:else if shape.kind === "line"}
  <Token token={shape.token} {words} />
{:else}
  {@const c = color[shape.mode]}
  {@const joiner = shape.mode === "any" || shape.mode === "none" ? t("route.or") : t("route.and")}
  <div class={["grid gap-1", nested && "rounded-field bg-base-200/60 py-1.5 pr-2"]}>
    <span class="text-[0.875rem] font-semibold" style="color: {c}">{t(heading[shape.mode])}</span>
    <ul class="grid border-l-2 pl-2.5" style="border-color: color-mix(in oklab, {c} 55%, transparent)">
      {#each shape.items as item, i}
        {#if i > 0}
          <li class="py-0.5 text-[0.8rem] font-semibold" style="color: {c}" aria-hidden="true">{joiner}</li>
        {/if}
        <li class="min-w-0">
          {#if item.kind === "token"}<Token token={item.token} {words} />{:else}<Self shape={item.shape} {words} nested />{/if}
        </li>
      {/each}
    </ul>
  </div>
{/if}
