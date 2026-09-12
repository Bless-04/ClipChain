import { beforeEach, vi } from 'vitest';
import { commands, events } from '$lib/generated/bindings';
import { isTauri } from '@tauri-apps/api/core';
vi.mock('@tauri-apps/api/core', async (original) => ({ ...await original<typeof import('@tauri-apps/api/core')>(), isTauri: vi.fn(() => true) }));
vi.mock('$lib/generated/bindings', async (original) => ({
  ...await original<typeof import('$lib/generated/bindings')>(),
  commands: {
    getRules: vi.fn(), saveRule: vi.fn(), deleteRule: vi.fn(), reorderRules: vi.fn(), previewRule: vi.fn(),
    getHistory: vi.fn(), clearHistory: vi.fn(), getSettings: vi.fn(), saveSettings: vi.fn(), getMonitorStatus: vi.fn(),
  },
  events: {
    rulesUpdated: { listen: vi.fn() }, clipboardChanged: { listen: vi.fn() }, historyCleared: { listen: vi.fn() },
    settingsUpdated: { listen: vi.fn() }, monitorChanged: { listen: vi.fn() },
  },
}));
export const mockCommands = vi.mocked(commands);
export const mockEvents = vi.mocked(events, true);
export const mockAvailable = vi.mocked(isTauri);
beforeEach(() => {
  Object.values(mockCommands).forEach((command) => command.mockReset());
  Object.values(mockEvents).forEach((event) => { event.listen.mockReset(); event.listen.mockResolvedValue(vi.fn()); });
  mockAvailable.mockReturnValue(true);
});
