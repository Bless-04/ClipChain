<script lang="ts">
  import type { HistoryState } from '$lib/state/history.svelte';
  import { UI } from '$lib/constants/ui';
  import Button from './ui/Button.svelte';
  import Modal from './ui/Modal.svelte';
  import LoadError from './ui/LoadError.svelte';
  import HistoryItem from './HistoryItem.svelte';
  let { state: history, available, recording }: { state: HistoryState; available: boolean; recording: boolean | undefined } = $props();
  let confirming = $state(false), page = $state(0);
  const lastPage = $derived(Math.max(0, Math.ceil(history.entries.length / UI.HISTORY_PAGE_SIZE) - 1));
  const currentPage = $derived(Math.min(page, lastPage));
  const entries = $derived(history.entries.slice(currentPage * UI.HISTORY_PAGE_SIZE, (currentPage + 1) * UI.HISTORY_PAGE_SIZE));
  async function clear() { const result = await history.clear(); if (result !== undefined) { confirming = false; page = 0; } }
</script>
<div class="page-heading"><div><p class="eyebrow">YOUR CLIPBOARD, IN CONTEXT</p><h1>History</h1><p class="muted">See what changed, one copy at a time.</p></div><Button variant="danger" onclick={() => { confirming = true; }} disabled={!available || !history.entries.length || history.saving}>Clear history</Button></div>
{#if recording === false}<p class="notice">History recording is off. Existing entries remain here; new transformations are not recorded.</p>{/if}
<LoadError error={history.error} onretry={() => { void history.load(); }} busy={history.saving || history.loading} />
<section class="panel"><div class="section-heading"><h2>Recorded transformations</h2><span class="muted">{history.entries.length} / {UI.MAX_HISTORY_ITEMS} retained</span></div>
  {#if history.loading && !history.value}<p role="status">Loading history…</p>{:else if !entries.length}<div class="empty"><span class="empty-symbol">↳</span><h3>No recorded transformations</h3><p>{available ? 'Changed clipboard text will appear here when history recording is enabled.' : 'History is available in the desktop app.'}</p></div>{:else}
    {#each entries as entry (entry.id)}<HistoryItem {entry} />{/each}
    <div class="pagination"><Button disabled={currentPage === 0} onclick={() => { page = currentPage - 1; }}>Previous</Button><span>Page {currentPage + 1} of {lastPage + 1}</span><Button disabled={currentPage === lastPage} onclick={() => { page = currentPage + 1; }}>Next</Button></div>
  {/if}
</section>
{#if confirming}<Modal title="Clear recorded history?" busy={history.saving} onclose={() => { confirming = false; }}><p>This permanently removes all retained before-and-after text from ClipChain. Rules and settings stay saved.</p><p class="muted">This does not securely erase data from disk.</p><LoadError error={history.error} onretry={clear} busy={history.saving} /><footer class="modal-actions"><Button onclick={() => { confirming = false; }} disabled={history.saving}>Cancel</Button><Button variant="danger" onclick={clear} busy={history.saving}>Clear all history</Button></footer></Modal>{/if}
