<script lang="ts">
  import type { SettingsState } from '$lib/state/settings.svelte';
  import { UI } from '$lib/constants/ui';
  import Switch from './ui/Switch.svelte';
  import LoadError from './ui/LoadError.svelte';
  let { state, available }: { state: SettingsState; available: boolean } = $props();
  const disabled = $derived(!available || !state.settings || state.saving);
</script>
<LoadError error={state.error} onretry={() => { void state.load(); }} busy={state.saving || state.loading} />
{#if state.loading && !state.settings}<p role="status">Loading saved settings…</p>{/if}
<section class="panel settings-panel"><h2>General</h2>
  <div class="setting"><div><h3>Monitor clipboard</h3><p>Apply enabled rules to new clipboard text. Resuming starts with a fresh baseline.</p></div><Switch label="Monitor clipboard" checked={state.settings?.isActive ?? false} {disabled} onchange={(value) => { void state.change('isActive', value); }} /></div>
  <div class="setting"><div><h3>Launch at startup</h3><p>Start ClipChain when you sign in to this computer.</p></div><Switch label="Launch at startup" checked={state.settings?.launchAtStartup ?? false} {disabled} onchange={(value) => { void state.change('launchAtStartup', value); }} /></div>
  <div class="setting"><div><h3>Record history</h3><p>Keep up to {UI.MAX_HISTORY_ITEMS} transformations locally. Turning this off stops future recording and keeps existing entries.</p></div><Switch label="Record history" checked={state.settings?.recordHistory ?? false} {disabled} onchange={(value) => { void state.change('recordHistory', value); }} /></div>
  <p class="muted" role="status">{state.saving ? 'Saving settings…' : state.settings ? 'Changes are shown after the desktop app confirms them.' : 'Saved settings are unavailable until the desktop app responds.'}</p>
</section>
<section class="panel privacy-panel"><span class="eyebrow">LOCAL BY DESIGN</span><h2>Your text stays on this computer.</h2><p>Rules, settings, and recorded clipboard text are stored in a local SQLite database. History contains ordinary, unencrypted before-and-after text. You can turn off recording or clear retained history at any time.</p></section>
