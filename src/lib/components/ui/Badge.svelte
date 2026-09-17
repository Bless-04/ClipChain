<script lang="ts">
  import type { Snippet } from 'svelte';
  let { children, tone = 'neutral' }: { children: Snippet; tone?: 'neutral' | 'green' | 'amber' } = $props();
</script>
<span class="badge {tone}">{@render children()}</span>
