<script lang="ts">
  // The rule picked on the route, to read and change beside it.
  import { setContext, tick } from "svelte";
  import { importRules } from "#tauri/commands";
  import type { Folder, Rule } from "#tauri/types";
  import { describeError } from "../api";
  import { uid, type ScriptEditor } from "../editor.svelte";
  import { t } from "../i18n.svelte";
  import { closesMenu } from "../menu";
  import { scriptFolders, type ScriptFolders } from "../rules/mailbox";
  import { casePathLabel, isSimple } from "../rules/route";
  import { session } from "../session.svelte";
  import BranchesEditor, { SUMMARY, type SummaryContext } from "./BranchesEditor.svelte";
  import CodeEditor from "./CodeEditor.svelte";
  import Icon from "./Icon.svelte";

  let {
    editor,
    extensions,
    serverFolders,
    onback,
  }: {
    editor: ScriptEditor;
    extensions: string[];
    serverFolders: Folder[] | null;
    /** In narrow windows the inspector replaces the route: back to it. */
    onback?: () => void;
  } = $props();

  // Case summaries read like the route.
  const summary: SummaryContext = $state({ keywords: {}, delimiter: null });
  $effect(() => {
    summary.keywords = session.keywordNames(editor.accountId);
    summary.delimiter = serverFolders?.find((f) => f.delimiter)?.delimiter ?? null;
  });
  setContext(SUMMARY, summary);

  // The picked case comes into view and lights up briefly.
  let body = $state<HTMLElement>();
  $effect(() => {
    const target = `${editor.selected}|${editor.focus}`;
    void tick().then(() => {
      if (!body || target !== `${editor.selected}|${editor.focus}`) return;
      const el = editor.focus ? body.querySelector<HTMLElement>(`[data-case="${editor.focus}"]`) : null;
      if (el) {
        el.scrollIntoView({ block: "start", behavior: "smooth" });
        el.animate([{ backgroundColor: "color-mix(in oklab, var(--color-primary) 14%, transparent)" }, { backgroundColor: "transparent" }], {
          duration: 1200,
          easing: "ease-out",
        });
      } else body.scrollTo({ top: 0 });
    });
  });

  const index = $derived(editor.rules.items.findIndex((i) => i.uid === editor.selected));
  const item = $derived(index >= 0 ? editor.rules.items[index] : null);

  /** Folders on the server and used anywhere in the script, for folder inputs. */
  const folders: ScriptFolders = $derived.by(() => {
    const raw = editor.rules.items.flatMap((i) =>
      i.type === "rule" ? i.actions.flatMap((a) => (a.type === "fileInto" && a.mailbox ? [a.mailbox] : [])) : [],
    );
    return scriptFolders(raw, serverFolders);
  });

  /** Mutates the rule with an undo step. */
  function change(fn: (r: Rule) => void, merge = false) {
    editor.edit((rules) => {
      const it = rules.items[index];
      if (it?.type === "rule") fn(it);
    }, merge);
  }

  let syntaxError = $state<string | null>(null);
  async function updateCode(next: string) {
    editor.edit((rules) => {
      const it = rules.items[index];
      if (it?.type === "raw") it.code = next;
    }, true);
    try {
      await importRules(next);
      syntaxError = null;
    } catch (e) {
      syntaxError = describeError(e);
    }
  }

  function duplicate() {
    const copy = { ...$state.snapshot(editor.rules.items[index]), uid: uid() };
    if (copy.type === "rule") copy.name = copy.name ? t("rule.copyOf", { name: copy.name }) : "";
    editor.edit((rules) => {
      rules.items.splice(index + 1, 0, copy);
    });
    editor.selected = copy.uid;
  }

  function remove() {
    editor.edit((rules) => {
      rules.items.splice(index, 1);
    });
    editor.selected = null;
  }

  function onkeydown(e: KeyboardEvent) {
    if (e.key === "Escape" && !(e.target as HTMLElement).closest("input, textarea, select, .cm-editor")) editor.selected = null;
  }
</script>

<svelte:window {onkeydown} />

{#if item}
  <aside class="flex h-full min-w-0 flex-col border-l border-base-300 bg-base-100" aria-label={t("inspector.label")}>
    <header class="flex items-start gap-2 border-b border-base-300 px-4 pt-3 pb-2.5">
      {#if onback}
        <button class="btn btn-ghost btn-sm btn-square -ml-1.5" aria-label={t("inspector.back")} title={t("inspector.back")} onclick={onback}>
          <Icon name="arrowLeft" size={17} />
        </button>
      {/if}
      <div class="min-w-0 grow">
        <p class="truncate text-[0.8rem] text-base-content/65">
          {#if item.type === "raw"}{t("raw.heading")}{:else if isSimple(item)}{t("inspector.rule")}{:else}{t("inspector.switch")}{/if}
          {#if item.type === "rule" && editor.focus}
            <span class="text-base-content/45">›</span> <span class="font-medium text-base-content/80">{casePathLabel(item, editor.focus)}</span>
          {/if}
        </p>
        {#if item.type === "rule"}
          <input
            class="w-full rounded-field bg-transparent text-lg font-semibold outline-none placeholder:font-normal placeholder:text-base-content/45 focus:bg-base-200 focus:px-1.5"
            aria-label={t("rule.name")}
            placeholder={t("inspector.namePlaceholder")}
            value={item.name}
            oninput={(e) => change((r) => (r.name = e.currentTarget.value), true)}
          />
        {/if}
      </div>
      <div class="dropdown dropdown-end">
        <div tabindex="0" role="button" class="btn btn-ghost btn-sm btn-square" aria-label={t("menu.more")}><Icon name="more" size={17} /></div>
        <ul tabindex="-1" {@attach closesMenu} class="dropdown-content menu z-30 w-48 rounded-box border border-base-300 bg-base-100 p-1 shadow-lg">
          <li><button onclick={duplicate}>{t("rule.duplicate")}</button></li>
          <li><button class="text-error" onclick={remove}>{t("rule.delete")}</button></li>
        </ul>
      </div>
      <button class="btn btn-ghost btn-sm btn-square" aria-label={t("inspector.close")} title="{t('inspector.close')} (Esc)" onclick={() => (editor.selected = null)}>
        <Icon name="x" size={17} />
      </button>
    </header>

    <div class="min-h-0 grow overflow-y-auto px-4 py-4" bind:this={body}>
      {#if item.type === "rule"}
        <label class="mb-4 flex items-center gap-2.5 text-sm">
          <input type="checkbox" class="toggle toggle-sm toggle-primary" checked={item.enabled} onchange={(e) => change((r) => (r.enabled = e.currentTarget.checked))} />
          <span>{item.enabled ? t("inspector.on") : t("inspector.off")}</span>
        </label>
        <BranchesEditor
          block={item}
          {extensions}
          {folders}
          focus={editor.focus}
          singleFolder={editor.rules.singleFolder}
          mutate={(fn, merge) => change((r) => fn(r), merge)}
        />
      {:else}
        <p class="mb-3 text-sm text-base-content/70">{t("raw.explain")}</p>
        <CodeEditor value={item.code} label={t("raw.heading")} onchange={updateCode} />
        {#if syntaxError}
          <p role="alert" class="mt-2 text-sm text-error">{t("raw.invalid", { message: syntaxError })}</p>
        {/if}
      {/if}
    </div>
  </aside>
{/if}
