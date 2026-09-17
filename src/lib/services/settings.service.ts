import { commands, type AppSettings } from '$lib/generated/bindings';
import { request } from './result';
export const settingsService = {
  get: () => request('Load settings', () => commands.getSettings()),
  save: (settings: AppSettings) => request('Save settings', () => commands.saveSettings(settings)),
  status: () => request('Read monitor status', () => commands.getMonitorStatus()),
};
