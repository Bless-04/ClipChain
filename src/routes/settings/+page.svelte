<script lang="ts">
  import { useApp } from '$lib/state/app.svelte';
  import SettingsPanel from '$lib/components/SettingsPanel.svelte';
  const app = useApp();
</script>
<div class="page-heading"><div><p class="eyebrow">MAKE YOURSELF AT HOME</p><h1>Settings</h1><p class="muted">Set the rhythm for your clipboard.</p></div></div>
<SettingsPanel state={app.settings} available={app.available} />
