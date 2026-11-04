<script lang="ts">
  // Text with the parts matching the search marked. Addresses may break
  // before "@" and ".", so a narrow column never splits a word mid-way.
  import { marks } from "../../rules/route";
  let { text, words = [], breaks = false }: { text: string; words?: string[]; breaks?: boolean } = $props();

  const pieces = (s: string) => (breaks ? s.split(/(?=[@.])/) : [s]);
</script>

{#each marks(text, words) as part}{#if part.hit}<mark class="rounded-sm bg-warning/30 text-inherit"
      >{#each pieces(part.text) as piece, i}{#if i}<wbr />{/if}{piece}{/each}</mark
    >{:else}{#each pieces(part.text) as piece, i}{#if i}<wbr />{/if}{piece}{/each}{/if}{/each}
