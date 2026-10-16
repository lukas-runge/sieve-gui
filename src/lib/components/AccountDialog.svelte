<script lang="ts">
  import { untrack } from "svelte";
  import type { Account, AccountSecurity } from "#tauri/types";
  import { describeError } from "../api";
  import { t, type MessageKey } from "../i18n.svelte";
  import { session } from "../session.svelte";

  let { account, onclose }: { account: Account | null; onclose: () => void } = $props();

  let dialog: HTMLDialogElement;
  // Seed the form once from the account being edited.
  const initial = untrack(() => account);
  let label = $state(initial?.label ?? "");
  let host = $state(initial?.host ?? "");
  let security = $state<AccountSecurity>(initial?.security ?? "startTls");
  let port = $state(initial?.port ?? 4190);
  let username = $state(initial?.username ?? "");
  let password = $state("");
  // IMAP for the folder list: on by default for new accounts.
  let imapEnabled = $state(initial ? initial.imap !== null : true);
  let imapHost = $state(initial?.imap?.host ?? "");
  let imapSecurity = $state<AccountSecurity>(initial?.imap?.security ?? "implicitTls");
  let imapPort = $state(initial?.imap?.port ?? 993);
  let saving = $state(false);
  let error = $state<string | null>(null);

  $effect(() => dialog.showModal());

  async function submit(e: SubmitEvent) {
    e.preventDefault();
    saving = true;
    error = null;
    try {
      await session.saveAccount({
        id: initial?.id ?? null,
        label: label || host,
        host,
        port,
        security,
        username,
        imap: imapEnabled ? { host: imapHost.trim() || host, port: imapPort, security: imapSecurity } : null,
        password: password || (initial ? null : ""),
      });
      dialog.close();
    } catch (err) {
      error = describeError(err);
    } finally {
      saving = false;
    }
  }

  const securities: AccountSecurity[] = ["startTls", "implicitTls", "insecure"];

  /** Follows the standard IMAP port when the security changes, unless customized. */
  function setImapSecurity(value: AccountSecurity) {
    if (imapPort === 993 || imapPort === 143) imapPort = value === "implicitTls" ? 993 : 143;
    imapSecurity = value;
  }
</script>

<dialog bind:this={dialog} class="modal" {onclose} aria-labelledby="account-heading">
  <form class="modal-box grid max-w-md gap-3" onsubmit={submit}>
    <h2 id="account-heading" class="text-lg font-semibold">
      {initial ? t("accounts.edit") : t("accounts.add")}
    </h2>

    <fieldset class="fieldset">
      <label class="fieldset-legend" for="acc-label">{t("account.label")}</label>
      <input id="acc-label" class="input w-full" bind:value={label} placeholder={host || "Work"} />
      <p class="label whitespace-normal">{t("account.labelHint")}</p>
    </fieldset>

    <div class="grid grid-cols-[1fr_6rem] gap-2">
      <fieldset class="fieldset">
        <label class="fieldset-legend" for="acc-host">{t("account.host")}</label>
        <input id="acc-host" class="input w-full" bind:value={host} required placeholder="mail.example.com" autocapitalize="off" spellcheck="false" />
      </fieldset>
      <fieldset class="fieldset">
        <label class="fieldset-legend" for="acc-port">{t("account.port")}</label>
        <input id="acc-port" type="number" min="1" max="65535" class="input w-full" bind:value={port} required />
      </fieldset>
    </div>

    <fieldset class="fieldset">
      <label class="fieldset-legend" for="acc-security">{t("account.security")}</label>
      <select id="acc-security" class="select w-full" bind:value={security}>
        {#each securities as s}
          <option value={s}>{t(`account.security.${s}` as MessageKey)}</option>
        {/each}
      </select>
    </fieldset>

    <fieldset class="fieldset">
      <label class="fieldset-legend" for="acc-user">{t("account.username")}</label>
      <input id="acc-user" class="input w-full" bind:value={username} required autocomplete="username" autocapitalize="off" spellcheck="false" />
    </fieldset>

    <fieldset class="fieldset">
      <label class="fieldset-legend" for="acc-pass">{t("account.password")}</label>
      <input id="acc-pass" type="password" class="input w-full" bind:value={password} required={!initial} autocomplete="current-password" />
      <p class="label whitespace-normal">{initial ? t("account.passwordKeep") : t("account.passwordStored")}</p>
    </fieldset>

    <fieldset class="fieldset">
      <label class="label text-base-content">
        <input type="checkbox" class="toggle toggle-sm" bind:checked={imapEnabled} />
        {t("account.imap")}
      </label>
      <p class="label whitespace-normal">{t("account.imapHint")}</p>
    </fieldset>

    {#if imapEnabled}
      <div class="grid grid-cols-[1fr_6rem] gap-2">
        <fieldset class="fieldset">
          <label class="fieldset-legend" for="acc-imap-host">{t("account.imapHost")}</label>
          <input id="acc-imap-host" class="input w-full" bind:value={imapHost} placeholder={host || "imap.example.com"} autocapitalize="off" spellcheck="false" />
        </fieldset>
        <fieldset class="fieldset">
          <label class="fieldset-legend" for="acc-imap-port">{t("account.port")}</label>
          <input id="acc-imap-port" type="number" min="1" max="65535" class="input w-full" bind:value={imapPort} required />
        </fieldset>
      </div>
      <fieldset class="fieldset">
        <label class="fieldset-legend" for="acc-imap-security">{t("account.security")}</label>
        <select id="acc-imap-security" class="select w-full" value={imapSecurity} onchange={(e) => setImapSecurity(e.currentTarget.value as AccountSecurity)}>
          {#each securities as s}
            <option value={s}>{t(`account.imapSecurity.${s}` as MessageKey)}</option>
          {/each}
        </select>
      </fieldset>
    {/if}

    {#if error}<p role="alert" class="text-error">{error}</p>{/if}

    <div class="modal-action">
      <button type="button" class="btn btn-ghost" onclick={() => dialog.close()}>{t("account.cancel")}</button>
      <button class="btn btn-primary" disabled={saving}>
        {#if saving}<span class="loading loading-spinner loading-sm"></span>{/if}
        {t("account.save")}
      </button>
    </div>
  </form>
</dialog>
