<script lang="ts">
  // Where a message goes and what happens to it: the folder like a sorting
  // compartment (path quiet, last folder strong), labels as tags.
  import type { Outcome } from "../../rules/route";
  import { t } from "../../i18n.svelte";
  import Icon, { type IconName } from "../Icon.svelte";

  let { items, stop = false, singleFolder = false }: { items: Outcome[]; stop?: boolean; singleFolder?: boolean } = $props();

  const icon: Record<Outcome["kind"], IconName> = {
    move: "folder",
    copy: "copy",
    label: "tag",
    unlabel: "tag",
    setLabels: "tag",
    read: "check",
    unread: "mail",
    flag: "flag",
    system: "check",
    forward: "forward",
    reply: "reply",
    reject: "ban",
    delete: "trash",
    keep: "inbox",
  };

  /** A stable, calm hue per label name, so the same label looks the same everywhere. */
  function hue(name: string): number {
    let h = 0;
    for (const ch of name) h = (h * 31 + ch.charCodeAt(0)) % 360;
    return h;
  }
</script>

<div class="flex min-w-0 flex-wrap items-center gap-x-2 gap-y-1">
  {#each items as o}
    {#if o.kind === "move" || o.kind === "copy"}
      <span class="flex min-w-0 items-center gap-1.5" title={o.kind === "copy" ? t("route.copyTitle", { path: o.text }) : o.text}>
        <Icon name={icon[o.kind]} size={15} class="text-primary" />
        <span class="flex min-w-0 items-baseline">
          <!-- The direct parent says enough; the full path is in the tooltip. -->
          {#if o.path.length > 1}
            <span class="min-w-0 truncate text-[0.82rem] text-base-content/50"
              >{o.path.length > 2 ? "… / " : ""}{o.path.at(-2)}&nbsp;/&nbsp;</span
            >
          {/if}
          <span class="shrink-0 font-semibold">{o.path.at(-1) ?? o.text}</span>
        </span>
      </span>
    {:else if o.kind === "label" || o.kind === "unlabel" || o.kind === "setLabels"}
      {@const h = hue(o.text)}
      <span
        class="inline-flex items-center gap-1 rounded-full border px-2 py-px text-[0.82rem] font-medium whitespace-nowrap"
        style="border-color: oklch(0.7 0.09 {h} / 0.55); background: oklch(0.7 0.09 {h} / 0.12)"
        title={o.kind === "setLabels" ? t("route.setLabelsTitle") : undefined}
      >
        <span class="size-1.5 rounded-full" style="background: oklch(0.62 0.13 {h})"></span>
        {#if o.kind === "unlabel"}{t("route.remove")}{/if}{#if o.kind === "setLabels"}{t("route.only")}{/if}{o.text}
      </span>
    {:else}
      <span class="inline-flex items-center gap-1 text-[0.88rem] whitespace-nowrap text-base-content/80">
        <Icon name={icon[o.kind]} size={14} class="text-base-content/55" />{o.text}
      </span>
    {/if}
  {/each}
  {#if stop}
    <span
      class="inline-flex items-center gap-1 text-[0.8rem] whitespace-nowrap text-base-content/50"
      title={singleFolder ? t("route.stopScriptHint") : t("route.stopHint")}
    >
      <Icon name="stop" size={14} />{t("route.stop")}
    </span>
  {/if}
</div>
