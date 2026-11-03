<script lang="ts">
  // Conditions and actions of a rule or of a nested `if` ("also check"),
  // with its "otherwise if" (`elsif`) and "otherwise" (`else`) branches.
  // Nested ifs among the actions use this component again, to any depth.
  import type { Action } from "#tauri/types";
  import { t } from "../i18n.svelte";
  import type { ScriptFolders } from "../rules/mailbox";
  import { buildAction, newBranch, type Branches, type ConditionGroup as Group } from "../rules/view";
  import ActionRow from "./ActionRow.svelte";
  import ConditionGroup from "./ConditionGroup.svelte";

  let {
    block,
    extensions,
    folders,
    singleFolder,
    mutate,
  }: {
    block: Branches;
    extensions: string[];
    folders: ScriptFolders;
    singleFolder: boolean;
    /** Applies an edit (with an undo step) to this block. */
    mutate: (fn: (b: Branches) => void, merge?: boolean) => void;
  } = $props();

  type Where = { branch: "if" } | { branch: "elsif"; index: number } | { branch: "else" };

  function actionsOf(b: Branches, where: Where): Action[] {
    if (where.branch === "if") return b.actions;
    if (where.branch === "elsif") return b.elsif[where.index].actions;
    return (b.otherwise ??= []);
  }

  function conditionsMutate(get: (b: Branches) => Group) {
    return (fn: (group: Group, holder: null) => void, merge?: boolean) => mutate((b) => fn(get(b), null), merge);
  }
</script>

{#snippet actions(list: Action[], where: Where)}
  <div class="grid gap-2">
    {#each list as action, i}
      <ActionRow
        {action}
        {extensions}
        {folders}
        {singleFolder}
        onchange={(a, merge) => mutate((b) => (actionsOf(b, where)[i] = a), merge)}
        onremove={() => mutate((b) => actionsOf(b, where).splice(i, 1))}
      />
    {/each}
    <button
      class="btn btn-ghost btn-sm justify-self-start w-fit"
      onclick={() => mutate((b) => actionsOf(b, where).push(buildAction("move")))}
    >
      + {t("actions.add")}
    </button>
  </div>
{/snippet}

<div class="grid gap-3">
  <ConditionGroup group={block.conditions} {extensions} mutate={conditionsMutate((b) => b.conditions)} />
  <section class="grid gap-2" aria-label={t("actions.heading")}>
    <h4 class="font-semibold">{t("actions.heading")}</h4>
    {@render actions(block.actions, { branch: "if" })}
  </section>

  {#each block.elsif as branch, index}
    <div class="grid gap-3 border-t border-base-300 pt-3">
      <div class="flex items-start gap-2">
        <div class="grow">
          <ConditionGroup
            group={branch.conditions}
            heading={t("branch.elsif")}
            {extensions}
            mutate={conditionsMutate((b) => b.elsif[index].conditions)}
          />
        </div>
        <button
          class="btn btn-ghost btn-sm btn-square"
          aria-label={t("branch.remove")}
          title={t("branch.remove")}
          onclick={() => mutate((b) => b.elsif.splice(index, 1))}
        >
          ×
        </button>
      </div>
      <section class="grid gap-2" aria-label={t("actions.heading")}>
        <h4 class="font-semibold">{t("actions.heading")}</h4>
        {@render actions(branch.actions, { branch: "elsif", index })}
      </section>
    </div>
  {/each}

  {#if block.otherwise}
    <section class="grid gap-2 border-t border-base-300 pt-3" aria-label={t("branch.else")}>
      <div class="flex items-center gap-2">
        <h4 class="grow font-semibold">{t("branch.else")}</h4>
        <button
          class="btn btn-ghost btn-sm btn-square"
          aria-label={t("branch.remove")}
          title={t("branch.remove")}
          onclick={() => mutate((b) => (b.otherwise = null))}
        >
          ×
        </button>
      </div>
      {@render actions(block.otherwise, { branch: "else" })}
    </section>
  {/if}

  <div class="flex flex-wrap gap-2">
    <button class="btn btn-ghost btn-sm" onclick={() => mutate((b) => b.elsif.push(newBranch()))}>
      + {t("branch.addElsif")}
    </button>
    {#if !block.otherwise}
      <button class="btn btn-ghost btn-sm" onclick={() => mutate((b) => (b.otherwise = [buildAction("move")]))}>
        + {t("branch.addElse")}
      </button>
    {/if}
  </div>
</div>
