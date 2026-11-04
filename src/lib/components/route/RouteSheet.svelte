<script lang="ts">
  // The script as a route: incoming mail enters at the top and passes every
  // rule in order along the line on the left. Each rule is a stop on that
  // line: a dot for a simple rule, a diamond for a switch with several cases,
  // a square for code sieve-gui shows as written. Clicking a rule opens it in
  // the inspector.
  import { flip } from "svelte/animate";
  import type { Action, RuleItem } from "#tauri/types";
  import { session } from "../../session.svelte";
  import type { ScriptEditor } from "../../editor.svelte";
  import { uid, type EditorItem } from "../../editor.svelte";
  import { t } from "../../i18n.svelte";
  import { closesMenu } from "../../menu";
  import { isSimple, outcomes } from "../../rules/route";
  import { goesTo, matches, queryWords, type Target } from "../../rules/search";
  import { caseGoesTo, caseMatches, cases, type Outcome } from "../../rules/route";
  import type { ScriptRun } from "../../rules/simulate";
  import { trial as sample } from "../../trial.svelte";
  import { accountView } from "../../scope.svelte";
  import { newRule } from "../../rules/view";
  import { addresses } from "../../rules/simulate";
  import { templateRule, type TemplateKind } from "../../rules/templates";
  import Icon from "../Icon.svelte";
  import Destinations from "./Destinations.svelte";
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
    order = [],
    onopen,
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
    /** The running scripts in the order every message passes them. */
    order?: string[];
    /** Opens another script of the account. */
    onopen?: (name: string) => void;
  } = $props();

  /** How the sample message fares at a rule: applied cases, or why it wasn't decided. */
  function trialAt(index: number) {
    if (!trial) return null;
    return {
      hits: trial.hits.get(index) ?? [],
      misses: trial.misses.get(index) ?? [],
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
    const size: number[] = parents.map(() => 0);
    const grouped: boolean[] = parents.map(() => false);
    for (let i = 0; i < parents.length; ) {
      let j = i;
      while (parents[i] && parents[j + 1] === parents[i]) j++;
      if (parents[i] && j > i) {
        head[i] = parents[i];
        size[i] = j - i + 1;
        grouped.fill(true, i, j + 1);
      }
      i = j + 1;
    }
    // A rule right after a run, outside it: the section visibly ends.
    const ends = parents.map((_, i) => i > 0 && grouped[i - 1] && !grouped[i]);
    return { head, grouped, size, ends };
  });
  let dragFrom = $state<number | null>(null);
  let dropAt = $state<number | null>(null);

  const accountLabel = $derived(session.accounts.find((a) => a.id === editor.accountId)?.label ?? "");

  let why = $state(false);
  /** The provisional note, once read, can be put away for good. */
  let whyDismissed = $state(localStorage.getItem("whyDismissed") === "1");
  /** Whether a script running after this one moves mail too (so folders here are provisional). */
  const laterMoves = $derived.by(() => {
    const i = order.indexOf(editor.name);
    if (i < 0 || !others) return false;
    const moves = (actions: Action[]): boolean =>
      actions.some((a) => (a.type === "fileInto" && !a.copy) || (a.type === "if" && [a.actions, ...a.elsif.map((e) => e.actions), a.otherwise ?? []].some(moves)));
    return order
      .slice(i + 1)
      .some((n) => others?.find((o) => o.name === n)?.rules.items.some((it) => it.type === "rule" && it.enabled && [it.actions, ...it.elsif.map((e) => e.actions), it.otherwise ?? []].some(moves)));
  });

  /** The route, or where mail goes ("destinations"). */
  let view = $state<"route" | "dest">("route");
  /** The running scripts with their rules, the open one with its edits. */
  const runningScripts = $derived(
    order.length
      ? order.map((name) => ({ name, items: name === editor.name ? editor.rules.items : (others?.find((o) => o.name === name)?.rules.items ?? []) }))
      : [{ name: editor.name, items: editor.rules.items }],
  );

  /** Search this script, or every script of the account. */
  // "Where do mails from X go?" spans scripts, so searching starts there.
  let scope = $state<"running" | "all" | "script">("running");
  /** A folder or label clicked on the route: everything that goes there, in every script. */
  let target = $state<(Target & { text: string; folder: boolean }) | null>(null);
  let others = $state<{ name: string; rules: { items: RuleItem[] } }[] | null>(null);
  let withInactive = $state(false);
  /** Scripts that don't run but would file there too. */
  const inactiveTargets = $derived.by(() => {
    if (!target || withInactive || !others) return 0;
    const t = target;
    return others.filter((o) => !session.runs(editor.accountId, o.name) && o.rules.items.some((item) => goesTo(item, t))).length;
  });
  $effect(() => {
    if (!searching && !target && view !== "dest" && !next) return;
    const id = editor.accountId;
    void session.allRules(id).then((list) => {
      if (editor.accountId === id) others = list;
    });
  });
  /** Matches in every script: the open one with its unsaved edits, the others as saved. */
  const allResults = $derived.by(() => {
    if (!searching && !target) return [];
    const list = others ?? [];
    const scripts = list.some((o) => o.name === editor.name) ? list : [{ name: editor.name, rules: editor.rules }, ...list];
    return scripts
      .map(({ name, rules }) => {
        const items = name === editor.name ? editor.rules.items : rules.items;
        const hit = (item: RuleItem) => (target ? goesTo(item, target) : matches(item, words, keywords));
        return { name, hits: items.flatMap((item, index) => (hit(item) ? [{ item, index }] : [])) };
      })
      .filter((r) => r.hits.length > 0)
      // Where mail actually goes: what runs, unless asked for more.
      .filter((r) => (target ? withInactive : scope === "all") || session.runs(editor.accountId, r.name));
  });
  /** Showing matches of all scripts instead of the route. */
  const listing = $derived(!!target || (scope !== "script" && searching));
  const results = $derived(listing ? allResults : []);
  const resultCount = $derived(results.reduce((n, r) => n + r.hits.length, 0));
  /** Matching cases in those rules (a rule with several cases may match in several). */
  const caseCount = $derived(
    results.reduce(
      (n, r) =>
        n +
        r.hits.reduce((m, h) => {
          if (h.item.type !== "rule") return m + 1;
          const list = cases(h.item);
          const hit = list.filter((c) => (target ? caseGoesTo(c, target) : caseMatches(c, words, keywords, delimiter))).length;
          return m + Math.max(1, hit);
        }, 0),
      0,
    ),
  );
  /** While searching this script: matches in the others, to point at them. */
  const elsewhere = $derived(searching && scope === "script" ? allResults.filter((r) => r.name !== editor.name) : []);
  const caseFilter = $derived(target ? (c: Parameters<typeof caseGoesTo>[0]) => caseGoesTo(c, target!) : null);

  // The picked case stays in view, e.g. when the inspector narrows the route.
  let sheet = $state<HTMLElement>();
  $effect(() => {
    if (!editor.selected) return;
    void editor.focus;
    requestAnimationFrame(() => sheet?.querySelector<HTMLElement>('.route [aria-current="true"]')?.scrollIntoView({ block: "nearest", behavior: "smooth" }));
  });

  /** ↑/↓ move between cases and rules, Enter opens one. */
  function navigate(e: KeyboardEvent) {
    if ((e.key !== "ArrowDown" && e.key !== "ArrowUp") || e.altKey || !sheet) return;
    const current = (e.target as HTMLElement).closest<HTMLElement>("[data-nav]");
    if (!current) return;
    const all = [...sheet.querySelectorAll<HTMLElement>("[data-nav]")].filter((el) => el.offsetParent !== null);
    const next = all[all.indexOf(current) + (e.key === "ArrowDown" ? 1 : -1)];
    if (!next) return;
    e.preventDefault();
    next.focus();
    next.scrollIntoView({ block: "nearest" });
  }

  // Account-wide results: the page header says so instead of the script's name.
  $effect(() => {
    accountView.active = listing || view === "dest";
    accountView.kind = view === "dest" && !target ? "dest" : "search";
    return () => (accountView.active = false);
  });

  // Another script opened: start over.
  let shownFor: string | null = null;
  $effect(() => {
    const name = editor.name;
    if (shownFor !== null && name !== shownFor) {
      target = null;
      query = "";
      scope = "running";
    }
    shownFor = name;
  });

  function showTarget(o: Outcome) {
    view = "route";
    if (!o.target) return;
    withInactive = false;
    target = { ...o.target, text: o.kind === "move" || o.kind === "copy" ? o.path.join(" / ") : o.text, folder: o.target.kind === "folder" };
    query = "";
  }

  /** Opens a match: in this script right away, in another one after leaving this. */
  async function openMatch(name: string, index: number, path: string) {
    if (name === editor.name) {
      scope = "script";
      target = null;
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
    target = null;
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

  /** New items go after the picked rule (after its folder section), or at the end. */
  function insert(item: EditorItem) {
    editor.focus = "";
    let at = editor.rules.items.findIndex((i) => i.uid === editor.selected);
    while (at >= 0 && groups.grouped[at + 1] && !groups.head[at + 1]) at++;
    editor.edit((rules) => {
      rules.items.splice(at >= 0 ? at + 1 : rules.items.length, 0, item);
    });
    query = "";
    target = null;
    editor.selected = item.uid;
    fresh = item.uid;
    // Straight to the first thing to fill in.
    requestAnimationFrame(() =>
      requestAnimationFrame(() =>
        [...document.querySelectorAll<HTMLInputElement>("[data-inspector] input:not([type=checkbox]):not([data-name])")].find((i) => i.value === "")?.focus(),
      ),
    );
  }
  /** The rule just added, marked until another one is picked. */
  let fresh = $state<string | null>(null);
  $effect(() => {
    if (editor.selected !== fresh) fresh = null;
  });

  export function addRule() {
    insert({ type: "rule", ...newRule(t("rule.new")), uid: uid() });
  }

  function addTemplate(kind: TemplateKind, value = "") {
    insert({ type: "rule", ...templateRule(kind, "", value), uid: uid() });
  }

  /** The sender's domain of the sample message, for a rule made from it. */
  const sampleDomain = $derived(addresses([sample.mail.from])[0]?.split("@")[1] ?? "");

  function addCode() {
    insert({ type: "raw", code: "", uid: uid() });
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

<!-- svelte-ignore a11y_no_noninteractive_element_interactions: arrow keys move between the rule buttons inside -->
<section class="route-sheet rounded-box border border-base-300 bg-base-100" aria-label={t("route.heading")} bind:this={sheet} onkeydown={navigate}>
  <!-- Search: sticks to the top while the list scrolls. -->
  <div class="sticky top-0 z-20 flex flex-wrap items-center gap-2 rounded-t-box border-b border-base-300 bg-base-100 px-3 py-2">
    <div class="join" role="group" aria-label={t("dest.view")}>
      <button class={["btn btn-sm join-item gap-1.5", view === "route" ? "btn-active" : "btn-ghost"]} aria-pressed={view === "route"} onclick={() => (view = "route")}>
        <Icon name="route" size={15} />{t("dest.route")}
      </button>
      <button
        class={["btn btn-sm join-item gap-1.5", view === "dest" ? "btn-active" : "btn-ghost"]}
        aria-pressed={view === "dest"}
        title={t("dest.title")}
        onclick={() => {
          view = "dest";
          target = null;
          query = "";
        }}
      >
        <Icon name="folder" size={15} />{t("dest.button")}
      </button>
    </div>
    {#if target}
      <div class="flex min-w-0 grow items-center gap-2 px-1.5 py-1 text-sm">
        <span class="whitespace-nowrap text-base-content/70">{target.folder ? t("target.headingFolder") : t("target.headingLabel")}</span>
        <span class="inline-flex min-w-0 items-center gap-1.5 rounded-full bg-primary/10 py-0.5 pr-1 pl-2.5 font-semibold text-primary">
          <Icon name={target.folder ? "folder" : "tag"} size={14} /><span class="truncate">{target.text}</span>
          <button class="grid size-5 place-items-center rounded-full hover:bg-primary/15" aria-label={t("target.clear")} title={t("target.clear")} onclick={() => (target = null)}>
            <Icon name="x" size={13} />
          </button>
        </span>
        {#if target.folder}
          <label class="flex items-center gap-1.5 whitespace-nowrap text-[0.8125rem] text-base-content/75">
            <input type="checkbox" class="checkbox checkbox-sm rounded" bind:checked={target.subfolders} />{t("target.subfolders")}
          </label>
        {/if}
        <span class="whitespace-nowrap text-[0.8125rem] text-base-content/70" role="status">{withInactive
            ? t(resultCount === 1 ? "search.hitOne" : "search.countAll", { count: resultCount })
            : t(resultCount === 1 ? "target.countOne" : "target.count", { count: resultCount })}{caseCount > resultCount
            ? " · " + t("search.cases", { count: caseCount })
            : ""}</span>
      </div>
    {:else}
    <label class="flex min-w-64 grow basis-72 items-center gap-2 rounded-field px-1.5 py-1 focus-within:bg-base-200">
      <Icon name="search" size={15} class="text-base-content/70" />
      <input
        id="rule-search"
        type="search"
        class="min-w-0 grow bg-transparent text-sm outline-none placeholder:text-base-content/70"
        placeholder={t("search.placeholder")}
        aria-label={t("search.label")}
        bind:value={query}
        onkeydown={(e) => e.key === "Escape" && (query = "")}
      />
      {#if searching}
        <span class="text-[0.8125rem] whitespace-nowrap text-base-content/70" role="status">
          {scope !== "script"
            ? (resultCount === 1 ? t("search.hitOne") : t("search.countAll", { count: resultCount })) +
              (caseCount > resultCount ? " · " + t("search.cases", { count: caseCount }) : "")
            : t("search.count", { shown: shownCount, total: editor.rules.items.length })}
        </span>
      {/if}
    </label>
    {/if}
    <!-- Where to search: only while searching. -->
    <!-- Where to search, as a segmented control: only while searching. -->
    <div class={["flex rounded-field bg-base-200 p-0.5", (target || !searching) && "hidden"]} role="group" aria-label={t("search.scope")}>
      {#each [["running", t("search.running")], ["all", t("search.allScripts")], ["script", t("search.thisScript")]] as [value, label] (value)}
        <button
          class={["rounded-[calc(var(--radius-field)-2px)] px-2.5 py-1 text-[0.8125rem] font-medium", scope === value ? "bg-base-100 shadow-sm" : "text-base-content/70 hover:text-base-content"]}
          aria-pressed={scope === value}
          onclick={() => (scope = value as "running" | "all" | "script")}>{label}</button
        >
      {/each}
    </div>
    <!-- While results span scripts, the search gets the room. -->
    <div class={["join", (listing || view === "dest") && "hidden"]}>
      <button class="btn btn-primary btn-sm join-item gap-1" onclick={addRule}><Icon name="plus" size={15} />{t("editor.addRule")}</button>
      <div class="dropdown dropdown-end">
        <div tabindex="0" role="button" class="btn btn-primary btn-sm join-item border-l-primary-content/25 px-1.5" aria-label={t("route.moreAdd")}>
          <Icon name="chevronDown" size={15} />
        </div>
        <ul tabindex="-1" {@attach closesMenu} class="dropdown-content menu z-30 mt-1 w-80 rounded-box border border-base-300 bg-base-100 p-1 shadow-lg">
          <li class="menu-title">{t("template.heading")}</li>
          {#if sample.active && sampleDomain}
            <li>
              <button class="grid gap-0.5 text-left" onclick={() => addTemplate("sender", sampleDomain)}>
                <span class="flex items-center gap-2 font-medium"><Icon name="flask" size={15} class="text-success" />{t("template.sample", { domain: sampleDomain })}</span>
              </button>
            </li>
          {/if}
          <li>
            <button class="flex items-center gap-2" onclick={() => addTemplate("sender")}><Icon name="user" size={15} />{t("template.sender")}</button>
          </li>
          <li>
            <button class="flex items-center gap-2" onclick={() => addTemplate("recipient")}><Icon name="tag" size={15} />{t("template.recipient")}</button>
          </li>
          <li>
            <button class="flex items-center gap-2" onclick={() => addTemplate("subject")}><Icon name="text" size={15} />{t("template.subject")}</button>
          </li>
          <li class="mt-1 border-t border-base-300 pt-1">
            <button class="flex flex-col items-start gap-0.5 text-left" onclick={addCode}>
              <span class="flex items-center gap-2 font-medium"><Icon name="code" size={15} />{t("route.addCodeBlock")}</span>
              <span class="text-[0.8125rem] text-base-content/70">{t("route.addCodeHint")}</span>
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

  {#if view === "dest" && !target}
    <Destinations scripts={runningScripts} {delimiter} {keywords} ontest={trialOpen ? undefined : onshowtrial} onpick={(tg) => {
      withInactive = false;
      target = tg;
    }} />
  {:else if listing}
    <!-- Matches in every script of the account, in the order the scripts run. -->
    <div class="grid gap-4 px-3 py-3">
      <!-- Account-wide: said as a heading, so the script's title above doesn't suggest otherwise. -->
      <div class="flex flex-wrap items-baseline gap-x-2 border-b border-base-300 px-1 pb-2">
        <button
          class="btn btn-ghost btn-sm -ml-2 gap-1 self-center"
          onclick={() => {
            target = null;
            query = "";
            view = "route";
          }}><Icon name="arrowLeft" size={15} />{t("scope.back", { name: editor.name })}</button
        >
        <h2 class="flex items-center gap-1.5 font-semibold"><Icon name="server" size={15} class="text-base-content/70" />{t("scope.account", { account: accountLabel })}</h2>
        <span class="text-[0.8125rem] text-base-content/75">
          {(target ? !withInactive : scope === "running") ? t("scope.runningShort") : t("scope.allShort")}
          {#if target}· {t("dest.notFinal")}{/if}
        </span>
      </div>
      {#each results as r (r.name)}
        <section class="grid gap-1" aria-label={r.name}>
          <h3 class="flex items-center gap-2 px-1 text-[0.875rem] font-semibold">
            <Icon name="text" size={15} class="text-base-content/70" />{r.name}
            {#if !session.runs(editor.accountId, r.name)}
              <span class="rounded-full bg-base-200 px-2 text-[0.8125rem] leading-5 font-medium text-base-content/70">{t("status.off")}</span>
            {/if}
            <span class="font-normal text-base-content/70">{r.hits.length === 1 ? t("search.hitOne") : t("search.hits", { count: r.hits.length })}</span>
          </h3>
          {#each r.hits as hit (hit.index)}
            <div class="rounded-field px-2.5 py-2 hover:bg-base-200/50">
              {#if hit.item.type === "rule"}
                <RouteRule
                  rule={hit.item}
                  {delimiter}
                  {keywords}
                  singleFolder={editor.rules.singleFolder}
                  {words}
                  filter={caseFilter}
                  compact
                  ontarget={showTarget}
                  onpick={(path) => openMatch(r.name, hit.index, path)}
                />
              {:else}
                <button class="w-full text-left" onclick={() => openMatch(r.name, hit.index, "")}>
                  <span class="font-semibold">{t("raw.heading")}</span>
                  <span class="text-[0.8125rem] text-base-content/70">{t("route.codeNote")}</span>
                  <pre class="mt-1 max-h-16 overflow-hidden font-mono text-[0.8125rem] whitespace-pre-wrap text-base-content/75 [overflow-wrap:break-word]">{hit.item.code.trim()}</pre>
                </button>
              {/if}
            </div>
          {/each}
        </section>
      {:else}
        <p class="px-1 py-2 text-sm text-base-content/70">{others === null ? t("search.loading") : t("search.none")}</p>
      {/each}
      {#if inactiveTargets}
        <button class="btn btn-ghost btn-sm justify-self-start text-base-content/70" onclick={() => (withInactive = true)}>
          {t("target.inactive", { count: inactiveTargets })}
        </button>
      {/if}
    </div>
  {:else}
  <ol class="route relative py-1" role="list">
    <!-- Where the mail comes in. -->
    <li class="route-item text-[0.875rem] text-base-content/70">
      <span></span>
      <span class="route-node grid place-items-center"><Icon name="mail" size={15} /></span>
      <span class="flex flex-wrap items-center gap-x-1.5 gap-y-1 py-2">
        {#if order.length > 1}
          <!-- Every mail passes the running scripts in this order. -->
          <span>{t("route.entryOrder")}</span>
          {#each order as name, i (name)}
            {#if i > 0}<Icon name="chevronRight" size={13} class="text-base-content/45" />{/if}
            {#if name === editor.name}
              <span class="inline-flex items-center gap-1 rounded-full bg-primary/12 py-0.5 pr-2 pl-0.5 font-semibold text-primary" aria-current="step">
                <span class="grid size-[1.1rem] place-items-center rounded bg-primary text-[0.75rem] text-primary-content">{i + 1}</span>{name}
              </span>
            {:else}
              <button class="inline-flex items-center gap-1 rounded-full py-0.5 pr-2 pl-0.5 hover:bg-base-200" onclick={() => onopen?.(name)}>
                <span class="grid size-[1.1rem] place-items-center rounded bg-base-content/10 text-[0.75rem] font-semibold">{i + 1}</span>{name}
              </button>
            {/if}
          {/each}
        {:else}
          {t("route.entry")}
        {/if}
      </span>
    </li>
    {#if running && next && laterMoves && !whyDismissed}
      <!-- What's shown here isn't necessarily the end of the story. -->
      <li class="route-item text-[0.8125rem] text-base-content/75">
        <span></span><span></span>
        <div class="-mt-1 mb-1 px-0.5">
          <p class="flex flex-wrap items-center gap-x-1.5">
            <Icon name="info" size={14} class="shrink-0 text-primary" />{t("route.provisionalShort")}
            <button class="font-medium text-primary hover:underline" aria-expanded={why} onclick={() => (why = !why)}>{t("route.why")}</button>
          </p>
          {#if why}
            <p class="mt-0.5 pl-5">
              <button
                class="float-right ml-2 text-[0.8125rem] text-base-content/70 hover:underline"
                onclick={() => {
                  whyDismissed = true;
                  localStorage.setItem("whyDismissed", "1");
                }}>{t("route.whyDismiss")}</button
              >{editor.rules.singleFolder ? t("route.provisional", { name: next }) : t("route.provisionalMore", { name: next })}</p>
          {/if}
        </div>
      </li>
    {/if}
    <!-- What the right column means, once. -->
    <li class="route-item pt-1" hidden={searching}>
      <span></span><span></span>
      <span class="route-row px-2.5 text-[0.75rem] font-semibold tracking-wide text-base-content/70 uppercase">
        <span class="flex flex-wrap items-center gap-x-3">
          {t("route.colIf")}
          <!-- What the marks on the line mean. -->
          <span class="flex items-center gap-2.5 font-normal tracking-normal normal-case">
            <span class="flex items-center gap-1"><span class="route-mark route-mark-dot"></span>{t("route.legendRule")}</span>
            <span class="flex items-center gap-1"><span class="route-mark route-mark-diamond scale-75"></span>{t("route.legendCases")}</span>
            <span class="flex items-center gap-1"><span class="route-mark route-mark-square"></span>{t("route.legendCode")}</span>
          </span>
        </span>
        <span>{t("route.colDoes")}</span>
      </span>
    </li>

    {#each editor.rules.items as item, index (item.uid)}
      {@const selected = editor.selected === item.uid}
      {@const shape = item.type === "raw" ? "square" : cases(item).length === 1 ? "dot" : "diamond"}
      {@const tr = trialAt(index)}
      <li
        data-uid={item.uid}
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
        {#if groups.ends[index]}
          <div class="col-span-3 grid grid-cols-subgrid pt-2">
            <span></span><span></span>
            <span class="mx-2.5 h-px bg-base-300"></span>
          </div>
        {/if}
        {#if groups.head[index]}
          <!-- The folder a run of rules files into, once. -->
          <!-- A section: the folder a run of rules files into, said once. -->
          <div class="col-span-3 grid grid-cols-subgrid pt-3 pb-0.5">
            <span></span>
            <span class="route-node grid place-items-center"><span class="size-1.5 rounded-full bg-[var(--spine)]"></span></span>
            <div class="flex items-center gap-2 px-2.5 text-[0.875rem]">
              <span class="text-[0.8125rem] text-base-content/70">{t("route.groupHead")}</span>
              <span class="font-semibold text-base-content/90" title={t("route.groupTitle")}>{groups.head[index]}</span>
              <span class="h-px grow bg-base-300"></span>
              <span class="pr-2 text-[0.8125rem] text-base-content/70">{t("route.groupCount", { count: groups.size[index] })}</span>
            </div>
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
        <span class="route-node grid place-items-center pt-[0.95rem]" title={t(`route.shape.${shape}`)}>
          <span class="route-mark route-mark-{shape}" class:selected class:trial-hit={!!tr?.hits.length}></span>
        </span>
        <div class={["my-0.5 flex min-w-0 items-start gap-2 rounded-field", selected && (!editor.focus || (item.type === "rule" && cases(item).length === 1)) ? "ring-2 ring-primary/60" : selected ? "ring-1 ring-primary/20" : "hover:bg-base-200/50"]}>
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
                id={item.uid}
                ontarget={showTarget}
                {next}
                trial={tr?.reached ? tr : null}
                onpick={(path) => pick(item.uid, path)}
              />
            {:else}
              <!-- Own code: set apart, since its effect isn't summarised. -->
              <button
                class="w-full rounded-field border border-dashed border-base-content/30 bg-base-200/60 px-2.5 py-1.5 text-left"
                aria-current={selected ? "true" : undefined}
                onclick={() => pick(item.uid, "")}
              >
                <span class="flex flex-wrap items-baseline gap-x-2">
                  <span class="flex items-center gap-1.5 font-semibold"><Icon name="code" size={14} />{t("raw.heading")}</span>
                  <span class="text-[0.8125rem] text-base-content/75">{t("route.codeNote")}</span>
                  {#if editor.roundcubeRisks?.rawBlocks.includes(index)}
                    <span class="badge badge-sm badge-soft badge-warning">{t("roundcube.badge")}</span>
                  {/if}
                </span>
                <pre class="mt-1 max-h-16 overflow-hidden font-mono text-[0.8125rem] leading-snug whitespace-pre-wrap text-base-content/75 [overflow-wrap:break-word]">{item.code.trim() || "…"}</pre>
              </button>
            {/if}
          </div>
          <div class="flex shrink-0 items-center gap-1 pt-2 pr-1.5">
            {#if tr && !tr.reached}
              <span class="rounded-full bg-base-content/10 px-2 text-[0.75rem] leading-5 font-medium text-base-content/85">{t("trial.unreached")}</span>
            {/if}
            {#if fresh === item.uid}
              <span class="rounded-full bg-primary/12 px-2 text-[0.8125rem] leading-5 font-medium text-primary">{t("route.fresh")}</span>
            {/if}
            {#if tr?.unknown}
              <span class="inline-flex items-center gap-1 rounded-full bg-warning/15 px-2 text-[0.8125rem] leading-5 font-medium" title={t("trial.uncertain")}
                ><Icon name="helpCircle" size={13} />{t("trial.unknown")}</span
              >
            {/if}
            {#if item.type === "rule" && !item.enabled}
              <span class="rounded-full bg-base-200 px-2 text-[0.8125rem] leading-5 font-medium text-base-content/70">{t("route.off")}</span>
            {/if}
            {#if item.type === "rule" && editor.roundcubeRisks?.rules.includes(index)}
              <span class="badge badge-sm badge-soft badge-warning" title={t("roundcube.title")}>{t("roundcube.badge")}</span>
            {/if}
            <div class="dropdown dropdown-end">
              <div
                tabindex="0"
                role="button"
                class="grid size-7 place-items-center rounded-field text-base-content/70 opacity-0 group-hover:opacity-100 focus:opacity-100 hover:bg-base-200 {selected ? 'opacity-100' : ''}"
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
        <p class="py-3 text-sm text-base-content/70">{t("search.none")}</p>
      </li>
    {/if}
    {#if elsewhere.length}
      <!-- Not only here: the other scripts with matches, one click away. -->
      <li class="route-item">
        <span></span><span></span>
        <div class="flex flex-wrap items-center gap-x-2 gap-y-1 py-2 text-sm">
          <span class="text-base-content/70">{t("search.elsewhere")}</span>
          {#each elsewhere as r (r.name)}
            <button class="btn btn-sm btn-soft btn-primary gap-1" onclick={() => (scope = session.runs(editor.accountId, r.name) ? "running" : "all")}>
              {r.name}{#if !session.runs(editor.accountId, r.name)}<span class="font-normal">({t("status.off")})</span>{/if} <span class="font-normal">· {r.hits.length === 1 ? t("search.hitOne") : t("search.hits", { count: r.hits.length })}</span>
            </button>
          {/each}
        </div>
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
    <li class="route-item route-end text-[0.875rem] text-base-content/70" hidden={searching}>
      <span></span>
      <span class="route-node grid place-items-center pt-0.5"><span class="route-mark route-mark-end"></span></span>
      <span class="py-2">
        {#if !running}{t("route.exitOff")}{:else if next}
          <button class="inline-flex items-center gap-1.5 rounded-full bg-base-200 px-2.5 py-0.5 font-medium text-base-content/80 hover:bg-base-300" onclick={() => onopen?.(next)}>
            {t("route.exitNext", { name: next })}<Icon name="chevronRight" size={14} />
          </button>
        {:else}{t("route.exitInbox")}{/if}
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
    opacity: 0.55;
  }
  .route-off .route-mark {
    border-color: var(--spine);
  }
  .route-off :global(.route-row),
  .route-off :global(.rails) {
    opacity: 0.55;
  }
</style>
