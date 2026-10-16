<script lang="ts">
  // One or more text values; several values match if any of them matches.
  import { t } from "../i18n.svelte";

  let {
    values,
    label,
    onchange,
  }: { values: string[]; label: string; onchange: (values: string[], merge: boolean) => void } = $props();

  function set(i: number, value: string) {
    onchange(values.map((v, j) => (j === i ? value : v)), true);
  }
</script>

<div class="flex flex-col gap-1.5">
  {#each values as value, i}
    <div class="flex items-center gap-1.5">
      {#if i > 0}<span class="text-sm text-base-content/60 w-8 text-right">{t("sentence.or")}</span>{/if}
      <input
        class="input input-sm font-mono grow"
        aria-label={label}
        placeholder={t("value.placeholder")}
        {value}
        oninput={(e) => set(i, e.currentTarget.value)}
      />
      {#if values.length > 1}
        <button
          class="btn btn-ghost btn-sm btn-square"
          aria-label={t("value.remove")}
          title={t("value.remove")}
          onclick={() => onchange(values.filter((_, j) => j !== i), false)}>✕</button
        >
      {/if}
    </div>
  {/each}
  <button class="btn btn-ghost btn-xs justify-self-start w-fit" onclick={() => onchange([...values, ""], false)}>
    + {t("value.add")}
  </button>
</div>
