import { RULE_TRIM, RULE_REGEX, type Rule, type HistoryEntry, type AppSettings } from '$lib/generated/bindings';
export const trimRule = (id = 1, name = 'Trim edges'): Rule => ({ type: RULE_TRIM, data: { id, name, order: id - 1, isActive: true, trimStart: true, trimEnd: true } });
export const regexRule = (): Rule => ({ type: RULE_REGEX, data: { id: 7, order: 0, name: 'Redact', isActive: false, pattern: '(?m)^(?<name>secret).*$', replacement: '${name}' } });
export const settings: AppSettings = { isActive: true, launchAtStartup: false, recordHistory: false };
export const historyEntry = (id = 1): HistoryEntry => ({ id, originalText: '  <img src=x onerror=alert(1)>\n', transformedText: '<img src=x onerror=alert(1)>', createdAt: '2026-09-06T12:00:00Z' });
export function deferred<T>() {
  let resolve!: (value: T) => void;
  let reject!: (error: unknown) => void;
  const promise = new Promise<T>((yes, no) => { resolve = yes; reject = no; });
  return { promise, resolve, reject };
}
