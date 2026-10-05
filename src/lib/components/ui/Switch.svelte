<script lang="ts">
  let { label, checked, disabled = false, onchange }: { label: string; checked: boolean; disabled?: boolean; onchange: (checked: boolean) => void } = $props();
</script>
<button type="button" role="switch" aria-checked={checked} aria-label={label} {disabled} class="switch" class:checked onclick={() => onchange(!checked)}><span></span></button>
