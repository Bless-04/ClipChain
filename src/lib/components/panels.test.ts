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
  await waitFor(() => expect(screen.getByRole('switch', { name: label }), "The selected setting must show its enabled value after the save is confirmed.").toBeChecked());
  expect(mockCommands.saveSettings, "Toggling one setting must save exactly once with every other field preserved.").toHaveBeenCalledExactlyOnceWith(saved);
  expect(screen.getByRole('switch', { name: 'Monitor clipboard' }), "Changing startup or recording must leave monitoring enabled.").toBeChecked();
});

it('loads settings through an explicit retry after a failed initial read', async () => {
  const state = new SettingsState();
  mockCommands.getSettings.mockRejectedValueOnce(new Error('Settings unavailable'))
    .mockResolvedValueOnce({ status: 'ok', data: settings });
  await state.load(); render(SettingsPanel, { state, available: true });
  expect(screen.getByRole('alert'), "A failed initial settings read must show its error.").toHaveTextContent('Settings unavailable');
  expect(screen.getByRole('switch', { name: 'Monitor clipboard' }), "Settings must remain uneditable while their saved values are unknown.").toBeDisabled();
  await userEvent.setup().click(screen.getByRole('button', { name: 'Retry' }));
  await waitFor(() => expect(screen.getByRole('switch', { name: 'Monitor clipboard' }), "Retrying the settings read successfully must enable the controls.").toBeEnabled());
  expect(screen.queryByRole('alert'), "A successful retry must remove the initial loading error.").not.toBeInTheDocument();
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
  await waitFor(() => expect(screen.getByRole('switch', { name: 'Record history' }), "An overlapping save and reload must apply the backend's confirmed recording value.").toBeChecked());
  expect(screen.getByRole('switch', { name: 'Launch at startup' }), "The startup switch must use the confirmed response even if it differs from the requested value.").not.toBeChecked();
  expect(mockCommands.getSettings, "Multiple refresh requests during a settings save must produce one follow-up read.").toHaveBeenCalledOnce();
});

it('disables all saved settings controls in browser mode', async () => {
  const state = new SettingsState(); state.confirm(settings);
  render(SettingsPanel, { state, available: false });
  for (const control of screen.getAllByRole('switch')) {
    expect(control, "All persisted settings controls must be disabled in browser mode.").toBeDisabled(); await userEvent.setup().click(control);
  }
  expect(mockCommands.saveSettings, "Browser-mode clicks must never issue a native settings save.").not.toHaveBeenCalled();
});

it('cancels history clearing and restores focus to its opener', async () => {
  const state = new HistoryState(); state.confirm([historyEntry()]);
  render(HistoryPanel, { state, available: true, recording: true });
  const user = userEvent.setup(), opener = screen.getByRole('button', { name: 'Clear history' });
  await user.click(opener);
  await user.click(screen.getByRole('button', { name: 'Cancel' }));
  expect(screen.queryByRole('dialog'), "Cancel must close the clear-history confirmation.").not.toBeInTheDocument();
  expect(opener, "Closing confirmation must return keyboard focus to the button that opened it.").toHaveFocus();
  expect(mockCommands.clearHistory, "Canceling must not call the history-clear command.").not.toHaveBeenCalled();
  expect(state.entries, "Canceling the clear must preserve all retained entries.").toHaveLength(1);
});

it('disables clear for an empty history and for browser mode', async () => {
  const state = new HistoryState(); state.confirm([]);
  const { rerender } = render(HistoryPanel, { state, available: true, recording: true });
  expect(screen.getByRole('button', { name: 'Clear history' }), "Clear history must be disabled when there are no entries.").toBeDisabled();
  state.confirm([historyEntry()]);
  await rerender({ state, available: false, recording: true });
  await userEvent.setup().click(screen.getByRole('button', { name: 'Clear history' }));
  expect(screen.queryByRole('dialog'), "Browser mode must not open a destructive native-history dialog.").not.toBeInTheDocument();
  expect(mockCommands.clearHistory, "Browser mode must not issue a native history-clear command.").not.toHaveBeenCalled();
});

it('clamps the selected page when a live refresh removes its entries', async () => {
  const state = new HistoryState();
  state.confirm(Array.from({ length: UI.HISTORY_PAGE_SIZE + 1 }, (_, index) => historyEntry(21 - index)));
  render(HistoryPanel, { state, available: true, recording: true });
  expect(screen.getByRole('button', { name: 'Previous' }), "The first page must not allow navigation to a previous page.").toBeDisabled();
  await userEvent.setup().click(screen.getByRole('button', { name: 'Next' }));
  expect(screen.getByRole('button', { name: 'Next' }), "The last page must not allow navigation to a next page.").toBeDisabled();
  state.confirm([historyEntry(30)]);
  await waitFor(() => expect(screen.getByText('Page 1 of 1'), "If a refresh removes the selected page, pagination must return to the valid first page.").toBeInTheDocument());
  expect(screen.getByText('Transformation #30'), "After a shrinking refresh, the remaining history entry must be visible.").toBeInTheDocument();
  expect(screen.getByRole('button', { name: 'Previous' }), "After the page is clamped to the first page, Previous must be disabled.").toBeDisabled();
});

it('renders bounded Unicode excerpts only when a history row is expanded', async () => {
  const state = new HistoryState();
  const prefix = 'a'.repeat(UI.TEXT_EXCERPT_CHARS - 1);
  state.confirm([{ ...historyEntry(), originalText: prefix + '😀b', transformedText: prefix + '🚀c' }]);
  render(HistoryPanel, { state, available: true, recording: true });
  expect(screen.queryByLabelText('Before'), "Collapsed history rows must not render detailed clipboard text.").not.toBeInTheDocument();
  const user = userEvent.setup();
  await user.click(screen.getByText('Transformation #1'));
  expect(screen.getByLabelText('Before').textContent, "The original-text excerpt must retain the complete emoji at its length boundary.").toBe(prefix + '😀');
  expect(screen.getByLabelText('After').textContent, "The transformed-text excerpt must retain the complete emoji at its length boundary.").toBe(prefix + '🚀');
  expect(screen.getByText(/Long text: showing an excerpt/), "Truncated history text must tell the user that only an excerpt is shown.").toBeInTheDocument();
  await user.click(screen.getByText('Transformation #1'));
  await waitFor(() => expect(screen.queryByLabelText('Before'), "Collapsing a history row must remove its detailed text from the view.").not.toBeInTheDocument());
});
