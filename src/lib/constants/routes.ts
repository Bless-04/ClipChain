export const ROUTES = {
  DASHBOARD: '/',
  RULES: '/rules',
  RULES_DETAIL: '/rules/:id',
  HISTORY: '/history',
  SETTINGS: '/settings',
} as const;

/** Union type of all valid application route paths. */
export type AppRoute = (typeof ROUTES)[keyof typeof ROUTES];
