import { mockCommands } from '../../test/desktop-mock';
import { describe, it, expect, vi } from 'vitest';
import { render, screen, fireEvent, waitFor } from '@testing-library/svelte';
import userEvent from '@testing-library/user-event';
import { trimRule, regexRule, historyEntry, settings, deferred } from '../../test/fixtures';
import { RULE_REGEX, RULE_REPLACE, type Rule, type Result, type AppError } from '$lib/generated/bindings';
import { SettingsState } from '$lib/state/settings.svelte';
import { HistoryState } from '$lib/state/history.svelte';
import RuleEditor from './rules/RuleEditor.svelte';
import RuleList from './rules/RuleList.svelte';
import HistoryPanel from './HistoryPanel.svelte';
import SettingsPanel from './SettingsPanel.svelte';

describe('rule editing', () => {
  it('creates an enabled trim rule and independently edits its edges', async () => {
    const onsave = vi.fn<(rule: Rule) => Promise<void>>().mockResolvedValue();
    render(RuleEditor, { initial: null, onsave, oncancel: vi.fn() });
    const user = userEvent.setup();
    await user.type(screen.getByLabelText('Rule name'), 'Clean edges');
    await user.click(screen.getByLabelText('Remove leading whitespace'));
    await user.click(screen.getByRole('button', { name: 'Save rule' }));
    expect(onsave).toHaveBeenCalledWith({ type: 'trim', data: { id: 0, order: 0, name: 'Clean edges', isActive: true, trimStart: false, trimEnd: true } });
  });
  it('rejects blank names and empty replace targets but accepts an empty replacement', async () => {
    const onsave = vi.fn<(rule: Rule) => Promise<void>>().mockResolvedValue();
    render(RuleEditor, { initial: null, onsave, oncancel: vi.fn() });
    const user = userEvent.setup();
    await user.click(screen.getByRole('button', { name: 'Save rule' }));
    expect(screen.getByRole('alert')).toHaveTextContent('Name must contain');
    await user.type(screen.getByLabelText('Rule name'), 'Remove');
    await user.selectOptions(screen.getByLabelText('Rule type'), RULE_REPLACE);
    await user.click(screen.getByRole('button', { name: 'Save rule' }));
    expect(screen.getByRole('alert')).toHaveTextContent('Enter the literal text');
    await user.type(screen.getByLabelText('Text to replace'), 'unwanted');
    await user.click(screen.getByRole('button', { name: 'Save rule' }));
    expect(onsave).toHaveBeenCalledWith(expect.objectContaining({ type: RULE_REPLACE, data: expect.objectContaining({ target: 'unwanted', replacement: '' }) }));
  });
  it('retains edits on failed save, prevents double submit, then retries', async () => {
    const pending = deferred<void>();
    const onsave = vi.fn<(rule: Rule) => Promise<void>>().mockReturnValueOnce(pending.promise).mockResolvedValueOnce();
    render(RuleEditor, { initial: trimRule(), onsave, oncancel: vi.fn() });
    const user = userEvent.setup();
    await user.clear(screen.getByLabelText('Rule name')); await user.type(screen.getByLabelText('Rule name'), 'Unsaved draft');
    await user.dblClick(screen.getByRole('button', { name: 'Save rule' })); expect(onsave).toHaveBeenCalledTimes(1);
    expect(screen.getByRole('button', { name: 'Saving…' })).toBeDisabled();
    pending.reject(new Error('Disk is full'));
    await waitFor(() => expect(screen.getByRole('alert')).toHaveTextContent('Disk is full'));
    expect(screen.getByLabelText('Rule name')).toHaveValue('Unsaved draft');
    await user.click(screen.getByRole('button', { name: 'Save rule' })); expect(onsave).toHaveBeenCalledTimes(2);
  });
  it('preserves an existing arbitrary raw regex and previews an inactive draft through Rust', async () => {
    const initial = regexRule(); render(RuleEditor, { initial, onsave: vi.fn(), oncancel: vi.fn() });
    expect(screen.getByLabelText('Rust regex pattern')).toHaveValue(initial.type === RULE_REGEX ? initial.data.pattern : '');
    expect(screen.getByLabelText('Enable in pipeline')).not.toBeChecked();
    mockCommands.previewRule.mockResolvedValue({ status: 'ok', data: 'native output' });
    const user = userEvent.setup(); await user.type(screen.getByLabelText('Sample text'), 'secret value');
    await user.click(screen.getByRole('button', { name: 'Preview with Rust' }));
    await waitFor(() => expect(screen.getByLabelText('Preview result')).toHaveValue('native output'));
    expect(mockCommands.previewRule).toHaveBeenCalledWith(initial, 'secret value');
  });
  it('shows Rust preview failures, retains the pattern and supports retry', async () => {
    render(RuleEditor, { initial: regexRule(), onsave: vi.fn(), oncancel: vi.fn() });
    mockCommands.previewRule.mockResolvedValueOnce({ status: 'error', error: { kind: 'validation', message: 'Unsupported look-around' } }).mockResolvedValueOnce({ status: 'ok', data: '' });
    await fireEvent.input(screen.getByLabelText('Rust regex pattern'), { target: { value: '(?=secret)' } });
    const user = userEvent.setup(); await user.click(screen.getByRole('button', { name: 'Preview with Rust' }));
    expect(await screen.findByRole('alert')).toHaveTextContent('Unsupported look-around');
    expect(screen.getByLabelText('Rust regex pattern')).toHaveValue('(?=secret)');
    await user.click(screen.getByRole('button', { name: 'Preview with Rust' }));
    await waitFor(() => expect(screen.getByLabelText('Preview result')).toHaveValue(''));
  });
  it('does not display a preview for a draft that changed while the request was pending', async () => {
    const pending = deferred<Result<string, AppError>>(); mockCommands.previewRule.mockReturnValue(pending.promise);
    render(RuleEditor, { initial: trimRule(), onsave: vi.fn(), oncancel: vi.fn() });
    const user = userEvent.setup(); await user.click(screen.getByRole('button', { name: 'Preview with Rust' }));
    await user.type(screen.getByLabelText('Sample text'), 'a different sample');
    pending.resolve({ status: 'ok', data: 'old result' });
    await waitFor(() => expect(screen.getByRole('button', { name: 'Preview with Rust' })).toBeEnabled());
    expect(screen.queryByLabelText('Preview result')).not.toBeInTheDocument();
  });
  it('escapes visual blocks and changes the raw pattern only after Apply', async () => {
    const initial = regexRule(); render(RuleEditor, { initial, onsave: vi.fn(), oncancel: vi.fn() });
    const user = userEvent.setup(); await user.click(screen.getByText('Build a pattern visually'));
    await user.type(screen.getByLabelText('Starts with'), 'file.'); await user.type(screen.getByLabelText('Contains'), 'a+b');
    await user.selectOptions(screen.getByLabelText('Then match'), '\\d+'); await user.type(screen.getByLabelText('Ends with'), '.txt');
    expect(screen.getByLabelText('Rust regex pattern')).toHaveValue(initial.type === RULE_REGEX ? initial.data.pattern : '');
    await user.click(screen.getByRole('button', { name: 'Apply built pattern' }));
    expect(screen.getByLabelText('Rust regex pattern')).toHaveValue('^file\\..*a\\+b.*\\d+.*\\.txt$');
  });
  it('allows browser-only draft exploration and cancel but disables native operations', async () => {
    const oncancel = vi.fn(); render(RuleEditor, { initial: null, onsave: vi.fn(), oncancel, available: false });
    expect(screen.getByRole('button', { name: 'Save rule' })).toBeDisabled();
    expect(screen.getByRole('button', { name: 'Preview with Rust' })).toBeDisabled();
    await userEvent.setup().click(screen.getByRole('button', { name: 'Cancel' })); expect(oncancel).toHaveBeenCalledOnce();
  });
});

describe('pipeline controls', () => {
  it('provides boundary-safe reordering, toggle, edit and explicit delete callbacks', async () => {
    const rules = [trimRule(1, 'First'), trimRule(2, 'Second')];
    const callbacks = { onedit: vi.fn(), ondelete: vi.fn(), ontoggle: vi.fn(), onreorder: vi.fn() };
    render(RuleList, { rules, busy: false, ...callbacks });
    const user = userEvent.setup();
    expect(screen.getByRole('button', { name: 'Move First up' })).toBeDisabled();
    expect(screen.getByRole('button', { name: 'Move Second down' })).toBeDisabled();
    await user.click(screen.getByRole('button', { name: 'Move First down' })); expect(callbacks.onreorder).toHaveBeenCalledWith([2, 1]);
    await user.click(screen.getByRole('switch', { name: 'Enable First' })); expect(callbacks.ontoggle).toHaveBeenCalledWith(rules[0]);
    await user.click(screen.getAllByRole('button', { name: 'Edit' })[0]!); expect(callbacks.onedit).toHaveBeenCalledWith(rules[0]);
    await user.click(screen.getAllByRole('button', { name: 'Delete' })[0]!); expect(callbacks.ondelete).toHaveBeenCalledWith(rules[0]);
  });
});

describe('history and settings', () => {
  it('renders clipboard text safely with whitespace and a change highlight', async () => {
    const state = new HistoryState(); state.confirm([historyEntry()]);
    const { container } = render(HistoryPanel, { state, available: true, recording: false });
    expect(screen.getByText(/History recording is off/)).toBeInTheDocument();
    await userEvent.setup().click(screen.getByText('Transformation #1'));
    expect(screen.getByLabelText('Before').textContent).toBe(historyEntry().originalText);
    expect(screen.getByLabelText('After').textContent).toBe(historyEntry().transformedText);
    expect(container.querySelector('img')).toBeNull(); expect(container.querySelector('del')).not.toBeNull();
  });
  it('requires clear confirmation and retains history on failure', async () => {
    const state = new HistoryState(); state.confirm([historyEntry()]);
    mockCommands.clearHistory.mockRejectedValueOnce(new Error('Database locked')).mockResolvedValueOnce({ status: 'ok', data: null });
    render(HistoryPanel, { state, available: true, recording: true });
    const user = userEvent.setup(); await user.click(screen.getByRole('button', { name: 'Clear history' }));
    expect(mockCommands.clearHistory).not.toHaveBeenCalled(); expect(screen.getByRole('dialog')).toBeInTheDocument();
    await user.click(screen.getByRole('button', { name: 'Clear all history' }));
    await waitFor(() => expect(screen.getAllByRole('alert')[0]).toHaveTextContent('Database locked'));
    expect(state.entries).toHaveLength(1);
    await user.click(screen.getByRole('button', { name: 'Clear all history' }));
    await waitFor(() => expect(screen.queryByRole('dialog')).not.toBeInTheDocument()); expect(state.entries).toHaveLength(0);
  });
  it('bounds history pages', async () => {
    const state = new HistoryState(); state.confirm(Array.from({ length: 21 }, (_, index) => historyEntry(21 - index)));
    const { container } = render(HistoryPanel, { state, available: true, recording: true });
    expect(container.querySelectorAll('.history-item')).toHaveLength(20);
    await userEvent.setup().click(screen.getByRole('button', { name: 'Next' }));
    expect(container.querySelectorAll('.history-item')).toHaveLength(1); expect(screen.getByText('Page 2 of 2')).toBeInTheDocument();
  });
  it('disables settings until loaded and shows backend-confirmed saves', async () => {
    const state = new SettingsState(); render(SettingsPanel, { state, available: true });
    expect(screen.getByRole('switch', { name: 'Monitor clipboard' })).toBeDisabled();
    state.confirm(settings); await waitFor(() => expect(screen.getByRole('switch', { name: 'Monitor clipboard' })).toBeEnabled());
    const pending = deferred<Result<typeof settings, AppError>>(); mockCommands.saveSettings.mockReturnValue(pending.promise);
    await userEvent.setup().click(screen.getByRole('switch', { name: 'Monitor clipboard' }));
    expect(screen.getByRole('switch', { name: 'Monitor clipboard' })).toHaveAttribute('aria-checked', 'true');
    expect(screen.getByRole('switch', { name: 'Record history' })).toBeDisabled();
    expect(mockCommands.saveSettings).toHaveBeenCalledWith({ ...settings, isActive: false });
    pending.resolve({ status: 'ok', data: { ...settings, isActive: false } });
    await waitFor(() => expect(screen.getByRole('switch', { name: 'Monitor clipboard' })).toHaveAttribute('aria-checked', 'false'));
  });
  it('leaves a failed settings toggle at its confirmed value', async () => {
    const state = new SettingsState(); state.confirm(settings);
    mockCommands.saveSettings.mockRejectedValue(new Error('Startup unavailable'));
    render(SettingsPanel, { state, available: true });
    await userEvent.setup().click(screen.getByRole('switch', { name: 'Launch at startup' }));
    expect(await screen.findByRole('alert')).toHaveTextContent('Startup unavailable');
    expect(screen.getByRole('switch', { name: 'Launch at startup' })).toHaveAttribute('aria-checked', 'false');
  });
});
