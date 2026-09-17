import type { HistoryEntry } from '$lib/generated/bindings';
import { historyService } from '$lib/services/history.service';
import { Resource } from './resource.svelte';
export class HistoryState extends Resource<HistoryEntry[]> {
  entries = $derived(this.value ?? []);
  constructor(private readonly service = historyService) { super(service.getAll); }
  clear() { return this.mutate(() => this.service.clear(), () => []); }
}
