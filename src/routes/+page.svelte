<script lang="ts">
  import { onMount } from "svelte";
  import { MediaQuery } from "svelte/reactivity";
  import type { Account } from "#tauri/types";
  import AccountDialog from "../lib/components/AccountDialog.svelte";
  import AppDialog from "../lib/components/AppDialog.svelte";
  import CodeEditor from "../lib/components/CodeEditor.svelte";
  import DeployDialog from "../lib/components/DeployDialog.svelte";
  import Icon from "../lib/components/Icon.svelte";
  import RuleList from "../lib/components/RuleList.svelte";
  import Sidebar from "../lib/components/Sidebar.svelte";
  import { dialogs } from "../lib/dialogs.svelte";
  import { i18n, t } from "../lib/i18n.svelte";
  import { session } from "../lib/session.svelte";

  let accountDialog = $state<{ account: Account | null } | null>(null);
  let deploying = $state(false);
  // The code sidebar stays open or closed across restarts.
  let showCode = $state(localStorage.getItem("showCode") === "1");
  $effect(() => localStorage.setItem("showCode", showCode ? "1" : "0"));
  // Wide windows show the code beside the rules, narrow ones instead of them.
  const wide = new MediaQuery("min-width: 1280px");

  const editor = $derived(session.editor);
  const active = $derived(editor ? session.isActive(editor.accountId, editor.name) : false);

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
    }
  }
</script>

<svelte:window {onkeydown} />

<div class={["grid h-screen", showCode && editor && wide.current ? "grid-cols-[15rem_1fr_clamp(20rem,36vw,44rem)]" : "grid-cols-[15rem_1fr]"]}>
  <aside class="border-r border-base-300 bg-base-200">
    <Sidebar onaccount={(account) => (accountDialog = { account })} />
  </aside>

  <main class="flex min-w-0 flex-col overflow-hidden bg-base-200">
    {#if editor}
      <header class="flex flex-wrap items-center gap-2 border-b border-base-300 px-5 py-3">
        <div class="min-w-0 grow">
          <h1 class="truncate text-xl font-semibold">{editor.name}</h1>
          <!-- -ml-1 lines the undo icon up with the name above. -->
          <div class="-ml-1 flex items-center gap-1">
            <button class="btn btn-ghost btn-xs btn-square" disabled={!editor.canUndo} onclick={() => editor.undo()} aria-label={t("editor.undo")} title="{t('editor.undo')} (⌘Z)">
              <Icon name="undo" size={15} />
            </button>
            <button class="btn btn-ghost btn-xs btn-square" disabled={!editor.canRedo} onclick={() => editor.redo()} aria-label={t("editor.redo")} title="{t('editor.redo')} (⇧⌘Z)">
              <Icon name="redo" size={15} />
            </button>
            <p class="ml-1 truncate text-sm text-base-content/60" aria-live="polite">
              {editor.dirty ? t("editor.unsaved") : t("editor.saved")}
            </p>
          </div>
        </div>
        <div class="ml-auto flex items-center gap-1">
        <button class="btn btn-primary btn-sm" disabled={!editor.dirty} onclick={() => (deploying = true)} title="⌘S">
          {t("editor.deploy")}
        </button>
        <button
          class={["btn btn-ghost btn-sm gap-1.5", showCode && "btn-active"]}
          aria-pressed={showCode}
          title={showCode ? t("editor.hideCode") : t("editor.showCode")}
          onclick={() => (showCode = !showCode)}
        >
          <Icon name="panelRight" size={16} />
          {t("editor.code")}
        </button>
        </div>
      </header>

      {#if showCode && !wide.current}
        <section class="flex min-h-0 grow flex-col gap-2 p-3" aria-label={t("code.heading")}>
          {@render codePanel(editor.code)}
        </section>
      {:else}
        <div class="min-h-0 grow overflow-y-auto">
          <div class="mx-auto grid max-w-3xl grid-cols-[minmax(0,1fr)] gap-4 px-5 py-5">
            {#if !active}
              <p class="rounded-box border border-warning/40 bg-warning/10 px-3 py-2 text-sm">{t("editor.inactive")}</p>
          {/if}
          {#if editor.roundcubeRisks && (editor.roundcubeRisks.rawBlocks.length > 0 || editor.roundcubeRisks.bracketComments)}
            {@const risks = editor.roundcubeRisks}
            <div role="note" class="grid gap-1 rounded-box border border-warning/40 bg-warning/10 px-3 py-2 text-sm">
              <p class="font-semibold">{t("roundcube.title")}</p>
              {#if risks.bracketComments}
                <p>{t("roundcube.bracket")}</p>
              {:else}
                <p>{risks.rawBlocks.length === 1 ? t("roundcube.rawOne") : t("roundcube.rawMany", { count: risks.rawBlocks.length })}</p>
              {/if}
            </div>
          {/if}
          {#if session.error}
            <p role="alert" class="rounded-box border border-error/40 bg-error/10 px-3 py-2 text-sm">{session.error}</p>
          {/if}
          <RuleList
            {editor}
            extensions={session.extensions}
            serverFolders={session.folders[editor.accountId]?.status === "ready" ? session.folders[editor.accountId].folders : null}
          />
        </div>
      </div>
      {/if}
    {:else}
      <div class="grid h-full place-items-center p-8 text-center text-base-content/70">
        {#if session.loadingScript}
          <p class="flex items-center gap-2"><span class="loading loading-spinner"></span>{t("editor.loading")}</p>
        {:else}
          <div class="grid gap-2">
            {#if session.error}<p role="alert" class="text-error">{session.error}</p>{/if}
            <p>{t("editor.empty")}</p>
          </div>
        {/if}
      </div>
    {/if}
  </main>

  {#if showCode && editor && wide.current}
    <aside class="flex min-w-0 flex-col gap-2 overflow-hidden border-l border-base-300 bg-base-200 p-3" aria-label={t("code.heading")}>
      {@render codePanel(editor.code)}
    </aside>
  {/if}
</div>

{#snippet codePanel(code: string)}
  <div class="flex items-center gap-2">
    <h2 class="grow font-semibold">{t("code.heading")}</h2>
    <button class="btn btn-ghost btn-sm btn-square" aria-label={t("editor.hideCode")} title={t("editor.hideCode")} onclick={() => (showCode = false)}>✕</button>
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
