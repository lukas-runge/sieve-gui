<script lang="ts">
  // Where mail goes, the other way round: every folder (as a tree) and label
  // the running scripts send mail to, with how many rules do. Picking one
  // lists those rules.
  import type { RuleItem } from "#tauri/types";
  import { t } from "../../i18n.svelte";
  import { cases, labelHue, outcomes, type Case } from "../../rules/route";
  import type { Target } from "../../rules/search";
  import Icon from "../Icon.svelte";

  let {
    scripts,
    delimiter,
    keywords,
    onpick,
  }: {
    /** The running scripts, in order. */
    scripts: { name: string; items: RuleItem[] }[];
    delimiter: string | null;
    keywords: Record<string, string>;
    onpick: (target: Target & { text: string; folder: boolean }) => void;
  } = $props();

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

  const sorted = (f: Folder) => [...f.children.values()].sort((a, b) => a.name.localeCompare(b.name));
  const count = (n: number) => (n === 1 ? t("search.hitOne") : t("search.hits", { count: n }));
</script>

{#snippet folder(f: Folder, depth: number)}
  <li>
    <button
      class="flex w-full items-center gap-2 rounded-field py-1 pr-2 text-left hover:bg-base-200"
      style="padding-left: {0.5 + depth * 1.25}rem"
      title={t("target.title", { name: f.path.join(" / ") })}
      onclick={() => onpick({ kind: "folder", key: f.raw, subfolders: f.rules.size === 0, text: f.path.join(" / "), folder: true })}
    >
      <Icon name="folder" size={15} class={f.rules.size ? "text-primary" : "text-base-content/50"} />
      <span class={["truncate", f.rules.size ? "font-semibold" : "text-base-content/75"]}>{f.name}</span>
      <span class="h-px grow"></span>
      <span class="shrink-0 text-[0.8rem] text-base-content/70">
        {#if f.rules.size}{count(f.rules.size)}{:else}{t("dest.below", { count: f.all.size })}{/if}
      </span>
    </button>
    {#if f.children.size}
      <ul>
        {#each sorted(f) as child (child.name)}{@render folder(child, depth + 1)}{/each}
      </ul>
    {/if}
  </li>
{/snippet}

<div class="grid gap-5 px-3 py-4">
  <p class="px-1 text-sm text-base-content/70">{t("dest.intro", { count: scripts.length })}</p>
  <section aria-label={t("dest.folders")}>
    <h3 class="mb-1 px-1 text-[0.8rem] font-semibold tracking-wide text-base-content/70 uppercase">{t("dest.folders")}</h3>
    <ul class="grid">
      {#each sorted(tree.root) as f (f.name)}{@render folder(f, 0)}{/each}
      <li class="flex items-center gap-2 py-1 pr-2 pl-2 text-base-content/70">
        <Icon name="inbox" size={15} />{t("dest.inbox")}
      </li>
    </ul>
  </section>
  {#if tree.labels.length}
    <section aria-label={t("dest.labels")}>
      <h3 class="mb-2 px-1 text-[0.8rem] font-semibold tracking-wide text-base-content/70 uppercase">{t("dest.labels")}</h3>
      <div class="flex flex-wrap gap-2 px-1">
        {#each tree.labels as l (l.key)}
          {@const h = labelHue(l.text)}
          <button
            class="inline-flex items-center gap-1.5 rounded-full border px-2.5 py-0.5 text-[0.875rem] font-medium hover:brightness-95"
            style="border-color: oklch(0.7 0.09 {h} / 0.55); background: oklch(0.7 0.09 {h} / 0.12)"
            title={t("target.title", { name: l.text })}
            onclick={() => onpick({ kind: "label", key: l.key, text: l.text, folder: false })}
          >
            <span class="size-1.5 rounded-full" style="background: oklch(0.62 0.13 {h})"></span>{l.text}
            <span class="font-normal text-base-content/70">{l.rules.size}</span>
          </button>
        {/each}
      </div>
    </section>
  {/if}
</div>
