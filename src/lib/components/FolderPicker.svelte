<script lang="ts">
  // Picks the folder a rule files into: the current one as a path, and on
  // click a searchable list of the server's folders, as a tree until you type.
  // Typing matches every word anywhere in the path ("bank n26"); a name that
  // isn't there yet can be used as a new folder.
  import { tick } from "svelte";
  import { t } from "../i18n.svelte";
  import { folderExists, type ScriptFolders } from "../rules/mailbox";
  import Icon from "./Icon.svelte";
  import Mark from "./route/Mark.svelte";

  let {
    value,
    folders,
    missing = false,
    describedby,
    onpick,
  }: {
    /** The folder, readable (not as written in the script). */
    value: string;
    folders: ScriptFolders;
    /** Not on the server: the field says so. */
    missing?: boolean;
    describedby?: string;
    onpick: (label: string) => void;
  } = $props();

  const parts = (label: string) => (folders.delimiter ? label.split(folders.delimiter) : [label]);

  interface Entry {
    label: string;
    parts: string[];
    /** On the server (or nothing is known about the server). */
    known: boolean;
  }

  /** Every folder once, in tree order (INBOX and what's below it first). */
  const entries = $derived.by(() => {
    const seen = new Set<string>();
    const list: Entry[] = [];
    for (const label of folders.labels) {
      if (seen.has(label)) continue;
      seen.add(label);
      list.push({ label, parts: parts(label), known: folders.existing === null || folderExists(folders.existing, label) });
    }
    const key = (e: Entry) => e.parts.map((p, i) => (i === 0 && p.toUpperCase() === "INBOX" ? "" : p.toLocaleLowerCase())).join("\u0001");
    return list.sort((a, b) => (key(a) < key(b) ? -1 : key(a) > key(b) ? 1 : 0));
  });

  let open = $state(false);
  let query = $state("");
  let active = $state(0);
  let field = $state<HTMLElement>();
  let search = $state<HTMLInputElement>();
  let pop = $state<HTMLElement>();
  let place = $state({ top: 0, left: 0, width: 0, up: false });

  const words = $derived(query.toLocaleLowerCase().split(/[\s/]+/).filter(Boolean));
  const shown = $derived(words.length ? entries.filter((e) => words.every((w) => e.label.toLocaleLowerCase().includes(w))) : entries);
  /** What was typed, as a folder name: "/" between parts becomes the server's separator. */
  const typed = $derived.by(() => {
    const raw = query.trim();
    if (!raw) return "";
    return folders.delimiter && folders.delimiter !== "/" ? raw.split(/\s*\/\s*/).join(folders.delimiter) : raw;
  });
  const offerNew = $derived(!!typed && !entries.some((e) => e.label === typed));
  /** The rows: matching folders, then (if typed and new) the typed name. */
  const options = $derived([...shown.map((e) => ({ label: e.label, entry: e as Entry | null })), ...(offerNew ? [{ label: typed, entry: null }] : [])]);

  function position() {
    if (!field) return;
    const r = field.getBoundingClientRect();
    const up = window.innerHeight - r.bottom < 320 && r.top > window.innerHeight - r.bottom;
    place = { top: up ? r.top - 4 : r.bottom + 4, left: r.left, width: Math.max(r.width, 280), up };
  }

  async function show(initial = "") {
    if (open) return;
    query = initial;
    position();
    open = true;
    await tick();
    pop?.showPopover?.();
    search?.focus();
    const at = initial ? 0 : options.findIndex((o) => o.label === value);
    active = Math.max(0, at);
    await tick();
    pop?.querySelector(`[data-i="${active}"]`)?.scrollIntoView({ block: "center" });
  }

  function hide(refocus = true) {
    if (!open) return;
    open = false;
    pop?.hidePopover?.();
    if (refocus) field?.focus();
  }

  function pick(label: string) {
    onpick(label);
    hide();
  }

  async function move(by: number) {
    if (!options.length) return;
    active = (active + by + options.length) % options.length;
    await tick();
    pop?.querySelector(`[data-i="${active}"]`)?.scrollIntoView({ block: "nearest" });
  }

  function onkey(e: KeyboardEvent) {
    if (e.key === "ArrowDown") (e.preventDefault(), move(1));
    else if (e.key === "ArrowUp") (e.preventDefault(), move(-1));
    else if (e.key === "Enter") {
      e.preventDefault();
      const o = options[active];
      if (o) pick(o.label);
    } else if (e.key === "Escape") {
      e.preventDefault();
      e.stopPropagation();
      hide();
    } else if (e.key === "Tab") hide(false);
  }

  // Closes on a click elsewhere; follows the field while the page scrolls.
  $effect(() => {
    if (!open) return;
    const down = (e: PointerEvent) => {
      const target = e.target as Node;
      if (!pop?.contains(target) && !field?.contains(target)) hide(false);
    };
    const follow = () => position();
    window.addEventListener("pointerdown", down, true);
    window.addEventListener("scroll", follow, true);
    window.addEventListener("resize", follow);
    return () => {
      window.removeEventListener("pointerdown", down, true);
      window.removeEventListener("scroll", follow, true);
      window.removeEventListener("resize", follow);
    };
  });

  // A new query starts at the first match.
  $effect(() => {
    void query;
    active = 0;
  });
</script>

<!-- The folder as a path; the last part (where mail ends up) stands out. -->
<button
  type="button"
  bind:this={field}
  class={["input input-sm w-full cursor-pointer justify-start gap-2 text-left", missing && "input-warning"]}
  aria-label={t("action.folder")}
  aria-haspopup="listbox"
  aria-expanded={open}
  aria-describedby={describedby}
  onclick={() => (open ? hide() : show())}
  onkeydown={(e) => {
    if (e.key === "ArrowDown" || e.key === "Enter" || e.key === " ") {
      e.preventDefault();
      show();
    } else if (e.key.length === 1 && !e.metaKey && !e.ctrlKey && !e.altKey) {
      e.preventDefault();
      show(e.key);
    }
  }}
>
  <Icon name="folder" size={15} class="shrink-0 text-primary" />
  {#if value}
    {@const p = parts(value)}
    <span class="min-w-0 grow truncate">
      {#each p.slice(0, -1) as part}<span class="text-base-content/65">{part}</span><span class="px-1 text-base-content/40">/</span>{/each}<span class="font-medium">{p.at(-1)}</span>
    </span>
  {:else}
    <span class="grow text-base-content/55">{t("folder.choose")}</span>
  {/if}
  <Icon name="chevronDown" size={14} class="shrink-0 text-base-content/55" />
</button>

{#if open}
  <div
    bind:this={pop}
    popover="manual"
    class="m-0 flex max-h-80 flex-col overflow-hidden rounded-box border border-base-300 bg-base-100 p-0 text-base-content shadow-xl"
    style="position: fixed; inset: auto; left: {place.left}px; width: {place.width}px; {place.up ? `bottom: ${window.innerHeight - place.top}px` : `top: ${place.top}px`}"
  >
    <label class="flex items-center gap-2 border-b border-base-300 px-3 py-2">
      <Icon name="search" size={15} class="text-base-content/60" />
      <input
        bind:this={search}
        bind:value={query}
        class="min-w-0 grow bg-transparent text-sm outline-none placeholder:text-base-content/55"
        placeholder={t("folder.search")}
        aria-label={t("folder.search")}
        role="combobox"
        aria-expanded="true"
        aria-controls="folder-options"
        aria-activedescendant={options[active] ? `folder-option-${active}` : undefined}
        onkeydown={onkey}
      />
    </label>
    <ul id="folder-options" role="listbox" class="min-h-0 grow overflow-y-auto p-1" aria-label={t("action.folder")}>
      {#each options as o, i (o.label + (o.entry ? "" : "\u0000new"))}
        <li
          id="folder-option-{i}"
          data-i={i}
          role="option"
          aria-selected={i === active}
          class={["flex cursor-pointer items-center gap-2 rounded-field px-2 py-1 text-sm", i === active ? "bg-primary/12" : "hover:bg-base-200"]}
          onpointerenter={() => (active = i)}
          onpointerdown={(e) => {
            e.preventDefault();
            pick(o.label);
          }}
        >
          {#if !o.entry}
            <Icon name="plus" size={14} class="shrink-0 text-primary" />
            <span class="min-w-0 truncate"><span class="text-base-content/70">{t("folder.new")}</span> <span class="font-medium">{parts(o.label).join(" / ")}</span></span>
          {:else if words.length}
            <!-- Searching: the whole path, matches marked. -->
            <Icon name="folder" size={14} class="shrink-0 {o.entry.known ? 'text-base-content/60' : 'text-warning'}" />
            <span class="min-w-0 grow truncate">
              {#each o.entry.parts.slice(0, -1) as part}<span class="text-base-content/65"><Mark text={part} {words} /></span><span class="px-1 text-base-content/40">/</span>{/each}<span class="font-medium"><Mark text={o.entry.parts.at(-1) ?? ""} {words} /></span>
            </span>
          {:else}
            <!-- Browsing: a tree, each folder under its parent. -->
            <span class="shrink-0" style="width: {(o.entry.parts.length - 1) * 1.1}rem"></span>
            <Icon name="folder" size={14} class="shrink-0 {o.entry.known ? 'text-base-content/60' : 'text-warning'}" />
            <span class={["min-w-0 grow truncate", o.entry.parts.length === 1 && "font-medium"]}>{o.entry.parts.at(-1)}</span>
          {/if}
          {#if o.entry && !o.entry.known}<span class="shrink-0 text-[0.75rem] text-base-content/60">{t("folder.scriptOnly")}</span>{/if}
          {#if o.label === value}<Icon name="check" size={14} class="shrink-0 text-primary" />{/if}
        </li>
      {:else}
        <li class="px-2 py-2 text-sm text-base-content/65">{t("folder.none")}</li>
      {/each}
    </ul>
  </div>
{/if}
