<script lang="ts">
  import { onMount } from "svelte";
  import type { Account } from "#tauri/types";
  import AccountDialog from "../lib/components/AccountDialog.svelte";
  import AppDialog from "../lib/components/AppDialog.svelte";
  import CodeEditor from "../lib/components/CodeEditor.svelte";
  import DeployDialog from "../lib/components/DeployDialog.svelte";
  import RuleList from "../lib/components/RuleList.svelte";
  import Sidebar from "../lib/components/Sidebar.svelte";
  import { dialogs } from "../lib/dialogs.svelte";
  import { i18n, t } from "../lib/i18n.svelte";
  import { session } from "../lib/session.svelte";

  let accountDialog = $state<{ account: Account | null } | null>(null);
  let deploying = $state(false);
  let showCode = $state(false);

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

<div class="grid h-screen grid-cols-[15rem_1fr]">
  <aside class="border-r border-base-300 bg-base-200">
    <Sidebar onaccount={(account) => (accountDialog = { account })} />
  </aside>

  <main class="flex min-w-0 flex-col overflow-hidden bg-base-200">
    {#if editor}
      <header class="flex flex-wrap items-center gap-2 border-b border-base-300 px-5 py-3">
        <div class="min-w-0 grow">
          <h1 class="truncate text-xl font-semibold">{editor.name}</h1>
          <p class="text-sm text-base-content/60" aria-live="polite">
            {editor.dirty ? t("editor.unsaved") : t("editor.saved")}
          </p>
        </div>
        <button class="btn btn-ghost btn-sm" disabled={!editor.canUndo} onclick={() => editor.undo()} title="{t('editor.undo')} (⌘Z)">
          {t("editor.undo")}
        </button>
        <button class="btn btn-ghost btn-sm" disabled={!editor.canRedo} onclick={() => editor.redo()} title="{t('editor.redo')} (⇧⌘Z)">
          {t("editor.redo")}
        </button>
        <button class="btn btn-ghost btn-sm" aria-pressed={showCode} onclick={() => (showCode = !showCode)}>
          {showCode ? t("editor.hideCode") : t("editor.showCode")}
        </button>
        <button class="btn btn-primary btn-sm" disabled={!editor.dirty} onclick={() => (deploying = true)} title="⌘S">
          {t("editor.deploy")}
        </button>
      </header>

      <div class="relative flex min-h-0 grow">
      <div class="min-w-0 grow overflow-y-auto">
        <div class="mx-auto grid max-w-3xl gap-4 px-5 py-5">
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

      {#if showCode}
        <!-- Beside the rules on wide windows, otherwise over their right edge. -->
        <aside
          class="absolute inset-y-0 right-0 z-20 flex w-[min(32rem,95%)] flex-col gap-2 border-l border-base-300 bg-base-200 p-3 shadow-xl xl:static xl:w-[32rem] xl:shrink-0 xl:shadow-none"
        >
          <div class="flex items-center gap-2">
            <h2 class="grow font-semibold">{t("code.heading")}</h2>
            <button class="btn btn-ghost btn-sm btn-square" aria-label={t("editor.hideCode")} title={t("editor.hideCode")} onclick={() => (showCode = false)}>✕</button>
          </div>
          <div class="min-h-0 overflow-auto">
            <CodeEditor value={editor.code} readonly label={t("code.heading")} />
          </div>
        </aside>
      {/if}
      </div>
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

</div>

{#if accountDialog}
  <AccountDialog account={accountDialog.account} onclose={() => (accountDialog = null)} />
{/if}
{#if deploying && editor}
  <DeployDialog {editor} onclose={() => (deploying = false)} />
{/if}
{#key dialogs.current}
  {#if dialogs.current}<AppDialog request={dialogs.current} />{/if}
{/key}
