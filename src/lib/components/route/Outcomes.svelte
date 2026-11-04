<script lang="ts">
  // Where a message goes and what happens to it: the folder like a sorting
  // compartment (path quiet, last folder strong), labels as tags.
  import { labelHue as hue, type Outcome } from "../../rules/route";
  import { t } from "../../i18n.svelte";
  import Icon, { type IconName } from "../Icon.svelte";
  import Mark from "./Mark.svelte";

  let {
    items,
    stop = false,
    singleFolder = false,
    words = [],
    hideParent = false,
    next = null,
    fullPath = false,
    ontarget,
  }: {
    items: Outcome[];
    stop?: boolean;
    singleFolder?: boolean;
    words?: string[];
    hideParent?: boolean;
    /** The script that runs next: a stop in single-folder mode continues there. */
    next?: string | null;
    /** The whole folder path, not just the parent. */
    fullPath?: boolean;
    /** Clicking a folder or label shows everything that goes there. */
    ontarget?: (o: Outcome) => void;
  } = $props();

  /** Folders and labels lead to everything else that goes there. */
  function target(o: Outcome) {
    return (e: MouseEvent | KeyboardEvent) => {
      if (!ontarget || !o.target) return;
      if (e instanceof KeyboardEvent && e.key !== "Enter") return;
      e.stopPropagation();
      e.preventDefault();
      ontarget(o);
    };
  }

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

</script>

<div class="flex min-w-0 flex-wrap items-center gap-x-2 gap-y-1">
  {#each items as o}
    {#if o.kind === "move" || o.kind === "copy"}
      <!-- svelte-ignore a11y_no_noninteractive_tabindex: a link when `ontarget` is set -->
      <span
        class={["flex min-w-0 items-center gap-1.5", ontarget && "target rounded-sm"]}
        title={ontarget ? t("target.title", { name: o.text }) : o.kind === "copy" ? t("route.copyTitle", { path: o.text }) : o.text}
        role={ontarget ? "link" : undefined}
        tabindex={ontarget ? 0 : undefined}
        onclick={target(o)}
        onkeydown={target(o)}
      >
        <Icon name={icon[o.kind]} size={15} class="text-primary" />
        <span class="flex min-w-0 items-baseline">
          <!-- The direct parent says enough (the full path is in the tooltip),
               unless the search found a word higher up. -->
          {#if o.path.length > 1 && !hideParent}
            <span class="min-w-0 truncate text-[0.875rem] text-base-content/70"
              >{#if words.length || fullPath}<Mark text={o.path.slice(0, -1).join(" / ")} {words} />{:else}{o.path.length > 2 ? "… / " : ""}<Mark
                  text={o.path.at(-2) ?? ""}
                  {words}
                />{/if}&nbsp;/&nbsp;</span
            >
          {/if}
          {#if o.text === ""}
            <span class="text-[0.875rem] text-base-content/70 italic">{t("route.noFolder")}</span>
          {:else}
            <span class="shrink-0 font-semibold"><Mark text={o.path.at(-1) ?? o.text} {words} /></span>
          {/if}
        </span>
      </span>
    {:else if o.kind === "label" || o.kind === "unlabel" || o.kind === "setLabels"}
      {@const h = hue(o.text)}
      <!-- svelte-ignore a11y_no_noninteractive_tabindex: a link when `ontarget` is set -->
      <span
        class={["inline-flex items-center gap-1 rounded-full border px-2 py-px text-[0.875rem] font-medium whitespace-nowrap", ontarget && o.target && "target"]}
        style="border-color: color-mix(in oklab, var(--color-base-content) 22%, transparent); background: color-mix(in oklab, var(--color-base-content) 4%, transparent)"
        title={o.text.startsWith("$")
          ? t("route.unnamedLabel", { name: o.text })
          : ontarget && o.target
            ? t("target.title", { name: o.text })
            : o.kind === "setLabels"
              ? t("route.setLabelsTitle")
              : undefined}
        role={ontarget && o.target ? "link" : undefined}
        tabindex={ontarget && o.target ? 0 : undefined}
        onclick={target(o)}
        onkeydown={target(o)}
      >
        <span class="size-2 rounded-full" style="background: oklch(0.62 0.13 {h})"></span>
        {#if o.kind === "unlabel"}{t("route.remove")}{/if}{#if o.kind === "setLabels"}{t("route.only")}{/if}<Mark text={o.text} {words} />
      </span>
    {:else}
      <span class="inline-flex items-center gap-1 text-[0.875rem] whitespace-nowrap text-base-content/80">
        <Icon name={icon[o.kind]} size={14} class="text-base-content/70" /><Mark text={o.text} {words} />
      </span>
    {/if}
  {/each}
  {#if stop}
    <span
      class="inline-flex items-start gap-1 text-[0.875rem] text-base-content/70"
      title={singleFolder ? t("route.stopScriptHint") : t("route.stopHint")}
    >
      <Icon name="stop" size={14} class="mt-[0.2rem]" />{singleFolder ? (next ? t("route.stopScriptNext", { name: next }) : t("route.stopScriptRules")) : t("route.stopRules")}
    </span>
  {/if}
</div>

<style>
  .target {
    cursor: pointer;
  }
  .target:hover {
    text-decoration: underline;
    text-underline-offset: 3px;
  }
  .target:focus-visible {
    outline: 2px solid var(--color-primary);
    outline-offset: 2px;
  }
</style>
