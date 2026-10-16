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
  const parts = $derived(view ? partsFor(view.field, extensions, view.part) : []);

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
        class="input input-sm w-40 font-mono"
        aria-label={t("field.headerName")}
        placeholder={t("field.headerName")}
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
      {#each operatorsFor(view.field, view.part, extensions) as op}
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
        {#if view.operator.toLowerCase().includes("matches")}
          <p class="mt-1 text-xs text-base-content/60">{t("op.patternHint")}</p>
        {:else if view.operator.endsWith("WithSubdomains")}
          <p class="mt-1 text-xs text-base-content/60">{t("op.subdomainHint")}</p>
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
