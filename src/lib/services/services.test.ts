import { mockCommands, mockEvents, mockAvailable } from '../../test/desktop-mock';
import { describe, it, expect, vi } from 'vitest';
import { trimRule, settings, deferred } from '../../test/fixtures';
import { rulesService } from './rules.service';
import { historyService } from './history.service';
import { settingsService } from './settings.service';
import { desktopService, type DesktopHandlers } from './desktop.service';
import { errorMessage } from './result';

describe('generated command services', () => {
  it('returns canonical rules and forwards preview/reorder arguments', async () => {
    const rule = trimRule();
    mockCommands.getRules.mockResolvedValue({ status: 'ok', data: [rule] });
    mockCommands.saveRule.mockResolvedValue({ status: 'ok', data: rule });
    mockCommands.reorderRules.mockResolvedValue({ status: 'ok', data: [rule] });
    mockCommands.previewRule.mockResolvedValue({ status: 'ok', data: 'native result' });
    expect(await rulesService.getAll()).toEqual([rule]);
    expect(await rulesService.save(trimRule(0))).toEqual(rule);
    expect(await rulesService.reorder([1])).toEqual([rule]);
    expect(mockCommands.reorderRules).toHaveBeenCalledWith([1]);
    expect(await rulesService.preview(rule, 'sample')).toBe('native result');
    expect(mockCommands.previewRule).toHaveBeenCalledWith(rule, 'sample');
  });
  it('forwards complete settings and unwraps history and status', async () => {
    mockCommands.getSettings.mockResolvedValue({ status: 'ok', data: settings });
    mockCommands.saveSettings.mockResolvedValue({ status: 'ok', data: settings });
    mockCommands.getHistory.mockResolvedValue({ status: 'ok', data: [] });
    mockCommands.clearHistory.mockResolvedValue({ status: 'ok', data: null });
    mockCommands.deleteRule.mockResolvedValue({ status: 'ok', data: null });
    mockCommands.getMonitorStatus.mockResolvedValue({ status: 'ok', data: { isRunning: false, lastError: null } });
    expect(await settingsService.get()).toEqual(settings);
    expect(await settingsService.save(settings)).toEqual(settings);
    expect(mockCommands.saveSettings).toHaveBeenCalledWith(settings);
    expect(await historyService.getAll()).toEqual([]);
    expect(await historyService.clear()).toBeNull();
    expect(await rulesService.delete(1)).toBeNull();
    expect(await settingsService.status()).toEqual({ isRunning: false, lastError: null });
  });
  it.each([
    { kind: 'validation', message: 'Invalid pattern' },
    new Error('Transport failed'), 'Connection lost', { message: 'Native failure' }, null,
  ])('keeps context for typed or unknown failures: %s', async (error) => {
    mockCommands.saveRule.mockRejectedValue(error);
    await expect(rulesService.save(trimRule())).rejects.toThrow(`Save rule: ${errorMessage(error)}`);
  });
  it('unwraps typed backend errors', async () => {
    mockCommands.saveRule.mockResolvedValue({ status: 'error', error: { kind: 'validation', message: 'Bad regex' } });
    await expect(rulesService.save(trimRule())).rejects.toThrow('Save rule: Bad regex');
  });
  it('refuses persistence in a browser', async () => {
    mockAvailable.mockReturnValue(false);
    expect(desktopService.available()).toBe(false);
    await expect(rulesService.save(trimRule())).rejects.toThrow('Open ClipChain on your desktop');
    expect(mockCommands.saveRule).not.toHaveBeenCalled();
  });
});

function handlers(): DesktopHandlers { return { rules: vi.fn(), history: vi.fn(), settings: vi.fn(), monitor: vi.fn(), error: vi.fn() }; }
describe('desktop listener lifecycle', () => {
  it('cleans up every listener once', async () => {
    const releases = Object.values(mockEvents).map((event) => { const release = vi.fn(); event.listen.mockResolvedValue(release); return release; });
    const subscription = desktopService.subscribe(handlers());
    await subscription.ready; subscription.dispose(); subscription.dispose();
    releases.forEach((release) => expect(release).toHaveBeenCalledTimes(1));
  });
  it('rolls back partial registration', async () => {
    const release = vi.fn();
    mockEvents.rulesUpdated.listen.mockResolvedValue(release);
    mockEvents.historyCleared.listen.mockRejectedValue(new Error('Listener failed'));
    const subscription = desktopService.subscribe(handlers());
    await expect(subscription.ready).rejects.toThrow('Listener failed');
    expect(release).toHaveBeenCalledOnce();
    expect(mockEvents.settingsUpdated.listen).not.toHaveBeenCalled();
  });
  it('disposes listeners that finish registering after teardown', async () => {
    const pending = deferred<() => void>(), release = vi.fn();
    mockEvents.rulesUpdated.listen.mockReturnValue(pending.promise);
    const subscription = desktopService.subscribe(handlers());
    subscription.dispose(); pending.resolve(release); await subscription.ready;
    expect(release).toHaveBeenCalledOnce();
    expect(mockEvents.clipboardChanged.listen).not.toHaveBeenCalled();
  });
  it('guards callbacks after disposal and delivers monitor payloads', async () => {
    const callbacks = handlers(); const subscription = desktopService.subscribe(callbacks); await subscription.ready;
    const callback = mockEvents.monitorChanged.listen.mock.calls[0]?.[0];
    const event = { event: 'monitor-changed', id: 1, payload: { isRunning: true, lastError: null } };
    callback?.(event); expect(callbacks.monitor).toHaveBeenCalledWith(event.payload);
    subscription.dispose(); callback?.(event); expect(callbacks.monitor).toHaveBeenCalledTimes(1);
  });
  it('continues cleanup when one unlistener throws', async () => {
    mockEvents.rulesUpdated.listen.mockResolvedValue(() => { throw new Error('Cleanup failed'); });
    const release = vi.fn(); mockEvents.clipboardChanged.listen.mockResolvedValue(release);
    const callbacks = handlers(); const subscription = desktopService.subscribe(callbacks); await subscription.ready;
    subscription.dispose(); expect(callbacks.error).toHaveBeenCalled(); expect(release).toHaveBeenCalledOnce();
  });
});
