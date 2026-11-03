<script lang="ts">
  // Shows a server certificate the OS doesn't trust and lets the user trust
  // it (pin its fingerprint), see managesieve::tls::PinningVerifier.
  import type { UntrustedCertificate } from "#tauri/types";
  import { outsideValidity } from "../certificate";
  import { i18n, t } from "../i18n.svelte";
  import Fingerprint from "./Fingerprint.svelte";

  let {
    untrusted,
    host,
    ontrust,
    onclose,
  }: { untrusted: UntrustedCertificate; host: string; ontrust: () => void; onclose: () => void } = $props();

  let dialog: HTMLDialogElement;
  $effect(() => dialog.showModal());

  const cert = $derived(untrusted.certificate);
  const changed = $derived(untrusted.pinned !== null);
  const service = $derived(t(untrusted.service === "manageSieve" ? "certificate.manageSieve" : "certificate.imap"));
  const invalidPeriod = $derived(outsideValidity(cert.notBefore, cert.notAfter));

  function date(seconds: number | null): string {
    if (seconds === null) return "–";
    return new Intl.DateTimeFormat(i18n.locale, { dateStyle: "medium" }).format(new Date(seconds * 1000));
  }

  function trust(e: SubmitEvent) {
    e.preventDefault();
    ontrust();
    dialog.close();
  }
</script>

<dialog bind:this={dialog} class="modal" {onclose} aria-labelledby="certificate-heading">
  <form class="modal-box grid max-w-lg gap-4" onsubmit={trust}>
    <div class="grid gap-1">
      <h2 id="certificate-heading" class="text-lg font-semibold">
        {changed ? t("certificate.changedTitle") : t("certificate.title")}
      </h2>
      <p class="text-sm text-base-content/70">{t("certificate.server", { host, service })}</p>
    </div>

    {#if changed}
      <p role="alert" class="rounded-box border border-warning/50 bg-warning/15 px-3 py-2 text-sm">
        {t("certificate.changedText")}
      </p>
    {:else}
      <p class="text-sm">{t("certificate.text")}</p>
    {/if}

    <dl class="grid grid-cols-[auto_1fr] gap-x-4 gap-y-1.5 text-sm">
      {#if cert.subject}
        <dt class="text-base-content/70">{t("certificate.subject")}</dt>
        <dd class="break-words">{cert.subject}</dd>
      {/if}
      {#if cert.issuer}
        <dt class="text-base-content/70">{t("certificate.issuer")}</dt>
        <dd class="break-words">{cert.subject === cert.issuer ? t("certificate.selfSigned") : cert.issuer}</dd>
      {/if}
      {#if cert.names.length}
        <dt class="text-base-content/70">{t("certificate.names")}</dt>
        <dd class="break-words">{cert.names.join(", ")}</dd>
      {/if}
      {#if cert.notBefore !== null || cert.notAfter !== null}
        <dt class="text-base-content/70">{t("certificate.validity")}</dt>
        <dd>
          {t("certificate.period", { from: date(cert.notBefore), to: date(cert.notAfter) })}
          {#if invalidPeriod}<span class="badge badge-warning badge-sm ml-1">{t("certificate.notValidNow")}</span>{/if}
        </dd>
      {/if}
      <dt class="text-base-content/70">{t("certificate.fingerprint")}</dt>
      <dd><Fingerprint sha256={cert.sha256} /></dd>
      {#if untrusted.pinned}
        <dt class="text-base-content/70">{t("certificate.previous")}</dt>
        <dd><Fingerprint sha256={untrusted.pinned} class="text-base-content/70" /></dd>
      {/if}
    </dl>

    <p class="text-xs text-base-content/70">{t("certificate.verifyHint")}</p>

    <div class="modal-action mt-0">
      <button type="button" class="btn btn-ghost" onclick={() => dialog.close()}>{t("account.cancel")}</button>
      <button class={["btn", changed ? "btn-warning" : "btn-primary"]}>
        {changed ? t("certificate.trustNew") : t("certificate.trust")}
      </button>
    </div>
  </form>
</dialog>
