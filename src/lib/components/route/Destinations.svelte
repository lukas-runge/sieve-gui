<script lang="ts">
  // Where mail goes, the other way round: every folder (as a tree) and label
  // the running scripts send mail to, with how many rules do. Picking one
  // lists those rules.
  import type { RuleItem } from "#tauri/types";
  import { t } from "../../i18n.svelte";
  import { cases, labelStyle, outcomes, type Case } from "../../rules/route";
  import { session } from "../../session.svelte";
  import type { Target } from "../../rules/search";
  import { SvelteSet } from "svelte/reactivity";
  import Icon from "../Icon.svelte";

  let {
    scripts,
    delimiter,
    keywords,
    onadd,
    onpick,
  }: {
    /** The running scripts, in order. */
    scripts: { name: string; items: RuleItem[] }[];
    delimiter: string | null;
    keywords: Record<string, string>;
    /** Adds a rule that files into the folder, to the open script. */
    onadd?: (target: Target & { text: string; folder: boolean }) => void;
    onpick: (target: Target & { text: string; folder: boolean }) => void;
  } = $props();
  // Tag colours of the open script's account.
  const colors = $derived(session.editor ? session.keywordColors(session.editor.accountId) : {});

  interface Folder {
    name: string;
    /** The folder as stored, for filtering. */
    raw: string;
    path: string[];
    /** Rules filing right here. */
    rules: Set<string>;
    /** Rules filing here or below. */
    all: Set<string>;
    children: Map<string, Folder>;
  }

  const tree = $derived.by(() => {
    const root: Folder = { name: "", raw: "", path: [], rules: new Set(), all: new Set(), children: new Map() };
    const labels = new Map<string, { text: string; key: string; rules: Set<string> }>();
    const visit = (c: Case, id: string) => {
      for (const o of outcomes(c.actions, keywords, delimiter)) {
        if (!o.target) continue;
        if (o.target.kind === "label") {
          const entry = labels.get(o.target.key) ?? { text: o.text, key: o.target.key, rules: new Set<string>() };
          entry.rules.add(id);
          labels.set(o.target.key, entry);
          continue;
        }
        // Walk down the path, making the folders above on the way.
        const segments = o.path.length ? o.path : [o.text];
        const rawParts = delimiter ? o.target.key.split(delimiter) : [o.target.key];
        let node = root;
        segments.forEach((name, i) => {
          let next = node.children.get(name);
          if (!next) {
            const raw = rawParts.length === segments.length ? rawParts.slice(0, i + 1).join(delimiter ?? "") : o.target!.key;
            next = { name, raw, path: segments.slice(0, i + 1), rules: new Set(), all: new Set(), children: new Map() };
            node.children.set(name, next);
          }
          next.all.add(id);
          node = next;
        });
        node.rules.add(id);
      }
      for (const n of c.nested) for (const inner of cases(n)) visit(inner, id);
    };
    for (const script of scripts) {
      for (const [index, item] of script.items.entries()) {
        if (item.type !== "rule" || !item.enabled) continue;
        for (const c of cases(item)) visit(c, `${script.name}:${index}`);
      }
    }
    return { root, labels: [...labels.values()].sort((a, b) => a.text.localeCompare(b.text)) };
  });

  /** Folder branches folded away. */
  const closed = new SvelteSet<string>();
  const sorted = (f: Folder) => [...f.children.values()].sort((a, b) => a.name.localeCompare(b.name));
  const count = (n: number) => (n === 1 ? t("search.hitOne") : t("search.hits", { count: n }));
</script>

{#snippet folder(f: Folder, depth: number)}
  <li class="relative">
    {#if f.children.size}
      <!-- Folds the folders below; a button of its own beside the folder. -->
      <button
        class="absolute top-1 grid size-6 place-items-center rounded hover:bg-base-300"
        style="left: {depth * 1.25 - 0.9}rem"
        aria-expanded={!closed.has(f.raw)}
        aria-label={closed.has(f.raw) ? t("dest.open", { name: f.name }) : t("dest.close", { name: f.name })}
        onclick={() => (closed.has(f.raw) ? closed.delete(f.raw) : closed.add(f.raw))}
        ><Icon name={closed.has(f.raw) ? "chevronRight" : "chevronDown"} size={13} /></button
      >
    {/if}
    <div class="group/row flex items-center gap-1">
    <button
      class="flex min-w-0 grow items-center gap-2 rounded-field py-1 pr-2 text-left hover:bg-base-200"
      style="padding-left: {0.5 + depth * 1.25}rem"
      title={t("target.title", { name: f.path.join(" / ") })}
      onclick={() => onpick({ kind: "folder", key: f.raw, subfolders: f.rules.size === 0, text: f.path.join(" / "), folder: true })}
    >
      <Icon name="folder" size={15} class={f.rules.size ? "text-primary" : "text-base-content/50"} />
      <span class={["truncate", f.rules.size ? "font-semibold" : "text-base-content/75"]}>{f.name}</span>
      <span class="h-px grow"></span>
      <span class="shrink-0 text-[0.8125rem] text-base-content/70">
        {[f.rules.size ? t("dest.direct", { rules: count(f.rules.size) }) : "", f.all.size > f.rules.size ? t("dest.below", { count: f.all.size - f.rules.size }) : ""].filter(Boolean).join(" · ")}
      </span>
    </button>
    {#if onadd}
      <!-- Thinking by destination: a new rule for this folder, right here. -->
      <button
        class="grid size-7 shrink-0 place-items-center rounded-field text-base-content/70 opacity-0 group-hover/row:opacity-100 focus-visible:opacity-100 hover:bg-base-200 hover:text-base-content"
        aria-label={t("dest.addFor", { name: f.path.join(" / ") })}
        title={t("dest.addFor", { name: f.path.join(" / ") })}
        onclick={() => onadd({ kind: "folder", key: f.raw, subfolders: false, text: f.path.join(" / "), folder: true })}><Icon name="plus" size={15} /></button
      >
    {/if}
    </div>
    {#if f.children.size && !closed.has(f.raw)}
      <ul>
        {#each sorted(f) as child (child.name)}{@render folder(child, depth + 1)}{/each}
      </ul>
    {/if}
  </li>
{/snippet}

<div class="grid gap-5 py-4 pr-3 pl-6">
  {#if tree.labels.length}
    <section aria-label={t("dest.labels")}>
      <h3 class="mb-2 px-1 text-[0.8125rem] font-semibold tracking-wide text-base-content/70 uppercase">{t("dest.labels")}</h3>
      <div class="flex flex-wrap gap-2 px-1">
        {#each tree.labels as l (l.key)}
          {@const style = labelStyle(l.text, colors[l.key])}
          <button
            class="inline-flex items-center gap-1.5 rounded-full border px-2.5 py-0.5 text-[0.875rem] font-medium hover:brightness-95"
            style={style.chip}
            title={t("target.title", { name: l.text })}
            onclick={() => onpick({ kind: "label", key: l.key, text: l.text, folder: false })}
          >
            <span class="size-2 rounded-full" style={style.dot}></span>{l.text}
            <span class="font-normal text-base-content/70">{l.rules.size}</span>
          </button>
        {/each}
      </div>
    </section>
  {/if}
  <section aria-label={t("dest.folders")}>
    <h3 class="mb-1 px-1 text-[0.8125rem] font-semibold tracking-wide text-base-content/70 uppercase">{t("dest.folders")}</h3>
    <ul class="grid">
      {#each sorted(tree.root) as f (f.name)}{@render folder(f, 0)}{/each}
      <!-- Right edge in line with the counts above (which leave room for their "+"). -->
      <li class={["flex items-center gap-2 py-1 pl-2 text-base-content/70", onadd ? "pr-10" : "pr-2"]} title={t("dest.inbox")}>
        <Icon name="inbox" size={15} />{t("dest.inboxShort")}<span class="h-px grow"></span><span class="text-[0.8125rem]">{t("dest.inboxRest")}</span>
      </li>
    </ul>
  </section>
</div>
