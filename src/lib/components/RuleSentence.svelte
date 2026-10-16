<script lang="ts">
  // A rule read as a sentence: "If Sender contains @tax… then move to Taxes".
  import type { Action, Rule } from "#tauri/types";
  import { i18n, t, type MessageKey } from "../i18n.svelte";
  import { actionKind, hasValues, viewCondition } from "../rules/view";
  import { formatDate, formatSize } from "../rules/format";
  import { mailboxLabel } from "../rules/mailbox";

  let { rule }: { rule: Rule } = $props();

  const joinWord = $derived(t(rule.conditions.join === "any" ? "sentence.or" : "sentence.and"));

  function actionParam(a: Action): string | null {
    switch (a.type) {
      case "fileInto":
        return mailboxLabel(a.mailbox);
      case "redirect":
        return a.address;
      case "reject":
        return a.reason;
      case "vacation":
        return a.subject ?? a.reason.split("\n")[0];
      case "flags": {
        const kind = actionKind(a);
        return kind === "markRead" || kind === "flag" ? null : a.flags.join(" ");
      }
      default:
        return null;
    }
  }
</script>

<!-- Spaces are explicit ({" "}) so they survive between inline elements. -->
<p class="leading-relaxed text-[0.95rem]">
  {#if rule.conditions.items.length === 0}
    <span>{t("sentence.allMessages")}</span>
  {:else}
    <span class="font-semibold">{t("sentence.if")}</span>
    {#each rule.conditions.items as condition, i}
      {@const view = viewCondition(condition)}
      {#if i > 0}{" "}<span class="text-base-content/60">{joinWord}</span>{/if}
      {" "}
      {#if view}
        <span class="slot">{view.field === "header" ? view.header : t(`field.${view.field}` as MessageKey)}</span
        >{" "}<span>{t(`op.${view.operator}` as MessageKey)}</span>
        {#if hasValues(view.operator)}
          {#each view.values as value, j}
            {#if j > 0}{" "}<span class="text-base-content/60">{t("sentence.or")}</span>{/if}
            {" "}<span class="slot slot-code"
              >{view.field === "size"
                ? formatSize(Number(value))
                : view.field === "date"
                  ? formatDate(value, i18n.locale)
                  : value || "…"}</span
            >
          {/each}
        {/if}
      {:else}
        <span class="slot slot-code">{t("field.other")}</span>
      {/if}
    {/each}
  {/if}
  {" "}<span class="font-semibold">{t("sentence.then")}</span>
  {#if rule.actions.length === 0}
    {" "}<span>{t("sentence.noActions")}</span>
  {/if}
  {#each rule.actions as action, i}
    {@const param = actionParam(action)}
    {#if i > 0}<span>,</span>{/if}
    {" "}<span>{t(`sentence.${actionKind(action)}` as MessageKey)}</span>
    {#if param !== null}
      {" "}<span class="slot slot-code">{param || "…"}</span>
    {/if}
  {/each}
</p>
