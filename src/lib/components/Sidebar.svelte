<script lang="ts">
  import { flip } from "svelte/animate";
  import { SvelteSet } from "svelte/reactivity";
  import type { Account, Detection } from "#tauri/types";
  import type { ScriptNode } from "../session.svelte";
  import { t, type MessageKey } from "../i18n.svelte";
  import CertificateDialog from "./CertificateDialog.svelte";
  import CopyDialog from "./CopyDialog.svelte";
  import ProviderDialog from "./ProviderDialog.svelte";
  import { ask, askText, tell } from "../dialogs.svelte";
  import { closesMenu } from "../menu";
  import { ROOT_SCRIPT, session } from "../session.svelte";
  import Icon from "./Icon.svelte";
  import { flowView } from "../scope.svelte";
  import AccountIcon from "./AccountIcon.svelte";
  import { describeError } from "../api";
  import { startUpdate, update } from "../update.svelte";

  let {
    onaccount,
    ontoggle,
  }: {
    onaccount: (account: Account | null) => void;
    /** Hides the sidebar (its toggle sits in its header). */
    ontoggle?: () => void;
  } = $props();

  /** The filter provider whose take-over dialog is open. */
  let providerPrompt = $state<{ accountId: string; detection: Detection } | null>(null);
  const providerName = (id: string) => t(`provider.${id}` as MessageKey);

  /** The update notice, until "Not now" for this run of the app. */
  let updateDismissed = $state(false);

  async function installUpdate(version: string) {
    if (!(await session.confirmLeave())) return;
    const message = t(update.channel === "package" ? "update.confirmPackage" : "update.confirm", { version });
    if (!(await ask(message, t("update.confirmAction")))) return;
    try {
      await startUpdate();
    } catch (e) {
      await tell(t("update.failed", { message: describeError(e) }));
    }
  }


  async function open(id: string, name: string) {
    if (session.editor?.accountId === id && session.editor.name === name) {
      flowView.account = null;
      return;
    }
    if (await session.confirmLeave()) {
      await session.openScript(id, name);
      flowView.account = null;
    }
  }

  /**
   * One account at a time: the one picked above, following the open script.
   * Each account remembers the script last open in it, to come back to.
   */
  let picked = $state<string | null>(localStorage.getItem("account"));
  $effect(() => {
    const ed = session.editor;
    if (!ed) return;
    picked = ed.accountId;
    localStorage.setItem(`lastScript:${ed.accountId}`, ed.name);
  });
  $effect(() => {
    if (picked) localStorage.setItem("account", picked);
  });
  const current = $derived(session.accounts.find((a) => a.id === picked) ?? session.accounts[0] ?? null);

  /** Switches to another account, opening its last script (or its first). */
  async function pickAccount(id: string) {
    if (current?.id === id && session.editor?.accountId === id) return;
    if (session.editor && session.editor.accountId !== id && !(await session.confirmLeave())) return;
    flowView.account = null;
    picked = id;
    const tree = session.scriptTree(id);
    const names = [...tree.running, ...tree.inactive].map((n) => n.name);
    const last = localStorage.getItem(`lastScript:${id}`);
    const name = last && names.includes(last) ? last : (tree.running.find((n) => n.depth === 0)?.name ?? names[0]);
    if (name) await session.openScript(id, name);
    else session.editor = null;
  }

  /** The account icons scroll sideways when there are more than fit; the open one stays in view. */
  let iconStrip = $state<HTMLElement>();
  /** More icons that way: the edge fades out to say so. */
  let more = $state({ left: false, right: false });
  function measure() {
    const el = iconStrip;
    if (!el) return;
    more = { left: el.scrollLeft > 1, right: el.scrollLeft + el.clientWidth < el.scrollWidth - 1 };
  }
  $effect(() => {
    void session.accounts.length;
    const el = iconStrip;
    if (!el) return;
    measure();
    const ro = new ResizeObserver(measure);
    ro.observe(el);
    return () => ro.disconnect();
  });
  $effect(() => {
    const id = current?.id;
    if (id) iconStrip?.querySelector(`[data-account="${id}"]`)?.scrollIntoView({ inline: "nearest", block: "nearest" });
  });

  /** Something the account needs: a problem or a decision waiting. */
  function needsAttention(id: string): boolean {
    return !!(session.copies[id] || session.adoption[id] || session.displaced[id] || session.folders[id]?.status === "error");
  }

  /** All running scripts of the account together; opens the first if none of its scripts is open. */
  async function openFlow(id: string, order: string[]) {
    if (session.editor?.accountId !== id) {
      if (!(await session.confirmLeave())) return;
      await session.openScript(id, order[0]);
    }
    if (session.editor) session.editor.selected = null;
    flowView.account = id;
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
  {@const current = session.editor?.accountId === id && session.editor.name === name && flowView.account !== id}
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
        class="ml-1 grid size-6 shrink-0 cursor-grab place-items-center rounded-md bg-primary/12 text-[0.8rem] font-bold text-primary hover:bg-primary/20"
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
        <span class="badge-num">{index + 1}</span>
      </button>
    {:else if node.depth > 0}
      <span class="ml-1 grid size-6 shrink-0 place-items-center text-base-content/35" aria-hidden="true"><Icon name="nested" size={14} /></span>
    {:else if running && position >= 0}
      <span class="ml-1 grid size-6 shrink-0 place-items-center rounded-md bg-primary/12 text-[0.8rem] font-bold text-primary" aria-hidden="true"><span class="badge-num">1</span></span>
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
          tabindex={current ? 0 : -1}
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
        tabindex={current ? 0 : -1}
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

{#snippet accountIcon(account: Account)}
  {@const server = session.servers[account.id]}
  {@const status = !server || server.status === "connecting" ? "connecting" : server.status === "error" ? "error" : "ready"}
  {@const on = current?.id === account.id}
  <!-- Only what's out of the ordinary is marked: connected is the normal case and shows the icon as it is. -->
  {@const badge = status === "error" ? "error" : needsAttention(account.id) ? "attention" : null}
  <button
    class={[
      "relative size-7 shrink-0 rounded-lg transition",
      on ? "ring-1 ring-primary ring-offset-2 ring-offset-base-200" : "hover:brightness-110",
      status === "connecting" && "animate-pulse opacity-50",
    ]}
    aria-pressed={on}
    data-account={account.id}
    aria-label="{account.label} ({t(`accounts.status.${status}`)}{badge === 'attention' ? ', ' + t('accounts.attention') : ''})"
    title="{account.label} · {account.username}@{account.host}{status !== 'ready' ? ' · ' + t(`accounts.status.${status}`) : ''}{badge === 'attention' ? ' · ' + t('accounts.attention') : ''}"
    onclick={() => pickAccount(account.id)}
  >
    <AccountIcon label={account.label} icon={account.icon} size={28} />
    {#if badge}
      <!-- On the corner, half outside, like a Dock badge: it covers little of the icon. -->
      <span
        class={[
          "absolute -top-1 -right-1 grid size-3 place-items-center rounded-full text-[0.5rem] leading-none font-black ring-2 ring-base-200",
          badge === "error" ? "bg-error text-error-content" : "bg-warning text-warning-content",
        ]}
        aria-hidden="true">!</span
      >
    {/if}
  </button>
{/snippet}

<nav class="flex h-full flex-col" aria-label={t("accounts.heading")}>
  <!-- The top of the window: its controls (macOS), the accounts as icons, the toggle. -->
  <div class="sidebar-top flex h-13 shrink-0 items-center pr-2 pl-3" data-tauri-drag-region>
    <!-- Scrolls sideways with many accounts; the padding keeps rings and dots from being cut off. -->
    <div
      bind:this={iconStrip}
      class="flex min-w-0 items-center gap-3 overflow-x-auto py-1 pr-0 pl-1 [scrollbar-width:none]"
      style:mask-image={more.left || more.right
        ? `linear-gradient(to right, ${more.left ? "transparent, black 12px" : "black"}, ${more.right ? "black calc(100% - 12px), transparent" : "black"})`
        : null}
      onscroll={measure}
      role="group"
      aria-label={t("accounts.heading")}
      onwheel={(e) => {
        // A mouse wheel scrolls it too, not only a trackpad's sideways swipe.
        if (Math.abs(e.deltaY) > Math.abs(e.deltaX) && e.currentTarget.scrollWidth > e.currentTarget.clientWidth) {
          e.currentTarget.scrollLeft += e.deltaY;
          e.preventDefault();
        }
      }}
    >
      {#each session.accounts as account (account.id)}{@render accountIcon(account)}{/each}
      <button
        class="grid size-7 shrink-0 place-items-center rounded-lg border border-dashed border-base-content/25 text-base-content/60 hover:border-base-content/50 hover:text-base-content"
        aria-label={t("accounts.add")}
        title={t("accounts.add")}
        onclick={() => onaccount(null)}><Icon name="plus" size={14} /></button
      >
    </div>
    <!-- Room to drag the window; with many accounts it shrinks to a sliver, so the fade ends near the toggle. -->
    <span class="h-full min-w-1 grow" data-tauri-drag-region></span>
    {#if ontoggle}
      <button class="btn btn-ghost btn-sm btn-square shrink-0 text-base-content/60" aria-label={t("sidebar.hide")} title={t("sidebar.hide")} onclick={ontoggle}>
        <Icon name="panelLeft" size={17} />
      </button>
    {/if}
  </div>
  <!-- The open account: plain on the sidebar, outside the scrolling list, set off by an inset divider. -->
  {#if current}
    {@const account = current}
    <header class="shrink-0 border-t border-base-300">
      <div class="flex items-center gap-2 py-3 pr-3 pl-4">
        <div class="min-w-0 grow">
          <h2 class="truncate text-[0.9375rem] leading-5 font-semibold" title="{account.label} · {account.username}@{account.host}">{account.label}</h2>
          <p class="mt-0.5 truncate text-[0.75rem] leading-4 text-base-content/65" title="{account.username}@{account.host}">{account.username}@{account.host}</p>
        </div>
        <div class="dropdown dropdown-end shrink-0">
          <div
            tabindex="0"
            role="button"
            class="grid size-7 place-items-center rounded-md text-base-content/65 hover:bg-base-content/10 hover:text-base-content focus-visible:ring-2 focus-visible:ring-primary/50 focus-visible:outline-none"
            aria-label="{t('accounts.edit')}: {account.label}"
          ><Icon name="more" size={16} /></div>
          <ul tabindex="-1" {@attach closesMenu} class="dropdown-content menu z-30 w-48 rounded-box border border-base-300 bg-base-100 p-1 shadow-lg">
            <li><button onclick={() => onaccount(account)}>{t("accounts.edit")}</button></li>
            {#if session.roots[account.id]}
              <li><button onclick={() => session.viewRoot(account.id)}>{t("root.view")}</button></li>
            {/if}
            <li><button class="text-error" onclick={() => removeAccount(account)}>{t("accounts.delete")}</button></li>
          </ul>
        </div>
      </div>
      <div class="mx-4 h-px bg-base-content/10"></div>
    </header>
  {/if}
  <div class="min-h-0 grow overflow-y-auto pt-3">
  <div class="grid grid-cols-[minmax(0,1fr)] gap-5 px-2 pb-3">
  {#each current ? [current] : [] as account (account.id)}
    {@const server = session.servers[account.id]}
    <section class="grid grid-cols-[minmax(0,1fr)] gap-1">
      {#if !server || server.status === "connecting"}
        <p class="flex items-center gap-2 px-2 text-sm text-base-content/60">
          <span class="loading loading-spinner loading-xs"></span>{t("accounts.connecting")}
        </p>
      {:else if server.status === "error"}
        <!-- A problem of this account only: said quietly, with the way out. -->
        <div role="alert" class="mx-1 grid gap-1.5 px-2 py-1 text-[0.8125rem]">
          <p class="flex items-start gap-1.5 text-base-content/80"><Icon name="alert" size={14} class="mt-0.5 shrink-0 text-error" />{server.error}</p>
          {#if server.certificate}
            {@const untrusted = server.certificate}
            <button class="btn btn-xs justify-self-start" onclick={() => session.showCertificate(account.id, untrusted)}>{t("certificate.show")}</button>
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
        {@const cleanup = session.cleanup[account.id]}
        {#if cleanup}
          <div role="status" class="mx-1 grid gap-2 rounded-field bg-primary/8 px-2.5 py-2 text-[0.8rem]">
            {@render callout("info", t(cleanup.done ? "provider.cleanupDone" : "provider.cleanup", { provider: providerName(cleanup.provider) }))}
            <button class="btn btn-xs justify-self-start" onclick={() => delete session.cleanup[account.id]}>
              {t(cleanup.done ? "provider.dismiss" : "provider.hide")}
            </button>
          </div>
        {/if}
        {@const reclaimed = session.reclaimed[account.id]}
        {#if reclaimed}
          <div role="status" class="mx-1 grid gap-2 rounded-field bg-primary/8 px-2.5 py-2 text-[0.8rem]">
            {@render callout("info", t("provider.reclaimed", { provider: providerName(reclaimed) }))}
            <button class="btn btn-xs justify-self-start" onclick={() => delete session.reclaimed[account.id]}>{t("provider.dismiss")}</button>
          </div>
        {/if}
        {#each session.providers[account.id] ?? [] as detection (detection.provider)}
          {@const later = account.filtersAlone?.provider === detection.provider}
          {@const counts = { provider: providerName(detection.provider), filters: detection.filters, entries: detection.listEntries }}
          <div role="alert" class="mx-1 grid gap-2 rounded-field bg-warning/10 px-2.5 py-2 text-[0.8rem]">
            {@render callout("warning", t(later ? "provider.foundLater" : "provider.found", counts))}
            <button class="btn btn-xs btn-primary justify-self-start" onclick={() => (providerPrompt = { accountId: account.id, detection })}>
              {t("provider.takeOver")}
            </button>
          </div>
        {/each}
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
        <ul class="grid grid-cols-[minmax(0,1fr)] gap-1">
          {#if order.length > 1}
            <!-- The account's running scripts together: the way every message takes. -->
            {@const flowOpen = flowView.account === account.id && session.editor?.accountId === account.id}
            <li>
              <button
                class={[
                  "flex w-full items-center gap-1 rounded-field py-1.5 pr-2 text-left text-[0.9rem]",
                  flowOpen ? "bg-base-100 font-semibold shadow-sm ring-1 ring-base-300" : "hover:bg-base-100/70",
                ]}
                aria-current={flowOpen ? "page" : undefined}
                title={t("sidebar.flowTitle")}
                onclick={() => openFlow(account.id, order)}
              >
                <span class="ml-1 grid size-6 shrink-0 place-items-center rounded-md text-primary"><Icon name="route" size={15} /></span>
                <span class="min-w-0 grow truncate pl-1">{t("sidebar.flow")}</span>
                <span class="shrink-0 text-[0.8rem] font-normal text-base-content/60">{order.length}</span>
              </button>
            </li>
          {/if}
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
  </div>
  {#if update.available && !updateDismissed}
    {@const version = update.available}
    <!-- Linux only (see update.svelte.ts): the newer release, one click from installing it. -->
    <aside class="m-2 grid shrink-0 gap-2 rounded-box border border-base-300 bg-base-100 px-3 py-2.5 text-[0.8125rem]" aria-live="polite">
      <p class="flex items-center gap-2 font-medium"><Icon name="download" size={14} class="shrink-0 text-primary" />{t("update.available", { version })}</p>
      <div class="flex gap-1.5">
        <button class="btn btn-primary btn-xs" disabled={update.installing} onclick={() => installUpdate(version)}>{t("update.install")}</button>
        <button class="btn btn-ghost btn-xs" onclick={() => (updateDismissed = true)}>{t("update.dismiss")}</button>
      </div>
    </aside>
  {/if}

</nav>

{#if providerPrompt}
  <ProviderDialog accountId={providerPrompt.accountId} detection={providerPrompt.detection} onclose={() => (providerPrompt = null)} />
{/if}
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
