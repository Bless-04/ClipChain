import { mockCommands } from '../../test/desktop-mock';
import { beforeEach, it, expect, vi } from 'vitest';
import { render, screen, waitFor, fireEvent } from '@testing-library/svelte';
import userEvent from '@testing-library/user-event';
import { trimRule, deferred } from '../../test/fixtures';
import type { Rule, Result, AppError } from '$lib/generated/bindings';
import { AppState } from '$lib/state/app.svelte';
import RulesPage from './+page.svelte';

let app: AppState;
vi.mock('$lib/state/app.svelte', async (original) => ({ ...await original<typeof import('$lib/state/app.svelte')>(), useApp: () => app }));
beforeEach(() => { app = new AppState(); app.available = true; app.rules.confirm([]); });

it('closes a successfully created rule even when the event refresh fails', async () => {
  mockCommands.saveRule.mockImplementation(async () => {
    app.rules.refresh(); return { status: 'ok', data: trimRule(10, 'Saved once') };
  });
  mockCommands.getRules.mockRejectedValue(new Error('Refresh failed'));
  render(RulesPage); const user = userEvent.setup();
  await user.click(screen.getByRole('button', { name: '+ Add rule' }));
  await user.type(screen.getByLabelText('Rule name'), 'Saved once');
  await user.click(screen.getByRole('button', { name: 'Save rule' }));
  await waitFor(() => expect(screen.queryByRole('dialog')).not.toBeInTheDocument());
  expect(screen.getByRole('heading', { name: 'Saved once' })).toBeInTheDocument();
  expect(screen.getByRole('alert')).toHaveTextContent('Refresh failed');
  expect(mockCommands.saveRule).toHaveBeenCalledTimes(1);
});

it('requires explicit confirmation to delete, allows cancel, and recovers from failure', async () => {
  app.rules.confirm([trimRule()]);
  mockCommands.deleteRule.mockRejectedValueOnce(new Error('Delete failed')).mockResolvedValueOnce({ status: 'ok', data: null });
  render(RulesPage); const user = userEvent.setup();
  await user.click(screen.getByRole('button', { name: 'Delete' }));
  expect(mockCommands.deleteRule).not.toHaveBeenCalled();
  await user.click(screen.getByRole('button', { name: 'Cancel' }));
  expect(screen.queryByRole('dialog')).not.toBeInTheDocument();
  await user.click(screen.getByRole('button', { name: 'Delete' }));
  await user.click(screen.getByRole('button', { name: 'Delete rule' }));
  expect(screen.getByRole('dialog')).toHaveTextContent('Delete failed');
  expect(app.rules.count).toBe(1);
  await user.click(screen.getByRole('button', { name: 'Delete rule' }));
  await waitFor(() => expect(screen.queryByRole('dialog')).not.toBeInTheDocument());
  expect(app.rules.count).toBe(0);
});

it('keeps an open draft intact when live data reloads', async () => {
  app.rules.confirm([trimRule()]); render(RulesPage); const user = userEvent.setup();
  await user.click(screen.getByRole('button', { name: 'Edit' }));
  await user.clear(screen.getByLabelText('Rule name')); await user.type(screen.getByLabelText('Rule name'), 'My unfinished edit');
  app.rules.confirm([trimRule(1, 'External edit')]);
  expect(screen.getByLabelText('Rule name')).toHaveValue('My unfinished edit');
  await user.click(screen.getByRole('button', { name: 'Cancel' }));
  await waitFor(() => expect(screen.getByRole('heading', { name: 'External edit' })).toBeInTheDocument());
});

it('saves an edited rule with its existing identity and updates the pipeline', async () => {
  app.rules.confirm([trimRule()]);
  const saved = trimRule(1, 'Edited');
  mockCommands.saveRule.mockResolvedValue({ status: 'ok', data: saved });
  render(RulesPage); const user = userEvent.setup();
  await user.click(screen.getByRole('button', { name: 'Edit' }));
  await user.clear(screen.getByLabelText('Rule name'));
  await user.type(screen.getByLabelText('Rule name'), 'Edited');
  await user.click(screen.getByRole('button', { name: 'Save rule' }));
  await waitFor(() => expect(screen.queryByRole('dialog')).not.toBeInTheDocument());
  expect(screen.getByRole('heading', { name: 'Edited' })).toBeInTheDocument();
  expect(mockCommands.saveRule).toHaveBeenCalledExactlyOnceWith(saved);
  expect(app.rules.count).toBe(1);
});

it('honors Escape cancellation but keeps a pending save open', async () => {
  const pending = deferred<Result<Rule, AppError>>();
  mockCommands.saveRule.mockReturnValue(pending.promise);
  render(RulesPage); const user = userEvent.setup();
  const opener = screen.getByRole('button', { name: '+ Add rule' });
  await user.click(opener);
  await fireEvent(screen.getByRole('dialog'), new Event('cancel', { cancelable: true }));
  expect(screen.queryByRole('dialog')).not.toBeInTheDocument();
  expect(opener).toHaveFocus();
  await user.click(opener);
  await user.type(screen.getByLabelText('Rule name'), 'Saving draft');
  await user.click(screen.getByRole('button', { name: 'Save rule' }));
  await fireEvent(screen.getByRole('dialog'), new Event('cancel', { cancelable: true }));
  expect(screen.getByRole('dialog')).toBeInTheDocument();
  expect(screen.getByRole('button', { name: 'Close dialog' })).toBeDisabled();
  pending.resolve({ status: 'ok', data: trimRule(10, 'Saving draft') });
  await waitFor(() => expect(screen.queryByRole('dialog')).not.toBeInTheDocument());
});
