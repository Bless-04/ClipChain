<script lang="ts">
  import { untrack } from 'svelte';
  import { RULE_TRIM, RULE_REPLACE, RULE_REGEX, MAX_TEXT_BYTES, type Rule, type RuleType } from '$lib/generated/bindings';
  import { RULE_LABELS } from '$lib/constants/rules';
  import { newRule, validateDraft, utf8Bytes } from '$lib/rules/draft';
  import { rulesService } from '$lib/services/rules.service';
  import { errorMessage } from '$lib/services/result';
  import Input from '../ui/Input.svelte';
  import Button from '../ui/Button.svelte';
  import TrimRuleEditor from './TrimRuleEditor.svelte';
  import ReplaceRuleEditor from './ReplaceRuleEditor.svelte';
  import RegexRuleEditor from './RegexRuleEditor.svelte';
  let { initial, onsave, oncancel, available = true, onbusy = () => {} }: {
    initial: Rule | null; onsave: (rule: Rule) => Promise<void>; oncancel: () => void; available?: boolean; onbusy?: (busy: boolean) => void;
  } = $props();
  let draft = $state<Rule>(untrack(() => initial ? structuredClone($state.snapshot(initial)) : newRule()));
  let saving = $state(false), previewing = $state(false), saveError = $state<string | null>(null), previewError = $state<string | null>(null);
  let input = $state(''), preview = $state<{ signature: string; output: string } | null>(null);
  const signature = $derived(JSON.stringify([draft, input]));
  const validation = $derived(validateDraft(draft));
  function changeType(type: RuleType) {
    const next = newRule(type);
    next.data.name = draft.data.name; next.data.isActive = draft.data.isActive; draft = next;
  }
  async function save() {
    if (saving) return;
    saveError = validation;
    if (saveError) return;
    saving = true; onbusy(true);
    try { await onsave($state.snapshot(draft)); }
    catch (error: unknown) { saveError = errorMessage(error); }
    finally { saving = false; onbusy(false); }
  }
  async function runPreview() {
    if (previewing) return;
    previewError = validation;
    if (utf8Bytes(input) > MAX_TEXT_BYTES) previewError = `Preview input must be at most ${MAX_TEXT_BYTES.toLocaleString()} UTF-8 bytes.`;
    if (previewError) return;
    previewing = true; preview = null;
    const requested = signature;
    try { const output = await rulesService.preview($state.snapshot(draft), input); if (requested === signature) preview = { signature: requested, output }; }
    catch (error: unknown) { if (requested === signature) previewError = errorMessage(error); }
    finally { previewing = false; }
  }
</script>
<form onsubmit={(event) => { event.preventDefault(); void save(); }}>
  <fieldset disabled={saving}>
    <Input label="Rule name" bind:value={draft.data.name} placeholder="Give this step a useful name" />
    {#if !initial}<label class="field">Rule type<select value={draft.type} onchange={(event) => { const value = event.currentTarget.value; if (value === RULE_TRIM || value === RULE_REPLACE || value === RULE_REGEX) changeType(value); }}>
      {#each Object.entries(RULE_LABELS) as [type, label] (type)}<option value={type}>{label}</option>{/each}
    </select></label>{/if}
    {#if draft.type === RULE_TRIM}<TrimRuleEditor bind:data={draft.data} />{:else if draft.type === RULE_REPLACE}<ReplaceRuleEditor bind:data={draft.data} />{:else}<RegexRuleEditor bind:data={draft.data} />{/if}
    <label class="check-row"><input type="checkbox" bind:checked={draft.data.isActive} /> Enable in pipeline</label>
    <section class="preview-panel"><h3>Try it first</h3><p class="muted">Tests this draft, even when disabled. Your clipboard is unchanged.</p>
      <Input label="Sample text" bind:value={input} multiline />
      <Button onclick={runPreview} busy={previewing} disabled={!available}>{previewing ? 'Testing…' : 'Preview with Rust'}</Button>
      {#if previewError}<p role="alert" class="error">{previewError}</p>{/if}
      {#if preview && preview.signature === signature}<label class="field">Preview result<textarea readonly value={preview.output} rows="3"></textarea></label>{/if}
    </section>
    {#if !available}<p class="notice">Open the desktop app to preview and save this rule.</p>{/if}
    {#if saveError}<p role="alert" class="error">{saveError}</p>{/if}
  </fieldset>
  <footer class="modal-actions"><Button onclick={oncancel} disabled={saving}>Cancel</Button><Button type="submit" variant="primary" busy={saving} disabled={!available}>{saving ? 'Saving…' : 'Save rule'}</Button></footer>
</form>
