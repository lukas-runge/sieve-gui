<script lang="ts">
  // Text with the parts matching the search marked.
  import { marks } from "../../rules/route";
  let { text, words = [] }: { text: string; words?: string[] } = $props();
</script>

{#each marks(text, words) as part}{#if part.hit}<mark class="rounded-sm bg-warning/30 text-inherit">{part.text}</mark>{:else}{part.text}{/if}{/each}
