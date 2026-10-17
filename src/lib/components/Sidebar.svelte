<script lang="ts">
  import { flip } from "svelte/animate";
  import { SvelteSet } from "svelte/reactivity";
  import type { Account } from "#tauri/types";
  import { t } from "../i18n.svelte";
  import { ask, askText, tell } from "../dialogs.svelte";
  import { closesMenu } from "../menu";
  import { ROOT_SCRIPT, session } from "../session.svelte";

  let { onaccount }: { onaccount: (account: Account | null) => void } = $props();

  async function open(id: string, name: string) {
    if (session.editor?.accountId === id && session.editor.name === name) return;
    if (await session.confirmLeave()) await session.openScript(id, name);
  }

  async function create(id: string) {
    const name = await askText(t("scripts.newName"), t("scripts.create"));
    if (name && (await session.confirmLeave())) await session.newScript(id, name);
  }

  async function rename(id: string, name: string) {
    const next = await askText(t("scripts.renamePrompt", { name }), t("scripts.rename"), name);
    if (next && next !== name) await session.rename(id, name, next);
  }

  async function remove(id: string, name: string, active: boolean) {
    if (active) return tell(t("scripts.deleteActive"));
    if (await ask(t("scripts.deleteConfirm", { name }), t("scripts.delete"), true)) await session.deleteScript(id, name);
  }

  /** The script being dragged to a new place in the list. */
  let drag = $state<{ account: string; from: number } | null>(null);
  let dropAt = $state<number | null>(null);

  /** Scripts (`account:name`) whose active state is being changed. */
  const toggling = new SvelteSet<string>();

  async function toggle(id: string, name: string, active: boolean) {
    const key = `${id}:${name}`;
    toggling.add(key);
    try {
      if (active) await session.deactivate(id, name);
      else await session.activate(id, name);
    } finally {
      toggling.delete(key);
    }
  }

  function moveTo(id: string, order: string[], from: number, to: number) {
    if (to < 0 || to >= order.length || from === to) return;
    const next = [...order];
    next.splice(to, 0, ...next.splice(from, 1));
    void session.reorderScripts(id, next);
  }

  function onhandlekey(e: KeyboardEvent, id: string, order: string[], index: number) {
    if (!e.altKey || (e.key !== "ArrowUp" && e.key !== "ArrowDown")) return;
    e.preventDefault();
    const to = index + (e.key === "ArrowUp" ? -1 : 1);
    moveTo(id, order, index, to);
    // Keep focus on the moved handle.
    requestAnimationFrame(() => document.querySelector<HTMLElement>(`[data-script-handle="${id}:${to}"]`)?.focus());
  }

  async function removeAccount(account: Account) {
    if (await ask(t("accounts.deleteConfirm", { label: account.label }), t("accounts.delete"), true)) {
      await session.deleteAccount(account.id);
    }
  }
</script>

{#snippet row(id: string, name: string, active: boolean, index: number, order: string[])}
  <!-- index: position in the list if it can be rearranged, -1 otherwise. -->
  {@const current = session.editor?.accountId === id && session.editor.name === name}
  {@const arrangeable = session.multipleActive(id)}
  {@const key = `${id}:${name}`}
  <div class="group flex items-center rounded-field {current ? 'bg-base-100 shadow-[inset_3px_0_0_var(--color-primary)]' : 'hover:bg-base-100/60'}">
    {#if index >= 0 && order.length > 1}
      <button
        class="btn btn-ghost btn-xs btn-square ml-0.5 cursor-grab text-base-content/50"
        draggable="true"
        data-script-handle="{id}:{index}"
        aria-label="{t('scripts.drag')}: {name} (Alt+↑/↓)"
        title="{t('scripts.drag')} (Alt+↑/↓)"
        ondragstart={(e) => {
          drag = { account: id, from: index };
          e.dataTransfer?.setData("text/plain", name);
        }}
        ondragend={() => (drag = dropAt = null)}
        onkeydown={(e) => onhandlekey(e, id, order, index)}
      >
        ⠿
      </button>
    {:else if arrangeable}
      <!-- Lines the name up with those that have a handle. -->
      <span class="ml-0.5 w-6 shrink-0" aria-hidden="true"></span>
    {/if}
    <button
      class="grow truncate px-2 py-1 text-left text-sm {current ? 'font-semibold' : ''}"
      aria-current={current ? "page" : undefined}
      onclick={() => open(id, name)}
    >
      {name}
    </button>
    <button
      class={[
        "badge badge-sm mr-1 cursor-pointer whitespace-nowrap",
        active ? "badge-primary hover:brightness-110" : "badge-ghost opacity-60 group-hover:opacity-100 focus-visible:opacity-100",
      ]}
      aria-pressed={active}
      aria-label="{name}: {t('scripts.active')}"
      title={active ? t("scripts.deactivate") : t("scripts.activate")}
      disabled={toggling.has(key)}
      onclick={() => toggle(id, name, active)}
    >
      {#if toggling.has(key)}<span class="loading loading-spinner loading-xs"></span>{/if}
      {active ? t("scripts.active") : t("scripts.off")}
    </button>
    <div class="dropdown dropdown-end">
      <div tabindex="0" role="button" class="btn btn-ghost btn-xs btn-square opacity-60 group-hover:opacity-100 focus:opacity-100" aria-label="{name}: …">⋯</div>
      <ul tabindex="-1" {@attach closesMenu} class="dropdown-content menu z-10 w-48 rounded-box border border-base-300 bg-base-100 p-1">
        <li><button onclick={() => rename(id, name)}>{t("scripts.rename")}</button></li>
        <li><button class="text-error" onclick={() => remove(id, name, active)}>{t("scripts.delete")}</button></li>
      </ul>
    </div>
  </div>
{/snippet}

<nav class="flex h-full flex-col gap-4 overflow-y-auto p-3" aria-label={t("accounts.heading")}>
  {#each session.accounts as account (account.id)}
    {@const server = session.servers[account.id]}
    <section class="grid gap-1">
      <div class="flex items-center gap-1">
        <h2 class="grow truncate font-semibold" title="{account.username}@{account.host}">{account.label}</h2>
        <div class="dropdown dropdown-end">
          <div tabindex="0" role="button" class="btn btn-ghost btn-xs btn-square" aria-label="{t('accounts.edit')}: {account.label}">⋯</div>
          <ul tabindex="-1" {@attach closesMenu} class="dropdown-content menu z-10 w-48 rounded-box border border-base-300 bg-base-100 p-1">
            <li><button onclick={() => onaccount(account)}>{t("accounts.edit")}</button></li>
            {#if session.roots[account.id]}
              <li><button onclick={() => session.viewRoot(account.id)}>{t("root.view")}</button></li>
            {/if}
            <li><button class="text-error" onclick={() => removeAccount(account)}>{t("accounts.delete")}</button></li>
          </ul>
        </div>
      </div>

      {#if !server || server.status === "connecting"}
        <p class="flex items-center gap-2 text-sm text-base-content/60">
          <span class="loading loading-spinner loading-xs"></span>{t("accounts.connecting")}
        </p>
      {:else if server.status === "error"}
        <div role="alert" class="grid gap-1 text-sm">
          <p class="text-error">{server.error}</p>
          <button class="btn btn-xs self-start" onclick={() => session.connect(account.id)}>{t("accounts.retry")}</button>
        </div>
      {:else if server.info}
        {@const displaced = session.displaced[account.id]}
        {#if displaced}
          <div role="alert" class="grid gap-1.5 rounded-box border border-warning/40 bg-warning/10 px-2 py-1.5 text-xs">
            <p>
              {displaced.active ? t("root.displaced", { name: displaced.active }) : t("root.displacedNone")}
            </p>
            <button class="btn btn-xs justify-self-start" onclick={() => session.reactivateRoot(account.id)}>
              {t("root.reactivate", { name: ROOT_SCRIPT })}
            </button>
          </div>
        {/if}
        <!-- The root script follows the account settings; it isn't listed. -->
        {@const order = session.scriptOrder(account.id)}
        {@const arrangeable = session.multipleActive(account.id)}
        <ul class="grid gap-0.5">
          {#each order as name, index (name)}
            {@const active = session.isActive(account.id, name)}
            {@const dragging = drag?.account === account.id}
            <li
              animate:flip={{ duration: 150 }}
              class="relative {dragging && drag?.from === index ? 'opacity-50' : ''}"
              ondragover={(e) => {
                if (!dragging) return;
                e.preventDefault();
                const rect = e.currentTarget.getBoundingClientRect();
                dropAt = e.clientY < rect.top + rect.height / 2 ? index : index + 1;
              }}
              ondrop={(e) => {
                e.preventDefault();
                if (drag?.account === account.id) {
                  const at = dropAt ?? index;
                  moveTo(account.id, order, drag.from, drag.from < at ? at - 1 : at);
                }
                drag = dropAt = null;
              }}
            >
              {#if dragging && dropAt === index}
                <div class="absolute -top-0.5 left-0 right-0 h-0.5 rounded bg-primary"></div>
              {/if}
              <!-- Only the run order of several scripts can be arranged. -->
              {@render row(account.id, name, active, arrangeable ? index : -1, order)}
              {#if dragging && dropAt === order.length && index === order.length - 1}
                <div class="absolute -bottom-0.5 left-0 right-0 h-0.5 rounded bg-primary"></div>
              {/if}
            </li>
          {:else}
            <li class="px-2 text-sm text-base-content/60">{t("scripts.none")}</li>
          {/each}
        </ul>
        <button class="btn btn-ghost btn-xs justify-self-start w-fit" onclick={() => create(account.id)}>+ {t("scripts.new")}</button>
        {@const folders = session.folders[account.id]}
        {#if folders?.status === "error"}
          <div role="alert" class="grid gap-1 text-xs">
            <p class="text-base-content/70">{t("folders.error", { message: folders.error ?? "" })}</p>
            <button class="btn btn-xs self-start" onclick={() => session.loadFolders(account.id)}>{t("accounts.retry")}</button>
          </div>
        {/if}
      {/if}
    </section>
  {:else}
    <p class="text-sm text-base-content/70">{t("accounts.empty")}</p>
  {/each}

  <button class="btn btn-sm mt-auto" onclick={() => onaccount(null)}>+ {t("accounts.add")}</button>
</nav>
