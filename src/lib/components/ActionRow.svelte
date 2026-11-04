<script lang="ts">
  import type { Action } from "#tauri/types";
  import { t, type MessageKey } from "../i18n.svelte";
  import { ACTION_KINDS, actionExtension, actionKind, buildAction, type ActionKind } from "../rules/view";
  import { folderExists, mailboxLabel, mailboxRaw, type ScriptFolders } from "../rules/mailbox";
  import BranchesEditor from "./BranchesEditor.svelte";
  import { askText } from "../dialogs.svelte";
  import { isKeyword } from "../keywords";
  import { session } from "../session.svelte";
  import Icon from "./Icon.svelte";
  import { flagFromText, flagName, labelHue, SYSTEM_FLAG_LIST } from "../rules/route";

  let {
    action,
    extensions,
    folders,
    singleFolder,
    depth = 1,
    path = "",
    focus = "",
    onchange,
    onremove,
  }: {
    action: Action;
    extensions: string[];
    folders: ScriptFolders;
    /** Single-folder mode: "stop" ends only this script, and `:create`
     * would turn a move into a direct `fileinto`, so it isn't offered. */
    singleFolder: boolean;
    /** Nesting level a check in this action would have. */
    depth?: number;
    /** Case path prefix a check in this action has (see `CasePath`). */
    path?: string;
    /** The case the inspector shows first. */
    focus?: string;
    onchange: (a: Action, merge: boolean) => void;
    onremove: () => void;
  } = $props();

  const kind = $derived(actionKind(action));
  // Offer what the server supports (and always the current kind).
  const kinds = $derived(
    ACTION_KINDS.filter((k) => k === kind || !actionExtension(k) || extensions.includes(actionExtension(k)!)),
  );
  const folderListId = `folders-${Math.random().toString(36).slice(2)}`;

  const accountId = $derived(session.editor?.accountId ?? null);
  const keywordNames = $derived(accountId ? session.keywordNames(accountId) : {});

  async function nameKeyword(keyword: string) {
    if (!accountId) return;
    const name = await askText(t("keywords.namePrompt", { keyword }), t("keywords.save"), keywordNames[keyword] ?? "");
    if (name !== null) await session.setKeywordName(accountId, keyword, name);
  }

</script>

<!-- The action as one line where it fits, the remove button at its end. -->
<div class="grid grid-cols-[minmax(0,1fr)_auto] items-start gap-x-1 rounded-field bg-base-200/70 p-2">
  <div class="flex min-w-0 flex-wrap items-start gap-2">
  <select
    class="select select-sm w-auto"
    aria-label={t("actions.heading")}
    value={kind}
    onchange={(e) => onchange(buildAction(e.currentTarget.value as ActionKind, action), false)}
  >
    {#each kinds as k}
      <option value={k}>{t(k === "stop" && singleFolder ? "action.stopScript" : (`action.${k}` as MessageKey))}</option>
    {/each}
  </select>

  {#if action.type === "fileInto"}
    {@const label = mailboxLabel(action.mailbox)}
    {@const missing = folders.existing !== null && label !== "" && !folderExists(folders.existing, label)}
    <div class="grow min-w-48">
      <label class={["input input-sm w-full", missing && "input-warning"]}>
      <Icon name="folder" size={15} class="text-primary" />
      <input
        class="grow"
        aria-label={t("action.folder")}
        aria-describedby={missing ? `${folderListId}-missing` : undefined}
        placeholder={t("action.folder")}
        list={folderListId}
        value={label}
        oninput={(e) => onchange({ ...action, mailbox: mailboxRaw(e.currentTarget.value, folders.encoding) }, true)}
      />
      </label>
      <datalist id={folderListId}>
        {#each folders.labels as folder}<option value={folder}></option>{/each}
      </datalist>
      {#if missing}
        <div id="{folderListId}-missing" class="mt-1 grid gap-1 text-[0.8rem]">
          <p class="justify-self-start rounded-field border border-warning/40 bg-warning/10 px-2 py-0.5">{t("action.folderMissing")}</p>
          {#if extensions.includes("mailbox") && (!singleFolder || action.copy || action.create)}
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
      class="input input-sm grow min-w-48"
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
    {@const flags = action.flags}
    <div class="grid grow min-w-48 gap-1.5">
      <!-- One field: the labels as chips, typing adds another. -->
      <label class="input input-sm flex h-auto min-h-8 w-full flex-wrap items-center gap-1 py-1">
        {#each flags as flag, i}
          {@const unnamed = isKeyword(flag) && !keywordNames[flag]}
          <span
            class="inline-flex items-center gap-0.5 rounded-full border py-px pr-0.5 pl-2 text-sm font-medium"
            style="border-color: oklch(0.7 0.09 {labelHue(flagName(flag, keywordNames))} / 0.55); background: oklch(0.7 0.09 {labelHue(flagName(flag, keywordNames))} / 0.12)"
          >
            {#if isKeyword(flag) && accountId}
              <button
                type="button"
                class={["hover:underline", unnamed && "font-mono text-[0.8rem] text-base-content/70"]}
                title={t("keywords.nameTitle", { keyword: flag })}
                onclick={() => nameKeyword(flag)}>{flagName(flag, keywordNames)}</button
              >
            {:else}
              <span>{flagName(flag, keywordNames)}</span>
            {/if}
            <button
              type="button"
              class="grid size-5 place-items-center rounded-full text-base-content/65 hover:bg-base-200 hover:text-base-content"
              aria-label={t("action.flagRemove", { name: flagName(flag, keywordNames) })}
              onclick={() => onchange({ ...action, flags: flags.filter((_, j) => j !== i) }, false)}><Icon name="x" size={12} /></button
            >
          </span>
        {/each}
        <input
          class="min-w-32 grow bg-transparent outline-none"
          aria-label={t("action.flags")}
          placeholder={t("action.flagAdd")}
          list="{folderListId}-flags"
          onkeydown={(e) => {
            if (e.key !== "Enter" && e.key !== ",") return;
            e.preventDefault();
            const flag = flagFromText(e.currentTarget.value, keywordNames);
            if (flag && !flags.includes(flag)) onchange({ ...action, flags: [...flags, flag] }, false);
            e.currentTarget.value = "";
          }}
          onchange={(e) => {
            const flag = flagFromText(e.currentTarget.value, keywordNames);
            if (flag && !flags.includes(flag)) onchange({ ...action, flags: [...flags, flag] }, false);
            e.currentTarget.value = "";
          }}
        />
      </label>
      <datalist id="{folderListId}-flags">
        {#each SYSTEM_FLAG_LIST as f}<option value={flagName(f, {})}></option>{/each}
        {#each Object.values(keywordNames) as name}<option value={name}></option>{/each}
      </datalist>
      {#if flags.some((f) => isKeyword(f) && !keywordNames[f])}
        <p class="text-[0.8rem] text-base-content/65">{t("action.flagNameHint")}</p>
      {/if}
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

  </div>
  <button
    class="btn btn-ghost btn-sm btn-square text-base-content/65"
    aria-label={t("actions.remove")}
    title={t("actions.remove")}
    onclick={onremove}><Icon name="trash" size={15} /></button
  >

  {#if action.type === "if"}
    {@const nested = action}
    <!-- "Also check": an if with its own branches, nested to any depth. -->
    <div class="basis-full border-l-2 pl-3" style="border-color: var(--rail-{Math.min(depth, 3)})">
      <BranchesEditor
        block={nested}
        {extensions}
        {folders}
        {singleFolder}
        {depth}
        {path}
        {focus}
        mutate={(fn, merge) => {
          const copy = $state.snapshot(nested);
          fn(copy);
          onchange(copy, merge ?? false);
        }}
      />
    </div>
  {/if}
</div>
