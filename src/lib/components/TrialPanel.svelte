<script lang="ts">
  // Trying the rules with a sample message: where it ends up, and its way
  // through the scripts that run, case by case. Each step opens its rule.
  import type { Action, Conditions, RuleItem } from "#tauri/types";
  import type { ScriptEditor } from "../editor.svelte";
  import { t } from "../i18n.svelte";
  import { caseChain, casePathLabel, conditionShape, isSimple, outcomes, stops, type Outcome } from "../rules/route";
  import RouteConditions from "./route/Conditions.svelte";
  import { looksLikeMail, parseMail, type SampleMail } from "../rules/simulate";
  import { session } from "../session.svelte";
  import { trial } from "../trial.svelte";
  import Icon from "./Icon.svelte";
  import Outcomes from "./route/Outcomes.svelte";

  let {
    editor,
    delimiter,
    keywords,
    onback,
  }: {
    editor: ScriptEditor;
    delimiter: string | null;
    keywords: Record<string, string>;
    /** In narrow windows the panel replaces the route: back to it. */
    onback?: () => void;
  } = $props();

  $effect(() => {
    void trial.load(editor.accountId);
  });

  const result = $derived(trial.result);
  const running = $derived(session.runs(editor.accountId, editor.name));
  const itemsOf = (script: string) => (script === editor.name ? editor.rules.items : (trial.saved?.scripts.find((s) => s.name === script)?.rules.items ?? []));

  /** Where the message ends up, as the route shows outcomes. */
  const delivery = $derived.by((): Outcome[] => {
    if (!result) return [];
    const d = result.delivery;
    const actions: Action[] = d.folders.map((mailbox) => ({ type: "fileInto", mailbox, copy: false, create: false, flags: null }));
    if (d.inbox) actions.push({ type: "keep", flags: null });
    if (d.flags.length) actions.push({ type: "flags", op: "add", flags: d.flags });
    return outcomes(actions, keywords, delimiter).map((o) => (o.kind === "keep" ? { ...o, text: t("trial.inbox") } : o));
  });

  function deliveryOf(r: NonNullable<typeof result>): Outcome[] {
    const d = r.delivery;
    const actions: Action[] = d.folders.map((mailbox) => ({ type: "fileInto", mailbox, copy: false, create: false, flags: null }));
    if (d.inbox) actions.push({ type: "keep", flags: null });
    if (d.flags.length) actions.push({ type: "flags", op: "add", flags: d.flags });
    return outcomes(actions, keywords, delimiter).map((o) => (o.kind === "keep" ? { ...o, text: t("trial.inbox") } : o));
  }
  /** Whether the unsaved edits change where the message ends up. */
  const changed = $derived.by(() => {
    if (!result || !trial.before) return false;
    const key = (r: NonNullable<typeof result>) => JSON.stringify({ ...r.delivery, sources: null });
    return key(result) !== key(trial.before);
  });

  /** What each script contributed to the result, in the order they ran. */
  const contributions = $derived.by(() => {
    if (!result) return [];
    const d = result.delivery;
    const by = new Map<string, string[]>();
    const add = (key: string, text: string) => {
      const script = d.sources[key];
      if (script) by.set(script, [...(by.get(script) ?? []), text]);
    };
    for (const f of d.folders) add(`folder:${f}`, outcomes([{ type: "fileInto", mailbox: f, copy: false, create: false, flags: null }], keywords, delimiter)[0]?.path.at(-1) ?? f);
    for (const f of d.flags) add(`flag:${f}`, outcomes([{ type: "flags", op: "add", flags: [f] }], keywords, delimiter)[0]?.text ?? f);
    return trial.order.filter((n) => by.has(n)).map((script) => ({ script, items: by.get(script)! }));
  });

  /** A pasted message fills all fields at once. */
  function onpaste(e: ClipboardEvent) {
    const text = e.clipboardData?.getData("text/plain") ?? "";
    if (!looksLikeMail(text)) return;
    e.preventDefault();
    trial.mail = parseMail(text);
    more = trial.mail.headers !== "" || trial.mail.body !== "";
  }

  let more = $state(trial.mail.headers !== "" || trial.mail.body !== "");

  async function openStep(script: string, index: number, path: string) {
    if (script !== editor.name) {
      if (!(await session.confirmLeave())) return;
      await session.openScript(editor.accountId, script);
    }
    const ed = session.editor;
    const item = ed?.rules.items[index];
    if (ed && item) {
      ed.selected = item.uid;
      ed.focus = item.type === "rule" && isSimple(item) ? "" : path;
    }
  }

  /** The script running after `script`, where a stop in it continues. */
  function nextOf(script: string): string | null {
    const i = trial.order.indexOf(script);
    return i >= 0 ? (trial.order[i + 1] ?? null) : null;
  }

  function stepTitle(script: string, index: number, path: string): { name: string; where: string; conditions: Conditions | null } {
    const item = itemsOf(script)[index];
    if (!item || item.type !== "rule") return { name: t("raw.heading"), where: "", conditions: null };
    return {
      name: item.name || t("trial.ruleN", { n: index + 1 }),
      where: isSimple(item) ? "" : casePathLabel(item, path),
      // Why it applied: what the case tests.
      conditions: caseChain(item, path)?.at(-1)?.case.conditions ?? null,
    };
  }

  const fields: { key: keyof SampleMail; label: string; placeholder: string }[] = $derived([
    { key: "from", label: t("trial.from"), placeholder: t("trial.fromPlaceholder") },
    { key: "to", label: t("trial.to"), placeholder: t("trial.toPlaceholder") },
    { key: "subject", label: t("trial.subject"), placeholder: t("trial.subjectPlaceholder") },
  ]);
</script>

<aside class="flex h-full min-w-0 flex-col border-l border-base-300 bg-base-100" aria-label={t("trial.heading")}>
  <header class="flex items-center gap-2 border-b border-base-300 px-4 py-3">
    {#if onback}
      <button class="btn btn-ghost btn-sm btn-square -ml-1.5" aria-label={t("inspector.back")} title={t("inspector.back")} onclick={onback}>
        <Icon name="arrowLeft" size={17} />
      </button>
    {/if}
    <span class="grid size-7 place-items-center rounded-full bg-success/12 text-success"><Icon name="flask" size={15} /></span>
    <h2 class="grow text-lg font-semibold">{t("trial.heading")}</h2>
    {#if trial.active}
      <button class="btn btn-ghost btn-sm" onclick={() => trial.clear()}>{t("trial.clear")}</button>
    {/if}
    <button class="btn btn-ghost btn-sm btn-square" aria-label={t("trial.close")} title={t("trial.close")} onclick={() => (trial.open = false)}>
      <Icon name="x" size={17} />
    </button>
  </header>

  <div class="min-h-0 grow overflow-y-auto px-4 py-4">
    <p class="mb-3 text-sm text-base-content/70">{t("trial.intro")}</p>
    <div class="grid gap-2">
      {#each fields as f (f.key)}
        <label class="grid grid-cols-[4.5rem_minmax(0,1fr)] items-center gap-2">
          <span class="text-sm font-medium text-base-content/75">{f.label}</span>
          <input class="input input-sm w-full text-sm" placeholder={f.placeholder} bind:value={trial.mail[f.key]} {onpaste} />
        </label>
      {/each}
    </div>
    <button class="mt-2 flex items-center gap-1 text-[0.8125rem] font-medium text-base-content/70 hover:text-base-content" aria-expanded={more} onclick={() => (more = !more)}>
      <Icon name={more ? "chevronDown" : "chevronRight"} size={14} />{t("trial.more")}
    </button>
    {#if more}
      <div class="mt-2 grid gap-2">
        <label class="grid gap-1">
          <span class="text-[0.8125rem] text-base-content/70">{t("trial.headers")}</span>
          <textarea class="textarea textarea-sm min-h-16 w-full font-mono text-[0.8125rem]" placeholder="X-Spam-Flag: YES" bind:value={trial.mail.headers} {onpaste}></textarea>
        </label>
        <label class="grid gap-1">
          <span class="text-[0.8125rem] text-base-content/70">{t("trial.body")}</span>
          <textarea class="textarea textarea-sm min-h-16 w-full" bind:value={trial.mail.body} {onpaste}></textarea>
        </label>
      </div>
    {/if}
    {#if !trial.active}
      <p class="mt-3 flex gap-2 text-[0.8125rem] text-base-content/70"><Icon name="info" size={14} class="mt-0.5" />{t("trial.pasteTip")}</p>
    {/if}

    {#if result}
      {#if !running}
        <p class="mt-4 flex items-start gap-2 rounded-field bg-base-200 px-3 py-2 text-sm text-base-content/75">
          <Icon name="info" size={15} class="mt-0.5 text-primary" />{t("trial.notRunning")}
        </p>
      {/if}

      <!-- Where it ends up. -->
      <section class="mt-4 rounded-box border border-success/35 bg-success/[0.07] px-3.5 py-3" aria-live="polite">
        <h3 class="mb-1.5 text-[0.8125rem] font-semibold tracking-wide text-success uppercase">{t("trial.result")}</h3>
        <div class="text-[1.05rem]"><Outcomes items={delivery} fullPath /></div>
        {#if result.delivery.folders.length + (result.delivery.inbox ? 1 : 0) > 1}
          <p class="mt-1 text-[0.8125rem] text-base-content/80">{t("trial.copies")}</p>
        {/if}
        {#if changed && trial.before}
          <!-- Unsaved edits change where this message goes: before and after. -->
          <div class="mt-2 rounded-field bg-base-100/70 px-2.5 py-1.5 text-[0.875rem]">
            <span class="text-[0.8125rem] font-medium text-base-content/75">{t("trial.before")}</span>
            <span class="line-through decoration-base-content/40 opacity-80"><Outcomes items={deliveryOf(trial.before)} fullPath /></span>
          </div>
        {/if}
        {#if contributions.length > 1}
          <!-- Several scripts had a hand in it: which did what. -->
          <ul class="mt-2 grid gap-0.5 text-[0.8125rem] text-base-content/75">
            {#each contributions as c (c.script)}
              <li><span class="font-medium">{c.script}:</span> {c.items.join(", ")}</li>
            {/each}
          </ul>
        {/if}
        {#if result.delivery.forwards.length || result.delivery.rejected || result.delivery.discarded || result.delivery.replied}
          <ul class="mt-1.5 grid gap-0.5 text-sm text-base-content/80">
            {#each result.delivery.forwards as address}<li class="flex items-center gap-1.5"><Icon name="forward" size={14} />{t("trial.forwarded", { address })}</li>{/each}
            {#if result.delivery.replied}<li class="flex items-center gap-1.5"><Icon name="reply" size={14} />{t("trial.replied")}</li>{/if}
            {#if result.delivery.rejected}<li class="flex items-center gap-1.5"><Icon name="ban" size={14} />{t("trial.rejected")}</li>{/if}
            {#if result.delivery.discarded}<li class="flex items-center gap-1.5"><Icon name="trash" size={14} />{t("trial.discarded")}</li>{/if}
          </ul>
        {/if}
        {#if result.uncertain}
          <p class="mt-2 flex items-start gap-1.5 text-[0.8125rem] text-base-content/75">
            <Icon name="alert" size={14} class="mt-0.5 text-warning" />{t("trial.uncertain")}
          </p>
        {/if}
      </section>

      <!-- Its way: every script in order, the cases that applied. -->
      <h3 class="mt-5 mb-2 text-[0.8125rem] font-semibold tracking-wide text-base-content/70 uppercase">{t("trial.way")}</h3>
      <ol class="grid gap-3">
        {#each result.scripts as run, i (run.name)}
          {@const steps = result.steps.filter((s) => s.script === run.name)}
          <li class="grid grid-cols-[1.5rem_minmax(0,1fr)] gap-x-2">
            <span class="grid size-6 place-items-center rounded-md bg-primary/12 text-[0.8125rem] font-bold text-primary">{i + 1}</span>
            <p class={["pt-0.5 font-semibold", run.name === editor.name && "text-primary"]}>{run.name}</p>
            <span></span>
            <div class="grid gap-1 pt-1">
              {#if run.skipped}
                <p class="text-sm text-base-content/70">{t("trial.skipped")}</p>
              {:else if steps.length === 0 && run.unknown.length === 0}
                <p class="text-sm text-base-content/70">{t("trial.noMatch")}</p>
              {/if}
              {#each steps as step}
                {@const title = stepTitle(step.script, step.index, step.path)}
                {@const items = outcomes(step.actions, keywords, delimiter)}
                <button
                  class="grid gap-1 rounded-field border-l-2 border-success bg-base-200/50 px-2.5 py-1.5 text-left hover:bg-base-200"
                  onclick={() => openStep(step.script, step.index, step.path)}
                >
                  <span class="flex min-w-0 items-baseline gap-1.5 text-sm">
                    <Icon name="check" size={14} class="self-center text-success" />
                    <span class="truncate font-medium">{title.name}</span>
                    {#if title.where}<span class="shrink-0 text-base-content/70">› {title.where}</span>{/if}
                  </span>
                  {#if title.conditions && title.conditions.items.length}
                    <span class="pl-5 text-[0.875rem]" title={t("trial.because")}>
                      <span class="text-[0.8125rem] text-base-content/70">{t("trial.because")}</span>
                      <RouteConditions shape={conditionShape(title.conditions, false, 2)} />
                    </span>
                  {/if}
                  {#if items.length || stops(step.actions)}
                    <span class={["pl-5", step.overridden && "line-through decoration-base-content/50 opacity-60"]} title={step.overridden ? t("trial.overridden") : undefined}>
                      <Outcomes {items} stop={stops(step.actions)} singleFolder={editor.rules.singleFolder} next={nextOf(step.script)} />
                    </span>
                    {#if step.overridden}<span class="pl-5 text-[0.8125rem] text-base-content/70">{t("trial.overridden")}</span>{/if}
                  {/if}
                </button>
              {/each}
              {#each run.unknown as index}
                {@const title = stepTitle(run.name, index, "")}
                <button
                  class="flex items-center gap-1.5 rounded-field border-l-2 border-warning bg-warning/[0.07] px-2.5 py-1.5 text-left text-sm"
                  onclick={() => openStep(run.name, index, "")}
                >
                  <Icon name="helpCircle" size={14} class="text-warning" /><span class="font-medium">{title.name}</span>
                  <span class="text-base-content/70">– {t("trial.unknown")}</span>
                </button>
              {/each}
            </div>
          </li>
        {/each}
      </ol>
    {/if}
  </div>
</aside>
