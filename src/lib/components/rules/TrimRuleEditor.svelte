<script lang="ts">
  import type { TrimRule } from '$lib/generated/bindings';
  let { data = $bindable() }: { data: TrimRule } = $props();
</script>
<label class="check-row"><input type="checkbox" bind:checked={data.trimStart} /> Remove leading whitespace</label>
<label class="check-row"><input type="checkbox" bind:checked={data.trimEnd} /> Remove trailing whitespace</label>
<p class="muted">Includes spaces, tabs, and line breaks at the selected edges.</p>
