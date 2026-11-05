<script lang="ts">
  // The account's tags (custom keywords with a name and a colour), and
  // taking them over from a tag provider (Synology MailPlus).
  import type { MailPlusSettings, UntrustedCertificate } from "#tauri/types";
  import { providerTags } from "#tauri/commands";
  import { describeError, isAppError, untrustedCertificate } from "../api";
  import { askText } from "../dialogs.svelte";
  import { t } from "../i18n.svelte";
  import { labelHex, labelStyle, mergeTags, type Tag } from "../keywords";
  import Icon from "./Icon.svelte";

  const DSM_PORT = 5001;

  let {
    accountId,
    provider,
    host,
    tags = $bindable(),
    mailplus = $bindable(),
    oncertificate,
  }: {
    /** `null` for an account not saved yet. */
    accountId: string | null;
    /** The tag provider the account runs on (see `session.tagProvider`); `null` hides taking tags over. */
    provider: "mailplus" | null;
    /** The account's server, where DSM runs too. */
    host: string;
    tags: Tag[];
    mailplus: MailPlusSettings | null;
    /** DSM's certificate isn't trusted: show it; `retry` once it is. */
    oncertificate: (untrusted: UntrustedCertificate, address: string, retry: () => void) => void;
  } = $props();

  let port = $state(mailplus?.port ?? DSM_PORT);
  const address = $derived(`${host}:${port}`);
  let loading = $state(false);
  let error = $state<string | null>(null);
  let result = $state<{ added: number; changed: number; unmatched: string[] } | null>(null);

  function setColor(i: number, color: string | null) {
    tags[i] = { ...tags[i], color };
  }

  async function takeOver(otp: string | null = null): Promise<void> {
    if (!accountId || !provider) return;
    loading = true;
    error = null;
    result = null;
    try {
      const provided = await providerTags(accountId, provider, port, otp);
      const merged = mergeTags(tags, provided);
      tags = merged.tags;
      result = merged;
      mailplus = { port, certificate: mailplus?.certificate ?? null };
    } catch (e) {
      const certificate = untrustedCertificate(e);
      if (certificate?.service === "mailPlus") {
        oncertificate(certificate, address, () => void takeOver(otp));
      } else if (isAppError(e) && (e.kind === "secondFactorRequired" || e.kind === "secondFactorFailed")) {
        const prompt = t(e.kind === "secondFactorFailed" ? "tags.otpRetry" : "tags.otpPrompt");
        const code = await askText(prompt, t("tags.otpConfirm"));
        if (code) {
          loading = false;
          return takeOver(code);
        }
      } else if (isAppError(e) && e.kind === "notFound") {
        error = t("tags.notMailPlus", { address });
      } else {
        error = describeError(e);
      }
    } finally {
      loading = false;
    }
  }
</script>

<fieldset class="fieldset gap-2">
  <legend class="fieldset-legend">{t("tags.heading")}</legend>
  <p class="label -mt-1 whitespace-normal">{t("tags.hint")}</p>

  {#if tags.length}
    <ul class="grid gap-1.5" aria-label={t("tags.heading")}>
      {#each tags as tag, i (i)}
        {@const label = tag.name || tag.keyword || t("tags.new")}
        {@const style = labelStyle(tag.name || tag.keyword, tag.color)}
        <li class="flex items-center gap-1.5">
          <!-- The colour dot opens the system's colour picker. -->
          <label
            class="relative grid size-7 shrink-0 cursor-pointer place-items-center rounded-full border hover:brightness-95"
            style={style.chip}
            title={t("tags.color", { name: label })}
          >
            <span class="size-3 rounded-full" style={style.dot}></span>
            <input
              type="color"
              class="absolute inset-0 size-full cursor-pointer opacity-0"
              aria-label={t("tags.color", { name: label })}
              value={tag.color ?? labelHex(tag.name || tag.keyword)}
              oninput={(e) => setColor(i, e.currentTarget.value.toLowerCase())}
            />
          </label>
          <input
            class="input input-sm min-w-0 grow"
            aria-label={t("tags.name", { keyword: tag.keyword || "…" })}
            placeholder={tag.keyword || t("tags.namePlaceholder")}
            bind:value={tags[i].name}
          />
          <input
            class="input input-sm w-44 shrink-0 font-mono text-[0.8rem]"
            aria-label={t("tags.keyword", { name: label })}
            placeholder="$label1"
            bind:value={tags[i].keyword}
            autocapitalize="off"
            spellcheck="false"
          />
          <div class="flex w-14 shrink-0 justify-end">
            {#if tag.color}
              <button
                type="button"
                class="btn btn-ghost btn-xs btn-square"
                title={t("tags.colorAuto")}
                aria-label="{t('tags.colorAuto')}: {label}"
                onclick={() => setColor(i, null)}><Icon name="undo" size={13} /></button
              >
            {/if}
            <button
              type="button"
              class="btn btn-ghost btn-xs btn-square"
              aria-label={t("tags.remove", { name: label })}
              title={t("tags.remove", { name: label })}
              onclick={() => (tags = tags.filter((_, j) => j !== i))}><Icon name="x" size={13} /></button
            >
          </div>
        </li>
      {/each}
    </ul>
  {:else}
    <p class="text-sm text-base-content/70">{t("tags.empty")}</p>
  {/if}
  <button type="button" class="btn btn-ghost btn-sm justify-self-start gap-1.5" onclick={() => (tags = [...tags, { keyword: "", name: "", color: null }])}>
    <Icon name="plus" size={14} />{t("tags.add")}
  </button>

  <!-- Tag provider: MailPlus' labels through DSM, only for accounts on MailPlus. -->
  {#if provider === "mailplus"}
    <div class="grid gap-2 rounded-box border border-base-300 p-3">
      <div class="flex flex-wrap items-center gap-2">
        <button type="button" class="btn btn-sm gap-1.5" disabled={loading} onclick={() => takeOver()}>
          {#if loading}<span class="loading loading-spinner loading-xs"></span>{:else}<Icon name="tag" size={14} />{/if}
          {t("tags.fromProvider", { provider: t("provider.mailplus") })}
        </button>
        <label class="flex items-center gap-1.5 text-sm text-base-content/80">
          {t("tags.port")}
          <input type="number" min="1" max="65535" class="input input-sm w-20" bind:value={port} />
        </label>
      </div>
      <p class="label whitespace-normal">
        {t("tags.mailplusHint", { host })}
      </p>
      {#if result}
        <p role="status" class="text-sm">
          {result.added || result.changed ? t("tags.merged", { added: result.added, changed: result.changed }) : t("tags.mergedNone")}
          {#if result.unmatched.length}<br /><span class="text-base-content/70">{t("tags.unmatched", { names: result.unmatched.join(", ") })}</span>{/if}
        </p>
      {/if}
      {#if error}<p role="alert" class="text-sm text-error">{error}</p>{/if}
    </div>
  {/if}
</fieldset>
