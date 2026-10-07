import { mockCommands, mockEvents, mockAvailable } from '../../test/desktop-mock';
import { afterEach, describe, it, expect, vi } from 'vitest';
import type { Rule, Result, AppError } from '$lib/generated/bindings';
import { trimRule, historyEntry, settings, deferred } from '../../test/fixtures';
import { RulesState } from './rules.svelte';
import { HistoryState } from './history.svelte';
import { SettingsState } from './settings.svelte';
import { AppState } from './app.svelte';
import { APP_EVENTS } from '$lib/constants/events';

describe('confirmed state and races', () => {
  it('starts without guessed settings', () => { expect(new SettingsState().settings, "Settings must remain unknown until the backend confirms them.").toBeUndefined(); });
  it('keeps canonical create results even when the event refresh fails', async () => {
    const state = new RulesState(), pending = deferred<Result<Rule, AppError>>();
    mockCommands.saveRule.mockReturnValue(pending.promise);
    mockCommands.getRules.mockRejectedValue(new Error('Read failed'));
    const save = state.save(trimRule(0)); state.refresh();
    pending.resolve({ status: 'ok', data: trimRule(42) });
    expect((await save)?.data.id, "A successful create must return the backend-assigned ID even if a later refresh fails.").toBe(42);
    await vi.waitFor(() => expect(state.error, "A failed refresh must remain visible as a read error.").toContain('Read failed'));
    expect(state.rules[0]?.data.id, "A refresh failure must not discard the rule that was successfully saved.").toBe(42);
  });
  it('does not let an old read undo a saved rule', async () => {
    const state = new RulesState(), old = deferred<Result<Rule[], AppError>>();
    mockCommands.getRules.mockReturnValue(old.promise);
    const read = state.load();
    mockCommands.saveRule.mockResolvedValue({ status: 'ok', data: trimRule(1, 'New') });
    await state.save(trimRule(0));
    old.resolve({ status: 'ok', data: [] }); await read;
    expect(state.rules[0]?.data.name, "A stale read started before saving must not erase the newly saved rule.").toBe('New'); expect(state.loading, "Finishing a stale read must still clear its loading state.").toBe(false);
  });
  it('keeps the newer of overlapping reads', async () => {
    const old = deferred<Result<Rule[], AppError>>(), state = new RulesState();
    mockCommands.getRules.mockReturnValueOnce(old.promise).mockResolvedValueOnce({ status: 'ok', data: [trimRule(9)] });
    const first = state.load(); await state.load(); old.resolve({ status: 'ok', data: [] }); await first;
    expect(state.rules[0]?.data.id, "When reads finish out of order, the newest request must determine the displayed rules.").toBe(9);
  });
  it('retains confirmed data on failed mutations and prevents duplicate submissions', async () => {
    const state = new RulesState(), pending = deferred<Result<Rule, AppError>>(); state.confirm([trimRule()]);
    mockCommands.saveRule.mockReturnValue(pending.promise);
    const save = state.toggle(trimRule());
    expect(state.rules[0]?.data.isActive, "A pending toggle must leave the last confirmed enabled state visible.").toBe(true);
    expect(await state.toggle(trimRule()), "A second toggle while saving must be ignored rather than submitted twice.").toBeUndefined();
    state.refresh(); mockCommands.getRules.mockResolvedValue({ status: 'ok', data: [trimRule()] });
    pending.reject(new Error('Save failed')); await save;
    await vi.waitFor(() => expect(state.loading, "After a failed save and queued refresh settle, loading must end.").toBe(false));
    expect(state.error, "A failed save must remain visible so the user knows the toggle did not succeed.").toContain('Save failed'); expect(state.rules[0]?.data.isActive, "A failed toggle must preserve the last confirmed enabled state.").toBe(true);
    expect(mockCommands.saveRule, "Repeated clicks during one pending save must produce only one backend save.").toHaveBeenCalledTimes(1);
  });
  it.each([[1], [1, 1], [1, 3]])('rejects invalid reorder %s', async (...ids) => {
    const state = new RulesState(); state.confirm([trimRule(1), trimRule(2)]);
    await state.reorder(ids); expect(mockCommands.reorderRules, "Invalid rule orders must be rejected before calling the backend.").not.toHaveBeenCalled(); expect(state.error, "The reorder error must explain that every saved rule ID is required exactly once.").toContain('exactly once');
  });
  it('uses one complete reorder command and canonical positions', async () => {
    const state = new RulesState(); state.confirm([trimRule(1), trimRule(2)]);
    const first = trimRule(2), second = trimRule(1); first.data.order = 0; second.data.order = 1;
    mockCommands.reorderRules.mockResolvedValue({ status: 'ok', data: [first, second] });
    await state.reorder([2, 1]); expect(state.rules, "Reordering must use the backend-confirmed rule list and positions.").toEqual([first, second]);
    expect(mockCommands.reorderRules, "Reordering must send one complete sequence of IDs to the backend.").toHaveBeenCalledExactlyOnceWith([2, 1]); expect(mockCommands.saveRule, "Reordering must not be implemented as separate rule saves.").not.toHaveBeenCalled();
  });
  it('deletes only after confirmation and compacts positions', async () => {
    const state = new RulesState(); state.confirm([trimRule(1), trimRule(2)]);
    mockCommands.deleteRule.mockResolvedValue({ status: 'ok', data: null }); await state.delete(1);
    expect(state.rules, "After the backend confirms deletion, only the remaining rule should be displayed.").toHaveLength(1); expect(state.rules[0]?.data.order, "Deleting the first rule must move the remaining rule to position zero.").toBe(0);
  });
  it('preserves all settings fields and applies the canonical response', async () => {
    const state = new SettingsState(); state.confirm(settings);
    const canonical = { ...settings, isActive: false, launchAtStartup: true };
    mockCommands.saveSettings.mockResolvedValue({ status: 'ok', data: canonical }); await state.change('isActive', false);
    expect(mockCommands.saveSettings, "Changing one setting must send all fields so unrelated settings are preserved.").toHaveBeenCalledWith({ ...settings, isActive: false }); expect(state.settings, "The displayed settings must use the backend's confirmed response.").toEqual(canonical);
  });
  it('reconciles history recorded while a clear response is in flight', async () => {
    const state = new HistoryState(); state.confirm([historyEntry(1)]);
    const pending = deferred<Result<null, AppError>>(); mockCommands.clearHistory.mockReturnValue(pending.promise);
    mockCommands.getHistory.mockResolvedValue({ status: 'ok', data: [historyEntry(2)] });
    const clear = state.clear(); state.refresh(); state.refresh(); pending.resolve({ status: 'ok', data: null }); await clear;
    await vi.waitFor(() => expect(state.entries.map((entry) => entry.id), "History recorded while clearing is pending must appear after the queued refresh.").toEqual([2])); expect(mockCommands.getHistory, "Repeated refresh requests during clearing must result in only one follow-up read.").toHaveBeenCalledOnce();
  });
  it('retains history after a failed clear and supports retry', async () => {
    const state = new HistoryState(); state.confirm([historyEntry()]);
    mockCommands.clearHistory.mockRejectedValueOnce(new Error('Locked')).mockResolvedValueOnce({ status: 'ok', data: null });
    await state.clear(); expect(state.entries, "A failed clear must leave previously loaded history visible.").toHaveLength(1); expect(state.error, "A failed clear must show the underlying database error.").toContain('Locked');
    await state.clear(); expect(state.entries, "Retrying a failed clear successfully must remove the retained entries.").toHaveLength(0); expect(state.error, "A successful clear retry must remove the old error message.").toBeNull();
  });
  it('clears a failed mutation diagnostic after an explicit successful reload', async () => {
    const state = new RulesState(); state.confirm([trimRule()]);
    mockCommands.deleteRule.mockRejectedValue(new Error('Stale rule'));
    await state.delete(1); expect(state.error, "A failed deletion must expose the backend's explanation.").toContain('Stale rule');
    mockCommands.getRules.mockResolvedValue({ status: 'ok', data: [] });
    await state.load(); expect(state.error, "A successful explicit reload must clear the previous mutation error.").toBeNull(); expect(state.rules, "The successful reload must replace stale rules with the backend's current list.").toEqual([]);
  });
});

describe('app lifecycle', () => {
  it('does not load native data in a browser', () => {
    mockAvailable.mockReturnValue(false); const app = new AppState(); app.start();
    expect(app.status, "A browser session must clearly report that desktop services are unavailable.").toBe('Desktop unavailable'); expect(mockCommands.getRules, "Starting the app in a browser must not try to load native rules.").not.toHaveBeenCalled(); app.stop();
  });
  it('subscribes before reads and avoids initialization after teardown', async () => {
    const pending = deferred<() => void>(); mockEvents.rulesUpdated.listen.mockReturnValue(pending.promise);
    const app = new AppState(); app.start(); expect(mockCommands.getRules, "Initial reads must wait until event listeners are installed so changes are not missed.").not.toHaveBeenCalled(); app.stop();
    const release = vi.fn(); pending.resolve(release); await vi.waitFor(() => expect(release, "A listener registered after the app stops must be released.").toHaveBeenCalled());
    expect(mockCommands.getRules, "Stopping during initialization must prevent delayed database reads.").not.toHaveBeenCalled();
  });
  it('shows native health independently from enabled settings', () => {
    const app = new AppState(); app.available = true; app.settings.confirm(settings);
    app.monitor.confirm({ isRunning: false, lastError: 'Clipboard locked' }); expect(app.status, "An enabled preference alone must not hide a stopped or unhealthy worker.").toBe('Worker needs attention');
    app.monitor.confirm({ isRunning: true, lastError: null }); expect(app.status, "A healthy running worker must be shown as monitoring.").toBe('Monitoring clipboard');
    app.settings.confirm({ ...settings, isActive: false }); expect(app.status, "Turning off the monitoring preference must show the paused state.").toBe('Paused');
  });
});

describe('live data and reconnection', () => {
  const apps: AppState[] = [];
  afterEach(() => { apps.splice(0).forEach((app) => app.stop()); });
  async function startApp() {
    mockCommands.getRules.mockResolvedValue({ status: 'ok', data: [trimRule()] });
    mockCommands.getHistory.mockResolvedValue({ status: 'ok', data: [historyEntry()] });
    mockCommands.getSettings.mockResolvedValue({ status: 'ok', data: settings });
    mockCommands.getMonitorStatus.mockResolvedValue({ status: 'ok', data: { isRunning: true, lastError: null } });
    const app = new AppState(); apps.push(app); app.start();
    await vi.waitFor(() => expect(app.rules.count, "The app fixture must finish loading its saved rule before lifecycle assertions run.").toBe(1));
    return app;
  }

  it('loads every domain after subscriptions and routes every native event', async () => {
    const app = await startApp();
    expect(app.status, "Initial loading must include the worker's actual health.").toBe('Monitoring clipboard');
    expect(app.history.entries, "Initial loading must include retained history.").toHaveLength(1);
    expect(app.settings.settings, "Initial loading must include all saved settings.").toEqual(settings);
    mockCommands.getRules.mockResolvedValue({ status: 'ok', data: [trimRule(2)] });
    mockEvents.rulesUpdated.listen.mock.calls[0]?.[0]({ event: APP_EVENTS.RULES_UPDATED, id: 1, payload: null });
    await vi.waitFor(() => expect(app.rules.rules[0]?.data.id, "A rules-updated event must refresh the displayed rules.").toBe(2));
    mockCommands.getHistory.mockResolvedValue({ status: 'ok', data: [historyEntry(2)] });
    mockEvents.clipboardChanged.listen.mock.calls[0]?.[0]({ event: APP_EVENTS.CLIPBOARD_CHANGED, id: 2, payload: { entry: historyEntry(2) } });
    await vi.waitFor(() => expect(app.history.entries[0]?.id, "A clipboard-change event must refresh recorded history.").toBe(2));
    mockCommands.getHistory.mockResolvedValue({ status: 'ok', data: [] });
    mockEvents.historyCleared.listen.mock.calls[0]?.[0]({ event: APP_EVENTS.HISTORY_CLEARED, id: 3, payload: null });
    await vi.waitFor(() => expect(app.history.entries, "A history-cleared event must remove obsolete entries from the displayed list.").toEqual([]));
    mockCommands.getSettings.mockResolvedValue({ status: 'ok', data: { ...settings, isActive: false } });
    mockEvents.settingsUpdated.listen.mock.calls[0]?.[0]({ event: APP_EVENTS.SETTINGS_UPDATED, id: 4, payload: null });
    await vi.waitFor(() => expect(app.status, "A settings-updated event must refresh the displayed pause state.").toBe('Paused'));
    expect(mockCommands.getMonitorStatus, "Refreshing settings must also refresh worker health.").toHaveBeenCalledTimes(2);
    mockEvents.monitorChanged.listen.mock.calls[0]?.[0]({ event: APP_EVENTS.MONITOR_CHANGED, id: 5, payload: { isRunning: false, lastError: 'Worker stopped' } });
    expect(app.status, "A worker-health event must update the displayed error state immediately.").toBe('Worker needs attention');
  });

  it('loads data after subscription failure and reconnects without duplicate listeners', async () => {
    const releases = Object.values(mockEvents).map((event) => {
      const release = vi.fn(); event.listen.mockResolvedValue(release); return release;
    });
    mockEvents.historyCleared.listen.mockRejectedValueOnce(new Error('Event bridge unavailable'));
    const app = await startApp();
    expect(app.connectionError, "Subscription failure must produce a visible live-update warning while data remains available.").toContain('Live updates unavailable');
    expect(releases[0], "Failed registration must release the rules listener installed before the failure.").toHaveBeenCalledOnce();
    expect(releases[1], "Failed registration must release the clipboard listener installed before the failure.").toHaveBeenCalledOnce();
    app.start();
    await vi.waitFor(() => expect(mockCommands.getRules, "Reconnecting must reload saved rules after listeners become available.").toHaveBeenCalledTimes(2));
    expect(app.connectionError, "A successful reconnect must clear the live-update warning.").toBeNull();
    expect(mockEvents.settingsUpdated.listen, "Reconnecting must not install duplicate settings listeners.").toHaveBeenCalledOnce();
    app.stop();
    expect(releases[0], "Rules listeners from the failed attempt and successful reconnect must each be released.").toHaveBeenCalledTimes(2);
    expect(releases[1], "Clipboard listeners from both registration attempts must each be released.").toHaveBeenCalledTimes(2);
    releases.slice(2).forEach((release) => expect(release, "Listeners installed only after reconnect must be released exactly once when stopping.").toHaveBeenCalledOnce());
  });

  it('ignores events and pending domain reads after teardown', async () => {
    const app = await startApp();
    const pending = deferred<Result<Rule[], AppError>>();
    mockCommands.getRules.mockReturnValue(pending.promise);
    const load = app.rules.load();
    app.stop();
    pending.resolve({ status: 'ok', data: [trimRule(99)] }); await load;
    expect(app.rules.rules[0]?.data.id, "A pending read that completes after teardown must not replace the last confirmed rules.").toBe(1);
    expect(app.rules.loading, "Teardown must end the rules loading indicator.").toBe(false);
    mockEvents.rulesUpdated.listen.mock.calls[0]?.[0]({ event: APP_EVENTS.RULES_UPDATED, id: 1, payload: null });
    mockEvents.monitorChanged.listen.mock.calls[0]?.[0]({ event: APP_EVENTS.MONITOR_CHANGED, id: 2, payload: { isRunning: false, lastError: 'Late event' } });
    expect(mockCommands.getRules, "Events received after teardown must not start another database read.").toHaveBeenCalledTimes(2);
    expect(app.status, "Late worker events must not change state after the app has stopped.").toBe('Monitoring clipboard');
  });

  it('coalesces event bursts during a read into one fresh read', async () => {
    const state = new RulesState();
    const pending = deferred<Result<Rule[], AppError>>();
    mockCommands.getRules.mockReturnValueOnce(pending.promise).mockResolvedValueOnce({ status: 'ok', data: [trimRule(2)] });
    const load = state.load(); state.refresh(); state.refresh(); state.refresh();
    expect(mockCommands.getRules, "A burst of events during a pending read must not start parallel reads.").toHaveBeenCalledOnce();
    pending.resolve({ status: 'ok', data: [trimRule(1)] }); await load;
    await vi.waitFor(() => expect(state.rules[0]?.data.id, "The queued follow-up read must eventually display the newest rule data.").toBe(2));
    expect(mockCommands.getRules, "Many events during one read must collapse into one additional read.").toHaveBeenCalledTimes(2);
  });
});
