<script lang="ts">
  // The open confirm/prompt/alert request from dialogs.svelte.ts.
  import { untrack } from "svelte";
  import { cancel, dialogs, type DialogRequest } from "../dialogs.svelte";
  import { t } from "../i18n.svelte";

  let { request }: { request: DialogRequest } = $props();

  let dialog: HTMLDialogElement;
  // A prompt's text, seeded once from the request.
  let value = $state(untrack(() => (request.kind === "prompt" ? request.value : "")));
  let done = false;

  $effect(() => dialog.showModal());

  function finish(fn: () => void) {
    done = true;
    // Resolve first: `request` follows dialogs.current and is gone after this.
    const current = request;
    fn();
    if (dialogs.current === current) dialogs.current = null;
  }

  function submit(e: SubmitEvent) {
    e.preventDefault();
    if (request.kind === "confirm") finish(() => request.resolve(true));
    else if (request.kind === "prompt") finish(() => request.resolve(value));
    else finish(() => request.resolve());
  }

  // Escape, the cancel button and replacing the request all end up here.
  function onclose() {
    if (!done) cancel(request);
  }
</script>

<dialog bind:this={dialog} class="modal" {onclose} aria-labelledby="app-dialog-message">
  <form class="modal-box grid max-w-sm gap-4" onsubmit={submit}>
    {#if request.kind === "prompt"}
      <label class="grid gap-2">
        <span id="app-dialog-message" class="font-semibold">{request.message}</span>
        <!-- svelte-ignore a11y_autofocus -->
        <input class="input w-full" bind:value autofocus autocapitalize="off" spellcheck="false" />
      </label>
    {:else}
      <p id="app-dialog-message">{request.message}</p>
    {/if}

    <div class="modal-action mt-0">
      {#if request.kind !== "alert"}
        <button type="button" class="btn btn-ghost" onclick={() => dialog.close()}>{t("account.cancel")}</button>
      {/if}
      <!-- svelte-ignore a11y_autofocus -->
      <button
        class={["btn", request.kind === "confirm" && request.danger ? "btn-error" : "btn-primary"]}
        disabled={request.kind === "prompt" && !value.trim()}
        autofocus={request.kind !== "prompt"}
      >
        {request.kind === "alert" ? t("dialog.ok") : request.confirmLabel}
      </button>
    </div>
  </form>
</dialog>
