import type { AppSettings } from '$lib/generated/bindings';
import { settingsService } from '$lib/services/settings.service';
import { Resource } from './resource.svelte';
export class SettingsState extends Resource<AppSettings> {
  settings = $derived(this.value);
  constructor(private readonly service = settingsService) { super(service.get); }
  save(settings: AppSettings) { return this.mutate(() => this.service.save(settings), (saved) => saved); }
  change(field: keyof AppSettings, value: boolean) {
    if (!this.settings) return Promise.resolve(undefined);
    return this.save({ ...this.settings, [field]: value });
  }
}
