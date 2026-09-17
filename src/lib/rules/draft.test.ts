import { describe, it, expect } from 'vitest';
import { MAX_NAME_CHARS, MAX_FIELD_BYTES, RULE_REPLACE, RULE_REGEX } from '$lib/generated/bindings';
import { newRule, validateDraft, escapeLiteral, buildPattern } from './draft';
import { textDiff } from '$lib/history/diff';
import { UI } from '$lib/constants/ui';

describe('draft validation and display', () => {
  it('counts Unicode characters in names, not UTF-16 units', () => {
    const rule = newRule(); rule.data.name = '😀'.repeat(MAX_NAME_CHARS);
    expect(validateDraft(rule)).toBeNull(); rule.data.name += 'x'; expect(validateDraft(rule)).toContain('Name');
  });
  it('enforces UTF-8 bytes for multibyte fields', () => {
    const rule = newRule(RULE_REPLACE); rule.data.name = 'Replace';
    if (rule.type !== RULE_REPLACE) throw new Error('Wrong fixture');
    rule.data.target = 'é'.repeat(MAX_FIELD_BYTES / 2); expect(validateDraft(rule)).toBeNull();
    rule.data.target += 'é'; expect(validateDraft(rule)).toContain('UTF-8 bytes');
  });
  it('leaves regex validation to Rust and allows empty pattern/replacement', () => {
    const rule = newRule(RULE_REGEX); rule.data.name = 'Regex'; expect(validateDraft(rule)).toBeNull();
  });
  it('escapes all regex metacharacters literally', () => {
    expect(escapeLiteral('.*+?^${}()|[]\\')).toBe('\\.\\*\\+\\?\\^\\$\\{\\}\\(\\)\\|\\[\\]\\\\');
    expect(buildPattern('', 'a.b', '', '')).toBe('a\\.b');
  });
  it('highlights changes without losing whitespace or Unicode', () => {
    const diff = textDiff('😀 a\n', '😀 b\n'); expect(diff).toMatchObject({ prefix: '😀 ', removed: 'a', added: 'b', suffix: '\n' });
    expect(textDiff('', 'new').added).toBe('new'); expect(textDiff('old', '').removed).toBe('old');
  });
  it('bounds long-text rendering', () => {
    const diff = textDiff('a'.repeat(UI.TEXT_EXCERPT_CHARS + 1), 'b');
    expect(diff.truncated).toBe(true); expect(diff.removed.length).toBe(UI.TEXT_EXCERPT_CHARS);
  });
  it('never splits a Unicode character at an excerpt boundary', () => {
    const input = 'a'.repeat(UI.TEXT_EXCERPT_CHARS - 1) + '😀x';
    const diff = textDiff(input, '');
    expect(diff.removed.endsWith('😀')).toBe(true); expect(diff.truncated).toBe(true);
  });
});
