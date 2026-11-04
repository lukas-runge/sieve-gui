<script lang="ts">
  // Deploy flow (concept §4): diff against the server version, CHECKSCRIPT,
  // then PUTSCRIPT with conflict detection.
  import { untrack } from "svelte";
  import { checkScript, deployScript, scriptContent } from "#tauri/commands";
  import type { Diagnostic } from "#tauri/types";
  import { describeError, isAppError } from "../api";
  import { diffLines } from "../diff";
  import type { ScriptEditor } from "../editor.svelte";
  import { t } from "../i18n.svelte";
  import { session } from "../session.svelte";

  let { editor, onclose }: { editor: ScriptEditor; onclose: () => void } = $props();

  let dialog: HTMLDialogElement;
  let phase = $state<"checking" | "ready" | "invalid" | "saving" | "conflict" | "failed">("checking");
  let diagnostics = $state<Diagnostic[]>([]);
  let error = $state<string | null>(null);

  // What gets checked and saved is the code at the moment the dialog opened.
  const { code, diff, active } = untrack(() => ({
    code: editor.code,
    diff: diffLines(editor.original ?? "", editor.code),
    active: session.runs(editor.accountId, editor.name),
  }));
  const changed = diff.some((l) => l.kind !== "same");
  const count = (d: { kind: string }[]) => d.filter((l) => l.kind !== "same").length;

  // Only what the script does: without comments, layout and spellings that
  // mean the same (`sieve_core::content`). Saving in sieve-gui's layout the
  // first time rewrites the whole text; this shows what really changes.
  let onlyContent = $state(localStorage.getItem("deployContentOnly") === "1");
  $effect(() => localStorage.setItem("deployContentOnly", onlyContent ? "1" : "0"));
  let contentDiff = $state<ReturnType<typeof diffLines> | null>(null);
  void (async () => {
    const [before, after] = await Promise.all([scriptContent(editor.original ?? ""), scriptContent(code)]);
    // A side that doesn't parse can't be compared by content: the text diff stays.
    if (before !== null && after !== null) contentDiff = diffLines(editor.original ? before : "", after);
  })();
  const shown = $derived(onlyContent && contentDiff ? contentDiff : diff);

  $effect(() => {
    dialog.showModal();
    check();
  });

  async function check() {
    phase = "checking";
    try {
      diagnostics = await checkScript(editor.accountId, code);
      phase = "ready";
    } catch (e) {
      diagnostics = isAppError(e) ? e.diagnostics : [];
      error = describeError(e);
      phase = isAppError(e) && e.kind === "server" ? "invalid" : "failed";
    }
  }

  async function save(activate: boolean) {
    phase = "saving";
    try {
      diagnostics = await deployScript(editor.accountId, editor.name, code, editor.original);
      editor.markSaved(code);
      session.scriptChanged(editor.accountId, editor.name);
      if (activate) await session.activate(editor.accountId, editor.name);
      else await session.refreshScripts(editor.accountId);
      dialog.close();
    } catch (e) {
      error = describeError(e);
      phase = isAppError(e) && e.kind === "conflict" ? "conflict" : "failed";
    }
  }

  async function reload() {
    dialog.close();
    await session.openScript(editor.accountId, editor.name);
  }
</script>

<dialog bind:this={dialog} class="modal" {onclose} aria-labelledby="deploy-heading">
  <div class="modal-box flex max-h-[85vh] w-11/12 max-w-3xl flex-col gap-3">
    <h2 id="deploy-heading" class="text-lg font-semibold">{t("deploy.heading", { name: editor.name })}</h2>

    <div role="status" aria-live="polite">
      {#if phase === "checking"}
        <p class="flex items-center gap-2"><span class="loading loading-spinner loading-sm"></span>{t("deploy.checking")}</p>
      {:else if phase === "ready"}
        <p class="text-success">{diagnostics.length ? t("deploy.warnings") : t("deploy.ok")}</p>
      {:else if phase === "invalid"}
        <p class="text-error">{t("deploy.errors")}</p>
      {:else if phase === "conflict"}
        <p class="text-error">{t("deploy.conflict")}</p>
      {:else if phase === "failed"}
        <p class="text-error">{error}</p>
      {/if}
    </div>

    {#if diagnostics.length}
      <ul class="grid gap-1 text-sm">
        {#each diagnostics as d}
          <li class={d.severity === "error" ? "text-error" : "text-warning"}>
            {#if d.line}<span class="font-semibold">{t("deploy.line", { line: d.line })}:</span>{/if}
            {d.message}
          </li>
        {/each}
      </ul>
    {/if}

    <div class="flex flex-wrap items-center gap-x-4 gap-y-1">
      <h3 class="font-semibold">{t("deploy.diff")}</h3>
      {#if changed && contentDiff}
        <span class="text-[0.8125rem] text-base-content/70">
          {t("deploy.lineCounts", { all: count(diff), content: count(contentDiff) })}
        </span>
        <label class="ml-auto flex items-center gap-2 text-sm">
          <input type="checkbox" class="toggle toggle-sm toggle-primary" bind:checked={onlyContent} />
          {t("deploy.onlyContent")}
        </label>
      {/if}
    </div>
    {#if changed && onlyContent && contentDiff && count(contentDiff) === 0}
      <!-- Saving changes the text, not what it does. -->
      <p class="rounded-field bg-base-200 px-3 py-2 text-sm text-base-content/80">{t("deploy.noContentChanges")}</p>
    {:else if changed}
      <pre class="min-h-24 overflow-auto rounded-field bg-base-200 p-2 font-mono text-xs leading-relaxed">{#each shown as line}<span
            class="block {line.kind === 'add'
              ? 'bg-success/15 text-success'
              : line.kind === 'remove'
                ? 'bg-error/15 text-error line-through decoration-error/40'
                : 'text-base-content/70'}"><span aria-hidden="true"
              >{line.kind === "add" ? "+ " : line.kind === "remove" ? "− " : "  "}</span
            >{line.text || " "}</span
          >{/each}</pre>
    {:else}
      <p class="text-base-content/70">{t("deploy.noChanges")}</p>
    {/if}

    <div class="modal-action">
      <button class="btn btn-ghost" onclick={() => dialog.close()}>{t("deploy.cancel")}</button>
      {#if phase === "conflict"}
        <button class="btn btn-error" onclick={reload}>{t("deploy.reload")}</button>
      {:else}
        {#if !active}
          <button class="btn" disabled={phase !== "ready"} onclick={() => save(true)}>{t("deploy.saveAndActivate")}</button>
        {/if}
        <button class="btn btn-primary" disabled={phase !== "ready"} onclick={() => save(false)}>
          {#if phase === "saving"}<span class="loading loading-spinner loading-sm"></span>{/if}
          {t("deploy.save")}
        </button>
      {/if}
    </div>
  </div>
</dialog>
