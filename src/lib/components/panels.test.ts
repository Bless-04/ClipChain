import { mockCommands } from '../../test/desktop-mock';
import { it, expect } from 'vitest';
import { render, screen, waitFor } from '@testing-library/svelte';
import userEvent from '@testing-library/user-event';
import { deferred, historyEntry, settings } from '../../test/fixtures';
import type { AppError, AppSettings, Result } from '$lib/generated/bindings';
import { UI } from '$lib/constants/ui';
import { HistoryState } from '$lib/state/history.svelte';
import { SettingsState } from '$lib/state/settings.svelte';
import HistoryPanel from './HistoryPanel.svelte';
import SettingsPanel from './SettingsPanel.svelte';

it.each([
  ['Launch at startup', 'launchAtStartup'],
  ['Record history', 'recordHistory'],
] as const)('saves %s with every other setting preserved', async (label, field) => {
  const state = new SettingsState(); state.confirm(settings);
  const saved = { ...settings, [field]: true };
  mockCommands.saveSettings.mockResolvedValue({ status: 'ok', data: saved });
  render(SettingsPanel, { state, available: true });
  await userEvent.setup().click(screen.getByRole('switch', { name: label }));
  await waitFor(() => expect(screen.getByRole('switch', { name: label })).toBeChecked());
  expect(mockCommands.saveSettings).toHaveBeenCalledExactlyOnceWith(saved);
  expect(screen.getByRole('switch', { name: 'Monitor clipboard' })).toBeChecked();
});

it('loads settings through an explicit retry after a failed initial read', async () => {
  const state = new SettingsState();
  mockCommands.getSettings.mockRejectedValueOnce(new Error('Settings unavailable'))
    .mockResolvedValueOnce({ status: 'ok', data: settings });
  await state.load(); render(SettingsPanel, { state, available: true });
  expect(screen.getByRole('alert')).toHaveTextContent('Settings unavailable');
  expect(screen.getByRole('switch', { name: 'Monitor clipboard' })).toBeDisabled();
  await userEvent.setup().click(screen.getByRole('button', { name: 'Retry' }));
  await waitFor(() => expect(screen.getByRole('switch', { name: 'Monitor clipboard' })).toBeEnabled());
  expect(screen.queryByRole('alert')).not.toBeInTheDocument();
});

it('applies canonical startup settings when a save and a live reload overlap', async () => {
  const state = new SettingsState(); state.confirm(settings);
  const pending = deferred<Result<AppSettings, AppError>>();
  mockCommands.saveSettings.mockReturnValue(pending.promise);
  // Native startup readback can differ from the requested registration value.
  const canonical = { ...settings, launchAtStartup: false, recordHistory: true };
  mockCommands.getSettings.mockResolvedValue({ status: 'ok', data: canonical });
  render(SettingsPanel, { state, available: true });
  await userEvent.setup().click(screen.getByRole('switch', { name: 'Launch at startup' }));
  state.refresh(); state.refresh();
  pending.resolve({ status: 'ok', data: canonical });
  await waitFor(() => expect(screen.getByRole('switch', { name: 'Record history' })).toBeChecked());
  expect(screen.getByRole('switch', { name: 'Launch at startup' })).not.toBeChecked();
  expect(mockCommands.getSettings).toHaveBeenCalledOnce();
});

it('disables all saved settings controls in browser mode', async () => {
  const state = new SettingsState(); state.confirm(settings);
  render(SettingsPanel, { state, available: false });
  for (const control of screen.getAllByRole('switch')) {
    expect(control).toBeDisabled(); await userEvent.setup().click(control);
  }
  expect(mockCommands.saveSettings).not.toHaveBeenCalled();
});

it('cancels history clearing and restores focus to its opener', async () => {
  const state = new HistoryState(); state.confirm([historyEntry()]);
  render(HistoryPanel, { state, available: true, recording: true });
  const user = userEvent.setup(), opener = screen.getByRole('button', { name: 'Clear history' });
  await user.click(opener);
  await user.click(screen.getByRole('button', { name: 'Cancel' }));
  expect(screen.queryByRole('dialog')).not.toBeInTheDocument();
  expect(opener).toHaveFocus();
  expect(mockCommands.clearHistory).not.toHaveBeenCalled();
  expect(state.entries).toHaveLength(1);
});

it('disables clear for an empty history and for browser mode', async () => {
  const state = new HistoryState(); state.confirm([]);
  const { rerender } = render(HistoryPanel, { state, available: true, recording: true });
  expect(screen.getByRole('button', { name: 'Clear history' })).toBeDisabled();
  state.confirm([historyEntry()]);
  await rerender({ state, available: false, recording: true });
  await userEvent.setup().click(screen.getByRole('button', { name: 'Clear history' }));
  expect(screen.queryByRole('dialog')).not.toBeInTheDocument();
  expect(mockCommands.clearHistory).not.toHaveBeenCalled();
});

it('clamps the selected page when a live refresh removes its entries', async () => {
  const state = new HistoryState();
  state.confirm(Array.from({ length: UI.HISTORY_PAGE_SIZE + 1 }, (_, index) => historyEntry(21 - index)));
  render(HistoryPanel, { state, available: true, recording: true });
  expect(screen.getByRole('button', { name: 'Previous' })).toBeDisabled();
  await userEvent.setup().click(screen.getByRole('button', { name: 'Next' }));
  expect(screen.getByRole('button', { name: 'Next' })).toBeDisabled();
  state.confirm([historyEntry(30)]);
  await waitFor(() => expect(screen.getByText('Page 1 of 1')).toBeInTheDocument());
  expect(screen.getByText('Transformation #30')).toBeInTheDocument();
  expect(screen.getByRole('button', { name: 'Previous' })).toBeDisabled();
});

it('renders bounded Unicode excerpts only when a history row is expanded', async () => {
  const state = new HistoryState();
  const prefix = 'a'.repeat(UI.TEXT_EXCERPT_CHARS - 1);
  state.confirm([{ ...historyEntry(), originalText: prefix + '😀b', transformedText: prefix + '🚀c' }]);
  render(HistoryPanel, { state, available: true, recording: true });
  expect(screen.queryByLabelText('Before')).not.toBeInTheDocument();
  const user = userEvent.setup();
  await user.click(screen.getByText('Transformation #1'));
  expect(screen.getByLabelText('Before').textContent).toBe(prefix + '😀');
  expect(screen.getByLabelText('After').textContent).toBe(prefix + '🚀');
  expect(screen.getByText(/Long text: showing an excerpt/)).toBeInTheDocument();
  await user.click(screen.getByText('Transformation #1'));
  await waitFor(() => expect(screen.queryByLabelText('Before')).not.toBeInTheDocument());
});
