<script lang="ts">
  import type { HistoryEntry } from '$lib/generated/bindings';
  import { textDiff } from '$lib/history/diff';
  import { UI } from '$lib/constants/ui';
  let { entry }: { entry: HistoryEntry } = $props();
  let open = $state(false);
  const diff = $derived(open ? textDiff(entry.originalText, entry.transformedText) : null);
</script>
<details class="history-item" bind:open><summary><span class="history-icon" aria-hidden="true">↳</span><span>Transformation #{entry.id}</span><time datetime={entry.createdAt}>{new Date(entry.createdAt).toLocaleString()}</time></summary>
  {#if diff}<div class="diff-grid"><div><h4>Before <span class="muted">− removed</span></h4><pre aria-label="Before">{diff.prefix}<del>{diff.removed}</del>{diff.suffix}</pre></div><div><h4>After <span class="muted">+ added</span></h4><pre aria-label="After">{diff.prefix}<ins>{diff.added}</ins>{diff.suffix}</pre></div></div>
    {#if diff.truncated}<p class="muted">Long text: showing an excerpt of the first {UI.TEXT_EXCERPT_CHARS.toLocaleString()} characters on each side.</p>{/if}
    <p class="diff-caption">Whitespace is preserved. Highlights span the first through last difference in this excerpt.</p>
  {/if}
</details>
