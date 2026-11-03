<script lang="ts">
  // A SHA-256 fingerprint as "AB:CD:…", wrapping only after a colon.
  import { formatFingerprint } from "../certificate";

  let { sha256, class: className = "" }: { sha256: string; class?: string } = $props();
  const pairs = $derived(formatFingerprint(sha256).split(":"));
</script>

<code class="font-mono text-xs leading-5 {className}">{#each pairs as pair, i}{pair}{#if i < pairs.length - 1}:<wbr />{/if}{/each}</code>
