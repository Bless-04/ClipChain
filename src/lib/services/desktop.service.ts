import { isTauri } from '@tauri-apps/api/core';
import { events, type MonitorStatus } from '$lib/generated/bindings';
type Unlisten = () => void;
export interface DesktopHandlers {
  rules: () => void;
  history: () => void;
  settings: () => void;
  monitor: (status: MonitorStatus) => void;
  error: (error: unknown) => void;
}
export const desktopService = {
  available: () => isTauri(),
  subscribe(handlers: DesktopHandlers) {
    let disposed = false;
    const listeners: Unlisten[] = [];
    const release = (unlisten: Unlisten) => {
      try { unlisten(); } catch (error: unknown) { handlers.error(error); }
    };
    const dispose = () => { disposed = true; listeners.splice(0).forEach(release); };
    const registrations = [
      () => events.rulesUpdated.listen(() => { if (!disposed) handlers.rules(); }),
      () => events.clipboardChanged.listen(() => { if (!disposed) handlers.history(); }),
      () => events.historyCleared.listen(() => { if (!disposed) handlers.history(); }),
      () => events.settingsUpdated.listen(() => { if (!disposed) handlers.settings(); }),
      () => events.monitorChanged.listen(({ payload }) => { if (!disposed) handlers.monitor(payload); }),
    ];
    const ready = (async () => {
      try {
        for (const register of registrations) {
          if (disposed) return;
          const unlisten = await register();
          if (disposed) release(unlisten); else listeners.push(unlisten);
        }
      } catch (error: unknown) { dispose(); throw error; }
    })();
    return { ready, dispose };
  },
};
