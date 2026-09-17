<script lang="ts">
  import type { ReplaceRule } from '$lib/generated/bindings';
  import Input from '../ui/Input.svelte';
  let { data = $bindable() }: { data: ReplaceRule } = $props();
</script>
<Input label="Text to replace" bind:value={data.target} multiline hint="Literal text, including spaces and line breaks. Required." />
<Input label="Replacement" bind:value={data.replacement} multiline hint="Literal text. Leave empty to remove matches; $1 has no special meaning here." />
<label class="check-row"><input type="checkbox" bind:checked={data.matchCase} /> Match case</label>
