<script lang="ts">
  // One condition as a field chip, how it compares and its values:
  // [From] ends with @school.example
  import type { ConditionToken } from "../../rules/route";
  import { t } from "../../i18n.svelte";
  import Mark from "./Mark.svelte";

  let { token, words = [] }: { token: ConditionToken; words?: string[] } = $props();

  /** "not … a or b" leaves open what "not" covers: say "none of" instead. */
  const noneOf = $derived(token.negated && token.values.length + token.more.length > 1);
</script>

<span class="inline-flex max-w-full flex-wrap items-baseline gap-x-1.5 gap-y-0.5" title={token.sentence}>
  <span
    class="rounded-md bg-base-200 px-1.5 py-px text-[0.8rem] font-medium whitespace-nowrap text-base-content/80"
    title={token.fieldTitle}><Mark text={token.field} {words} /></span
  >
  {#if token.negated && !noneOf}<span class="text-[0.875rem] font-semibold text-error">{t("route.not")}</span>{/if}
  {#if token.word}<span class="text-[0.875rem] text-base-content/70">{token.word}</span>{/if}
  {#if noneOf}<span class="text-[0.875rem] font-semibold text-error">{t("route.noneOf")}</span>{/if}
  {#each token.values as v, i}
    <span class={["max-w-full font-medium [overflow-wrap:break-word]", token.pattern ? "font-mono text-[0.875rem]" : "text-[0.94rem]"]}
      >{#if v.before}<span class="text-primary">{v.before}</span>{/if}<Mark text={v.text} {words} breaks={!token.pattern} />{#if v.after}<span
          class="font-normal text-base-content/70">{v.after}</span
        >{/if}{#if i < token.values.length - 1}<span class="font-normal text-base-content/60">{noneOf ? ", " : t("route.listOr")}</span>{/if}</span
    >
  {/each}
  {#if token.more.length}
    <span class="text-[0.875rem] whitespace-nowrap text-base-content/70" title={token.more.join(", ")}
      >{t("route.more", { count: token.more.length })}</span
    >
  {/if}
</span>
