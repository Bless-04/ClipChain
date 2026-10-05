<script lang="ts">
  let { label, value = $bindable(''), hint, error, disabled = false, multiline = false, placeholder = '' }: {
    label: string; value?: string; hint?: string; error?: string | null; disabled?: boolean; multiline?: boolean; placeholder?: string;
  } = $props();
  const id = $props.id();
</script>
<div class="field">
  <label for={id}>{label}</label>
  {#if multiline}<textarea {id} bind:value {disabled} {placeholder} aria-invalid={!!error} aria-describedby={hint || error ? `${id}-help` : undefined} rows="3"></textarea>
  {:else}<input {id} bind:value {disabled} {placeholder} aria-invalid={!!error} aria-describedby={hint || error ? `${id}-help` : undefined} />{/if}
  {#if error || hint}<small id="{id}-help" class:error={!!error}>{error || hint}</small>{/if}
</div>
