import { mockCommands } from '../../test/desktop-mock';
import { describe, it, expect, vi } from 'vitest';
import { render, screen, fireEvent, waitFor } from '@testing-library/svelte';
import userEvent from '@testing-library/user-event';
import { trimRule, regexRule, historyEntry, settings, deferred } from '../../test/fixtures';
import { RULE_REGEX, RULE_REPLACE, MAX_TEXT_BYTES, type Rule, type Result, type AppError } from '$lib/generated/bindings';
import { SettingsState } from '$lib/state/settings.svelte';
import { HistoryState } from '$lib/state/history.svelte';
import RuleEditor from './rules/RuleEditor.svelte';
import RuleList from './rules/RuleList.svelte';
import HistoryPanel from './HistoryPanel.svelte';
import SettingsPanel from './SettingsPanel.svelte';

describe('rule editing', () => {
  it('saves literal replacement options without interpreting capture syntax', async () => {
    const onsave = vi.fn<(rule: Rule) => Promise<void>>().mockResolvedValue();
    const initial: Rule = { type: RULE_REPLACE, data: { id: 4, order: 2, name: 'Replace', isActive: true, target: 'old', replacement: '', matchCase: true } };
    render(RuleEditor, { initial, onsave, oncancel: vi.fn() });
    expect(screen.queryByLabelText('Rule type'), "Editing a saved rule must not offer to change its rule type.").not.toBeInTheDocument();
    const user = userEvent.setup();
    await user.clear(screen.getByLabelText('Text to replace'));
    await fireEvent.input(screen.getByLabelText('Text to replace'), { target: { value: 'a\nb' } });
    await user.type(screen.getByLabelText('Replacement'), '$1');
    await user.click(screen.getByLabelText('Match case'));
    await user.click(screen.getByRole('button', { name: 'Save rule' }));
    expect(onsave, "Saving a literal replacement must preserve line breaks and treat $1 as literal text.").toHaveBeenCalledWith({ type: RULE_REPLACE, data: { ...initial.data, target: 'a\nb', replacement: '$1', matchCase: false } });
    expect(initial.data.target, "Editing the draft must not mutate the original saved rule object.").toBe('old');
  });
  it('keeps a new rule name and enabled choice when switching editor types', async () => {
    render(RuleEditor, { initial: null, onsave: vi.fn(), oncancel: vi.fn() });
    const user = userEvent.setup();
    await user.type(screen.getByLabelText('Rule name'), 'Draft');
    await user.click(screen.getByLabelText('Enable in pipeline'));
    await user.selectOptions(screen.getByLabelText('Rule type'), RULE_REGEX);
    expect(screen.getByLabelText('Rule name'), "Switching editor types must keep the name already entered.").toHaveValue('Draft');
    expect(screen.getByLabelText('Enable in pipeline'), "Switching editor types must keep the user's disabled choice.").not.toBeChecked();
    expect(screen.getByLabelText('Rust regex pattern'), "A newly selected regex editor must start with an empty pattern.").toHaveValue('');
  });
  it('rejects oversized multibyte preview input before calling native code', async () => {
    render(RuleEditor, { initial: trimRule(), onsave: vi.fn(), oncancel: vi.fn() });
    await fireEvent.input(screen.getByLabelText('Sample text'), { target: { value: 'é'.repeat(MAX_TEXT_BYTES / 2 + 1) } });
    await userEvent.setup().click(screen.getByRole('button', { name: 'Preview with Rust' }));
    expect(screen.getByRole('alert'), "Oversized preview input must show a useful size-limit error.").toHaveTextContent('Preview input must be at most');
    expect(mockCommands.previewRule, "Input over the UTF-8 byte limit must be rejected before invoking Rust.").not.toHaveBeenCalled();
  });
  it('creates an enabled trim rule and independently edits its edges', async () => {
    const onsave = vi.fn<(rule: Rule) => Promise<void>>().mockResolvedValue();
    render(RuleEditor, { initial: null, onsave, oncancel: vi.fn() });
    const user = userEvent.setup();
    await user.type(screen.getByLabelText('Rule name'), 'Clean edges');
    await user.click(screen.getByLabelText('Remove leading whitespace'));
    await user.click(screen.getByRole('button', { name: 'Save rule' }));
    expect(onsave, "A new trim rule must remain enabled and preserve independent leading/trailing trim choices.").toHaveBeenCalledWith({ type: 'trim', data: { id: 0, order: 0, name: 'Clean edges', isActive: true, trimStart: false, trimEnd: true } });
  });
  it('rejects blank names and empty replace targets but accepts an empty replacement', async () => {
    const onsave = vi.fn<(rule: Rule) => Promise<void>>().mockResolvedValue();
    render(RuleEditor, { initial: null, onsave, oncancel: vi.fn() });
    const user = userEvent.setup();
    await user.click(screen.getByRole('button', { name: 'Save rule' }));
    expect(screen.getByRole('alert'), "Saving a blank rule name must explain the name requirement.").toHaveTextContent('Name must contain');
    await user.type(screen.getByLabelText('Rule name'), 'Remove');
    await user.selectOptions(screen.getByLabelText('Rule type'), RULE_REPLACE);
    await user.click(screen.getByRole('button', { name: 'Save rule' }));
    expect(screen.getByRole('alert'), "A literal replacement without a target must explain which text is missing.").toHaveTextContent('Enter the literal text');
    await user.type(screen.getByLabelText('Text to replace'), 'unwanted');
    await user.click(screen.getByRole('button', { name: 'Save rule' }));
    expect(onsave, "An empty replacement must be accepted so a rule can remove matching text.").toHaveBeenCalledWith(expect.objectContaining({ type: RULE_REPLACE, data: expect.objectContaining({ target: 'unwanted', replacement: '' }) }));
  });
  it('retains edits on failed save, prevents double submit, then retries', async () => {
    const pending = deferred<void>();
    const onsave = vi.fn<(rule: Rule) => Promise<void>>().mockReturnValueOnce(pending.promise).mockResolvedValueOnce();
    render(RuleEditor, { initial: trimRule(), onsave, oncancel: vi.fn() });
    const user = userEvent.setup();
    await user.clear(screen.getByLabelText('Rule name')); await user.type(screen.getByLabelText('Rule name'), 'Unsaved draft');
    await user.dblClick(screen.getByRole('button', { name: 'Save rule' })); expect(onsave, "Double-clicking Save while a request is pending must submit only once.").toHaveBeenCalledTimes(1);
    expect(screen.getByRole('button', { name: 'Saving…' }), "The saving button must stay disabled until the request completes.").toBeDisabled();
    pending.reject(new Error('Disk is full'));
    await waitFor(() => expect(screen.getByRole('alert'), "A failed save must show the backend's explanation.").toHaveTextContent('Disk is full'));
    expect(screen.getByLabelText('Rule name'), "A save failure must preserve the user's unfinished rule name.").toHaveValue('Unsaved draft');
    await user.click(screen.getByRole('button', { name: 'Save rule' })); expect(onsave, "After failure, retry must submit the preserved draft once more.").toHaveBeenCalledTimes(2);
  });
  it('preserves an existing arbitrary raw regex and previews an inactive draft through Rust', async () => {
    const initial = regexRule(); render(RuleEditor, { initial, onsave: vi.fn(), oncancel: vi.fn() });
    expect(screen.getByLabelText('Rust regex pattern'), "Opening an existing regex rule must preserve its raw pattern exactly.").toHaveValue(initial.type === RULE_REGEX ? initial.data.pattern : '');
    expect(screen.getByLabelText('Enable in pipeline'), "Opening an inactive regex rule must preserve its disabled state.").not.toBeChecked();
    mockCommands.previewRule.mockResolvedValue({ status: 'ok', data: 'native output' });
    const user = userEvent.setup(); await user.type(screen.getByLabelText('Sample text'), 'secret value');
    await user.click(screen.getByRole('button', { name: 'Preview with Rust' }));
    await waitFor(() => expect(screen.getByLabelText('Preview result'), "The preview result must display the text returned by Rust.").toHaveValue('native output'));
    expect(mockCommands.previewRule, "Preview must send the inactive draft and sample text to Rust unchanged.").toHaveBeenCalledWith(initial, 'secret value');
  });
  it('shows Rust preview failures, retains the pattern and supports retry', async () => {
    render(RuleEditor, { initial: regexRule(), onsave: vi.fn(), oncancel: vi.fn() });
    mockCommands.previewRule.mockResolvedValueOnce({ status: 'error', error: { kind: 'validation', message: 'Unsupported look-around' } }).mockResolvedValueOnce({ status: 'ok', data: '' });
    await fireEvent.input(screen.getByLabelText('Rust regex pattern'), { target: { value: '(?=secret)' } });
    const user = userEvent.setup(); await user.click(screen.getByRole('button', { name: 'Preview with Rust' }));
    expect(await screen.findByRole('alert'), "A rejected regex preview must display Rust's validation explanation.").toHaveTextContent('Unsupported look-around');
    expect(screen.getByLabelText('Rust regex pattern'), "A failed preview must preserve the pattern so the user can edit or retry it.").toHaveValue('(?=secret)');
    await user.click(screen.getByRole('button', { name: 'Preview with Rust' }));
    await waitFor(() => expect(screen.getByLabelText('Preview result'), "A successful retry with empty output must still display a valid preview result.").toHaveValue(''));
  });
  it('does not display a preview for a draft that changed while the request was pending', async () => {
    const pending = deferred<Result<string, AppError>>(); mockCommands.previewRule.mockReturnValue(pending.promise);
    render(RuleEditor, { initial: trimRule(), onsave: vi.fn(), oncancel: vi.fn() });
    const user = userEvent.setup(); await user.click(screen.getByRole('button', { name: 'Preview with Rust' }));
    await user.type(screen.getByLabelText('Sample text'), 'a different sample');
    pending.resolve({ status: 'ok', data: 'old result' });
    await waitFor(() => expect(screen.getByRole('button', { name: 'Preview with Rust' }), "Once an outdated preview request finishes, the Preview button must become usable again.").toBeEnabled());
    expect(screen.queryByLabelText('Preview result'), "A result for an earlier draft must not be displayed as the current preview.").not.toBeInTheDocument();
  });
  it('escapes visual blocks and changes the raw pattern only after Apply', async () => {
    const initial = regexRule(); render(RuleEditor, { initial, onsave: vi.fn(), oncancel: vi.fn() });
    const user = userEvent.setup(); await user.click(screen.getByText('Build a pattern visually'));
    await user.type(screen.getByLabelText('Starts with'), 'file.'); await user.type(screen.getByLabelText('Contains'), 'a+b');
    await user.selectOptions(screen.getByLabelText('Then match'), '\\d+'); await user.type(screen.getByLabelText('Ends with'), '.txt');
    expect(screen.getByLabelText('Rust regex pattern'), "Changing visual builder fields must not overwrite the raw regex until Apply is clicked.").toHaveValue(initial.type === RULE_REGEX ? initial.data.pattern : '');
    await user.click(screen.getByRole('button', { name: 'Apply built pattern' }));
    expect(screen.getByLabelText('Rust regex pattern'), "Applying the visual pattern must escape literal dots and plus signs while retaining the chosen digit matcher.").toHaveValue('^file\\..*a\\+b.*\\d+.*\\.txt$');
  });
  it('allows browser-only draft exploration and cancel but disables native operations', async () => {
    const oncancel = vi.fn(); render(RuleEditor, { initial: null, onsave: vi.fn(), oncancel, available: false });
    expect(screen.getByRole('button', { name: 'Save rule' }), "Browser-only draft editing must not allow native saves.").toBeDisabled();
    expect(screen.getByRole('button', { name: 'Preview with Rust' }), "Browser-only draft editing must not allow Rust previews.").toBeDisabled();
    await userEvent.setup().click(screen.getByRole('button', { name: 'Cancel' })); expect(oncancel, "Cancel must still work when native operations are unavailable.").toHaveBeenCalledOnce();
  });
});

describe('pipeline controls', () => {
  it('disables every mutation while the pipeline is busy', async () => {
    const callbacks = { onedit: vi.fn(), ondelete: vi.fn(), ontoggle: vi.fn(), onreorder: vi.fn() };
    render(RuleList, { rules: [trimRule(1), trimRule(2)], busy: true, ...callbacks });
    for (const control of [...screen.getAllByRole('button'), ...screen.getAllByRole('switch')]) {
      expect(control, "Every pipeline mutation control must be disabled while a mutation is pending.").toBeDisabled(); await userEvent.setup().click(control);
    }
    Object.values(callbacks).forEach((callback) => expect(callback, "Clicking disabled pipeline controls must not call any mutation callback.").not.toHaveBeenCalled());
  });
  it('provides boundary-safe reordering, toggle, edit and explicit delete callbacks', async () => {
    const rules = [trimRule(1, 'First'), trimRule(2, 'Second')];
    const callbacks = { onedit: vi.fn(), ondelete: vi.fn(), ontoggle: vi.fn(), onreorder: vi.fn() };
    render(RuleList, { rules, busy: false, ...callbacks });
    const user = userEvent.setup();
    expect(screen.getByRole('button', { name: 'Move First up' }), "The first rule cannot move further up the pipeline.").toBeDisabled();
    expect(screen.getByRole('button', { name: 'Move Second down' }), "The last rule cannot move further down the pipeline.").toBeDisabled();
    await user.click(screen.getByRole('button', { name: 'Move First down' })); expect(callbacks.onreorder, "Moving a rule must submit the complete new ID order.").toHaveBeenCalledWith([2, 1]);
    await user.click(screen.getByRole('switch', { name: 'Enable First' })); expect(callbacks.ontoggle, "Toggling a rule must target the selected rule.").toHaveBeenCalledWith(rules[0]);
    await user.click(screen.getAllByRole('button', { name: 'Edit' })[0]!); expect(callbacks.onedit, "Edit must open the selected rule.").toHaveBeenCalledWith(rules[0]);
    await user.click(screen.getAllByRole('button', { name: 'Delete' })[0]!); expect(callbacks.ondelete, "Delete must request deletion of the selected rule.").toHaveBeenCalledWith(rules[0]);
  });
});

describe('history and settings', () => {
  it('renders clipboard text safely with whitespace and a change highlight', async () => {
    const state = new HistoryState(); state.confirm([historyEntry()]);
    const { container } = render(HistoryPanel, { state, available: true, recording: false });
    expect(screen.getByText(/History recording is off/), "History must explain when future recording is turned off.").toBeInTheDocument();
    await userEvent.setup().click(screen.getByText('Transformation #1'));
    expect(screen.getByLabelText('Before').textContent, "The Before view must preserve original clipboard text and whitespace.").toBe(historyEntry().originalText);
    expect(screen.getByLabelText('After').textContent, "The After view must preserve the transformed clipboard text.").toBe(historyEntry().transformedText);
    expect(container.querySelector('img'), "Clipboard text containing HTML must be displayed as text, not create an image element.").toBeNull(); expect(container.querySelector('del'), "Removed text must be visibly marked as a deletion.").not.toBeNull();
  });
  it('requires clear confirmation and retains history on failure', async () => {
    const state = new HistoryState(); state.confirm([historyEntry()]);
    mockCommands.clearHistory.mockRejectedValueOnce(new Error('Database locked')).mockResolvedValueOnce({ status: 'ok', data: null });
    render(HistoryPanel, { state, available: true, recording: true });
    const user = userEvent.setup(); await user.click(screen.getByRole('button', { name: 'Clear history' }));
    expect(mockCommands.clearHistory, "Opening the clear dialog must not clear history before confirmation.").not.toHaveBeenCalled(); expect(screen.getByRole('dialog'), "Clearing history must present an explicit confirmation dialog.").toBeInTheDocument();
    await user.click(screen.getByRole('button', { name: 'Clear all history' }));
    await waitFor(() => expect(screen.getAllByRole('alert')[0], "A failed history clear must display the database error.").toHaveTextContent('Database locked'));
    expect(state.entries, "A failed clear must preserve the visible history entry.").toHaveLength(1);
    await user.click(screen.getByRole('button', { name: 'Clear all history' }));
    await waitFor(() => expect(screen.queryByRole('dialog'), "A successful retry must close the clear-history dialog.").not.toBeInTheDocument()); expect(state.entries, "History entries must disappear only after a successful clear.").toHaveLength(0);
  });
  it('bounds history pages', async () => {
    const state = new HistoryState(); state.confirm(Array.from({ length: 21 }, (_, index) => historyEntry(21 - index)));
    const { container } = render(HistoryPanel, { state, available: true, recording: true });
    expect(container.querySelectorAll('.history-item'), "The first history page must contain at most 20 entries.").toHaveLength(20);
    await userEvent.setup().click(screen.getByRole('button', { name: 'Next' }));
    expect(container.querySelectorAll('.history-item'), "The second page must show only the one remaining entry.").toHaveLength(1); expect(screen.getByText('Page 2 of 2'), "Pagination must identify the selected page and total page count.").toBeInTheDocument();
  });
  it('disables settings until loaded and shows backend-confirmed saves', async () => {
    const state = new SettingsState(); render(SettingsPanel, { state, available: true });
    expect(screen.getByRole('switch', { name: 'Monitor clipboard' }), "Settings controls must remain disabled until saved values are known.").toBeDisabled();
    state.confirm(settings); await waitFor(() => expect(screen.getByRole('switch', { name: 'Monitor clipboard' }), "Settings controls must become available once saved values are loaded.").toBeEnabled());
    const pending = deferred<Result<typeof settings, AppError>>(); mockCommands.saveSettings.mockReturnValue(pending.promise);
    await userEvent.setup().click(screen.getByRole('switch', { name: 'Monitor clipboard' }));
    expect(screen.getByRole('switch', { name: 'Monitor clipboard' }), "A pending pause request must retain the last confirmed monitoring value.").toHaveAttribute('aria-checked', 'true');
    expect(screen.getByRole('switch', { name: 'Record history' }), "Other settings controls must be disabled while a save is pending.").toBeDisabled();
    expect(mockCommands.saveSettings, "Pausing must send the complete settings object with only the active field changed.").toHaveBeenCalledWith({ ...settings, isActive: false });
    pending.resolve({ status: 'ok', data: { ...settings, isActive: false } });
    await waitFor(() => expect(screen.getByRole('switch', { name: 'Monitor clipboard' }), "The monitor switch must show paused only after the backend confirms it.").toHaveAttribute('aria-checked', 'false'));
  });
  it('leaves a failed settings toggle at its confirmed value', async () => {
    const state = new SettingsState(); state.confirm(settings);
    mockCommands.saveSettings.mockRejectedValue(new Error('Startup unavailable'));
    render(SettingsPanel, { state, available: true });
    await userEvent.setup().click(screen.getByRole('switch', { name: 'Launch at startup' }));
    expect(await screen.findByRole('alert'), "A failed startup setting change must display the backend error.").toHaveTextContent('Startup unavailable');
    expect(screen.getByRole('switch', { name: 'Launch at startup' }), "A failed startup toggle must keep the last confirmed disabled value.").toHaveAttribute('aria-checked', 'false');
  });
});
