import { RULE_TRIM, RULE_REPLACE, RULE_REGEX, MAX_NAME_CHARS, MAX_FIELD_BYTES, type Rule, type RuleType } from '$lib/generated/bindings';

export function newRule(type: RuleType = RULE_TRIM): Rule {
  const common = { id: 0, order: 0, name: '', isActive: true };
  switch (type) {
    case RULE_TRIM: return { type, data: { ...common, trimStart: true, trimEnd: true } };
    case RULE_REPLACE: return { type, data: { ...common, target: '', replacement: '', matchCase: true } };
    case RULE_REGEX: return { type, data: { ...common, pattern: '', replacement: '' } };
  }
}
export const utf8Bytes = (text: string) => new TextEncoder().encode(text).length;
export function validateDraft(rule: Rule): string | null {
  if (!rule.data.name.trim() || [...rule.data.name].length > MAX_NAME_CHARS) return `Name must contain 1–${MAX_NAME_CHARS} Unicode characters.`;
  if (rule.type === RULE_REPLACE && !rule.data.target) return 'Enter the literal text to replace.';
  const fields = rule.type === RULE_TRIM ? [] : rule.type === RULE_REPLACE ? [rule.data.target, rule.data.replacement] : [rule.data.pattern, rule.data.replacement];
  if (fields.some((field) => utf8Bytes(field) > MAX_FIELD_BYTES)) return `Each rule field must be at most ${MAX_FIELD_BYTES.toLocaleString()} UTF-8 bytes.`;
  return null;
}
/** Escapes literal visual blocks. Pattern validation/execution belongs to Rust. */
export function escapeLiteral(value: string): string { return value.replace(/[.*+?^${}()|[\]\\]/g, '\\$&'); }
export function buildPattern(start: string, contains: string, token: string, end: string): string {
  return [start ? `^${escapeLiteral(start)}` : '', escapeLiteral(contains), token, end ? `${escapeLiteral(end)}$` : ''].filter(Boolean).join('.*');
}
export function summarizeRule(rule: Rule): string {
  switch (rule.type) {
    case RULE_TRIM: return rule.data.trimStart && rule.data.trimEnd ? 'Remove leading and trailing whitespace' : rule.data.trimStart ? 'Remove leading whitespace' : rule.data.trimEnd ? 'Remove trailing whitespace' : 'Leaves whitespace unchanged';
    case RULE_REPLACE: return `“${rule.data.target}” → “${rule.data.replacement}” · ${rule.data.matchCase ? 'Match case' : 'Ignore case'}`;
    case RULE_REGEX: return `/${rule.data.pattern}/ → ${rule.data.replacement || '(remove matches)'}`;
  }
}
