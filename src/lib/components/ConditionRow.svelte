<script lang="ts">
  import type { Condition } from "#tauri/types";
  import { print } from "../rules/describe";
  import { t, type MessageKey } from "../i18n.svelte";
  import {
    buildCondition,
    fieldsFor,
    hasValues,
    isCompare,
    offersPart,
    operatorsFor,
    otherFieldsFor,
    partsFor,
    viewCondition,
    withField,
    withOther,
    withPart,
    type ConditionView,
    type Field,
    type Operator,
    type Part,
  } from "../rules/view";
  import { SIZE_UNITS, splitSize } from "../rules/format";
  import ValueList from "./ValueList.svelte";
  import Token from "./route/Token.svelte";
  import { conditionToken } from "../rules/route";
  import Icon from "./Icon.svelte";
  import { checkRegex, regexMatches, type RegexProblem } from "../rules/regex";
  import { mailboxLabel, mailboxRaw, type ScriptFolders } from "../rules/mailbox";
  import FolderPicker from "./FolderPicker.svelte";

  let {
    condition,
    extensions,
    folders,
    singleFolder,
    compact = false,
    onchange,
    onremove,
  }: {
    condition: Condition;
    extensions: string[];
    folders: ScriptFolders;
    /** Single-folder mode: the folder chosen so far can be tested. */
    singleFolder: boolean;
    /** One of several: shows as a line to read until it's opened for editing. */
    compact?: boolean;
    onchange: (c: Condition, merge: boolean) => void;
    onremove: () => void;
  } = $props();

  /** The operator chosen here last: an empty "starts with" is written as `*` and would read back as a pattern. */
  let picked = $state<Operator | undefined>(undefined);
  const view = $derived(viewCondition(condition, picked));
  /** Editing this condition (a compact one opens on click; empty ones start open). */
  let editing = $state(false);
  const open = $derived(!compact || editing || !view || (hasValues(view.operator) && view.values.every((v) => v === "")));
  const size = $derived(view?.field === "size" ? splitSize(Number(view.values[0])) : null);
  const parts = $derived(view && offersPart(view) ? partsFor(view.field, extensions, view.part) : []);
  // The current field stays selectable, e.g. a variable on a server without `variables`.
  const fields = $derived(view ? [...new Set([...fieldsFor(extensions, singleFolder), view.field])] : []);

  const regex = $derived(view?.operator === "regex" || view?.operator === "notRegex");
  // The current operator stays selectable, e.g. a regex on a server without `regex`.
  const operators = $derived(
    view ? [...new Set([...operatorsFor(view.field, view.part, extensions), view.operator])] : [],
  );
  const regexUnsupported = $derived(regex && extensions.length > 0 && !extensions.includes("regex"));
  let sample = $state("");
  let regexHelp = $state(false);
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

<!-- One sentence: [field] [compare] [values], the remove button at its end. -->
<div class="grid grid-cols-[minmax(0,1fr)_auto] items-start gap-x-1 rounded-field bg-base-200/70 p-2">
  <div class="flex min-w-0 flex-wrap items-start gap-2">
  {#if !open}
    <!-- Read like on the route; click to change. -->
    <button class="flex min-w-0 grow items-start gap-2 rounded-field px-1 py-1 text-left hover:bg-base-100" title={t("conditions.edit")} onclick={() => (editing = true)}>
      <span class="min-w-0 grow"><Token token={conditionToken(condition, Infinity)} /></span>
      <Icon name="pencil" size={14} class="mt-1 text-base-content/65" />
    </button>
  {:else if view}
    <select
      class="select select-sm w-auto"
      aria-label={t("conditions.heading")}
      value={view.field}
      onchange={(e) => view && onchange(buildCondition(withField(view, e.currentTarget.value as Field)), false)}
    >
      {#each fields as field}
        <option value={field}>{t(`field.${field}` as MessageKey)}</option>
      {/each}
    </select>
    {#if view.field === "variable"}
      <input
        class="input input-sm w-40 font-mono"
        aria-label={t("field.variableName")}
        placeholder={t("field.variableName")}
        title={t("field.variableHint")}
        spellcheck="false"
        autocapitalize="off"
        value={view.header}
        oninput={(e) => update({ header: e.currentTarget.value.replace(/[^A-Za-z0-9_]/g, "") }, true)}
      />
    {/if}
    {#if view.field === "header"}
      <input
        class="input input-sm w-56"
        aria-label={t("field.headerName")}
        placeholder={t("field.headerName")}
        title={t("field.headerNameHint")}
        value={view.header}
        oninput={(e) => update({ header: e.currentTarget.value }, true)}
      />
    {/if}
    {#if parts.length > 0}
      <!-- Which part of the addresses: the whole line, the address, the domain … -->
      <select
        class="select select-sm w-auto"
        aria-label={t("part.label")}
        title={t("part.hint")}
        value={view.part}
        onchange={(e) => view && onchange(buildCondition(withPart(view, e.currentTarget.value as Part)), false)}
      >
        {#each parts as part}
          <option value={part}>{t(part === "header" && view.field === "header" ? "part.wholeHeader" : (`part.${part}` as MessageKey))}</option>
        {/each}
      </select>
    {/if}
    <select
      class="select select-sm w-auto"
      aria-label="Operator"
      value={view.operator}
      onchange={(e) => {
        picked = e.currentTarget.value as Operator;
        update({ operator: picked });
      }}
    >
      {#each operators as op}
        <option value={op}>{t(`op.${op}` as MessageKey)}</option>
      {/each}
    </select>
    {#if isCompare(view.operator)}
      {@const other = view.other ?? { field: "from", header: "" }}
      <!-- The field compared with; the address part applies to both. -->
      <select
        class="select select-sm w-auto"
        aria-label={t("field.compareWith")}
        value={other.field}
        onchange={(e) => view && onchange(buildCondition(withOther(view, { field: e.currentTarget.value as Field, header: "" })), false)}
      >
        {#each otherFieldsFor(view) as field}
          <option value={field}>{t(`field.${field}` as MessageKey)}</option>
        {/each}
      </select>
      {#if other.field === "header"}
        <input
          class="input input-sm w-56"
          aria-label={t("field.headerName")}
          placeholder={t("field.headerName")}
          title={t("field.headerNameHint")}
          value={other.header}
          oninput={(e) => update({ other: { field: "header", header: e.currentTarget.value } }, true)}
        />
      {/if}
      <p class="basis-full text-[0.8rem] text-base-content/70">{t(view.operator === "sameAs" ? "op.sameAsHint" : "op.differsFromHint")}</p>
    {:else if view.field === "folder" && view.values.length <= 1}
      <div class="grow min-w-48">
        <FolderPicker value={mailboxLabel(view.values[0] ?? "")} {folders} onpick={(name) => update({ values: [mailboxRaw(name, folders.encoding)] })} />
        <p class="mt-1 text-[0.8rem] text-base-content/70">{t("field.folderHint")}</p>
      </div>
    {:else if view.field === "size" && size}
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
      <div class="grow min-w-40">
        <ValueList
          values={view.values.length ? view.values : [""]}
          label={t(`op.${view.operator}` as MessageKey)}
          mono={regex || view.operator === "matches" || view.operator === "notMatches"}
          placeholder={view.part !== "header" ? samplePlaceholder(view) : undefined}
          onchange={(values, merge) => update({ values }, merge)}
        />
        {#if regex}
          <!-- POSIX ERE as the server evaluates it, see rules/regex.ts. -->
          {#each view.values as value, i}
            {@const problem = value ? checkRegex(value) : null}
            {#if problem}
              <p class="mt-1 text-[0.8rem] text-error" role="alert">
                {view.values.length > 1 ? t("regex.which", { n: i + 1 }) : ""}{problemText(problem)}
              </p>
            {/if}
          {/each}
          {#if regexUnsupported}
            <p class="mt-1 text-[0.8rem] text-warning" role="alert">{t("regex.unsupported")}</p>
          {/if}
          <!-- Help and trying it out, on request. -->
          <button class="mt-1 text-[0.8125rem] text-base-content/70 hover:text-base-content hover:underline" aria-expanded={regexHelp} onclick={() => (regexHelp = !regexHelp)}>
            {regexHelp ? t("regex.hideHelp") : t("regex.help")}
          </button>
          {#if regexHelp}
          <p class="mt-1 text-[0.8rem] text-base-content/70">{t("op.regexHint")}</p>
          <label class="mt-2 flex flex-wrap items-center gap-2 text-[0.8rem]">
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
          {/if}
        {:else if view.operator.toLowerCase().includes("matches")}
          <p class="mt-1 text-[0.8rem] text-base-content/70">{t("op.patternHint")}</p>
        {:else if view.operator.endsWith("WithSubdomains")}
          <p class="mt-1 text-[0.8rem] text-base-content/70">{t("op.subdomainHint")}</p>
        {:else if view.field === "parts"}
          <p class="mt-1 text-[0.8rem] text-base-content/70">{t("op.typeHint")}</p>
        {/if}
      </div>
    {/if}
  {:else}
    <div class="grow">
      <code class="block font-mono text-sm break-all">{print(condition)}</code>
      <p class="mt-1 text-[0.8rem] text-base-content/70">{t("conditions.advanced")}</p>
    </div>
  {/if}
  </div>
  <button
    class="btn btn-ghost btn-sm btn-square text-base-content/70"
    aria-label={t("conditions.remove")}
    title={t("conditions.remove")}
    onclick={onremove}><Icon name="trash" size={15} /></button
  >
</div>
