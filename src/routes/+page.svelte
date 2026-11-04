<script lang="ts">
  import { onMount } from "svelte";
  import { MediaQuery } from "svelte/reactivity";
  import type { Account } from "#tauri/types";
  import AccountDialog from "../lib/components/AccountDialog.svelte";
  import AppDialog from "../lib/components/AppDialog.svelte";
  import CodeEditor from "../lib/components/CodeEditor.svelte";
  import DeployDialog from "../lib/components/DeployDialog.svelte";
  import Icon from "../lib/components/Icon.svelte";
  import Inspector from "../lib/components/Inspector.svelte";
  import RouteSheet from "../lib/components/route/RouteSheet.svelte";
  import Sidebar from "../lib/components/Sidebar.svelte";
  import TrialPanel from "../lib/components/TrialPanel.svelte";
  import { dialogs } from "../lib/dialogs.svelte";
  import { i18n, t } from "../lib/i18n.svelte";
  import { ROOT_SCRIPT, session } from "../lib/session.svelte";
  import { trial } from "../lib/trial.svelte";
  import { accountView, routeNav } from "../lib/scope.svelte";

  let accountDialog = $state<{ account: Account | null } | null>(null);
  let deploying = $state(false);
  // The code sidebar stays open or closed across restarts.
  let showCode = $state(localStorage.getItem("showCode") === "1");
  $effect(() => localStorage.setItem("showCode", showCode ? "1" : "0"));
  // Large windows show the panel beside the route, medium ones over it,
  // narrow ones instead of it.
  // Below this, the sidebar steps aside while the panel is open, so the route keeps its columns.
  const roomy = new MediaQuery("min-width: 1500px");
  // Medium windows make room for the panel by folding the sidebar away.
  const medium = new MediaQuery("min-width: 860px");
  // Below this the panel floats over the route instead of narrowing it.
  const large = new MediaQuery("min-width: 1200px");

  const editor = $derived(session.editor);
  const accountLabel = $derived(session.accounts.find((a) => a.id === editor?.accountId)?.label ?? "");
  const active = $derived(editor ? session.runs(editor.accountId, editor.name) : false);
  const root = $derived(editor ? session.roots[editor.accountId] : null);
  /**
   * The open script runs with "last move wins", so it should record moves.
   * Not for scripts included unchanged (e.g. a webmail's own root script):
   * sieve-gui leaves those alone, so it doesn't offer to convert them.
   */
  const lastMoveWins = $derived(
    !!editor && !!root?.lastMoveWins && root.scripts.includes(editor.name) && !root.unchanged.includes(editor.name),
  );

  /** The root includes the open script unchanged (e.g. a webmail's own root script). */
  const unchanged = $derived(!!editor && !!root?.unchanged.includes(editor.name));
  /** The script running after this one in the root's order, if any. */
  const next = $derived.by(() => {
    if (!editor || !root) return null;
    const i = root.scripts.indexOf(editor.name);
    return i >= 0 ? (root.scripts[i + 1] ?? null) : null;
  });
  const serverFolders = $derived(
    editor && session.folders[editor.accountId]?.status === "ready" ? session.folders[editor.accountId].folders : null,
  );
  /** The server's folder hierarchy delimiter, to show folder paths. */
  const delimiter = $derived(serverFolders?.find((f) => f.delimiter)?.delimiter ?? null);
  /** The code or the picked rule beside the route. */
  const rightPanel = $derived(!!editor && (showCode || !!editor.selected || trial.open));
  /** The open script's part of the sample message's way, while trying rules. */
  const trialRun = $derived(editor ? (trial.result?.scripts.find((s) => s.name === editor.name) ?? null) : null);

  // The sidebar can be folded away; both choices persist.
  let sidebarOpen = $state(localStorage.getItem("sidebar") !== "0");
  $effect(() => localStorage.setItem("sidebar", sidebarOpen ? "1" : "0"));
  /** The panel sits beside the route (else it takes its place). */
  const beside = $derived(large.current);
  /**
   * Medium windows: the test floats over the route (its marks stay in
   * sight); the inspector and the code take the route's place, with a way back.
   */
  const overlay = $derived(!large.current && medium.current && trial.open && !showCode && !editor?.selected);
  /** Unless the window is roomy, the sidebar steps aside while the panel is open. */
  let sidebarPinned = $state(false);
  /** The floating panel starts below the header, so saving stays in reach. */
  let headerHeight = $state(0);
  // Without a script open there's no header to bring it back from: it stays.
  const sidebarShown = $derived(!editor || (sidebarOpen && (roomy.current || !rightPanel || sidebarPinned)));
  $effect(() => {
    if (!rightPanel) sidebarPinned = false;
  });
  function toggleSidebar() {
    sidebarOpen = !sidebarShown;
    sidebarPinned = sidebarOpen;
  }
  const clampWidth = (w: number) => Math.round(Math.min(Math.max(w, 340), Math.min(820, window.innerWidth * 0.6)));
  let panelWidth = $state(clampWidth(Number(localStorage.getItem("panelWidth")) || 480));
  $effect(() => localStorage.setItem("panelWidth", String(panelWidth)));

  function startResize(e: PointerEvent) {
    e.preventDefault();
    const move = (ev: PointerEvent) => (panelWidth = clampWidth(window.innerWidth - ev.clientX));
    const up = () => {
      window.removeEventListener("pointermove", move);
      window.removeEventListener("pointerup", up);
    };
    window.addEventListener("pointermove", move);
    window.addEventListener("pointerup", up);
  }

  /** From the inspector in its own view back to the route, at the rule just edited. */
  function backToRoute(ed: NonNullable<typeof editor>) {
    const id = ed.selected;
    ed.selected = null;
    requestAnimationFrame(() =>
      requestAnimationFrame(() => document.querySelector(`[data-uid="${id}"]`)?.scrollIntoView({ block: "center" })),
    );
  }

  /** Code, inspector and the test share the place on the right. */
  function toggleCode() {
    showCode = !showCode;
    if (showCode && editor) editor.selected = null;
    if (showCode) trial.open = false;
  }
  function toggleTrial() {
    trial.open = !trial.open;
    if (trial.open) {
      showCode = false;
      if (editor) editor.selected = null;
    }
  }
  $effect(() => {
    if (editor?.selected) {
      showCode = false;
      trial.open = false;
    }
  });

  const movesItself = $derived(
    unchanged && !!root?.lastMoveWins && !!editor && !!session.links[editor.accountId]?.[editor.name]?.filesMessages,
  );

  $effect(() => {
    document.documentElement.lang = i18n.locale;
  });

  onMount(async () => {
    await session.loadAccounts();
    for (const account of session.accounts) void session.connect(account.id);
  });

  function onkeydown(e: KeyboardEvent) {
    if (!editor || !(e.metaKey || e.ctrlKey)) return;
    // Leave text fields their own undo.
    const target = e.target as HTMLElement;
    if (target.closest("input, textarea, .cm-editor")) return;
    if (e.key.toLowerCase() === "z") {
      e.preventDefault();
      if (e.shiftKey) editor.redo();
      else editor.undo();
    } else if (e.key.toLowerCase() === "s") {
      e.preventDefault();
      deploying = true;
    } else if (e.key.toLowerCase() === "f") {
      const search = document.getElementById("rule-search");
      if (search) {
        e.preventDefault();
        search.focus();
      }
    }
  }
</script>

<svelte:window {onkeydown} />

<!-- Keyboard: past the sidebar straight to the rules. -->
<a
  href="#rule-search"
  class="sr-only z-50 rounded-field bg-primary px-3 py-2 text-primary-content focus:not-sr-only focus:fixed focus:top-2 focus:left-2"
  onclick={(e) => {
    e.preventDefault();
    document.getElementById("rule-search")?.focus();
  }}>{t("skip.rules")}</a
>

<div
  class="grid h-screen"
  style:grid-template-columns="{sidebarShown ? '17rem' : '0'} minmax(0, 1fr){rightPanel && beside ? ` ${panelWidth}px` : ''}"
  style:--header-h={editor && headerHeight ? `${headerHeight}px` : null}
>
  <aside class="min-h-0 overflow-hidden border-r border-base-300 bg-base-200" inert={!sidebarShown}>
    <Sidebar onaccount={(account) => (accountDialog = { account })} ontoggle={editor ? toggleSidebar : undefined} />
  </aside>

  <main class="flex min-w-0 flex-col overflow-hidden bg-base-200">
    {#if editor}
      <!-- One bar, like a native toolbar: what is open and its state on the left, what to do with it on the right. -->
      <header
        bind:offsetHeight={headerHeight}
        data-tauri-drag-region
        class={["flex h-13 shrink-0 items-center gap-2 bg-base-100 pr-3 pl-5 shadow-[inset_0_-1px_0_var(--color-base-300)]", !sidebarShown && "traffic-inset"]}
      >
        {#if !sidebarShown}
          <!-- The sidebar's own toggle is gone with it: bring it back from here. -->
          <button class="btn btn-ghost btn-sm btn-square -ml-2 shrink-0 text-base-content/60" aria-label={t("sidebar.show")} title={t("sidebar.show")} onclick={toggleSidebar}>
            <Icon name="panelLeft" size={17} />
          </button>
        {/if}
        <!-- Name and save state on one line; the mark sits between cap and x-height centre, so it looks level with any name. -->
        <div class="flex min-w-0 items-center gap-2" data-tauri-drag-region>
        {#if accountView.active}
          <!-- Results across scripts: the account is the subject. -->
          <h1 class="flex min-w-0 items-center gap-1.5 text-[0.95rem] font-semibold" data-tauri-drag-region>
            <Icon name="server" size={15} class="shrink-0 text-base-content/60" /><span class="truncate" data-tauri-drag-region>{accountLabel}</span>
            <span class="truncate font-normal text-base-content/65" data-tauri-drag-region>
              {accountView.kind === "dest" ? t("scope.headerDest") : accountView.kind === "flow" ? t("scope.headerFlow") : t("scope.headerSearch")}
            </span>
          </h1>
        {:else}
          <h1 class="min-w-0 truncate text-[0.95rem] font-semibold" data-tauri-drag-region>{editor.name}</h1>
        {/if}
        <!-- Saved: a quiet check; changes: an amber dot that says so. -->
        <p class="flex shrink-0 items-center gap-1.5 text-[0.8125rem] text-base-content/70" aria-live="polite" data-tauri-drag-region>
          {#if accountView.active && (editor.dirty || editor.selected)}<span class="font-medium">„{editor.name}“</span>{/if}
          {#if editor.dirty}
            <span class="size-1.5 rounded-full bg-warning"></span>{t("editor.unsavedShort")}
          {:else}
            <!-- 12px: no taller than a capital. -->
            <span class="-translate-y-[0.5px] text-base-content/50" title={t("editor.saved")}><Icon name="circleCheck" size={12} /></span><span class="sr-only">{t("editor.saved")}</span>
          {/if}
        </p>
        </div>
        <span class="h-full min-w-4 grow" data-tauri-drag-region></span>
        <!-- Undo and redo stay put, whatever opens or closes around them; a hairline sets them off from the panels. -->
        <div class="flex shrink-0 items-center border-r border-base-300 pr-1.5 text-base-content/70">
          <button class="btn btn-ghost btn-sm btn-square" disabled={!editor.canUndo} onclick={() => editor.undo()} aria-label={t("editor.undo")} title="{t('editor.undo')} (⌘Z)">
            <Icon name="undo" size={16} />
          </button>
          <button class="btn btn-ghost btn-sm btn-square" disabled={!editor.canRedo} onclick={() => editor.redo()} aria-label={t("editor.redo")} title="{t('editor.redo')} (⇧⌘Z)">
            <Icon name="redo" size={16} />
          </button>
        </div>
        <div class="flex shrink-0 items-center gap-1">
          <button
            class={["btn btn-ghost btn-sm gap-1.5 font-medium", trial.open && "btn-active", trial.active && "text-success"]}
            aria-pressed={trial.open}
            title={t("trial.buttonTitle")}
            onclick={toggleTrial}
          >
            <Icon name="flask" size={16} />
            <span class="max-xl:sr-only">{t("trial.button")}</span>
          </button>
          <button
            disabled={accountView.active}
            class={["btn btn-ghost btn-sm gap-1.5 font-medium", showCode && "btn-active"]}
            aria-pressed={showCode}
            title={showCode ? t("editor.hideCode") : t("editor.showCode")}
            onclick={toggleCode}
          >
            <Icon name="code" size={16} />
            <span class="max-xl:sr-only">{t("editor.code")}</span>
          </button>
          <button class="btn btn-primary btn-sm ml-1 gap-1.5" disabled={!editor.dirty} onclick={() => (deploying = true)} title="⌘S">
            <Icon name="save" size={15} />{t("editor.deploy")}
          </button>
        </div>
      </header>

      {#if rightPanel && !beside && !overlay}
        <!-- Narrow windows: the inspector or the code takes the place of the route. -->
        <section class="flex min-h-0 grow flex-col">
          {#if showCode}
            <div class="flex min-h-0 grow flex-col gap-2 p-3">{@render codePanel(editor.code)}</div>
          {:else if editor.selected}
            <Inspector
              {editor}
              extensions={session.extensions}
              serverFolders={serverFolders}
              backToList={routeNav.accountId === editor.accountId && (!!routeNav.target || routeNav.view === "dest" || (!!routeNav.query.trim() && routeNav.scope !== "script"))}
              onback={() => backToRoute(editor)}
            />
          {:else}
            <TrialPanel {editor} {delimiter} keywords={session.keywordNames(editor.accountId)} onback={() => (trial.open = false)} />
          {/if}
        </section>
      {:else}
        <div class="min-h-0 grow overflow-y-auto">
          <div class="mx-auto grid max-w-[60rem] grid-cols-[minmax(0,1fr)] gap-3 px-6 pt-5 pb-10">
            {#if !active}
              {@render notice("info", t("editor.inactive"), { label: t("scripts.activate"), run: () => session.activate(editor.accountId, editor.name) })}
            {/if}
            {#if unchanged}
              {@render notice("info", movesItself ? `${t("editor.unchanged")} ${t("editor.unchangedMoves")}` : t("editor.unchanged"))}
            {/if}
            <!-- Only when the script doesn't match the account's "last move wins". -->
            {#if lastMoveWins ? !editor.rules.singleFolder : editor.rules.singleFolder && !root?.lastMoveWins}
              {@render notice("warning", lastMoveWins ? t("root.notSingle") : t("root.singleOutside"), {
                label: lastMoveWins ? t("root.convert") : t("root.convertBack"),
                run: () => editor.edit((r) => (r.singleFolder = lastMoveWins)),
              })}
            {/if}
            {#if editor.roundcubeRisks && (editor.roundcubeRisks.rawBlocks.length + editor.roundcubeRisks.rules.length > 0 || editor.roundcubeRisks.bracketComments)}
              {@const risks = editor.roundcubeRisks}
              {@const count = risks.rawBlocks.length + risks.rules.length}
              {@render notice("warning", `${t("roundcube.title")} ${risks.bracketComments ? t("roundcube.bracket") : count === 1 ? t("roundcube.rawOne") : t("roundcube.rawMany", { count })}`)}
            {/if}
            {#if session.error}
              <p role="alert" class="flex items-start gap-2 rounded-field bg-error/10 px-3 py-2 text-sm text-error">
                <Icon name="alert" size={16} class="mt-0.5" />{session.error}
              </p>
            {/if}
            <RouteSheet
              {editor}
              delimiter={delimiter}
              keywords={session.keywordNames(editor.accountId)}
              {next}
              running={active}
              trial={trial.active ? trialRun : null}
              trialOpen={trial.open}
              onshowtrial={toggleTrial}
              order={active && root ? root.scripts : []}
              onopen={async (name) => (await session.confirmLeave()) && session.openScript(editor.accountId, name)}
            />
          </div>
        </div>
      {/if}
    {:else if session.rootView && session.rootSources[session.rootView] !== undefined}
      <header class="flex h-13 shrink-0 items-center gap-2 bg-base-100 px-5 shadow-[inset_0_-1px_0_var(--color-base-300)]" data-tauri-drag-region>
        <h1 class="flex shrink-0 items-center gap-1.5 text-[0.95rem] font-semibold" data-tauri-drag-region><Icon name="lock" size={15} />{ROOT_SCRIPT}</h1>
        <p class="min-w-0 truncate text-[0.8125rem] text-base-content/65" data-tauri-drag-region>{t("root.viewHint")}</p>
      </header>
      <section class="flex min-h-0 grow flex-col p-4" aria-label={ROOT_SCRIPT}>
        <div class="min-h-0 overflow-auto rounded-box border border-base-300 bg-base-100">
          <CodeEditor value={session.rootSources[session.rootView]} readonly label={ROOT_SCRIPT} />
        </div>
      </section>
    {:else}
      <!-- No bar here, but the top still moves the window. -->
      <div class="h-13 shrink-0" data-tauri-drag-region></div>
      <div class="grid min-h-0 grow place-items-center p-8 pt-0">
        {#if session.loadingScript}
          <p class="flex items-center gap-2 text-base-content/70"><span class="loading loading-spinner"></span>{t("editor.loading")}</p>
        {:else}
          <div class="grid max-w-sm justify-items-center gap-3 text-center">
            <span class="grid size-12 place-items-center rounded-full bg-base-100 text-primary"><Icon name="route" size={22} /></span>
            {#if session.error}<p role="alert" class="text-error">{session.error}</p>{/if}
            <h2 class="text-lg font-semibold">{t("welcome.title")}</h2>
            <p class="text-base-content/65">{t("welcome.body")}</p>
            {#each session.accounts as account (account.id)}
              {@const running = session.scriptTree(account.id).running.filter((n) => n.depth === 0)}
              {#if running.length}
                <div class="mt-2 grid w-full gap-1 text-left">
                  <p class="px-1 text-[0.8rem] text-base-content/65">{t("welcome.running", { account: account.label })}</p>
                  {#each running as node, i (node.name)}
                    <button
                      class="flex items-center gap-2.5 rounded-field bg-base-100 px-3 py-2 text-left ring-1 ring-base-300 hover:ring-primary/40"
                      onclick={async () => (await session.confirmLeave()) && session.openScript(account.id, node.name)}
                    >
                      <span class="grid size-6 place-items-center rounded-md bg-primary/12 text-[0.8rem] font-bold text-primary">{i + 1}</span>
                      <span class="grow font-medium">{node.name}</span>
                      <Icon name="chevronRight" size={16} class="text-base-content/55" />
                    </button>
                  {/each}
                </div>
              {/if}
            {/each}
          </div>
        {/if}
      </div>
    {/if}
  </main>

  {#if rightPanel && (beside || overlay)}
    <div
      class={[
        "flex min-h-0 min-w-0 flex-col",
        overlay ? "fixed right-0 bottom-0 z-40 w-[min(32rem,62vw)] shadow-2xl" : "relative",
      ]}
      style:top={overlay ? `${headerHeight}px` : undefined}
    >
    <!-- Drag to make the panel wider or narrower. -->
    <button
      hidden={overlay}
      aria-label={t("panel.resize")}
      title={t("panel.resize")}
      class="absolute inset-y-0 -left-1 z-30 w-2 cursor-col-resize hover:bg-primary/25 focus-visible:bg-primary/40"
      onpointerdown={startResize}
      onkeydown={(e) => {
        if (e.key === "ArrowLeft") panelWidth = clampWidth(panelWidth + 24);
        if (e.key === "ArrowRight") panelWidth = clampWidth(panelWidth - 24);
      }}
    ></button>
    {#if showCode}
      <aside class="flex min-w-0 flex-col gap-2 overflow-hidden border-l border-base-300 bg-base-100 p-3" aria-label={t("code.heading")}>
        {@render codePanel(editor!.code)}
      </aside>
    {:else if editor!.selected}
      <Inspector editor={editor!} extensions={session.extensions} serverFolders={serverFolders} />
    {:else}
      <TrialPanel editor={editor!} {delimiter} keywords={session.keywordNames(editor!.accountId)} />
    {/if}
    </div>
  {/if}
</div>

{#snippet notice(tone: "info" | "warning", text: string, action?: { label: string; run: () => void })}
  <div
    role="note"
    class={[
      "flex flex-wrap items-center gap-x-3 gap-y-1.5 rounded-field px-3 py-2 text-sm",
      tone === "warning" ? "bg-warning/10 text-base-content" : "bg-base-100 text-base-content/80 ring-1 ring-base-300",
    ]}
  >
    <Icon name={tone === "warning" ? "alert" : "info"} size={16} class={tone === "warning" ? "text-warning" : "text-primary"} />
    <p class="min-w-60 grow">{text}</p>
    {#if action}<button class="btn btn-xs" onclick={action.run}>{action.label}</button>{/if}
  </div>
{/snippet}

{#snippet codePanel(code: string)}
  <div class="flex items-center gap-2">
    <h2 class="grow font-semibold">{t("code.heading")}</h2>
    <button class="btn btn-ghost btn-sm btn-square" aria-label={t("editor.hideCode")} title={t("editor.hideCode")} onclick={() => (showCode = false)}><Icon name="x" size={16} /></button>
  </div>
  <div class="min-h-0 overflow-auto">
    <CodeEditor value={code} readonly label={t("code.heading")} />
  </div>
{/snippet}

{#if accountDialog}
  <AccountDialog account={accountDialog.account} onclose={() => (accountDialog = null)} />
{/if}
{#if deploying && editor}
  <DeployDialog {editor} onclose={() => (deploying = false)} />
{/if}
{#key dialogs.current}
  {#if dialogs.current}<AppDialog request={dialogs.current} />{/if}
{/key}
