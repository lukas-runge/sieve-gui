<script lang="ts">
  // What may happen depending on cases further down: alternatives, not
  // actions taken here. Quieter than real outcomes, in a dashed frame,
  // folders and labels apart.
  import type { Outcome } from "../../rules/route";
  import { t } from "../../i18n.svelte";
  import Outcomes from "./Outcomes.svelte";

  let {
    items,
    heading,
    words = [],
    compact = false,
  }: {
    items: Outcome[];
    heading: string;
    words?: string[];
    /** Just a quiet line of names (the cases below show the details). */
    compact?: boolean;
  } = $props();

  const folders = $derived(items.filter((o) => o.kind === "move" || o.kind === "copy" || o.kind === "keep"));
  const labels = $derived(items.filter((o) => o.kind === "label" || o.kind === "setLabels"));
  const rest = $derived(items.filter((o) => !folders.includes(o) && !labels.includes(o)));
</script>

{#if compact}
  <!-- A line of what may happen, quieter than real actions but with the same pills. -->
  <span class="flex flex-wrap items-center gap-x-2 gap-y-1 text-[0.8125rem] text-base-content/75">
    <span>{heading}</span>
    <span class="opacity-85"><Outcomes {items} {words} /></span>
  </span>
{:else}
<span class="grid gap-1 rounded-field border border-dashed border-base-content/25 px-2 py-1.5 text-[0.875rem]">
  <span class="text-[0.8125rem] font-medium text-base-content/70">{heading}</span>
  <span class="grid gap-1 opacity-85">
    {#if folders.length}
      <span class="flex flex-wrap items-baseline gap-x-2"><span class="text-[0.8125rem] text-base-content/70">{t("route.possibleFolders")}</span><Outcomes items={folders} {words} /></span>
    {/if}
    {#if labels.length}
      <span class="flex flex-wrap items-baseline gap-x-2"><span class="text-[0.8125rem] text-base-content/70">{t("route.possibleLabels")}</span><Outcomes items={labels} {words} /></span>
    {/if}
    {#if rest.length}<Outcomes items={rest} {words} />{/if}
  </span>
</span>
{/if}
