<script lang="ts">
  import { onMount, type Snippet } from 'svelte';
  import Button from './Button.svelte';
  let { title, children, onclose, busy = false }: { title: string; children: Snippet; onclose: () => void; busy?: boolean } = $props();
  let dialog: HTMLDialogElement;
  const id = $props.id();
  onMount(() => {
    const previous = document.activeElement;
    dialog.showModal();
    return () => { dialog.close(); if (previous instanceof HTMLElement && previous.isConnected) previous.focus(); };
  });
</script>
<dialog bind:this={dialog} aria-labelledby={id} oncancel={(event) => { event.preventDefault(); if (!busy) onclose(); }}>
  <div class="modal-header"><h2 {id}>{title}</h2><Button onclick={onclose} disabled={busy} label="Close dialog" variant="quiet">✕</Button></div>
  {@render children()}
</dialog>
