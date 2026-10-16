<script lang="ts">
  import { onMount } from "svelte";
  import { getAppInfo } from "tauri/commands";
  import type { AppInfo } from "tauri/types";

  let info = $state<AppInfo | null>(null);

  onMount(async () => {
    info = await getAppInfo();
  });
</script>

<main class="flex min-h-screen flex-col items-center justify-center gap-2">
  <h1 class="text-3xl font-bold">sieve-gui</h1>
  {#if info}
    <p class="text-base-content/60">v{info.version}</p>
  {/if}
</main>
