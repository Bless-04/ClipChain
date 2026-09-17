/**
 * Type-safe route path constants.
 *
 * All navigation in the application must reference these constants.
 * Never hardcode route strings in components or routers.
 *
 * @example
 * import { ROUTES } from '$lib/constants';
 * goto(ROUTES.RULES);
 */
export const ROUTES = {
  DASHBOARD: '/',
  RULES: '/rules',
  HISTORY: '/history',
  SETTINGS: '/settings',
} as const;

/** Union type of all valid application route paths. */
export type AppRoute = (typeof ROUTES)[keyof typeof ROUTES];
