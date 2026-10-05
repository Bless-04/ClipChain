import { RULE_TRIM, RULE_REPLACE, RULE_REGEX, type RuleType } from '$lib/generated/bindings';
export const RULE_LABELS = { [RULE_TRIM]: 'Trim whitespace', [RULE_REPLACE]: 'Replace text', [RULE_REGEX]: 'Regex substitution' } satisfies Record<RuleType, string>;
export const PATTERN_TOKENS = { NONE: '', DIGITS: '\\d+', WORD: '\\w+' } as const;
