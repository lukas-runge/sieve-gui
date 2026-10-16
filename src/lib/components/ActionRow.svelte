<script lang="ts">
  import type { Action } from "#tauri/types";
  import { t, type MessageKey } from "../i18n.svelte";
  import { ACTION_KINDS, actionExtension, actionKind, buildAction, type ActionKind } from "../rules/view";
  import { folderExists, mailboxLabel, mailboxRaw, type ScriptFolders } from "../rules/mailbox";

  let {
    action,
    extensions,
    folders,
    onchange,
    onremove,
  }: {
    action: Action;
    extensions: string[];
    folders: ScriptFolders;
    onchange: (a: Action, merge: boolean) => void;
    onremove: () => void;
  } = $props();

  const kind = $derived(actionKind(action));
  // Offer what the server supports (and always the current kind).
  const kinds = $derived(
    ACTION_KINDS.filter((k) => k === kind || !actionExtension(k) || extensions.includes(actionExtension(k)!)),
  );
  const folderListId = `folders-${Math.random().toString(36).slice(2)}`;

  function flagsText(flags: string[]) {
    return flags.join(" ");
  }
  function parseFlags(text: string) {
    return text.split(/\s+/).filter(Boolean);
  }
</script>

<div class="flex flex-wrap items-start gap-2 rounded-box bg-base-200/60 p-2">
  <select
    class="select select-sm w-auto"
    aria-label={t("actions.heading")}
    value={kind}
    onchange={(e) => onchange(buildAction(e.currentTarget.value as ActionKind, action), false)}
  >
    {#each kinds as k}
      <option value={k}>{t(`action.${k}` as MessageKey)}</option>
    {/each}
  </select>

  {#if action.type === "fileInto"}
    {@const label = mailboxLabel(action.mailbox)}
    {@const missing = folders.existing !== null && label !== "" && !folderExists(folders.existing, label)}
    <div class="grow min-w-48">
      <input
        class={["input input-sm w-full font-mono", missing && "input-warning"]}
        aria-label={t("action.folder")}
        aria-describedby={missing ? `${folderListId}-missing` : undefined}
        placeholder={t("action.folder")}
        list={folderListId}
        value={label}
        oninput={(e) => onchange({ ...action, mailbox: mailboxRaw(e.currentTarget.value, folders.encoding) }, true)}
      />
      <datalist id={folderListId}>
        {#each folders.labels as folder}<option value={folder}></option>{/each}
      </datalist>
      {#if missing}
        <div id="{folderListId}-missing" class="mt-1 grid gap-1 text-xs">
          <p class="justify-self-start rounded-field border border-warning/40 bg-warning/10 px-2 py-0.5">{t("action.folderMissing")}</p>
          {#if extensions.includes("mailbox")}
            <label class="flex items-center gap-2">
              <input
                type="checkbox"
                class="checkbox checkbox-xs"
                checked={action.create}
                onchange={(e) => onchange({ ...action, create: e.currentTarget.checked }, false)}
              />
              {t("action.folderCreate")}
            </label>
          {/if}
        </div>
      {/if}
    </div>
  {:else if action.type === "redirect"}
    <input
      type="email"
      class="input input-sm grow min-w-48 font-mono"
      aria-label={t("action.address")}
      placeholder="name@example.com"
      value={action.address}
      oninput={(e) => onchange({ ...action, address: e.currentTarget.value }, true)}
    />
  {:else if action.type === "reject"}
    <textarea
      class="textarea textarea-sm grow min-w-48"
      aria-label={t("action.message")}
      rows="2"
      value={action.reason}
      oninput={(e) => onchange({ ...action, reason: e.currentTarget.value }, true)}
    ></textarea>
  {:else if action.type === "flags" && kind !== "markRead" && kind !== "flag"}
    <div class="grow min-w-48">
      <input
        class="input input-sm w-full font-mono"
        aria-label={t("action.flags")}
        placeholder="\Seen \Flagged $Label1"
        value={flagsText(action.flags)}
        oninput={(e) => onchange({ ...action, flags: parseFlags(e.currentTarget.value) }, true)}
      />
      <p class="mt-1 text-xs text-base-content/60">{t("action.flagsHint")}</p>
    </div>
  {:else if action.type === "vacation"}
    <div class="grid grow min-w-64 gap-2">
      <label class="floating-label">
        <span>{t("action.vacationSubject")}</span>
        <input
          class="input input-sm w-full"
          placeholder={t("action.vacationSubject")}
          value={action.subject ?? ""}
          oninput={(e) => onchange({ ...action, subject: e.currentTarget.value || null }, true)}
        />
      </label>
      <textarea
        class="textarea textarea-sm w-full"
        aria-label={t("action.message")}
        placeholder={t("action.message")}
        rows="4"
        value={action.reason}
        oninput={(e) => onchange({ ...action, reason: e.currentTarget.value }, true)}
      ></textarea>
      <label class="flex items-center gap-2 text-sm">
        <span>{t("action.vacationDays")}</span>
        <input
          type="number"
          min="1"
          class="input input-sm w-20"
          value={action.period?.value ?? 7}
          oninput={(e) =>
            onchange({ ...action, period: { unit: action.period?.unit ?? "days", value: Number(e.currentTarget.value) || 1 } }, true)}
        />
      </label>
      <label class="floating-label">
        <span>{t("action.vacationAddresses")}</span>
        <input
          class="input input-sm w-full font-mono"
          placeholder={t("action.vacationAddresses")}
          value={action.addresses.join(", ")}
          oninput={(e) =>
            onchange({ ...action, addresses: e.currentTarget.value.split(/[\s,]+/).filter(Boolean) }, true)}
        />
      </label>
    </div>
  {/if}

  <button
    class="btn btn-ghost btn-sm btn-square ml-auto"
    aria-label={t("actions.remove")}
    title={t("actions.remove")}
    onclick={onremove}>✕</button
  >
</div>
