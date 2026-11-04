<script lang="ts">
  // An account's icon: its picture, or its text on its colour. Both get the
  // same hairline frame, so pictures and initials look the same size.
  import type { AccountIcon } from "#tauri/types";
  import { iconHue, iconText } from "../accountIcon";

  let { label, icon = null, size = 28 }: { label: string; icon?: AccountIcon | null; size?: number } = $props();
  const radius = $derived(Math.round(size * 0.28));
  const text = $derived(iconText(label, icon));
  /** An emoji reads at a larger size than letters. */
  const emoji = $derived(/\p{Extended_Pictographic}/u.test(text));
</script>

<span class="relative block shrink-0" style="width: {size}px; height: {size}px; border-radius: {radius}px">
  {#if icon?.image}
    <img src={icon.image} alt="" class="block size-full object-cover" style="border-radius: {radius}px" />
  {:else}
    <span
      class="account-icon grid size-full place-items-center font-bold tracking-tight"
      style="--h: {iconHue(label, icon)}; border-radius: {radius}px; font-size: {Math.round(size * (emoji ? 0.64 : 0.39))}px"
      aria-hidden="true">{text}</span
    >
  {/if}
  <span class="pointer-events-none absolute inset-0 ring-1 ring-base-content/12 ring-inset" style="border-radius: {radius}px"></span>
</span>
