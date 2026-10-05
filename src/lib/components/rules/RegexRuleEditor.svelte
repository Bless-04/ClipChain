<script lang="ts">
  import type { RegexRule } from '$lib/generated/bindings';
  import { PATTERN_TOKENS } from '$lib/constants/rules';
  import { buildPattern } from '$lib/rules/draft';
  import Input from '../ui/Input.svelte';
  import Button from '../ui/Button.svelte';
  let { data = $bindable() }: { data: RegexRule } = $props();
  let start = $state(''), contains = $state(''), token = $state<string>(PATTERN_TOKENS.NONE), end = $state('');
  const built = $derived(buildPattern(start, contains, token, end));
</script>
<Input label="Rust regex pattern" bind:value={data.pattern} multiline hint="Edit any Rust-compatible pattern directly. Preview checks it with the native engine." />
<details class="builder"><summary>Build a pattern visually</summary>
  <p class="muted">Blocks match in this order, with any non-newline text between them. Text fields are literal and escaped automatically. Applying replaces the pattern above.</p>
  <div class="two-column"><Input label="Starts with" bind:value={start} /><Input label="Contains" bind:value={contains} /></div>
  <label class="field">Then match<select bind:value={token}><option value={PATTERN_TOKENS.NONE}>No extra token</option><option value={PATTERN_TOKENS.DIGITS}>One or more digits</option><option value={PATTERN_TOKENS.WORD}>One or more word characters</option></select></label>
  <Input label="Ends with" bind:value={end} />
  <pre>{built || 'Add a block to begin'}</pre>
  <Button disabled={!built} onclick={() => { data.pattern = built; }}>Apply built pattern</Button>
</details>
<Input label="Replacement" bind:value={data.replacement} multiline hint="Use $1 or ${name} for captures, ${1}text to separate a capture from text, and $$ for a literal dollar sign. Empty removes matches." />
