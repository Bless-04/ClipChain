<script lang="ts">
  import type { Rule } from '$lib/generated/bindings';
  import { RULE_LABELS } from '$lib/constants/rules';
  import { summarizeRule } from '$lib/rules/draft';
  import Button from '../ui/Button.svelte';
  import Switch from '../ui/Switch.svelte';
  import Badge from '../ui/Badge.svelte';
  let { rule, first, last, busy, onedit, ondelete, ontoggle, onmove }: { rule: Rule; first: boolean; last: boolean; busy: boolean; onedit: () => void; ondelete: () => void; ontoggle: () => void; onmove: (direction: number) => void } = $props();
</script>
<article class="rule-card" class:inactive={!rule.data.isActive}>
  <div class="step-number">{String(rule.data.order + 1).padStart(2, '0')}</div>
  <div class="rule-info"><div class="row"><h3>{rule.data.name}</h3><Badge>{RULE_LABELS[rule.type]}</Badge></div><p class="rule-summary">{summarizeRule(rule)}</p></div>
  <Switch label={`Enable ${rule.data.name}`} checked={rule.data.isActive} disabled={busy} onchange={ontoggle} />
  <div class="rule-actions"><Button label={`Move ${rule.data.name} up`} disabled={busy || first} onclick={() => onmove(-1)}>↑</Button><Button label={`Move ${rule.data.name} down`} disabled={busy || last} onclick={() => onmove(1)}>↓</Button><Button disabled={busy} onclick={onedit}>Edit</Button><Button disabled={busy} onclick={ondelete} variant="quiet">Delete</Button></div>
</article>
