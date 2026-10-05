<script lang="ts">
  import type { Rule } from '$lib/generated/bindings';
  import RuleCard from './RuleCard.svelte';
  let { rules, busy, onedit, ondelete, ontoggle, onreorder }: { rules: Rule[]; busy: boolean; onedit: (rule: Rule) => void; ondelete: (rule: Rule) => void; ontoggle: (rule: Rule) => void; onreorder: (ids: number[]) => void } = $props();
  function move(index: number, direction: number) {
    const target = index + direction;
    if (target < 0 || target >= rules.length) return;
    const ids = rules.map((rule) => rule.data.id);
    const id = ids[index], other = ids[target];
    if (id === undefined || other === undefined) return;
    ids[index] = other; ids[target] = id; onreorder(ids);
  }
</script>
<ol class="rule-list" aria-label="Transformation pipeline">{#each rules as rule, index (rule.data.id)}<li><RuleCard {rule} first={index === 0} last={index === rules.length - 1} {busy} onedit={() => onedit(rule)} ondelete={() => ondelete(rule)} ontoggle={() => ontoggle(rule)} onmove={(direction) => move(index, direction)} /></li>{/each}</ol>
