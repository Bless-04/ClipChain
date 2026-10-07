import { mockCommands } from '../test/desktop-mock';
import { beforeEach, it, expect, vi } from 'vitest';
import { render, screen, waitFor, within } from '@testing-library/svelte';
import userEvent from '@testing-library/user-event';
import { deferred, historyEntry, regexRule, settings, trimRule } from '../test/fixtures';
import type { AppError, AppSettings, Result } from '$lib/generated/bindings';
import { AppState } from '$lib/state/app.svelte';
import { ROUTES } from '$lib/constants/routes';
import { UI } from '$lib/constants/ui';
import Dashboard from './+page.svelte';

let app: AppState;
vi.mock('$lib/state/app.svelte', async (original) => ({
  ...await original<typeof import('$lib/state/app.svelte')>(), useApp: () => app,
}));
beforeEach(() => { app = new AppState(); app.available = true; });

it('disables monitoring until saved settings load and in browser mode', async () => {
  render(Dashboard);
  expect(screen.getByText('Loading status'), "The dashboard must show that status is loading before settings are known.").toBeInTheDocument();
  expect(screen.getByRole('button', { name: 'Enable monitor' }), "Monitoring cannot be enabled until saved settings have loaded.").toBeDisabled();
  app.settings.confirm(settings);
  app.available = false;
  await waitFor(() => expect(screen.getByText('Desktop unavailable'), "Browser mode must clearly identify unavailable desktop services.").toBeInTheDocument());
  await userEvent.setup().click(screen.getByRole('button', { name: 'Pause monitor' }));
  expect(mockCommands.saveSettings, "Clicking monitoring controls in browser mode must not save native settings.").not.toHaveBeenCalled();
});

it('shows active and retained counts, recent entries, and links from confirmed data', () => {
  app.rules.confirm([trimRule(), regexRule()]);
  app.settings.confirm(settings);
  app.monitor.confirm({ isRunning: true, lastError: null });
  app.history.confirm(Array.from({ length: 5 }, (_, index) => historyEntry(5 - index)));
  const { container } = render(Dashboard);
  const rules = screen.getByText('Active rules').closest('section')!;
  const history = screen.getByText('Retained history').closest('section')!;
  expect(within(rules).getByText('1', { exact: false }), "The dashboard must distinguish enabled rules from the total number of saved rules.").toHaveTextContent('1 / 2');
  expect(within(history).getByText('5'), "The history count must show retained entries rather than lifetime transformations.").toBeInTheDocument();
  expect(screen.getByText('Recording is off'), "The dashboard must disclose that future history recording is off.").toBeInTheDocument();
  expect(screen.getByText('Monitoring clipboard'), "Worker health must show actual active monitoring.").toBeInTheDocument();
  expect(container.querySelectorAll('.history-item'), "The overview must limit recent history to the configured preview size.").toHaveLength(UI.RECENT_HISTORY_COUNT);
  expect(screen.getByText('Transformation #5'), "The newest retained transformation must appear in the overview.").toBeInTheDocument();
  expect(screen.queryByText('Transformation #2'), "Older entries beyond the overview limit must be omitted.").not.toBeInTheDocument();
  expect(screen.getByRole('link', { name: /Manage your pipeline/ }), "The pipeline link must navigate to the Rules page.").toHaveAttribute('href', ROUTES.RULES);
  expect(screen.getByRole('link', { name: /View history/ }), "The history link must navigate to the History page.").toHaveAttribute('href', ROUTES.HISTORY);
});

it('pauses and resumes only after confirmation and preserves unrelated settings', async () => {
  app.settings.confirm(settings);
  app.monitor.confirm({ isRunning: true, lastError: null });
  const pending = deferred<Result<AppSettings, AppError>>();
  mockCommands.saveSettings.mockReturnValueOnce(pending.promise).mockResolvedValueOnce({ status: 'ok', data: settings });
  render(Dashboard);
  const user = userEvent.setup();
  await user.dblClick(screen.getByRole('button', { name: 'Pause monitor' }));
  expect(mockCommands.saveSettings, "Double-clicking Pause must submit one complete settings update.").toHaveBeenCalledExactlyOnceWith({ ...settings, isActive: false });
  expect(screen.getByRole('button', { name: 'Pause monitor' }), "Pause must remain disabled while its save is pending.").toBeDisabled();
  expect(screen.getByText('Monitoring clipboard'), "A pending pause must not claim success before backend confirmation.").toBeInTheDocument();
  pending.resolve({ status: 'ok', data: { ...settings, isActive: false } });
  await waitFor(() => expect(screen.getByText('Paused'), "A confirmed pause must update the dashboard status.").toBeInTheDocument());
  await user.click(screen.getByRole('button', { name: 'Enable monitor' }));
  await waitFor(() => expect(screen.getByText('Monitoring clipboard'), "A confirmed resume must restore the monitoring status.").toBeInTheDocument());
  expect(mockCommands.saveSettings, "Resuming must preserve all unrelated saved settings.").toHaveBeenLastCalledWith(settings);
});

it('retains confirmed monitoring after a failed pause and permits retry', async () => {
  app.settings.confirm(settings);
  app.monitor.confirm({ isRunning: true, lastError: null });
  mockCommands.saveSettings.mockRejectedValueOnce(new Error('Database locked'))
    .mockResolvedValueOnce({ status: 'ok', data: { ...settings, isActive: false } });
  render(Dashboard);
  const user = userEvent.setup();
  await user.click(screen.getByRole('button', { name: 'Pause monitor' }));
  expect(await screen.findByRole('alert'), "A failed pause must display the database error.").toHaveTextContent('Database locked');
  expect(screen.getByText('Monitoring clipboard'), "A failed pause must retain the confirmed monitoring status.").toBeInTheDocument();
  await user.click(screen.getByRole('button', { name: 'Pause monitor' }));
  await waitFor(() => expect(screen.getByText('Paused'), "Retrying Pause successfully must change the status to Paused.").toBeInTheDocument());
  expect(screen.queryByRole('alert'), "A successful pause retry must clear the old error.").not.toBeInTheDocument();
});

it('shows a worker failure and reloads native health with Retry', async () => {
  app.settings.confirm(settings);
  app.monitor.confirm({ isRunning: false, lastError: 'Clipboard locked' });
  mockCommands.getMonitorStatus.mockResolvedValue({ status: 'ok', data: { isRunning: true, lastError: null } });
  render(Dashboard);
  expect(screen.getByText('Worker needs attention'), "A stopped worker with an error must be shown as needing attention.").toBeInTheDocument();
  expect(screen.getByRole('alert'), "The dashboard must display the reason the worker needs attention.").toHaveTextContent('Clipboard locked');
  await userEvent.setup().click(screen.getByRole('button', { name: 'Retry' }));
  await waitFor(() => expect(screen.getByText('Monitoring clipboard'), "Retry must replace stale failure status with freshly loaded healthy status.").toBeInTheDocument());
  expect(mockCommands.getMonitorStatus, "Retrying worker health must issue one backend status read.").toHaveBeenCalledOnce();
});

it('shows history loading, failure, and successful recovery', async () => {
  const pending = deferred<Result<ReturnType<typeof historyEntry>[], AppError>>();
  mockCommands.getHistory.mockReturnValueOnce(pending.promise)
    .mockResolvedValueOnce({ status: 'ok', data: [historyEntry()] });
  const load = app.history.load();
  render(Dashboard);
  expect(screen.getByRole('status'), "A pending history read must show a loading indicator.").toHaveTextContent('Loading history');
  pending.reject(new Error('History unavailable'));
  await load;
  expect(await screen.findByRole('alert'), "A failed history read must show the backend explanation.").toHaveTextContent('History unavailable');
  await userEvent.setup().click(screen.getByRole('button', { name: 'Retry' }));
  expect(await screen.findByText('Transformation #1'), "A successful history retry must display the loaded transformation.").toBeInTheDocument();
  expect(screen.queryByRole('alert'), "Successful history recovery must remove the error alert.").not.toBeInTheDocument();
});
