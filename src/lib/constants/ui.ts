import { MAX_HISTORY_ENTRIES } from '$lib/generated/bindings';
export const UI = {
  HISTORY_PAGE_SIZE: 20,
  RECENT_HISTORY_COUNT: 3,
  MAX_HISTORY_ITEMS: MAX_HISTORY_ENTRIES,
  TEXT_EXCERPT_CHARS: 2000,
} as const;
export type UiConstantKey = keyof typeof UI;
