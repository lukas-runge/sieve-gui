<script lang="ts">
  // One condition as a field chip and its values: [From] @school.example
  import type { ConditionToken } from "../../rules/route";
  import { t } from "../../i18n.svelte";
  import Mark from "./Mark.svelte";

  let { token, words = [] }: { token: ConditionToken; words?: string[] } = $props();
</script>

<span class="inline-flex max-w-full flex-wrap items-baseline gap-x-1.5 gap-y-0.5" title={token.sentence}>
  <span
    class="rounded-md bg-base-200 px-1.5 py-px text-[0.8rem] font-medium whitespace-nowrap text-base-content/80"
    title={token.fieldTitle}><Mark text={token.field} {words} /></span
  >
  {#if token.negated}<span class="text-[0.85rem] font-semibold text-error">{t("route.not")}</span>{/if}
  {#if token.word}<span class="text-[0.88rem] text-base-content/70">{token.word}</span>{/if}
  {#each token.values as v, i}
    <span class="max-w-full font-mono text-[0.86rem] [overflow-wrap:break-word]"
      >{#if v.before}<span class="text-primary">{v.before}</span>{/if}<Mark text={v.text} {words} />{#if v.after}<span class="text-primary"
          >{v.after}</span
        >{/if}{#if i < token.values.length - 1 || token.more.length}<span class="text-base-content/45">,</span>{/if}</span
    >
  {/each}
  {#if token.more.length}
    <span class="text-[0.82rem] whitespace-nowrap text-base-content/65" title={token.more.join(", ")}
      >{t("route.more", { count: token.more.length })}</span
    >
  {/if}
</span>
