<script lang="ts">
  // The script as a route: incoming mail enters at the top and passes every
  // rule in order along the line on the left. Each rule is a stop on that
  // line: a dot for a simple rule, a diamond for a switch with several cases,
  // a square for code sieve-gui shows as written. Clicking a rule opens it in
  // the inspector.
  import { flip } from "svelte/animate";
  import type { RuleItem } from "#tauri/types";
  import { session } from "../../session.svelte";
  import type { ScriptEditor } from "../../editor.svelte";
  import { uid } from "../../editor.svelte";
  import { t } from "../../i18n.svelte";
  import { closesMenu } from "../../menu";
  import { isSimple, outcomes } from "../../rules/route";
  import { matches, queryWords } from "../../rules/search";
  import type { ScriptRun } from "../../rules/simulate";
  import { trial as sample } from "../../trial.svelte";
  import { newRule } from "../../rules/view";
  import Icon from "../Icon.svelte";
  import RouteRule from "./RouteRule.svelte";

  let {
    editor,
    delimiter,
    keywords,
    next,
    running,
    trial = null,
    trialOpen = false,
    onshowtrial,
  }: {
    editor: ScriptEditor;
    delimiter: string | null;
    keywords: Record<string, string>;
    /** The script that runs after this one, if any. */
    next: string | null;
    /** Whether this script runs at all. */
    running: boolean;
    /** A sample message's way through this script, while trying rules. */
    trial?: ScriptRun | null;
    trialOpen?: boolean;
    onshowtrial?: () => void;
  } = $props();

  /** How the sample message fares at a rule: applied cases, or why it wasn't decided. */
  function trialAt(index: number) {
    if (!trial) return null;
    return {
      hits: trial.hits.get(index) ?? [],
      unknown: trial.unknown.includes(index),
      reached: !trial.skipped && (trial.stoppedAt === null || index <= trial.stoppedAt),
    };
  }

  let query = $state("");
  const words = $derived(queryWords(query));
  /** While searching, only matching items show and the order can't change. */
  const searching = $derived(words.length > 0);
  const shown = $derived(editor.rules.items.map((item) => !searching || matches(item, words, keywords)));
  const shownCount = $derived(shown.filter(Boolean).length);
  /**
   * Runs of simple rules that file into the same parent folder: the parent
   * shows once above them ("Accounts / Travel"), each row only the last folder.
   */
  const groups = $derived.by(() => {
    const parents = editor.rules.items.map((item) => {
      if (searching || item.type !== "rule" || !isSimple(item)) return null;
      const moves = outcomes(item.actions, {}, delimiter).filter((o) => o.kind === "move");
      return moves.length === 1 && moves[0].path.length > 1 ? moves[0].path.slice(0, -1).join(" / ") : null;
    });
    const head: (string | null)[] = parents.map(() => null);
    const grouped: boolean[] = parents.map(() => false);
    for (let i = 0; i < parents.length; ) {
      let j = i;
      while (parents[i] && parents[j + 1] === parents[i]) j++;
      if (parents[i] && j > i) {
        head[i] = parents[i];
        grouped.fill(true, i, j + 1);
      }
      i = j + 1;
    }
    return { head, grouped };
  });
  let dragFrom = $state<number | null>(null);
  let dropAt = $state<number | null>(null);

  /** Search this script, or every script of the account. */
  let scope = $state<"script" | "all">("script");
  let others = $state<{ name: string; rules: { items: RuleItem[] } }[] | null>(null);
  $effect(() => {
    if (scope !== "all" || !searching) return;
    const id = editor.accountId;
    void session.allRules(id).then((list) => {
      if (editor.accountId === id) others = list;
    });
  });
  /** Matches across scripts: the open one with its unsaved edits, the others as saved. */
  const results = $derived.by(() => {
    if (scope !== "all" || !searching) return [];
    const list = others ?? [];
    const scripts = list.some((o) => o.name === editor.name) ? list : [{ name: editor.name, rules: editor.rules }, ...list];
    return scripts
      .map(({ name, rules }) => {
        const items = name === editor.name ? editor.rules.items : rules.items;
        return {
          name,
          hits: items.flatMap((item, index) => (matches(item, words, keywords) ? [{ item, index }] : [])),
        };
      })
      .filter((r) => r.hits.length > 0);
  });
  const resultCount = $derived(results.reduce((n, r) => n + r.hits.length, 0));

  /** Opens a match: in this script right away, in another one after leaving this. */
  async function openMatch(name: string, index: number, path: string) {
    if (name === editor.name) {
      scope = "script";
      const item = editor.rules.items[index];
      if (item) pick(item.uid, path);
      return;
    }
    if (!(await session.confirmLeave())) return;
    await session.openScript(editor.accountId, name);
    const next = session.editor;
    const item = next?.rules.items[index];
    if (next && item) {
      next.selected = item.uid;
      next.focus = path;
    }
    scope = "script";
  }

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
      <Icon name="search" size={15} class="text-base-content/65" />
      <input
        id="rule-search"
        type="search"
        class="min-w-0 grow bg-transparent text-sm outline-none placeholder:text-base-content/65"
        placeholder={t("search.placeholder")}
        aria-label={t("search.label")}
        bind:value={query}
        onkeydown={(e) => e.key === "Escape" && (query = "")}
      />
      {#if searching}
        <span class="text-[0.8rem] whitespace-nowrap text-base-content/65" role="status">
          {scope === "all"
            ? t("search.countAll", { count: resultCount })
            : t("search.count", { shown: shownCount, total: editor.rules.items.length })}
        </span>
      {/if}
    </label>
    <div class="join" role="group" aria-label={t("search.scope")}>
      <button class={["btn btn-xs join-item", scope === "script" ? "btn-active" : "btn-ghost"]} aria-pressed={scope === "script"} onclick={() => (scope = "script")}>
        {t("search.thisScript")}
      </button>
      <button class={["btn btn-xs join-item", scope === "all" ? "btn-active" : "btn-ghost"]} aria-pressed={scope === "all"} onclick={() => (scope = "all")}>
        {t("search.allScripts")}
      </button>
    </div>
    <div class="join">
      <button class="btn btn-primary btn-sm join-item gap-1" onclick={addRule}><Icon name="plus" size={15} />{t("editor.addRule")}</button>
      <div class="dropdown dropdown-end">
        <div tabindex="0" role="button" class="btn btn-primary btn-sm join-item border-l-primary-content/25 px-1.5" aria-label={t("route.moreAdd")}>
          <Icon name="chevronDown" size={15} />
        </div>
        <ul tabindex="-1" {@attach closesMenu} class="dropdown-content menu z-30 mt-1 w-72 rounded-box border border-base-300 bg-base-100 p-1 shadow-lg">
          <li>
            <button class="grid gap-0.5 text-left" onclick={addCode}>
              <span class="flex items-center gap-2 font-medium"><Icon name="code" size={15} />{t("route.addCodeBlock")}</span>
              <span class="text-[0.8rem] text-base-content/70">{t("route.addCodeHint")}</span>
            </button>
          </li>
        </ul>
      </div>
    </div>
  </div>

  {#if trial && !trialOpen}
    <!-- While trying rules with the panel closed: say why rows are green. -->
    <div class="flex flex-wrap items-center gap-x-3 gap-y-1 border-b border-success/25 bg-success/[0.08] px-4 py-2 text-sm">
      <Icon name="flask" size={15} class="text-success" />
      <p class="grow">{t("trial.active")}</p>
      <button class="btn btn-xs" onclick={onshowtrial}>{t("trial.show")}</button>
      <button class="btn btn-ghost btn-xs" onclick={() => sample.clear()}>{t("trial.end")}</button>
    </div>
  {/if}

  {#if scope === "all" && searching}
    <!-- Matches in every script of the account, in the order the scripts run. -->
    <div class="grid gap-4 px-3 py-3">
      {#each results as r (r.name)}
        <section class="grid gap-1" aria-label={r.name}>
          <h3 class="flex items-center gap-2 px-1 text-[0.875rem] font-semibold">
            <Icon name="text" size={15} class="text-base-content/65" />{r.name}
            <span class="font-normal text-base-content/65">{t("search.hits", { count: r.hits.length })}</span>
          </h3>
          {#each r.hits as hit (hit.index)}
            <div class="rounded-field px-2.5 py-2 ring-1 ring-base-300 hover:bg-base-200/50">
              {#if hit.item.type === "rule"}
                <RouteRule
                  rule={hit.item}
                  {delimiter}
                  {keywords}
                  singleFolder={editor.rules.singleFolder}
                  {words}
                  onpick={(path) => openMatch(r.name, hit.index, path)}
                />
              {:else}
                <button class="w-full text-left" onclick={() => openMatch(r.name, hit.index, "")}>
                  <span class="font-semibold">{t("raw.heading")}</span>
                  <pre class="mt-1 max-h-16 overflow-hidden font-mono text-[0.8rem] text-base-content/65">{hit.item.code.trim()}</pre>
                </button>
              {/if}
            </div>
          {/each}
        </section>
      {:else}
        <p class="px-1 py-2 text-sm text-base-content/65">{others === null ? t("search.loading") : t("search.none")}</p>
      {/each}
    </div>
  {:else}
  <ol class="route relative py-1" role="list">
    <!-- Where the mail comes in. -->
    <li class="route-item text-[0.875rem] text-base-content/65">
      <span></span>
      <span class="route-node grid place-items-center"><Icon name="mail" size={15} /></span>
      <span class="py-2">{t("route.entry")}</span>
    </li>

    {#each editor.rules.items as item, index (item.uid)}
      {@const selected = editor.selected === item.uid}
      {@const shape = item.type === "raw" ? "square" : isSimple(item) ? "dot" : "diamond"}
      {@const tr = trialAt(index)}
      <li
        animate:flip={{ duration: 150 }}
        hidden={!shown[index]}
        class={[
          "route-item group relative",
          dragFrom === index && "opacity-40",
          item.type === "rule" && !item.enabled && "route-off",
          tr && !tr.reached && "route-unreached",
        ]}
        title={tr && !tr.reached ? t("trial.notReached") : undefined}
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
        {#if groups.head[index]}
          <!-- The folder a run of rules files into, once. -->
          <div class="col-span-3 grid grid-cols-subgrid pt-2">
            <span></span>
            <span class="route-node grid place-items-center"><span class="size-1.5 rounded-full bg-[var(--spine)]"></span></span>
            <span class="flex items-center gap-1.5 px-2.5 text-[0.875rem] font-medium text-base-content/70">
              <Icon name="folder" size={14} class="text-primary" />{groups.head[index]}
            </span>
          </div>
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
          <span class="route-mark route-mark-{shape}" class:selected class:trial-hit={!!tr?.hits.length}></span>
        </span>
        <div class={["my-0.5 flex min-w-0 items-start gap-2 rounded-field", selected ? "bg-primary/[0.05] ring-1 ring-primary/25" : "hover:bg-base-200/50"]}>
          <div class="min-w-0 grow rounded-field px-2.5 py-2">
            {#if item.type === "rule"}
              <RouteRule
                rule={item}
                {delimiter}
                {keywords}
                singleFolder={editor.rules.singleFolder}
                {words}
                picked={selected ? editor.focus : null}
                hideParent={groups.grouped[index]}
                trial={tr}
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
            {#if tr?.unknown}
              <span class="inline-flex items-center gap-1 rounded-full bg-warning/15 px-2 text-[0.8rem] leading-5 font-medium" title={t("trial.uncertain")}
                ><Icon name="helpCircle" size={13} />{t("trial.unknown")}</span
              >
            {/if}
            {#if item.type === "rule" && !item.enabled}
              <span class="rounded-full bg-base-200 px-2 text-[0.8rem] leading-5 font-medium text-base-content/60">{t("route.off")}</span>
            {/if}
            {#if item.type === "rule" && editor.roundcubeRisks?.rules.includes(index)}
              <span class="badge badge-sm badge-soft badge-warning" title={t("roundcube.title")}>{t("roundcube.badge")}</span>
            {/if}
            <div class="dropdown dropdown-end">
              <div
                tabindex="0"
                role="button"
                class="grid size-7 place-items-center rounded-field text-base-content/65 opacity-0 group-hover:opacity-100 focus:opacity-100 hover:bg-base-200 {selected ? 'opacity-100' : ''}"
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
    <li class="route-item route-end text-[0.875rem] text-base-content/65">
      <span></span>
      <span class="route-node grid place-items-center pt-0.5"><span class="route-mark route-mark-end"></span></span>
      <span class="py-2">
        {#if !running}{t("route.exitOff")}{:else if next}{t("route.exitNext", { name: next })}{:else}{t("route.exitInbox")}{/if}
      </span>
    </li>
  </ol>
  {/if}
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
  .route-mark.trial-hit {
    background: var(--color-success);
    border-color: var(--color-success);
  }
  .route-unreached > :not(.route-node) {
    opacity: 0.4;
  }
  .route-off .route-mark {
    border-color: var(--spine);
  }
  .route-off :global(.route-row),
  .route-off :global(.rails) {
    opacity: 0.55;
  }
</style>
