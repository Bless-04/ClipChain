import type { AppError, Result } from '$lib/generated/bindings';
import { isTauri } from '@tauri-apps/api/core';

export const DESKTOP_UNAVAILABLE = 'Open ClipChain on your desktop to use native clipboard features.';
export function errorMessage(error: unknown): string {
  if (error instanceof Error) return error.message;
  if (typeof error === 'string') return error;
  if (typeof error === 'object' && error !== null && 'message' in error && typeof error.message === 'string') return error.message;
  return 'An unexpected error occurred. Please retry.';
}
export async function request<T>(operation: string, run: () => Promise<Result<T, AppError>>): Promise<T> {
  try {
    if (!isTauri()) throw new Error(DESKTOP_UNAVAILABLE);
    const result = await run();
    if (result.status === 'error') throw new Error(errorMessage(result.error));
    return result.data;
  } catch (error: unknown) { throw new Error(`${operation}: ${errorMessage(error)}`, { cause: error }); }
}
