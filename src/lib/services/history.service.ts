import { commands } from '$lib/generated/bindings';
import { request } from './result';
export const historyService = {
  getAll: () => request('Load history', () => commands.getHistory()),
  clear: () => request('Clear history', () => commands.clearHistory()),
};
