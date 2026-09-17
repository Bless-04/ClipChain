import { mockCommands, mockEvents, mockAvailable } from '../../test/desktop-mock';
import { describe, it, expect, vi } from 'vitest';
import type { Rule, Result, AppError } from '$lib/generated/bindings';
import { trimRule, historyEntry, settings, deferred } from '../../test/fixtures';
import { RulesState } from './rules.svelte';
import { HistoryState } from './history.svelte';
import { SettingsState } from './settings.svelte';
import { AppState } from './app.svelte';

describe('confirmed state and races', () => {
  it('starts without guessed settings', () => { expect(new SettingsState().settings).toBeUndefined(); });
  it('keeps canonical create results even when the event refresh fails', async () => {
    const state = new RulesState(), pending = deferred<Result<Rule, AppError>>();
    mockCommands.saveRule.mockReturnValue(pending.promise);
    mockCommands.getRules.mockRejectedValue(new Error('Read failed'));
    const save = state.save(trimRule(0)); state.refresh();
    pending.resolve({ status: 'ok', data: trimRule(42) });
    expect((await save)?.data.id).toBe(42);
    await vi.waitFor(() => expect(state.error).toContain('Read failed'));
    expect(state.rules[0]?.data.id).toBe(42);
  });
  it('does not let an old read undo a saved rule', async () => {
    const state = new RulesState(), old = deferred<Result<Rule[], AppError>>();
    mockCommands.getRules.mockReturnValue(old.promise);
    const read = state.load();
    mockCommands.saveRule.mockResolvedValue({ status: 'ok', data: trimRule(1, 'New') });
    await state.save(trimRule(0));
    old.resolve({ status: 'ok', data: [] }); await read;
    expect(state.rules[0]?.data.name).toBe('New'); expect(state.loading).toBe(false);
  });
  it('keeps the newer of overlapping reads', async () => {
    const old = deferred<Result<Rule[], AppError>>(), state = new RulesState();
    mockCommands.getRules.mockReturnValueOnce(old.promise).mockResolvedValueOnce({ status: 'ok', data: [trimRule(9)] });
    const first = state.load(); await state.load(); old.resolve({ status: 'ok', data: [] }); await first;
    expect(state.rules[0]?.data.id).toBe(9);
  });
  it('retains confirmed data on failed mutations and prevents duplicate submissions', async () => {
    const state = new RulesState(), pending = deferred<Result<Rule, AppError>>(); state.confirm([trimRule()]);
    mockCommands.saveRule.mockReturnValue(pending.promise);
    const save = state.toggle(trimRule());
    expect(state.rules[0]?.data.isActive).toBe(true);
    expect(await state.toggle(trimRule())).toBeUndefined();
    state.refresh(); mockCommands.getRules.mockResolvedValue({ status: 'ok', data: [trimRule()] });
    pending.reject(new Error('Save failed')); await save;
    await vi.waitFor(() => expect(state.loading).toBe(false));
    expect(state.error).toContain('Save failed'); expect(state.rules[0]?.data.isActive).toBe(true);
    expect(mockCommands.saveRule).toHaveBeenCalledTimes(1);
  });
  it.each([[1], [1, 1], [1, 3]])('rejects invalid reorder %s', async (...ids) => {
    const state = new RulesState(); state.confirm([trimRule(1), trimRule(2)]);
    await state.reorder(ids); expect(mockCommands.reorderRules).not.toHaveBeenCalled(); expect(state.error).toContain('exactly once');
  });
  it('uses one complete reorder command and canonical positions', async () => {
    const state = new RulesState(); state.confirm([trimRule(1), trimRule(2)]);
    const first = trimRule(2), second = trimRule(1); first.data.order = 0; second.data.order = 1;
    mockCommands.reorderRules.mockResolvedValue({ status: 'ok', data: [first, second] });
    await state.reorder([2, 1]); expect(state.rules).toEqual([first, second]);
    expect(mockCommands.reorderRules).toHaveBeenCalledExactlyOnceWith([2, 1]); expect(mockCommands.saveRule).not.toHaveBeenCalled();
  });
  it('deletes only after confirmation and compacts positions', async () => {
    const state = new RulesState(); state.confirm([trimRule(1), trimRule(2)]);
    mockCommands.deleteRule.mockResolvedValue({ status: 'ok', data: null }); await state.delete(1);
    expect(state.rules).toHaveLength(1); expect(state.rules[0]?.data.order).toBe(0);
  });
  it('preserves all settings fields and applies the canonical response', async () => {
    const state = new SettingsState(); state.confirm(settings);
    const canonical = { ...settings, isActive: false, launchAtStartup: true };
    mockCommands.saveSettings.mockResolvedValue({ status: 'ok', data: canonical }); await state.change('isActive', false);
    expect(mockCommands.saveSettings).toHaveBeenCalledWith({ ...settings, isActive: false }); expect(state.settings).toEqual(canonical);
  });
  it('reconciles history recorded while a clear response is in flight', async () => {
    const state = new HistoryState(); state.confirm([historyEntry(1)]);
    const pending = deferred<Result<null, AppError>>(); mockCommands.clearHistory.mockReturnValue(pending.promise);
    mockCommands.getHistory.mockResolvedValue({ status: 'ok', data: [historyEntry(2)] });
    const clear = state.clear(); state.refresh(); state.refresh(); pending.resolve({ status: 'ok', data: null }); await clear;
    await vi.waitFor(() => expect(state.entries.map((entry) => entry.id)).toEqual([2])); expect(mockCommands.getHistory).toHaveBeenCalledOnce();
  });
  it('retains history after a failed clear and supports retry', async () => {
    const state = new HistoryState(); state.confirm([historyEntry()]);
    mockCommands.clearHistory.mockRejectedValueOnce(new Error('Locked')).mockResolvedValueOnce({ status: 'ok', data: null });
    await state.clear(); expect(state.entries).toHaveLength(1); expect(state.error).toContain('Locked');
    await state.clear(); expect(state.entries).toHaveLength(0); expect(state.error).toBeNull();
  });
  it('clears a failed mutation diagnostic after an explicit successful reload', async () => {
    const state = new RulesState(); state.confirm([trimRule()]);
    mockCommands.deleteRule.mockRejectedValue(new Error('Stale rule'));
    await state.delete(1); expect(state.error).toContain('Stale rule');
    mockCommands.getRules.mockResolvedValue({ status: 'ok', data: [] });
    await state.load(); expect(state.error).toBeNull(); expect(state.rules).toEqual([]);
  });
});

describe('app lifecycle', () => {
  it('does not load native data in a browser', () => {
    mockAvailable.mockReturnValue(false); const app = new AppState(); app.start();
    expect(app.status).toBe('Desktop unavailable'); expect(mockCommands.getRules).not.toHaveBeenCalled(); app.stop();
  });
  it('subscribes before reads and avoids initialization after teardown', async () => {
    const pending = deferred<() => void>(); mockEvents.rulesUpdated.listen.mockReturnValue(pending.promise);
    const app = new AppState(); app.start(); expect(mockCommands.getRules).not.toHaveBeenCalled(); app.stop();
    const release = vi.fn(); pending.resolve(release); await vi.waitFor(() => expect(release).toHaveBeenCalled());
    expect(mockCommands.getRules).not.toHaveBeenCalled();
  });
  it('shows native health independently from enabled settings', () => {
    const app = new AppState(); app.available = true; app.settings.confirm(settings);
    app.monitor.confirm({ isRunning: false, lastError: 'Clipboard locked' }); expect(app.status).toBe('Worker needs attention');
    app.monitor.confirm({ isRunning: true, lastError: null }); expect(app.status).toBe('Monitoring clipboard');
    app.settings.confirm({ ...settings, isActive: false }); expect(app.status).toBe('Paused');
  });
});
