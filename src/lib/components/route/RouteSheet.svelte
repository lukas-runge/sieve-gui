<script lang="ts">
  // The script as a route: incoming mail enters at the top and passes every
  // rule in order along the line on the left. Each rule is a stop on that
  // line: a dot for a simple rule, a diamond for a switch with several cases,
  // a square for code sieve-gui shows as written. Clicking a rule opens it in
  // the inspector.
  import { flip } from "svelte/animate";
  import type { ScriptEditor } from "../../editor.svelte";
  import { uid } from "../../editor.svelte";
  import { t } from "../../i18n.svelte";
  import { closesMenu } from "../../menu";
  import { isSimple } from "../../rules/route";
  import { matches, queryWords } from "../../rules/search";
  import { newRule } from "../../rules/view";
  import Icon from "../Icon.svelte";
  import RouteRule from "./RouteRule.svelte";

  let {
    editor,
    delimiter,
    keywords,
    next,
    running,
  }: {
    editor: ScriptEditor;
    delimiter: string | null;
    keywords: Record<string, string>;
    /** The script that runs after this one, if any. */
    next: string | null;
    /** Whether this script runs at all. */
    running: boolean;
  } = $props();

  let query = $state("");
  const words = $derived(queryWords(query));
  /** While searching, only matching items show and the order can't change. */
  const searching = $derived(words.length > 0);
  const shown = $derived(editor.rules.items.map((item) => !searching || matches(item, words, keywords)));
  const shownCount = $derived(shown.filter(Boolean).length);
  let dragFrom = $state<number | null>(null);
  let dropAt = $state<number | null>(null);

  function move(from: number, to: number) {
    const count = editor.rules.items.length;
    if (to < 0 || to >= count || from === to) return;
    editor.edit((rules) => {
      const [item] = rules.items.splice(from, 1);
      rules.items.splice(to, 0, item);
    });
  }

  function duplicate(index: number) {
    editor.focus = "";
    const copy = { ...$state.snapshot(editor.rules.items[index]), uid: uid() };
    if (copy.type === "rule") copy.name = copy.name ? t("rule.copyOf", { name: copy.name }) : "";
    editor.edit((rules) => {
      rules.items.splice(index + 1, 0, copy);
    });
    editor.selected = copy.uid;
  }

  function remove(index: number) {
    const gone = editor.rules.items[index]?.uid;
    editor.edit((rules) => {
      rules.items.splice(index, 1);
    });
    if (editor.selected === gone) editor.selected = null;
  }

  function toggle(index: number) {
    editor.edit((rules) => {
      const item = rules.items[index];
      if (item?.type === "rule") item.enabled = !item.enabled;
    });
  }

  export function addRule() {
    editor.focus = "";
    const item = { type: "rule" as const, ...newRule(t("rule.new")), uid: uid() };
    editor.edit((rules) => {
      rules.items.push(item);
    });
    query = "";
    editor.selected = item.uid;
  }

  function addCode() {
    editor.focus = "";
    const item = { type: "raw" as const, code: "", uid: uid() };
    editor.edit((rules) => {
      rules.items.push(item);
    });
    query = "";
    editor.selected = item.uid;
  }

  function onhandlekey(e: KeyboardEvent, index: number) {
    if (!e.altKey || (e.key !== "ArrowUp" && e.key !== "ArrowDown")) return;
    e.preventDefault();
    const to = index + (e.key === "ArrowUp" ? -1 : 1);
    move(index, to);
    requestAnimationFrame(() => document.querySelector<HTMLElement>(`[data-handle="${to}"]`)?.focus());
  }

  function ondrop(e: DragEvent, at: number) {
    e.preventDefault();
    if (dragFrom !== null) move(dragFrom, dragFrom < at ? at - 1 : at);
    dragFrom = dropAt = null;
  }

  /** Opens a rule (at one of its cases) in the inspector; again closes it. */
  function pick(id: string, path: string) {
    if (editor.selected === id && editor.focus === path) {
      editor.selected = null;
      return;
    }
    editor.selected = id;
    editor.focus = path;
  }
</script>

<section class="route-sheet rounded-box border border-base-300 bg-base-100" aria-label={t("route.heading")}>
  <!-- Search: sticks to the top while the list scrolls. -->
  <div class="sticky top-0 z-20 flex items-center gap-2 rounded-t-box border-b border-base-300 bg-base-100/95 px-3 py-2 backdrop-blur">
    <label class="flex min-w-0 grow items-center gap-2 rounded-field px-1.5 py-1 focus-within:bg-base-200">
      <Icon name="search" size={15} class="text-base-content/45" />
      <input
        id="rule-search"
        type="search"
        class="min-w-0 grow bg-transparent text-sm outline-none placeholder:text-base-content/45"
        placeholder={t("search.placeholder")}
        aria-label={t("search.label")}
        bind:value={query}
        onkeydown={(e) => e.key === "Escape" && (query = "")}
      />
      {#if searching}
        <span class="text-xs whitespace-nowrap text-base-content/60" role="status">
          {t("search.count", { shown: shownCount, total: editor.rules.items.length })}
        </span>
      {/if}
    </label>
    <button class="btn btn-ghost btn-sm gap-1.5 text-base-content/70" onclick={addCode} title={t("raw.explain")}><Icon name="code" size={15} />{t("route.addCode")}</button>
    <button class="btn btn-primary btn-sm gap-1" onclick={addRule}><Icon name="plus" size={15} />{t("editor.addRule")}</button>
  </div>

  <ol class="route relative py-1" role="list">
    <!-- Where the mail comes in. -->
    <li class="route-item text-[0.82rem] text-base-content/55">
      <span></span>
      <span class="route-node grid place-items-center"><Icon name="mail" size={15} /></span>
      <span class="py-2">{t("route.entry")}</span>
    </li>

    {#each editor.rules.items as item, index (item.uid)}
      {@const selected = editor.selected === item.uid}
      {@const shape = item.type === "raw" ? "square" : isSimple(item) ? "dot" : "diamond"}
      <li
        animate:flip={{ duration: 150 }}
        hidden={!shown[index]}
        class={[
          "route-item group relative",
          dragFrom === index && "opacity-40",
          item.type === "rule" && !item.enabled && "route-off",
        ]}
        ondragover={(e) => {
          if (dragFrom === null) return;
          e.preventDefault();
          const rect = e.currentTarget.getBoundingClientRect();
          dropAt = e.clientY < rect.top + rect.height / 2 ? index : index + 1;
        }}
        ondrop={(e) => ondrop(e, dropAt ?? index)}
      >
        {#if dropAt === index && dragFrom !== null}
          <div class="absolute -top-px right-2 left-8 z-10 h-0.5 rounded bg-primary"></div>
        {/if}
        {#if searching}
          <span></span>
        {:else}
          <button
            class="mt-2 grid h-6 w-5 cursor-grab place-items-center rounded text-base-content/35 opacity-0 group-hover:opacity-100 focus-visible:opacity-100 hover:text-base-content/70"
            draggable="true"
            data-handle={index}
            aria-label="{t('rule.drag')} (Alt+↑/↓)"
            title="{t('rule.drag')} (Alt+↑/↓)"
            ondragstart={(e) => {
              dragFrom = index;
              e.dataTransfer?.setData("text/plain", String(index));
            }}
            ondragend={() => (dragFrom = dropAt = null)}
            onkeydown={(e) => onhandlekey(e, index)}
          >
            <Icon name="grip" size={14} />
          </button>
        {/if}
        <span class="route-node grid place-items-center pt-[0.95rem]">
          <span class="route-mark route-mark-{shape}" class:selected></span>
        </span>
        <div class={["my-0.5 flex min-w-0 items-start gap-2 rounded-field", selected && "bg-primary/[0.05] ring-1 ring-primary/25"]}>
          <div class="min-w-0 grow rounded-field px-2.5 py-2 hover:bg-base-200/50">
            {#if item.type === "rule"}
              <RouteRule
                rule={item}
                {delimiter}
                {keywords}
                singleFolder={editor.rules.singleFolder}
                {words}
                picked={selected ? editor.focus : null}
                onpick={(path) => pick(item.uid, path)}
              />
            {:else}
              <button class="w-full text-left" aria-current={selected ? "true" : undefined} onclick={() => pick(item.uid, "")}>
                <span class="flex items-baseline gap-2">
                  <span class="font-semibold">{t("raw.heading")}</span>
                  {#if editor.roundcubeRisks?.rawBlocks.includes(index)}
                    <span class="badge badge-sm badge-soft badge-warning">{t("roundcube.badge")}</span>
                  {/if}
                </span>
                <pre class="mt-1 max-h-16 overflow-hidden font-mono text-[0.8rem] leading-snug text-base-content/65">{item.code.trim() || "…"}</pre>
              </button>
            {/if}
          </div>
          <div class="flex shrink-0 items-center gap-1 pt-2 pr-1.5">
            {#if item.type === "rule" && !item.enabled}
              <span class="rounded-full bg-base-200 px-2 text-[0.75rem] leading-5 font-medium text-base-content/60">{t("route.off")}</span>
            {/if}
            <!-- Always there on wide sheets, so destinations line up. -->
            <span class="hidden w-28 truncate text-right text-[0.8rem] text-base-content/45 @2xl:inline" title={item.type === "rule" ? item.name : undefined}
              >{item.type === "rule" && isSimple(item) ? item.name : ""}</span
            >
            {#if item.type === "rule" && editor.roundcubeRisks?.rules.includes(index)}
              <span class="badge badge-sm badge-soft badge-warning" title={t("roundcube.title")}>{t("roundcube.badge")}</span>
            {/if}
            <div class="dropdown dropdown-end">
              <div
                tabindex="0"
                role="button"
                class="grid size-7 place-items-center rounded-field text-base-content/45 opacity-0 group-hover:opacity-100 focus:opacity-100 hover:bg-base-200 {selected ? 'opacity-100' : ''}"
                aria-label={t("menu.more")}
              >
                <Icon name="more" size={16} />
              </div>
              <ul tabindex="-1" {@attach closesMenu} class="dropdown-content menu z-30 w-52 rounded-box border border-base-300 bg-base-100 p-1 shadow-lg">
                {#if item.type === "rule"}
                  <li><button onclick={() => toggle(index)}>{item.enabled ? t("route.turnOff") : t("route.turnOn")}</button></li>
                {/if}
                <li><button disabled={index === 0} onclick={() => move(index, index - 1)}>{t("rule.moveUp")}</button></li>
                <li><button disabled={index === editor.rules.items.length - 1} onclick={() => move(index, index + 1)}>{t("rule.moveDown")}</button></li>
                <li><button onclick={() => duplicate(index)}>{t("rule.duplicate")}</button></li>
                <li><button class="text-error" onclick={() => remove(index)}>{t("rule.delete")}</button></li>
              </ul>
            </div>
          </div>
        </div>
        {#if dropAt === index + 1 && index === editor.rules.items.length - 1 && dragFrom !== null}
          <div class="absolute right-2 -bottom-px left-8 z-10 h-0.5 rounded bg-primary"></div>
        {/if}
      </li>
    {/each}

    {#if searching && shownCount === 0}
      <li class="route-item">
        <span></span><span></span>
        <p class="py-3 text-sm text-base-content/60">{t("search.none")}</p>
      </li>
    {/if}
    {#if editor.rules.items.length === 0}
      <li class="route-item">
        <span></span><span></span>
        <div class="grid justify-items-start gap-2 py-4">
          <p class="text-base-content/70">{t("route.empty")}</p>
          <button class="btn btn-primary btn-sm gap-1" onclick={addRule}><Icon name="plus" size={15} />{t("route.emptyAdd")}</button>
        </div>
      </li>
    {/if}

    <!-- Where the mail goes when no rule kept it. -->
    <li class="route-item route-end text-[0.82rem] text-base-content/55">
      <span></span>
      <span class="route-node grid place-items-center pt-0.5"><span class="route-mark route-mark-end"></span></span>
      <span class="py-2">
        {#if !running}{t("route.exitOff")}{:else if next}{t("route.exitNext", { name: next })}{:else}{t("route.exitInbox")}{/if}
      </span>
    </li>
  </ol>
</section>

<style>
  /* The line every rule sits on: handle | line with the rule's mark | rule. */
  .route-item {
    display: grid;
    grid-template-columns: 1.4rem 1.5rem minmax(0, 1fr);
    padding-right: 0.25rem;
  }
  .route::before {
    content: "";
    position: absolute;
    top: 1.25rem;
    bottom: 1.4rem;
    left: calc(1.4rem + 0.75rem - 1px);
    border-left: 2px solid var(--spine);
  }
  .route-node {
    position: relative;
    z-index: 1;
    align-content: start;
    color: var(--color-base-content);
  }
  .route-item:first-child .route-node {
    padding-top: 0.45rem;
    background: var(--color-base-100);
    height: fit-content;
    border-radius: 999px;
  }
  .route-mark {
    display: block;
    background: var(--color-base-100);
    border: 2px solid var(--rail-0);
  }
  .route-mark-dot {
    width: 0.7rem;
    height: 0.7rem;
    border-radius: 999px;
  }
  .route-mark-diamond {
    width: 0.72rem;
    height: 0.72rem;
    transform: rotate(45deg);
    border-radius: 2px;
  }
  .route-mark-square {
    width: 0.7rem;
    height: 0.7rem;
    border-radius: 2px;
    border-color: var(--color-base-content);
    opacity: 0.55;
  }
  .route-mark-end {
    width: 0.75rem;
    height: 2px;
    border: 0;
    background: var(--spine);
  }
  .route-mark.selected {
    background: var(--rail-0);
  }
  .route-off .route-mark {
    border-color: var(--spine);
  }
  .route-off :global(.route-row),
  .route-off :global(.rails) {
    opacity: 0.55;
  }
</style>
