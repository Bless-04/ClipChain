<script lang="ts">
  import { useApp } from '$lib/state/app.svelte';
  import HistoryPanel from '$lib/components/HistoryPanel.svelte';
  const app = useApp();
</script>
<HistoryPanel state={app.history} available={app.available} recording={app.settings.settings?.recordHistory} />
