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
    expect(await rulesService.getAll(), "Loading rules must return the rule data supplied by the backend.").toEqual([rule]);
    expect(await rulesService.save(trimRule(0)), "Saving must return the persisted rule, including the ID assigned by the backend.").toEqual(rule);
    expect(await rulesService.reorder([1]), "Reordering must return the backend's confirmed rule order.").toEqual([rule]);
    expect(mockCommands.reorderRules, "The service must pass the complete requested ID order to the backend.").toHaveBeenCalledWith([1]);
    expect(await rulesService.preview(rule, 'sample'), "Preview must return the Rust transformation result without reimplementing it in JavaScript.").toBe('native result');
    expect(mockCommands.previewRule, "The draft and sample text must reach the preview command unchanged.").toHaveBeenCalledWith(rule, 'sample');
  });
  it('forwards complete settings and unwraps history and status', async () => {
    mockCommands.getSettings.mockResolvedValue({ status: 'ok', data: settings });
    mockCommands.saveSettings.mockResolvedValue({ status: 'ok', data: settings });
    mockCommands.getHistory.mockResolvedValue({ status: 'ok', data: [] });
    mockCommands.clearHistory.mockResolvedValue({ status: 'ok', data: null });
    mockCommands.deleteRule.mockResolvedValue({ status: 'ok', data: null });
    mockCommands.getMonitorStatus.mockResolvedValue({ status: 'ok', data: { isRunning: false, lastError: null } });
    expect(await settingsService.get(), "Loading settings must return the stored configuration.").toEqual(settings);
    expect(await settingsService.save(settings), "Saving settings must return the backend's confirmed configuration.").toEqual(settings);
    expect(mockCommands.saveSettings, "Saving settings must forward the entire settings object.").toHaveBeenCalledWith(settings);
    expect(await historyService.getAll(), "An empty history result must be returned as an empty list.").toEqual([]);
    expect(await historyService.clear(), "A successful history clear must unwrap the backend's empty success response.").toBeNull();
    expect(await rulesService.delete(1), "A successful deletion must unwrap the backend's empty success response.").toBeNull();
    expect(await settingsService.status(), "Worker status must come from the backend's health report.").toEqual({ isRunning: false, lastError: null });
  });
  it.each([
    { kind: 'validation', message: 'Invalid pattern' },
    new Error('Transport failed'), 'Connection lost', { message: 'Native failure' }, null,
  ])('keeps context for typed or unknown failures: %s', async (error) => {
    mockCommands.saveRule.mockRejectedValue(error);
    await expect(rulesService.save(trimRule()), "Service failures must retain both the operation name and the underlying error explanation.").rejects.toThrow(`Save rule: ${errorMessage(error)}`);
  });
  it('unwraps typed backend errors', async () => {
    mockCommands.saveRule.mockResolvedValue({ status: 'error', error: { kind: 'validation', message: 'Bad regex' } });
    await expect(rulesService.save(trimRule()), "A typed backend validation error must become a useful save-rule exception.").rejects.toThrow('Save rule: Bad regex');
  });
  it('refuses persistence in a browser', async () => {
    mockAvailable.mockReturnValue(false);
    expect(desktopService.available(), "A browser preview must report that desktop services are unavailable.").toBe(false);
    await expect(rulesService.save(trimRule()), "Trying to save in a browser must explain that the desktop app is required.").rejects.toThrow('Open ClipChain on your desktop');
    expect(mockCommands.saveRule, "Browser-only use must not attempt a native save command.").not.toHaveBeenCalled();
  });
});

function handlers(): DesktopHandlers { return { rules: vi.fn(), history: vi.fn(), settings: vi.fn(), monitor: vi.fn(), error: vi.fn() }; }
describe('desktop listener lifecycle', () => {
  it('cleans up every listener once', async () => {
    const releases = Object.values(mockEvents).map((event) => { const release = vi.fn(); event.listen.mockResolvedValue(release); return release; });
    const subscription = desktopService.subscribe(handlers());
    await subscription.ready; subscription.dispose(); subscription.dispose();
    releases.forEach((release) => expect(release, "Disposing a subscription repeatedly must release each listener only once.").toHaveBeenCalledTimes(1));
  });
  it('rolls back partial registration', async () => {
    const release = vi.fn();
    mockEvents.rulesUpdated.listen.mockResolvedValue(release);
    mockEvents.historyCleared.listen.mockRejectedValue(new Error('Listener failed'));
    const subscription = desktopService.subscribe(handlers());
    await expect(subscription.ready, "Partial listener registration must reject readiness with the original registration error.").rejects.toThrow('Listener failed');
    expect(release, "If registration fails, listeners already installed must be removed.").toHaveBeenCalledOnce();
    expect(mockEvents.settingsUpdated.listen, "After registration fails, later listeners must not be installed.").not.toHaveBeenCalled();
  });
  it('disposes listeners that finish registering after teardown', async () => {
    const pending = deferred<() => void>(), release = vi.fn();
    mockEvents.rulesUpdated.listen.mockReturnValue(pending.promise);
    const subscription = desktopService.subscribe(handlers());
    subscription.dispose(); pending.resolve(release); await subscription.ready;
    expect(release, "A listener that finishes registering after disposal must be released immediately.").toHaveBeenCalledOnce();
    expect(mockEvents.clipboardChanged.listen, "Disposal during registration must prevent installation of subsequent listeners.").not.toHaveBeenCalled();
  });
  it('guards callbacks after disposal and delivers monitor payloads', async () => {
    const callbacks = handlers(); const subscription = desktopService.subscribe(callbacks); await subscription.ready;
    const callback = mockEvents.monitorChanged.listen.mock.calls[0]?.[0];
    const event = { event: 'monitor-changed', id: 1, payload: { isRunning: true, lastError: null } };
    callback?.(event); expect(callbacks.monitor, "An active listener must deliver the worker's health payload to the app.").toHaveBeenCalledWith(event.payload);
    subscription.dispose(); callback?.(event); expect(callbacks.monitor, "Events arriving after disposal must not be delivered again.").toHaveBeenCalledTimes(1);
  });
  it('continues cleanup when one unlistener throws', async () => {
    mockEvents.rulesUpdated.listen.mockResolvedValue(() => { throw new Error('Cleanup failed'); });
    const release = vi.fn(); mockEvents.clipboardChanged.listen.mockResolvedValue(release);
    const callbacks = handlers(); const subscription = desktopService.subscribe(callbacks); await subscription.ready;
    subscription.dispose(); expect(callbacks.error, "A listener cleanup failure must be reported to the error handler.").toHaveBeenCalled(); expect(release, "One failed cleanup must not prevent other listeners from being released.").toHaveBeenCalledOnce();
  });
});
