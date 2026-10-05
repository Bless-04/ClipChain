<script lang="ts">
  import type { Snippet } from 'svelte';
  let { children, onclick, disabled = false, busy = false, variant = 'secondary', type = 'button', label }: {
    children: Snippet; onclick?: () => void; disabled?: boolean; busy?: boolean;
    variant?: 'primary' | 'secondary' | 'danger' | 'quiet'; type?: 'button' | 'submit'; label?: string;
  } = $props();
</script>
<button {type} {onclick} disabled={disabled || busy} aria-busy={busy} aria-label={label} class="button {variant}">{@render children()}</button>
