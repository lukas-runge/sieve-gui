<script lang="ts">
  // A case's conditions: one line, or a group under a heading that says how
  // many must match; groups inside groups get a box of their own.
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
  <span class="text-[0.92rem] text-base-content/70">{t("route.always")}</span>
{:else if shape.kind === "line"}
  <Token token={shape.token} {words} />
{:else}
  <div class={["grid gap-1.5", nested && "rounded-field bg-base-200/60 px-2.5 py-2"]}>
    <span class="text-[0.84rem] font-medium text-base-content/70">{t(heading[shape.mode])}</span>
    <ul class="grid gap-1.5 pl-1">
      {#each shape.items as item}
        <li class="flex items-baseline gap-2">
          {#if item.kind === "token"}
            <span class="size-1 shrink-0 translate-y-[-0.2em] rounded-full bg-base-content/40" aria-hidden="true"></span>
            <Token token={item.token} {words} />
          {:else}
            <div class="min-w-0 grow"><Self shape={item.shape} {words} nested /></div>
          {/if}
        </li>
      {/each}
    </ul>
  </div>
{/if}
