<script lang="ts">
  import { resolve } from '$app/paths';
  import { useApp } from '$lib/state/app.svelte';
  import { ROUTES } from '$lib/constants/routes';
  import { UI } from '$lib/constants/ui';
  import Button from '$lib/components/ui/Button.svelte';
  import LoadError from '$lib/components/ui/LoadError.svelte';
  import HistoryItem from '$lib/components/HistoryItem.svelte';
  const app = useApp();
</script>
<div class="page-heading"><div><p class="eyebrow">YOUR CLIPBOARD, REFINED</p><h1>A little less repetitive.</h1><p class="muted">Let your clipboard take care of the finishing touches.</p></div></div>
<section class="hero"><div><span class="hero-kicker">COPY. TRANSFORM. CARRY ON.</span><h2>Build a better<br />copy & paste.</h2><p>Chain simple rules together. Get text the way you want it, every time you copy.</p><a class="button primary" href={resolve(ROUTES.RULES)}>{app.rules.count ? 'Manage your pipeline' : 'Build your first chain'} <span aria-hidden="true">↗</span></a></div><div class="chain-art" aria-hidden="true"><div>01 <b>Trim</b><span>clean the edges</span></div><span>↓</span><div>02 <b>Replace</b><span>make it yours</span></div><span>↓</span><div>03 <b>Refine</b><span>ready to paste</span></div></div></section>
<div class="stats-grid"><section class="panel stat"><span>Clipboard monitor</span><strong class="status-text">{app.status}</strong><Button disabled={!app.available || !app.settings.settings || app.settings.saving} busy={app.settings.saving} onclick={() => { if (app.settings.settings) void app.settings.change('isActive', !app.settings.settings.isActive); }}>{app.settings.settings?.isActive ? 'Pause monitor' : 'Enable monitor'}</Button></section><section class="panel stat"><span>Active rules</span><strong>{app.rules.value ? app.rules.activeRules.length : '—'} <small>/ {app.rules.value ? app.rules.count : '—'}</small></strong><a href={resolve(ROUTES.RULES)}>View your pipeline →</a></section><section class="panel stat"><span>Retained history</span><strong>{app.history.value ? app.history.entries.length : '—'}</strong><small>{app.settings.settings?.recordHistory === false ? 'Recording is off' : `Up to ${UI.MAX_HISTORY_ITEMS} local entries`}</small></section></div>
<LoadError error={app.settings.error} onretry={() => { void app.settings.load(); }} busy={app.settings.saving} />
<LoadError error={app.rules.error} onretry={() => { void app.rules.load(); }} />
<LoadError error={app.monitor.error ?? app.monitor.value?.lastError ?? null} onretry={() => { void app.monitor.load(); }} />
<section class="panel"><div class="section-heading"><h2>Recent transformations</h2><a href={resolve(ROUTES.HISTORY)}>View history ↗</a></div><LoadError error={app.history.error} onretry={() => { void app.history.load(); }} />
{#if app.history.loading && !app.history.value}<p role="status">Loading history…</p>{:else if !app.history.entries.length}<div class="empty compact"><span class="empty-symbol">↳</span><h3>Your next copy could be the start.</h3><p>{app.settings.settings?.recordHistory === false ? 'Recording is off. Your rules still transform copied text.' : 'Once your rules transform copied text, recorded changes appear here.'}</p><a href={resolve(ROUTES.RULES)}>Explore your rules →</a></div>{:else}{#each app.history.entries.slice(0, UI.RECENT_HISTORY_COUNT) as entry (entry.id)}<HistoryItem {entry} />{/each}{/if}</section>
