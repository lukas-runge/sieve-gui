<script lang="ts">
  // Where a message goes and what happens to it: the folder like a sorting
  // compartment (path quiet, last folder strong), labels as tags.
  import type { Outcome } from "../../rules/route";
  import { t } from "../../i18n.svelte";
  import Icon, { type IconName } from "../Icon.svelte";
  import Mark from "./Mark.svelte";

  let {
    items,
    stop = false,
    singleFolder = false,
    words = [],
    hideParent = false,
  }: { items: Outcome[]; stop?: boolean; singleFolder?: boolean; words?: string[]; hideParent?: boolean } = $props();

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
          {#if o.path.length > 1 && !hideParent}
            <span class="min-w-0 truncate text-[0.875rem] text-base-content/65"
              >{o.path.length > 2 ? "… / " : ""}<Mark text={o.path.at(-2) ?? ""} {words} />&nbsp;/&nbsp;</span
            >
          {/if}
          <span class="shrink-0 font-semibold"><Mark text={o.path.at(-1) ?? o.text} {words} /></span>
        </span>
      </span>
    {:else if o.kind === "label" || o.kind === "unlabel" || o.kind === "setLabels"}
      {@const h = hue(o.text)}
      <span
        class="inline-flex items-center gap-1 rounded-full border px-2 py-px text-[0.875rem] font-medium whitespace-nowrap"
        style="border-color: oklch(0.7 0.09 {h} / 0.55); background: oklch(0.7 0.09 {h} / 0.12)"
        title={o.kind === "setLabels" ? t("route.setLabelsTitle") : undefined}
      >
        <span class="size-1.5 rounded-full" style="background: oklch(0.62 0.13 {h})"></span>
        {#if o.kind === "unlabel"}{t("route.remove")}{/if}{#if o.kind === "setLabels"}{t("route.only")}{/if}<Mark text={o.text} {words} />
      </span>
    {:else}
      <span class="inline-flex items-center gap-1 text-[0.875rem] whitespace-nowrap text-base-content/80">
        <Icon name={icon[o.kind]} size={14} class="text-base-content/60" /><Mark text={o.text} {words} />
      </span>
    {/if}
  {/each}
  {#if stop}
    <span
      class="inline-flex items-center gap-1 text-[0.875rem] whitespace-nowrap text-base-content/65"
      title={singleFolder ? t("route.stopScriptHint") : t("route.stopHint")}
    >
      <Icon name="stop" size={14} />{singleFolder ? t("route.stopScriptRules") : t("route.stopRules")}
    </span>
  {/if}
</div>
