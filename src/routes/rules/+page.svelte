<script lang="ts">
  import { useApp } from '$lib/state/app.svelte';
  import { MAX_RULES, type Rule } from '$lib/generated/bindings';
  import RuleList from '$lib/components/rules/RuleList.svelte';
  import RuleEditor from '$lib/components/rules/RuleEditor.svelte';
  import Button from '$lib/components/ui/Button.svelte';
  import Modal from '$lib/components/ui/Modal.svelte';
  import LoadError from '$lib/components/ui/LoadError.svelte';
  const app = useApp();
  let editing = $state<Rule | null>(null), editorOpen = $state(false), editorBusy = $state(false), deleting = $state<Rule | null>(null);
  function edit(rule: Rule | null) { editing = rule; editorOpen = true; }
  async function save(rule: Rule) {
    const saved = await app.rules.save(rule);
    if (!saved) throw new Error(app.rules.error ?? 'A save is already in progress. Please retry.');
    editorOpen = false;
  }
  async function remove() { if (deleting && await app.rules.delete(deleting.data.id) !== undefined) deleting = null; }
</script>
<div class="page-heading"><div><p class="eyebrow">BUILD YOUR CHAIN</p><h1>Rules</h1><p class="muted">Small steps. Cleaner text. Exactly your way.</p></div><Button variant="primary" onclick={() => edit(null)} disabled={app.rules.saving || app.rules.count >= MAX_RULES}>+ Add rule</Button></div>
<LoadError error={app.rules.error} onretry={() => { void app.rules.load(); }} busy={app.rules.loading || app.rules.saving} />
<section class="panel pipeline-panel"><div class="section-heading"><div><h2>Your pipeline</h2><p class="muted">Runs from top to bottom on each new copy.</p></div><span class="badge">{app.rules.activeRules.length} enabled · {app.rules.count} / {MAX_RULES} steps</span></div>
  {#if app.rules.loading && !app.rules.value}<p role="status">Loading rules…</p>{:else if !app.rules.count}<div class="empty"><span class="empty-symbol">⛓</span><h3>A cleaner clipboard starts here.</h3><p>Add a trim, replacement, or regex rule to create your first step.</p><Button onclick={() => edit(null)}>Create your first rule →</Button></div>{:else}<RuleList rules={app.rules.rules} busy={!app.available || app.rules.saving} onedit={edit} ondelete={(rule) => { deleting = rule; }} ontoggle={(rule) => { void app.rules.toggle(rule); }} onreorder={(ids) => { void app.rules.reorder(ids); }} />{/if}
</section>
<div class="tip"><span aria-hidden="true">↳</span><p><strong>Order makes a difference.</strong> Each step receives the previous step’s output. Use the arrows to change the sequence.</p></div>
{#if editorOpen}<Modal title={editing ? 'Edit rule' : 'Add a rule'} busy={editorBusy} onclose={() => { editorOpen = false; }}><RuleEditor initial={editing} onsave={save} oncancel={() => { editorOpen = false; }} available={app.available} onbusy={(busy) => { editorBusy = busy; }} /></Modal>{/if}
{#if deleting}<Modal title="Delete this rule?" busy={app.rules.saving} onclose={() => { deleting = null; }}><p>Remove “{deleting.data.name}” from your pipeline? This cannot be undone.</p><LoadError error={app.rules.error} onretry={remove} busy={app.rules.saving} /><footer class="modal-actions"><Button onclick={() => { deleting = null; }} disabled={app.rules.saving}>Cancel</Button><Button variant="danger" onclick={remove} busy={app.rules.saving}>Delete rule</Button></footer></Modal>{/if}
