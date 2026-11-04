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
  import { dialogs } from "../lib/dialogs.svelte";
  import { i18n, t } from "../lib/i18n.svelte";
  import { ROOT_SCRIPT, session } from "../lib/session.svelte";

  let accountDialog = $state<{ account: Account | null } | null>(null);
  let deploying = $state(false);
  // The code sidebar stays open or closed across restarts.
  let showCode = $state(localStorage.getItem("showCode") === "1");
  $effect(() => localStorage.setItem("showCode", showCode ? "1" : "0"));
  // Wide windows show the code beside the rules, narrow ones instead of them.
  const wide = new MediaQuery("min-width: 1100px");

  const editor = $derived(session.editor);
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
  /** "2nd of 3" when several scripts run in order. */
  const position = $derived.by(() => {
    if (!editor || !root || root.scripts.length < 2) return null;
    const i = root.scripts.indexOf(editor.name);
    return i >= 0 ? { n: i + 1, count: root.scripts.length } : null;
  });
  const serverFolders = $derived(
    editor && session.folders[editor.accountId]?.status === "ready" ? session.folders[editor.accountId].folders : null,
  );
  /** The server's folder hierarchy delimiter, to show folder paths. */
  const delimiter = $derived(serverFolders?.find((f) => f.delimiter)?.delimiter ?? null);
  /** The code or the picked rule beside the route. */
  const rightPanel = $derived(!!editor && (showCode || !!editor.selected));

  /** Code and inspector share the place on the right. */
  function toggleCode() {
    showCode = !showCode;
    if (showCode && editor) editor.selected = null;
  }
  $effect(() => {
    if (editor?.selected) showCode = false;
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

<div
  class={[
    "grid h-screen",
    rightPanel && wide.current ? "grid-cols-[16rem_minmax(0,1fr)_clamp(22rem,34vw,34rem)]" : "grid-cols-[16rem_minmax(0,1fr)]",
  ]}
>
  <aside class="min-h-0 border-r border-base-300 bg-base-200">
    <Sidebar onaccount={(account) => (accountDialog = { account })} />
  </aside>

  <main class="flex min-w-0 flex-col overflow-hidden bg-base-200">
    {#if editor}
      <header class="flex flex-wrap items-center gap-x-4 gap-y-2 border-b border-base-300 bg-base-100 px-6 py-3">
        <div class="min-w-0 grow">
          <div class="flex min-w-0 items-center gap-3">
            <h1 class="truncate text-[1.35rem] leading-tight font-semibold">{editor.name}</h1>
            <span
              class={[
                "inline-flex shrink-0 items-center gap-1.5 rounded-full px-2.5 py-0.5 text-[0.8rem] font-medium",
                active ? "bg-success/12 text-success" : "bg-base-200 text-base-content/60",
              ]}
            >
              <span class={["size-1.5 rounded-full", active ? "bg-success" : "bg-base-content/40"]}></span>
              {#if !active}{t("status.off")}{:else if position}{t("status.runsAt", position)}{:else}{t("status.runs")}{/if}
            </span>
          </div>
          <div class="-ml-1 flex items-center gap-0.5 text-sm text-base-content/60">
            <button class="btn btn-ghost btn-xs btn-square" disabled={!editor.canUndo} onclick={() => editor.undo()} aria-label={t("editor.undo")} title="{t('editor.undo')} (⌘Z)">
              <Icon name="undo" size={15} />
            </button>
            <button class="btn btn-ghost btn-xs btn-square" disabled={!editor.canRedo} onclick={() => editor.redo()} aria-label={t("editor.redo")} title="{t('editor.redo')} (⇧⌘Z)">
              <Icon name="redo" size={15} />
            </button>
            <p class="ml-1 flex items-center gap-1.5 truncate" aria-live="polite">
              {#if editor.dirty}<span class="size-1.5 rounded-full bg-warning"></span>{t("editor.unsaved")}{:else}{t("editor.saved")}{/if}
            </p>
          </div>
        </div>
        <div class="ml-auto flex items-center gap-1.5">
          <button
            class={["btn btn-ghost btn-sm gap-1.5", showCode && "btn-active"]}
            aria-pressed={showCode}
            title={showCode ? t("editor.hideCode") : t("editor.showCode")}
            onclick={toggleCode}
          >
            <Icon name="code" size={16} />
            {t("editor.code")}
          </button>
          <button class="btn btn-primary btn-sm gap-1.5" disabled={!editor.dirty} onclick={() => (deploying = true)} title="⌘S">
            <Icon name="save" size={15} />{t("editor.deploy")}
          </button>
        </div>
      </header>

      {#if rightPanel && !wide.current}
        <!-- Narrow windows: the inspector or the code takes the place of the route. -->
        <section class="flex min-h-0 grow flex-col">
          {#if showCode}
            <div class="flex min-h-0 grow flex-col gap-2 p-3">{@render codePanel(editor.code)}</div>
          {:else}
            <Inspector {editor} extensions={session.extensions} serverFolders={serverFolders} />
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
            />
          </div>
        </div>
      {/if}
    {:else if session.rootView && session.rootSources[session.rootView] !== undefined}
      <header class="border-b border-base-300 bg-base-100 px-6 py-3">
        <h1 class="flex items-center gap-2 truncate text-[1.35rem] font-semibold">
          <Icon name="lock" size={17} />{ROOT_SCRIPT}
        </h1>
        <p class="text-sm text-base-content/60">{t("root.viewHint")}</p>
      </header>
      <section class="flex min-h-0 grow flex-col p-4" aria-label={ROOT_SCRIPT}>
        <div class="min-h-0 overflow-auto rounded-box border border-base-300 bg-base-100">
          <CodeEditor value={session.rootSources[session.rootView]} readonly label={ROOT_SCRIPT} />
        </div>
      </section>
    {:else}
      <div class="grid h-full place-items-center p-8">
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
                  <p class="px-1 text-[0.8rem] text-base-content/55">{t("welcome.running", { account: account.label })}</p>
                  {#each running as node, i (node.name)}
                    <button
                      class="flex items-center gap-2.5 rounded-field bg-base-100 px-3 py-2 text-left ring-1 ring-base-300 hover:ring-primary/40"
                      onclick={async () => (await session.confirmLeave()) && session.openScript(account.id, node.name)}
                    >
                      <span class="grid size-6 place-items-center rounded-full bg-primary/12 text-[0.72rem] font-bold text-primary">{i + 1}</span>
                      <span class="grow font-medium">{node.name}</span>
                      <Icon name="chevronRight" size={16} class="text-base-content/40" />
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

  {#if rightPanel && wide.current}
    {#if showCode}
      <aside class="flex min-w-0 flex-col gap-2 overflow-hidden border-l border-base-300 bg-base-100 p-3" aria-label={t("code.heading")}>
        {@render codePanel(editor!.code)}
      </aside>
    {:else}
      <Inspector editor={editor!} extensions={session.extensions} serverFolders={serverFolders} />
    {/if}
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
