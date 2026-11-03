<script lang="ts">
  import type { Condition } from "#tauri/types";
  import { print } from "../rules/describe";
  import { t, type MessageKey } from "../i18n.svelte";
  import {
    buildCondition,
    fieldsFor,
    hasValues,
    operatorsFor,
    partsFor,
    viewCondition,
    withField,
    withPart,
    type ConditionView,
    type Field,
    type Operator,
    type Part,
  } from "../rules/view";
  import { SIZE_UNITS, splitSize } from "../rules/format";
  import ValueList from "./ValueList.svelte";
  import { checkRegex, regexMatches, type RegexProblem } from "../rules/regex";

  let {
    condition,
    extensions,
    onchange,
    onremove,
  }: {
    condition: Condition;
    extensions: string[];
    onchange: (c: Condition, merge: boolean) => void;
    onremove: () => void;
  } = $props();

  const view = $derived(viewCondition(condition));
  const size = $derived(view?.field === "size" ? splitSize(Number(view.values[0])) : null);
  // "exists" asks for the header, not a part of its addresses.
  const parts = $derived(view && hasValues(view.operator) ? partsFor(view.field, extensions, view.part) : []);

  const regex = $derived(view?.operator === "regex" || view?.operator === "notRegex");
  // The current operator stays selectable, e.g. a regex on a server without `regex`.
  const operators = $derived(
    view ? [...new Set([...operatorsFor(view.field, view.part, extensions), view.operator])] : [],
  );
  const regexUnsupported = $derived(regex && extensions.length > 0 && !extensions.includes("regex"));
  let sample = $state("");
  /** Whether the condition holds for the sample, or null if an expression is invalid. */
  const sampleResult = $derived.by(() => {
    if (!view || !regex || sample === "") return undefined;
    const matched = regexMatches(view.values, sample);
    return matched === null ? null : matched !== (view.operator === "notRegex");
  });

  function problemText(p: RegexProblem): string {
    switch (p.kind) {
      case "perlClass":
        return t("regex.perlClass", { escape: p.escape, instead: p.instead });
      case "groupSyntax":
        return t("regex.groupSyntax");
      case "lazy":
        return t("regex.lazy");
      case "escape":
        return t("regex.escape", { escape: p.escape });
      case "unknownClass":
        return t("regex.unknownClass", { name: p.name });
      case "syntax":
        return t("regex.syntax", { message: p.message });
    }
  }

  function samplePlaceholder(v: ConditionView): string {
    if (v.part === "domain") return "example.org";
    if (v.part === "localPart" || v.part === "user" || v.part === "detail") return "name";
    if (v.part === "all") return "name@example.org";
    return t("regex.sampleValue");
  }

  function update(patch: Partial<ConditionView>, merge = false) {
    if (!view) return;
    onchange(buildCondition({ ...view, ...patch }), merge);
  }
</script>

<div class="flex flex-wrap items-start gap-2 rounded-box bg-base-200/60 p-2">
  {#if view}
    <select
      class="select select-sm w-auto"
      aria-label={t("conditions.heading")}
      value={view.field}
      onchange={(e) => view && onchange(buildCondition(withField(view, e.currentTarget.value as Field)), false)}
    >
      {#each fieldsFor(extensions) as field}
        <option value={field}>{t(`field.${field}` as MessageKey)}</option>
      {/each}
    </select>
    {#if view.field === "header"}
      <input
        class="input input-sm w-56 font-mono"
        aria-label={t("field.headerName")}
        placeholder={t("field.headerName")}
        title={t("field.headerNameHint")}
        value={view.header}
        oninput={(e) => update({ header: e.currentTarget.value }, true)}
      />
    {/if}
    {#if parts.length > 0}
      <select
        class="select select-sm w-auto"
        aria-label={t("part.label")}
        value={view.part}
        onchange={(e) => view && onchange(buildCondition(withPart(view, e.currentTarget.value as Part)), false)}
      >
        {#each parts as part}
          <option value={part}>{t(`part.${part}` as MessageKey)}</option>
        {/each}
      </select>
    {/if}
    <select
      class="select select-sm w-auto"
      aria-label="Operator"
      value={view.operator}
      onchange={(e) => update({ operator: e.currentTarget.value as Operator })}
    >
      {#each operators as op}
        <option value={op}>{t(`op.${op}` as MessageKey)}</option>
      {/each}
    </select>
    {#if view.field === "size" && size}
      <div class="join">
        <input
          type="number"
          min="0"
          class="input input-sm join-item w-24"
          aria-label={t("field.size")}
          value={size.amount}
          oninput={(e) => update({ values: [String(Number(e.currentTarget.value) * size.factor)] }, true)}
        />
        <select
          class="select select-sm join-item w-auto"
          aria-label="Unit"
          value={size.factor}
          onchange={(e) => update({ values: [String(size.amount * Number(e.currentTarget.value))] })}
        >
          {#each SIZE_UNITS as unit}
            <option value={unit.factor}>{unit.label}</option>
          {/each}
        </select>
      </div>
    {:else if view.field === "date"}
      {@const value = view.values[0] ?? ""}
      {@const withTime = value.includes("T")}
      <input
        type={withTime ? "datetime-local" : "date"}
        class="input input-sm w-auto"
        aria-label={t("field.date")}
        {value}
        onchange={(e) => update({ values: [e.currentTarget.value], zone: null })}
      />
      <label class="flex items-center gap-1.5 text-sm">
        <input
          type="checkbox"
          class="checkbox checkbox-sm"
          checked={withTime}
          onchange={(e) => update({ values: [e.currentTarget.checked ? `${value.slice(0, 10)}T00:00` : value.slice(0, 10)] })}
        />
        {t("field.withTime")}
      </label>
    {:else if hasValues(view.operator)}
      <div class="grow min-w-48">
        <ValueList
          values={view.values.length ? view.values : [""]}
          label={t(`op.${view.operator}` as MessageKey)}
          onchange={(values, merge) => update({ values }, merge)}
        />
        {#if regex}
          <!-- POSIX ERE as the server evaluates it, see rules/regex.ts. -->
          {#each view.values as value, i}
            {@const problem = value ? checkRegex(value) : null}
            {#if problem}
              <p class="mt-1 text-xs text-error" role="alert">
                {view.values.length > 1 ? t("regex.which", { n: i + 1 }) : ""}{problemText(problem)}
              </p>
            {/if}
          {/each}
          {#if regexUnsupported}
            <p class="mt-1 text-xs text-warning" role="alert">{t("regex.unsupported")}</p>
          {/if}
          <p class="mt-1 text-xs text-base-content/60">{t("op.regexHint")}</p>
          <label class="mt-2 flex flex-wrap items-center gap-2 text-xs">
            <span class="text-base-content/70">{t("regex.try")}</span>
            <input
              class="input input-xs min-w-48 grow font-mono"
              placeholder={samplePlaceholder(view)}
              bind:value={sample}
              spellcheck="false"
              autocapitalize="off"
            />
            {#if sampleResult === true}
              <span class="badge badge-sm badge-success">{t("regex.holds")}</span>
            {:else if sampleResult === false}
              <span class="badge badge-sm badge-ghost">{t("regex.doesNotHold")}</span>
            {/if}
          </label>
        {:else if view.operator.toLowerCase().includes("matches")}
          <p class="mt-1 text-xs text-base-content/60">{t("op.patternHint")}</p>
        {:else if view.operator.endsWith("WithSubdomains")}
          <p class="mt-1 text-xs text-base-content/60">{t("op.subdomainHint")}</p>
        {:else if view.field === "parts"}
          <p class="mt-1 text-xs text-base-content/60">{t("op.typeHint")}</p>
        {/if}
      </div>
    {/if}
  {:else}
    <div class="grow">
      <code class="block font-mono text-sm break-all">{print(condition)}</code>
      <p class="mt-1 text-xs text-base-content/60">{t("conditions.advanced")}</p>
    </div>
  {/if}
  <button
    class="btn btn-ghost btn-sm btn-square ml-auto"
    aria-label={t("conditions.remove")}
    title={t("conditions.remove")}
    onclick={onremove}>✕</button
  >
</div>
