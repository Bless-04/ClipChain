<script lang="ts">
  import { resolve } from '$app/paths';
  import { onMount, type Snippet } from 'svelte';
  import { page } from '$app/state';
  import { ROUTES } from '$lib/constants/routes';
  import { provideApp } from '$lib/state/app.svelte';
  import LoadError from '$lib/components/ui/LoadError.svelte';
  import '../app.css';
  let { children }: { children: Snippet } = $props();
  const app = provideApp();
  const navigation = [
    { href: ROUTES.DASHBOARD, label: 'Overview', icon: '◫' },
    { href: ROUTES.RULES, label: 'Rules', icon: '⛓' },
    { href: ROUTES.HISTORY, label: 'History', icon: '↳' },
    { href: ROUTES.SETTINGS, label: 'Settings', icon: '⚙' },
  ];
  onMount(() => { app.start(); return () => app.stop(); });
</script>
<svelte:head><title>ClipChain — Your clipboard, refined</title><meta name="description" content="Build a local pipeline that cleans and transforms copied text." /></svelte:head>
<a class="skip-link" href="#main-content">Skip to content</a>
<div class="app-shell"><aside class="sidebar"><a href={resolve(ROUTES.DASHBOARD)} class="brand"><span class="brand-mark" aria-hidden="true">↔</span>ClipChain<span class="brand-dot">.</span></a><p class="sidebar-caption">A LITTLE ORDER. EVERY COPY.</p>
  <nav aria-label="Main navigation">{#each navigation as item (item.href)}<a href={resolve(item.href)} class:active={page.url.pathname === item.href} aria-current={page.url.pathname === item.href ? 'page' : undefined}><span aria-hidden="true">{item.icon}</span>{item.label}{#if item.href === ROUTES.RULES && app.rules.value}<small>{app.rules.count}</small>{/if}</a>{/each}</nav>
  <div class="sidebar-bottom"><span class="status-dot" class:running={app.available && app.monitor.value?.isRunning && !app.monitor.value.lastError && app.settings.settings?.isActive}></span><span>{app.status}</span><p>Made for your workflow.<br />Stored on your device.</p><span class="version">CLIPCHAIN / 0.1</span></div>
</aside><div class="workspace"><header class="topbar"><span>Personal workspace <span class="muted">/ {navigation.find((item) => item.href === page.url.pathname)?.label ?? 'ClipChain'}</span></span><span class="local-label">◉ &nbsp; LOCAL FIRST</span></header>
<main id="main-content" tabindex="-1">
  {#if !app.available}<div class="browser-notice"><strong>Browser preview</strong><span>Explore the interface. Clipboard monitoring and saved data are available in the ClipChain desktop app.</span></div>{/if}
  <LoadError error={app.connectionError} onretry={() => app.start()} />
  {@render children()}
</main><footer class="workspace-footer">Good text in. Better text out.<span>ClipChain</span></footer></div></div>
