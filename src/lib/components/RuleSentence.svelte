<script lang="ts">
  // A rule read as a sentence: "If Sender contains @tax… then move to Taxes".
  import type { Action, Condition, Join, Rule } from "#tauri/types";
  import { i18n, t, type MessageKey } from "../i18n.svelte";
  import { actionKind, hasValues, headerNames, viewCondition, type Branches } from "../rules/view";
  import { formatDate, formatSize } from "../rules/format";
  import { mailboxLabel } from "../rules/mailbox";
  import { flagLabel } from "../keywords";

  let {
    rule,
    singleFolder,
    keywords = {},
  }: { rule: Rule; singleFolder: boolean; keywords?: Record<string, string> } = $props();

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
        return kind === "markRead" || kind === "flag" ? null : a.flags.map((f) => flagLabel(f, keywords)).join(" ");
      }
      default:
        return null;
    }
  }
</script>

{#snippet conditions(items: Condition[], join: Join, nested: boolean)}
  <!-- Nested groups are put in parentheses: "A or (B and C)". -->
  <!-- Line breaks between these tags would render as spaces, e.g. after "(". -->
  {#if nested}{" "}<span class="text-base-content/60">(</span>{/if}{#each items as condition, i}{#if i > 0}{" "}<span
        class="text-base-content/60">{t(join === "any" ? "sentence.or" : "sentence.and")}</span
      >{/if}{#if i > 0 || !nested}{" "}{/if}{#if condition.test.type === "group"}{#if condition.negate}<span
          >{t("sentence.not")}</span
        >{/if}{@render conditions(condition.test.items, condition.test.join, true)}{:else}{@render single(
        condition,
      )}{/if}{/each}{#if nested}<span class="text-base-content/60">)</span>{/if}
{/snippet}

{#snippet single(condition: Condition)}
  {@const view = viewCondition(condition)}
  {#if view}<span class="slot"
      >{view.field === "header"
        ? headerNames(view.header).join(` ${t("sentence.or")} `)
        : t(`field.${view.field}` as MessageKey)}{#if view.part !== "header"}{" "}({t(
          `part.${view.part}` as MessageKey,
        )}){/if}</span
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
{/snippet}

<!-- Spaces are explicit ({" "}) so they survive between inline elements. -->
{#snippet actionList(actions: Action[])}
  {#if actions.length === 0}
    {" "}<span>{t("sentence.noActions")}</span>
  {/if}
  {#each actions as action, i}
    {#if action.type === "if"}
      <!-- "Also check" reads as its own indented sentence. -->
      <div class="my-1 ml-1 border-l-2 border-base-300 pl-3">{@render branches(action, true)}</div>
    {:else}
      {@const param = actionParam(action)}
      {@const kind = actionKind(action)}
      {#if i > 0 && actions[i - 1].type !== "if"}<span>,</span>{/if}
      {" "}<span>{t(kind === "stop" && singleFolder ? "sentence.stopScript" : (`sentence.${kind}` as MessageKey))}</span>
      {#if param !== null}
        {" "}<span class="slot slot-code">{param || "…"}</span>
      {/if}
    {/if}
  {/each}
{/snippet}

{#snippet branches(block: Branches, nested: boolean)}
  <div>
    {#if block.conditions.items.length === 0 && !nested}
      <span>{t("sentence.allMessages")}</span>
    {:else}
      <span class="font-semibold">{t(nested ? "sentence.alsoIf" : "sentence.if")}</span>
      {@render conditions(block.conditions.items, block.conditions.join, false)}
    {/if}
    {" "}<span class="font-semibold">{t("sentence.then")}</span>
    {@render actionList(block.actions)}
  </div>
  {#each block.elsif as branch}
    <div class="mt-1">
      <span class="font-semibold">{t("sentence.elsif")}</span>
      {@render conditions(branch.conditions.items, branch.conditions.join, false)}
      {" "}<span class="font-semibold">{t("sentence.then")}</span>
      {@render actionList(branch.actions)}
    </div>
  {/each}
  {#if block.otherwise}
    <div class="mt-1">
      <span class="font-semibold">{t("sentence.else")}</span>
      {@render actionList(block.otherwise)}
    </div>
  {/if}
{/snippet}

<div class="leading-relaxed text-[0.95rem]">
  {@render branches(rule, false)}
</div>
