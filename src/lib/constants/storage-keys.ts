/**
 * LocalStorage and SessionStorage key constants.
 *
 * All storage access must reference these constants — never use raw key strings.
 *
 * @example
 * import { STORAGE_KEYS } from '$lib/constants';
 * localStorage.setItem(STORAGE_KEYS.THEME_PREFERENCE, 'dark');
 */
export const STORAGE_KEYS = {
  /** Persists the user's last selected color theme ('light' | 'dark' | 'system'). */
  THEME_PREFERENCE: 'clipchain:theme',
  /** Persists the sidebar collapsed state across sessions. */
  SIDEBAR_COLLAPSED: 'clipchain:sidebar-collapsed',
  /** Persists the last active view route for restore-on-launch. */
  LAST_ACTIVE_ROUTE: 'clipchain:last-route',
} as const;

/** Union type of all valid storage keys. */
export type StorageKey = (typeof STORAGE_KEYS)[keyof typeof STORAGE_KEYS];
