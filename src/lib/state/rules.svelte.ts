import type { Rule } from '$lib/generated/bindings';
import { rulesService } from '$lib/services/rules.service';
import { Resource } from './resource.svelte';
export class RulesState extends Resource<Rule[]> {
  rules = $derived(this.value ?? []);
  activeRules = $derived(this.rules.filter((rule) => rule.data.isActive));
  count = $derived(this.rules.length);
  constructor(private readonly service = rulesService) { super(service.getAll); }
  save(rule: Rule) {
    return this.mutate(() => this.service.save(rule), (saved) =>
      [...this.rules.filter((item) => item.data.id !== saved.data.id), saved].sort((a, b) => a.data.order - b.data.order));
  }
  delete(id: number) {
    return this.mutate(() => this.service.delete(id), () =>
      this.rules.filter((rule) => rule.data.id !== id).map((rule, order) => { const copy = structuredClone($state.snapshot(rule)); copy.data.order = order; return copy; }));
  }
  toggle(rule: Rule) { const copy = structuredClone($state.snapshot(rule)); copy.data.isActive = !copy.data.isActive; return this.save(copy); }
  async reorder(ids: number[]) {
    if (ids.length !== this.rules.length || ids.some((id, index) => ids.indexOf(id) !== index || !this.rules.some((rule) => rule.data.id === id))) {
      this.mutationError = 'The order must contain each current rule exactly once. Reload and retry.'; return undefined;
    }
    return this.mutate(() => this.service.reorder(ids), (rules) => rules);
  }
}
