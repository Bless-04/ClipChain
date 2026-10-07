import { describe, it, expect } from 'vitest';
import { MAX_NAME_CHARS, MAX_FIELD_BYTES, RULE_REPLACE, RULE_REGEX } from '$lib/generated/bindings';
import { newRule, validateDraft, escapeLiteral, buildPattern } from './draft';
import { textDiff } from '$lib/history/diff';
import { UI } from '$lib/constants/ui';

describe('draft validation and display', () => {
  it('counts Unicode characters in names, not UTF-16 units', () => {
    const rule = newRule(); rule.data.name = '😀'.repeat(MAX_NAME_CHARS);
    expect(validateDraft(rule), "A rule name at the Unicode character limit must be valid even when it contains emoji.").toBeNull(); rule.data.name += 'x'; expect(validateDraft(rule), "Adding one character beyond the name limit must produce a name error.").toContain('Name');
  });
  it('enforces UTF-8 bytes for multibyte fields', () => {
    const rule = newRule(RULE_REPLACE); rule.data.name = 'Replace';
    if (rule.type !== RULE_REPLACE) throw new Error('Wrong fixture');
    rule.data.target = 'é'.repeat(MAX_FIELD_BYTES / 2); expect(validateDraft(rule), "A multibyte target exactly at the UTF-8 byte limit must be valid.").toBeNull();
    rule.data.target += 'é'; expect(validateDraft(rule), "A multibyte target beyond the byte limit must explain the UTF-8 size restriction.").toContain('UTF-8 bytes');
  });
  it('leaves regex validation to Rust and allows empty pattern/replacement', () => {
    const rule = newRule(RULE_REGEX); rule.data.name = 'Regex'; expect(validateDraft(rule), "An empty regex pattern and replacement must be allowed for validation by Rust.").toBeNull();
  });
  it('escapes all regex metacharacters literally', () => {
    expect(escapeLiteral('.*+?^${}()|[]\\'), "Literal builder text must escape every regex metacharacter.").toBe('\\.\\*\\+\\?\\^\\$\\{\\}\\(\\)\\|\\[\\]\\\\');
    expect(buildPattern('', 'a.b', '', ''), "A dot entered as literal text must match a dot, not any character.").toBe('a\\.b');
  });
  it('highlights changes without losing whitespace or Unicode', () => {
    const diff = textDiff('😀 a\n', '😀 b\n'); expect(diff, "Change highlighting must preserve the shared emoji, spaces, and line break around the edit.").toMatchObject({ prefix: '😀 ', removed: 'a', added: 'b', suffix: '\n' });
    expect(textDiff('', 'new').added, "Adding text to an empty input must highlight all new text.").toBe('new'); expect(textDiff('old', '').removed, "Removing all text must highlight the complete original text as deleted.").toBe('old');
  });
  it('bounds long-text rendering', () => {
    const diff = textDiff('a'.repeat(UI.TEXT_EXCERPT_CHARS + 1), 'b');
    expect(diff.truncated, "Long-text differences must indicate that the displayed text was truncated.").toBe(true); expect(diff.removed.length, "Rendered deleted text must not exceed the configured excerpt length.").toBe(UI.TEXT_EXCERPT_CHARS);
  });
  it('never splits a Unicode character at an excerpt boundary', () => {
    const input = 'a'.repeat(UI.TEXT_EXCERPT_CHARS - 1) + '😀x';
    const diff = textDiff(input, '');
    expect(diff.removed.endsWith('😀'), "An excerpt boundary must preserve a whole emoji rather than half a UTF-16 pair.").toBe(true); expect(diff.truncated, "Leaving characters beyond the complete emoji must still mark the excerpt as truncated.").toBe(true);
  });
});
