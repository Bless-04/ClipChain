/**
 * Barrel export for all frontend constants.
 *
 * Import from here rather than from individual files:
 * @example
 * import { ROUTES, APP_EVENTS, STORAGE_KEYS, UI } from '$lib/constants';
 */
export { APP_EVENTS } from './events';
export type { AppEvent } from './events';

export { ROUTES } from './routes';
export type { AppRoute } from './routes';

export { STORAGE_KEYS } from './storage-keys';
export type { StorageKey } from './storage-keys';

export { UI } from './ui';
export type { UiConstantKey } from './ui';
