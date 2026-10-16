<script lang="ts">
  import type { Account } from "#tauri/types";
  import { t } from "../i18n.svelte";
  import { ask, askText, tell } from "../dialogs.svelte";
  import { closesMenu } from "../menu";
  import { session } from "../session.svelte";

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

  async function removeAccount(account: Account) {
    if (await ask(t("accounts.deleteConfirm", { label: account.label }), t("accounts.delete"), true)) {
      await session.deleteAccount(account.id);
    }
  }
</script>

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
        <ul class="grid gap-0.5">
          {#each server.info.scripts as script (script.name)}
            {@const current = session.editor?.accountId === account.id && session.editor.name === script.name}
            <li class="group flex items-center rounded-field {current ? 'bg-base-100 shadow-[inset_3px_0_0_var(--color-primary)]' : 'hover:bg-base-100/60'}">
              <button
                class="grow truncate px-2 py-1 text-left text-sm {current ? 'font-semibold' : ''}"
                aria-current={current ? "page" : undefined}
                onclick={() => open(account.id, script.name)}
              >
                {script.name}
              </button>
              {#if script.active}
                <span class="badge badge-sm badge-primary mr-1">{t("scripts.active")}</span>
              {/if}
              <div class="dropdown dropdown-end">
                <div tabindex="0" role="button" class="btn btn-ghost btn-xs btn-square opacity-60 group-hover:opacity-100 focus:opacity-100" aria-label="{script.name}: …">⋯</div>
                <ul tabindex="-1" {@attach closesMenu} class="dropdown-content menu z-10 w-48 rounded-box border border-base-300 bg-base-100 p-1">
                  {#if script.active}
                    <li><button onclick={() => session.activate(account.id, null)}>{t("scripts.deactivate")}</button></li>
                  {:else}
                    <li><button onclick={() => session.activate(account.id, script.name)}>{t("scripts.activate")}</button></li>
                  {/if}
                  <li><button onclick={() => rename(account.id, script.name)}>{t("scripts.rename")}</button></li>
                  <li><button class="text-error" onclick={() => remove(account.id, script.name, script.active)}>{t("scripts.delete")}</button></li>
                </ul>
              </div>
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
