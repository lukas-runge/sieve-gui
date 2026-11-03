<script lang="ts">
  // Differences between the server and the account's local copy (concept
  // §3, D13): per script, the user decides which side wins.
  import { untrack } from "svelte";
  import type { CopyStatus, Difference, Side } from "#tauri/types";
  import { diffLines } from "../diff";
  import { t, type MessageKey } from "../i18n.svelte";

  let {
    status,
    label,
    onresolve,
    onclose,
  }: {
    status: CopyStatus;
    /** The account's name. */
    label: string;
    onresolve: (choices: { name: string; keep: Side }[], active: Side | null) => Promise<void>;
    onclose: () => void;
  } = $props();

  let dialog: HTMLDialogElement;
  $effect(() => dialog.showModal());

  /** What happened to a script, e.g. "deleted on the server". */
  function what(d: Difference): MessageKey {
    const server = d.changed.includes("server");
    const local = d.changed.includes("local");
    if (d.server === null) return server ? "copy.deletedOnServer" : "copy.onlyLocal";
    if (d.local === null) return local ? "copy.deletedLocally" : "copy.onlyOnServer";
    if (server && local) return "copy.changedBoth";
    return server ? "copy.changedOnServer" : "copy.changedLocally";
  }

  /**
   * Default choice: restore what the server lost (the copy exists for
   * that), keep changes made on purpose on either side.
   */
  function proposal(d: Difference): Side {
    if (d.server === null) return "local";
    return d.changed.includes("local") && !d.changed.includes("server") ? "local" : "server";
  }

  const initial = untrack(() => status);
  let choices = $state<Record<string, Side>>(Object.fromEntries(initial.differences.map((d) => [d.name, proposal(d)])));
  let active = $state<Side | null>(initial.active ? "local" : null);
  let open = $state<Record<string, boolean>>({});
  let busy = $state(false);

  function all(side: Side) {
    for (const d of status.differences) choices[d.name] = side;
    if (active) active = side;
  }

  async function apply(e: SubmitEvent) {
    e.preventDefault();
    busy = true;
    try {
      await onresolve(
        status.differences.map((d) => ({ name: d.name, keep: choices[d.name] })),
        active,
      );
      // Gone already once the differences are resolved.
      dialog?.close();
    } finally {
      busy = false;
    }
  }
</script>

{#snippet sides(name: string, get: () => Side | null, set: (side: Side) => void, localLabel = t("copy.keepLocal"))}
  <div class="join" role="radiogroup" aria-label={name}>
    {#each ["server", "local"] as const as side}
      <button
        type="button"
        role="radio"
        aria-checked={get() === side}
        class={["btn btn-xs join-item", get() === side && "btn-active btn-primary"]}
        onclick={() => set(side)}
      >
        {side === "server" ? t("copy.keepServer") : localLabel}
      </button>
    {/each}
  </div>
{/snippet}

<dialog bind:this={dialog} class="modal" {onclose} aria-labelledby="copy-heading">
  <form class="modal-box flex max-h-[85vh] w-11/12 max-w-3xl flex-col gap-3" onsubmit={apply}>
    <div class="grid gap-1">
      <h2 id="copy-heading" class="text-lg font-semibold">{t("copy.heading", { label })}</h2>
      <p class="text-sm text-base-content/70">{t("copy.intro")}</p>
    </div>

    <div class="flex flex-wrap items-center gap-2 text-sm">
      <span class="text-base-content/70">{t("copy.all")}</span>
      <button type="button" class="btn btn-xs" onclick={() => all("server")}>{t("copy.keepServer")}</button>
      <button type="button" class="btn btn-xs" onclick={() => all("local")}>{t("copy.keepLocal")}</button>
    </div>

    <ul class="grid min-h-0 gap-2 overflow-y-auto">
      {#if status.active}
        <li class="grid gap-1 rounded-box border border-base-300 p-2">
          <div class="flex flex-wrap items-center gap-2">
            <span class="grow font-semibold">{t("copy.activeScript")}</span>
            {@render sides(
              t("copy.activeScript"),
              () => active,
              (side) => (active = side),
              status.active.local ? t("copy.reactivate", { name: status.active.local }) : t("copy.deactivate"),
            )}
          </div>
          <p class="text-sm text-base-content/70">
            {t("copy.activeChanged", {
              server: status.active.server ?? t("copy.none"),
              local: status.active.local ?? t("copy.none"),
            })}
          </p>
        </li>
      {/if}
      {#each status.differences as d (d.name)}
        <li class="grid gap-1 rounded-box border border-base-300 p-2">
          <div class="flex flex-wrap items-center gap-2">
            <span class="grow font-mono text-sm font-semibold">{d.name}</span>
            {@render sides(d.name, () => choices[d.name], (side) => (choices[d.name] = side))}
          </div>
          <div class="flex flex-wrap items-center gap-2 text-sm">
            <span class={["grow", d.changed.includes("server") ? "text-warning" : "text-base-content/70"]}>{t(what(d))}</span>
            {#if d.server !== null && d.local !== null}
              <button type="button" class="link text-xs" onclick={() => (open[d.name] = !open[d.name])}>
                {open[d.name] ? t("copy.hideDiff") : t("copy.showDiff")}
              </button>
            {/if}
          </div>
          {#if open[d.name]}
            <!-- From the copy to the server: + is only on the server. -->
            <pre class="max-h-64 overflow-auto rounded-field bg-base-200 p-2 font-mono text-xs leading-relaxed">{#each diffLines(d.local ?? "", d.server ?? "") as line}<span
                  class="block {line.kind === 'add'
                    ? 'bg-success/15 text-success'
                    : line.kind === 'remove'
                      ? 'bg-error/15 text-error'
                      : 'text-base-content/70'}"><span aria-hidden="true"
                    >{line.kind === "add" ? "+ " : line.kind === "remove" ? "− " : "  "}</span
                  >{line.text || " "}</span
                >{/each}</pre>
            <p class="text-xs text-base-content/60">{t("copy.diffLegend")}</p>
          {/if}
        </li>
      {/each}
    </ul>

    <p class="text-xs text-base-content/60">{t("copy.where", { dir: status.dir })}</p>

    <div class="modal-action mt-0">
      <button type="button" class="btn btn-ghost" onclick={() => dialog.close()}>{t("copy.later")}</button>
      <button class="btn btn-primary" disabled={busy}>
        {#if busy}<span class="loading loading-spinner loading-sm"></span>{/if}
        {t("copy.apply")}
      </button>
    </div>
  </form>
</dialog>
