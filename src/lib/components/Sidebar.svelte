<script lang="ts">
  import { flip } from "svelte/animate";
  import { SvelteSet } from "svelte/reactivity";
  import type { Account } from "#tauri/types";
  import type { ScriptNode } from "../session.svelte";
  import { t } from "../i18n.svelte";
  import CertificateDialog from "./CertificateDialog.svelte";
  import CopyDialog from "./CopyDialog.svelte";
  import { ask, askText, tell } from "../dialogs.svelte";
  import { closesMenu } from "../menu";
  import { ROOT_SCRIPT, session } from "../session.svelte";
  import Icon from "./Icon.svelte";

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
    const parents = session.includedBy(id, name);
    const message = parents.length
      ? t("scripts.deleteIncluded", { name, names: quoted(parents) })
      : t("scripts.deleteConfirm", { name });
    if (await ask(message, t("scripts.delete"), true)) await session.deleteScript(id, name);
  }

  const quoted = (names: string[]) => names.map((n) => t("scripts.quoted", { name: n })).join(", ");

  async function takeOver(id: string, name: string, pending: boolean) {
    if (!(await ask(t("scripts.takeOverConfirm", { name }), t("scripts.takeOver")))) return;
    if (pending) await session.adopt(id, "takeOver");
    else await session.takeOver(id, name);
  }

  async function leaveUnchanged(id: string, name: string) {
    if (await ask(t("scripts.leaveUnchangedConfirm", { name }), t("scripts.leaveUnchanged"))) {
      await session.leaveUnchanged(id, name);
    }
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

  /** Moves a running script in the run order. */
  function moveTo(id: string, order: string[], from: number, to: number) {
    if (to < 0 || to >= order.length || from === to) return;
    const next = [...order];
    next.splice(to, 0, ...next.splice(from, 1));
    void session.reorderRunning(id, next);
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

{#snippet row(id: string, node: ScriptNode, active: boolean, running: boolean, index: number, order: string[])}
  <!-- index: position in the run order if it can be rearranged, -1 otherwise.
       running: the script runs, on its own or included by `node.via`. -->
  {@const name = node.name}
  {@const current = session.editor?.accountId === id && session.editor.name === name}
  {@const key = `${id}:${name}`}
  {@const root = session.roots[id]}
  {@const included = !!root?.scripts.includes(name)}
  {@const parents = session.includedBy(id, name)}
  {@const position = order.indexOf(name)}
  <!-- Listed under the script including it; other including scripts are named. -->
  {@const notes = parents.some((p) => p !== node.via)
    ? [t("scripts.includedBy", { names: quoted(parents.filter((p) => p !== node.via)) })]
    : []}
  <div
    class={[
      "group flex items-center gap-1 rounded-field pr-1",
      current ? "bg-base-100 shadow-sm ring-1 ring-base-300" : "hover:bg-base-100/70",
    ]}
    style:margin-left="{node.depth * 1.1}rem"
  >
    <!-- The run order: a number for running scripts, a hollow ring for the others. -->
    {#if index >= 0 && order.length > 1}
      <button
        class="ml-1 grid size-6 shrink-0 cursor-grab place-items-center rounded-full bg-primary/12 text-[0.8rem] font-bold text-primary hover:bg-primary/20"
        draggable="true"
        data-script-handle="{id}:{index}"
        aria-label="{t('scripts.drag')}: {name} (Alt+↑/↓)"
        title="{t('scripts.position', { n: index + 1 })} · {t('scripts.drag')} (Alt+↑/↓)"
        ondragstart={(e) => {
          drag = { account: id, from: index };
          e.dataTransfer?.setData("text/plain", name);
        }}
        ondragend={() => (drag = dropAt = null)}
        onkeydown={(e) => onhandlekey(e, id, order, index)}
      >
        {index + 1}
      </button>
    {:else if node.depth > 0}
      <span class="ml-1 grid size-6 shrink-0 place-items-center text-base-content/35" aria-hidden="true"><Icon name="nested" size={14} /></span>
    {:else if running && position >= 0}
      <span class="ml-1 grid size-6 shrink-0 place-items-center rounded-full bg-primary/12 text-[0.8rem] font-bold text-primary" aria-hidden="true">1</span>
    {:else}
      <span class="ml-1 grid size-6 shrink-0 place-items-center" aria-hidden="true">
        <span class="size-2.5 rounded-full border-2 border-base-content/25"></span>
      </span>
    {/if}
    <!-- contain: long names and notes truncate instead of widening the sidebar. -->
    <button
      class="min-w-0 grow py-1.5 pr-1 pl-1 text-left text-[0.9rem] [contain:inline-size]"
      aria-current={current ? "page" : undefined}
      onclick={() => open(id, name)}
    >
      <span class={["block truncate", current && "font-semibold", !running && "text-base-content/70"]}>{name}</span>
      {#if running && node.via}
        <span class="block truncate text-[0.8rem] text-base-content/65" title={t("scripts.viaHint", { name: node.via })}>{t("scripts.viaShort", { name: node.via })}</span>
      {/if}
      {#if notes.length}
        <span class="block text-[0.8rem] leading-snug break-words text-base-content/65">{notes.join(", ")}</span>
      {/if}
    </button>
    {#if !(running && node.via)}
      <!-- On/off as a switch: what it does is visible without a tooltip. -->
      {#if toggling.has(key)}
        <span class="grid w-7 shrink-0 place-items-center"><span class="loading loading-spinner loading-xs"></span></span>
      {:else}
        <input
          type="checkbox"
          class={[
            "toggle toggle-xs shrink-0",
            active && "toggle-success",
            "opacity-0 group-hover:opacity-100 focus-visible:opacity-100",
            current && "opacity-100",
          ]}
          checked={active}
          aria-label="{name}: {active ? t('scripts.deactivate') : t('scripts.activate')}"
          title={active ? t("scripts.deactivate") : t("scripts.activate")}
          onchange={(e) => {
            e.currentTarget.checked = active;
            toggle(id, name, active);
          }}
        />
      {/if}
    {/if}
    <div class="dropdown dropdown-end">
      <div
        tabindex="0"
        role="button"
        class="grid size-6 place-items-center rounded-full text-base-content/65 opacity-0 group-hover:opacity-100 focus:opacity-100 hover:bg-base-200 {current ? 'opacity-100' : ''}"
        aria-label="{name}: …"
      >
        <Icon name="more" size={15} />
      </div>
      <ul tabindex="-1" {@attach closesMenu} class="dropdown-content menu z-10 w-52 rounded-box border border-base-300 bg-base-100 p-1 shadow-lg">
        <li><button onclick={() => rename(id, name)}>{t("scripts.rename")}</button></li>
        {#if root?.lastMoveWins && included}
          {#if root.unchanged.includes(name)}
            <li><button onclick={() => takeOver(id, name, false)}>{t("scripts.takeOver")}…</button></li>
          {:else}
            <li><button onclick={() => leaveUnchanged(id, name)}>{t("scripts.leaveUnchanged")}…</button></li>
          {/if}
        {/if}
        <li><button class="text-error" onclick={() => remove(id, name, active)}>{t("scripts.delete")}</button></li>
      </ul>
    </div>
  </div>
{/snippet}

{#snippet callout(tone: "warning" | "info", text: string)}
  <div class="flex items-start gap-2">
    <Icon name={tone === "warning" ? "alert" : "info"} size={15} class={tone === "warning" ? "mt-px text-warning" : "mt-px text-primary"} />
    <p>{text}</p>
  </div>
{/snippet}

<nav class="flex h-full flex-col overflow-y-auto" aria-label={t("accounts.heading")}>
  <div class="flex items-center gap-2 px-4 pt-4 pb-3">
    <img src="/favicon.png" alt="" class="size-6 rounded-md" />
    <span class="font-semibold tracking-tight">sieve-gui</span>
  </div>
  <div class="grid gap-5 px-2 pb-3">
  {#each session.accounts as account (account.id)}
    {@const server = session.servers[account.id]}
    {@const status = !server || server.status === "connecting" ? "connecting" : server.status === "error" ? "error" : "ready"}
    <section class="grid gap-1">
      <div class="flex items-center gap-2 px-2">
        <span
          class={["size-2 shrink-0 rounded-full", status === "ready" ? "bg-success" : status === "error" ? "bg-error" : "bg-warning animate-pulse"]}
          title={t(`accounts.status.${status}`)}
        ></span>
        <div class="min-w-0 grow">
          <h2 class="truncate text-[0.95rem] font-semibold" title="{account.username}@{account.host}">{account.label}</h2>
          <p class="truncate text-[0.8rem] text-base-content/65">{account.username}@{account.host}</p>
        </div>
        <div class="dropdown dropdown-end">
          <div tabindex="0" role="button" class="grid size-7 place-items-center rounded-full text-base-content/65 hover:bg-base-100" aria-label="{t('accounts.edit')}: {account.label}"><Icon name="more" size={16} /></div>
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
        <p class="flex items-center gap-2 px-2 text-sm text-base-content/60">
          <span class="loading loading-spinner loading-xs"></span>{t("accounts.connecting")}
        </p>
      {:else if server.status === "error"}
        <div role="alert" class="mx-1 grid gap-2 rounded-field bg-error/8 px-2.5 py-2 text-[0.875rem]">
          <p class="text-error">{server.error}</p>
          {#if server.certificate}
            {@const untrusted = server.certificate}
            <button class="btn btn-xs self-start" onclick={() => session.showCertificate(account.id, untrusted)}>{t("certificate.show")}</button>
          {:else}
            <button class="btn btn-xs self-start" onclick={() => session.connect(account.id)}>{t("accounts.retry")}</button>
          {/if}
        </div>
      {:else if server.info}
        {@const copy = session.copies[account.id]}
        {#if copy}
          <div role="alert" class="mx-1 grid gap-2 rounded-field bg-warning/10 px-2.5 py-2 text-[0.8rem]">
            {@render callout("warning", t("copy.notice", { count: copy.differences.length + (copy.active ? 1 : 0) }))}
            <button class="btn btn-xs justify-self-start" onclick={() => (session.copyPrompt = account.id)}>
              {t("copy.show")}
            </button>
          </div>
        {/if}
        {@const adoption = session.adoption[account.id]}
        {#if adoption}
          <div role="alert" class="mx-1 grid gap-2 rounded-field bg-primary/8 px-2.5 py-2 text-[0.8rem]">
            {@render callout("info", t("adopt.notice", { name: adoption.active }))}
            <div class="flex flex-wrap gap-1">
              <button class="btn btn-xs btn-primary" onclick={() => session.adopt(account.id, "unchanged")}>{t("adopt.unchanged")}</button>
              {#if session.rootSettings(account.id).lastMoveWins}
                <button class="btn btn-xs" onclick={() => takeOver(account.id, adoption.active, true)}>{t("scripts.takeOver")}…</button>
              {/if}
            </div>
          </div>
        {/if}
        {@const displaced = session.displaced[account.id]}
        {#if displaced}
          <div role="alert" class="mx-1 grid gap-2 rounded-field bg-warning/10 px-2.5 py-2 text-[0.8rem]">
            {@render callout("warning", displaced.active ? t("root.displaced", { name: displaced.active }) : t("root.displacedNone"))}
            <button class="btn btn-xs justify-self-start" onclick={() => session.reactivateRoot(account.id)}>
              {t("root.reactivate", { name: ROOT_SCRIPT })}
            </button>
          </div>
        {/if}
        <!-- The root script follows the account settings; it isn't listed. -->
        {@const tree = session.scriptTree(account.id)}
        {@const arrangeable = session.multipleActive(account.id)}
        <!-- Only the run order of the running scripts can be arranged. -->
        {@const order = tree.running.filter((n) => n.depth === 0).map((n) => n.name)}
        {@const dragging = drag?.account === account.id}
        <ul class="grid gap-0.5">
          {#each tree.running as node (node.name)}
            {@const index = node.depth === 0 ? order.indexOf(node.name) : -1}
            <li
              animate:flip={{ duration: 150 }}
              class="relative {dragging && index >= 0 && drag?.from === index ? 'opacity-50' : ''}"
              ondragover={(e) => {
                if (!dragging || index < 0) return;
                e.preventDefault();
                const rect = e.currentTarget.getBoundingClientRect();
                dropAt = e.clientY < rect.top + rect.height / 2 ? index : index + 1;
              }}
              ondrop={(e) => {
                e.preventDefault();
                if (drag?.account === account.id && index >= 0) {
                  const at = dropAt ?? index;
                  moveTo(account.id, order, drag.from, drag.from < at ? at - 1 : at);
                }
                drag = dropAt = null;
              }}
            >
              {#if dragging && index >= 0 && dropAt === index}
                <div class="absolute -top-0.5 left-0 right-0 h-0.5 rounded bg-primary"></div>
              {/if}
              {@render row(account.id, node, node.depth === 0, true, arrangeable ? index : -1, order)}
              {#if dragging && index === order.length - 1 && dropAt === order.length}
                <div class="absolute -bottom-0.5 left-0 right-0 h-0.5 rounded bg-primary"></div>
              {/if}
            </li>
          {/each}
          {#if tree.running.length && tree.inactive.length}
            <li class="mt-2.5 mb-0.5 px-2 text-[0.8rem] font-medium text-base-content/65">{t("scripts.inactiveHeading")}</li>
          {/if}
          {#each tree.inactive as node (node.name)}
            <li animate:flip={{ duration: 150 }}>
              {@render row(account.id, node, false, false, -1, order)}
            </li>
          {/each}
          {#if !tree.running.length && !tree.inactive.length}
            <li class="px-2 text-sm text-base-content/60">{t("scripts.none")}</li>
          {/if}
        </ul>
        <button class="btn btn-ghost btn-xs ml-1 w-fit justify-self-start gap-1 text-base-content/60" onclick={() => create(account.id)}><Icon name="plus" size={13} />{t("scripts.new")}</button>
        {@const folders = session.folders[account.id]}
        {#if folders?.status === "error"}
          <div role="alert" class="mx-1 grid gap-1.5 rounded-field bg-base-100 px-2.5 py-2 text-[0.8rem]">
            {@render callout("warning", t("folders.error", { message: folders.error ?? "" }))}
            {#if folders.certificate}
              {@const untrusted = folders.certificate}
              <button class="btn btn-xs self-start" onclick={() => session.showCertificate(account.id, untrusted)}>{t("certificate.show")}</button>
            {:else}
              <button class="btn btn-xs self-start" onclick={() => session.loadFolders(account.id)}>{t("accounts.retry")}</button>
            {/if}
          </div>
        {/if}
      {/if}
    </section>
  {:else}
    <p class="px-2 text-sm text-base-content/70">{t("accounts.empty")}</p>
  {/each}
  </div>

  <div class="mt-auto border-t border-base-300 p-2">
    <button class="btn btn-ghost btn-sm w-full justify-start gap-2 text-base-content/70" onclick={() => onaccount(null)}><Icon name="plus" size={15} />{t("accounts.add")}</button>
  </div>
</nav>

{#if session.copyPrompt && session.copies[session.copyPrompt]}
  {@const id = session.copyPrompt}
  {#key id}
    <CopyDialog
      status={session.copies[id]}
      label={session.accounts.find((a) => a.id === id)?.label ?? ""}
      onresolve={(choices, active) => session.resolveCopy(id, choices, active)}
      onclose={() => (session.copyPrompt = null)}
    />
  {/key}
{/if}

{#if session.certificatePrompts[0]}
  {@const { accountId, untrusted } = session.certificatePrompts[0]}
  {@const account = session.accounts.find((a) => a.id === accountId)}
  {#key session.certificatePrompts[0]}
    <CertificateDialog
      {untrusted}
      host={untrusted.service === "imap" ? (account?.imap?.host ?? account?.host ?? "") : (account?.host ?? "")}
      ontrust={() => session.trustCertificate(accountId, untrusted)}
      onclose={() => session.closeCertificate()}
    />
  {/key}
{/if}
