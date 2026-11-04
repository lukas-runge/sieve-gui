<script lang="ts">
  // A case's conditions: one line, or a list under "any of these" / "all of
  // these", groups nested with their own heading.
  import type { ConditionShape } from "../../rules/route";
  import { t, type MessageKey } from "../../i18n.svelte";
  import Self from "./Conditions.svelte";
  import Token from "./Token.svelte";

  let { shape }: { shape: ConditionShape } = $props();
</script>

{#if shape.kind === "always"}
  <span class="text-[0.9rem] text-base-content/60">{t("route.always")}</span>
{:else if shape.kind === "line"}
  <Token token={shape.token} />
{:else}
  <div class="grid gap-1">
    <span class="text-[0.8rem] text-base-content/55">{t(`route.mode.${shape.mode}` as MessageKey)}</span>
    <ul class="grid gap-1 border-l-2 border-base-300 pl-2.5">
      {#each shape.items as item}
        <li>
          {#if item.kind === "token"}<Token token={item.token} />{:else}<Self shape={item.shape} />{/if}
        </li>
      {/each}
    </ul>
  </div>
{/if}
