<script lang="ts">
  // Taking over a filter provider's filters (concept §3, D19): what it does,
  // why both side by side doesn't work, and the rules it creates.
  import { untrack } from "svelte";
  import type { Detection, RuleItem, RuleSet } from "#tauri/types";
  import { describeError } from "../api";
  import { t, type MessageKey } from "../i18n.svelte";
  import { conditionToken } from "../rules/route";
  import { mailboxLabel } from "../rules/mailbox";
  import { session } from "../session.svelte";
  import { tell } from "../dialogs.svelte";
  import Token from "./route/Token.svelte";

  let {
    accountId,
    detection,
    onclose,
  }: {
    accountId: string;
    detection: Detection;
    onclose: () => void;
  } = $props();

  let dialog: HTMLDialogElement;
  $effect(() => dialog.showModal());

  const provider = $derived(t(`provider.${detection.provider}` as MessageKey));
  const taken = $derived(new Set((session.servers[accountId]?.info?.scripts ?? []).map((s) => s.name)));

  /** A name for the new script that isn't taken yet. */
  function freeName(base: string): string {
    if (!taken.has(base)) return base;
    for (let n = 2; ; n++) if (!taken.has(`${base} ${n}`)) return `${base} ${n}`;
  }

  let name = $state(untrack(() => freeName(t(`provider.${detection.provider}Script` as MessageKey))));
  let rules = $state<RuleSet | null>(null);
  let error = $state<string | null>(null);
  let busy = $state(false);
  const nameError = $derived(taken.has(name.trim()) ? t("provider.nameTaken") : null);

  $effect(() => {
    untrack(() =>
      session.previewProvider(accountId, detection).then(
        (r) => (rules = r),
        (e) => (error = describeError(e)),
      ),
    );
  });

  /** Where a rule moves the message, if it does. */
  function target(item: RuleItem): string | null {
    if (item.type !== "rule") return null;
    const move = item.actions.find((a) => a.type === "fileInto");
    return move?.type === "fileInto" ? mailboxLabel(move.mailbox) : null;
  }

  async function apply(e: SubmitEvent) {
    e.preventDefault();
    if (!rules?.items.length || nameError || !name.trim()) return;
    busy = true;
    const ok = await session.takeOverProvider(accountId, detection, name.trim());
    busy = false;
    if (ok) {
      dialog.close();
      void session.openScript(accountId, name.trim());
      void tell(t("provider.done", { provider }));
    } else {
      error = session.error;
    }
  }
</script>

<dialog bind:this={dialog} class="modal" {onclose} aria-labelledby="provider-heading">
  <form class="modal-box flex max-h-[85vh] w-11/12 max-w-2xl flex-col gap-3" onsubmit={apply}>
    <div class="grid gap-1.5">
      <h2 id="provider-heading" class="text-lg font-semibold">{t("provider.heading", { provider })}</h2>
      <p class="text-sm text-base-content/80">{t("provider.intro", { provider })}</p>
      <p class="rounded-field border border-warning/40 bg-warning/10 px-2.5 py-1.5 text-sm">{t("provider.why", { provider })}</p>
    </div>

    <label class="fieldset">
      <span class="fieldset-legend">{t("provider.scriptName")}</span>
      <input class="input w-full" bind:value={name} aria-invalid={!!nameError} spellcheck="false" />
      {#if nameError}<span class="label text-error">{nameError}</span>{/if}
    </label>

    <div class="grid min-h-0 gap-1.5">
      {#if error}
        <p role="alert" class="text-sm text-error">{error}</p>
      {:else if !rules}
        <p class="text-sm text-base-content/70">{t("provider.loading")}</p>
      {:else if rules.items.length === 0}
        <p class="text-sm text-base-content/70">{t("provider.empty")}</p>
      {:else}
        <p class="text-sm font-medium">{t("provider.preview")}</p>
        <ul class="grid min-h-0 gap-1.5 overflow-y-auto">
          {#each rules.items as item, i (i)}
            <li class="grid gap-1 rounded-box border border-base-300 px-2.5 py-2">
              {#if item.type === "rule"}
                <span class="font-semibold">{item.name}</span>
                <div class="flex flex-wrap items-baseline gap-x-2 gap-y-1">
                  {#each item.conditions.items as condition, c (c)}
                    {#if c > 0}<span class="text-sm text-base-content/70">{t(item.conditions.join === "all" ? "route.and" : "route.or")}</span>{/if}
                    <Token token={conditionToken(condition)} />
                  {/each}
                  {#if target(item)}<span class="text-sm">→ <span class="font-medium">{target(item)}</span></span>{/if}
                </div>
              {:else}
                <span class="font-semibold">{t("provider.raw")}</span>
                <pre class="overflow-x-auto text-xs">{item.code}</pre>
              {/if}
            </li>
          {/each}
        </ul>
      {/if}
    </div>

    <div class="modal-action mt-0">
      <button type="button" class="btn btn-ghost" onclick={() => dialog.close()}>{t("copy.later")}</button>
      <button class="btn btn-primary" disabled={busy || !rules?.items.length || !!nameError || !name.trim()}>
        {t("provider.confirm")}
      </button>
    </div>
  </form>
</dialog>
