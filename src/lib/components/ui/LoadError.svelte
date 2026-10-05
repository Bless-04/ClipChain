<script lang="ts">
  import Button from './Button.svelte';
  let { error, onretry, busy = false }: { error: string | null; onretry: () => void; busy?: boolean } = $props();
</script>
{#if error}<div class="error-panel" role="alert"><p>{error}</p><Button onclick={onretry} disabled={busy}>Retry</Button></div>{/if}
