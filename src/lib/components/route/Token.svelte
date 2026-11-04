<script lang="ts">
  // One condition as a field chip and its values: [From] …@school.example
  import type { ConditionToken } from "../../rules/route";
  import { t } from "../../i18n.svelte";

  let { token }: { token: ConditionToken } = $props();
</script>

<span class="inline-flex min-w-0 flex-wrap items-baseline gap-x-1.5 gap-y-0.5" title={token.sentence}>
  <span
    class="rounded-md bg-base-200 px-1.5 py-px text-[0.78rem] font-medium whitespace-nowrap text-base-content/75"
    title={token.fieldTitle}>{token.field}</span
  >
  {#if token.negated}<span class="text-[0.82rem] font-semibold text-error">{t("route.not")}</span>{/if}
  {#if token.word}<span class="text-[0.85rem] text-base-content/60">{token.word}</span>{/if}
  {#each token.values as v, i}
    <span class="max-w-full font-mono text-[0.84rem] [overflow-wrap:anywhere]"
      >{#if v.before}<span class="text-primary/80">{v.before}</span>{/if}{v.text}{#if v.after}<span class="text-primary/80"
          >{v.after}</span
        >{/if}{#if i < token.values.length - 1 || token.more.length}<span class="text-base-content/40">,</span>{/if}</span
    >
  {/each}
  {#if token.more.length}
    <span class="text-[0.8rem] whitespace-nowrap text-base-content/55" title={token.more.join(", ")}
      >{t("route.more", { count: token.more.length })}</span
    >
  {/if}
</span>
