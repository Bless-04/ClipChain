import { getContext, setContext } from 'svelte';
import { desktopService } from '$lib/services/desktop.service';
import { settingsService } from '$lib/services/settings.service';
import { errorMessage } from '$lib/services/result';
import { RulesState } from './rules.svelte';
import { HistoryState } from './history.svelte';
import { SettingsState } from './settings.svelte';
import { Resource } from './resource.svelte';
const APP_CONTEXT = Symbol('ClipChain app state');
export class AppState {
  readonly rules = new RulesState();
  readonly history = new HistoryState();
  readonly settings = new SettingsState();
  readonly monitor = new Resource(settingsService.status);
  available = $state(false);
  connectionError = $state<string | null>(null);
  private cleanup = () => {};
  status = $derived.by(() => {
    if (!this.available) return 'Desktop unavailable';
    if (this.monitor.error || this.monitor.value?.lastError) return 'Worker needs attention';
    if (!this.settings.settings) return 'Loading status';
    if (!this.settings.settings.isActive) return 'Paused';
    return this.monitor.value?.isRunning ? 'Monitoring clipboard' : 'Starting monitor';
  });
  start(): void {
    this.stop(); this.available = desktopService.available(); this.connectionError = null;
    if (!this.available) return;
    let disposed = false;
    const subscription = desktopService.subscribe({
      rules: () => this.rules.refresh(), history: () => this.history.refresh(),
      settings: () => { this.settings.refresh(); this.monitor.refresh(); },
      monitor: (status) => this.monitor.confirm(status),
      error: (error) => { if (!disposed) this.connectionError = errorMessage(error); },
    });
    this.cleanup = () => { disposed = true; subscription.dispose(); };
    void subscription.ready.then(() => { if (!disposed) this.reload(); }).catch((error: unknown) => {
      if (!disposed) { this.connectionError = `Live updates unavailable: ${errorMessage(error)}`; this.reload(); }
    });
  }
  reload(): void { this.rules.refresh(); this.history.refresh(); this.settings.refresh(); this.monitor.refresh(); }
  stop(): void {
    this.cleanup(); this.rules.cancelReads(); this.history.cancelReads(); this.settings.cancelReads(); this.monitor.cancelReads();
  }
}
export function provideApp(): AppState { return setContext(APP_CONTEXT, new AppState()); }
export function useApp(): AppState { return getContext<AppState>(APP_CONTEXT); }
