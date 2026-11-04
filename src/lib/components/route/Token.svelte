<script lang="ts">
  // One condition as a field chip, how it compares and its values:
  // [From] ends with @school.example
  import type { ConditionToken } from "../../rules/route";
  import { t } from "../../i18n.svelte";
  import Mark from "./Mark.svelte";

  let { token, words = [] }: { token: ConditionToken; words?: string[] } = $props();

</script>

<!-- Field, comparison and first value stay on one line (a long value is cut,
     in full as a tooltip); further values may wrap. -->
<span class="inline-flex max-w-full min-w-0 flex-wrap items-baseline gap-x-1.5 gap-y-0.5" title={token.sentence}>
  <span class="token-head inline-flex max-w-full min-w-0 flex-wrap items-baseline gap-x-1.5">
    <span
      class="shrink-0 rounded-md bg-base-content/[0.07] px-1.5 py-px text-[0.8125rem] font-medium text-base-content/85"
      title={token.fieldTitle}><Mark text={token.field} {words} /></span
    >
    {#if token.word}<span class={["shrink-0 text-[0.875rem]", token.negated ? "font-semibold text-base-content" : "text-base-content/70"]}>{token.word}</span>{/if}
    {#if token.values[0]}{@render value(token.values[0], 0)}{/if}
  </span>
  {#each token.values.slice(1) as v, i}
    {@render value(v, i + 1)}
  {/each}
  {#if token.more.length}
    <span class="text-[0.875rem] whitespace-nowrap text-base-content/70" title={token.more.join(", ")}
      >{t("route.more", { count: token.more.length })}</span
    >
  {/if}
</span>

{#snippet value(v: ConditionToken["values"][number], i: number)}
  <span class={["token-value min-w-0 text-base-content [overflow-wrap:break-word]", token.pattern ? "pattern-value font-mono text-[0.875rem]" : "text-[0.94rem]"]}
    >{#if v.before}<span class="text-base-content/65">{v.before}</span>{/if}<Mark text={v.text} {words} breaks={!token.pattern} />{#if v.after}<span
        class="font-normal text-base-content/70">{v.after}</span
      >{/if}{#if i < token.values.length - 1}<span class="font-normal text-base-content/70">{token.negated ? ", " : t("route.listOr")}</span>{/if}</span
  >
{/snippet}

<style>
  /* Code-like values never break mid-token: cut, in full as a tooltip and once picked. */
  :global(.pattern-value) {
    display: inline-block;
    max-width: 100%;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    vertical-align: bottom;
  }
  :global([aria-current="true"] .pattern-value) {
    white-space: normal;
    overflow-wrap: anywhere;
  }
</style>
