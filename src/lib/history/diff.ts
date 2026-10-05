import { UI } from '$lib/constants/ui';
function excerpt(text: string) {
  const characters: string[] = [];
  for (const character of text) {
    if (characters.length === UI.TEXT_EXCERPT_CHARS) return { characters, truncated: true };
    characters.push(character);
  }
  return { characters, truncated: false };
}
/** A bounded contiguous-change diff; multiple edits share one highlighted span. */
export function textDiff(before: string, after: string) {
  const original = excerpt(before), transformed = excerpt(after);
  const a = original.characters, b = transformed.characters;
  let start = 0, end = 0;
  while (start < Math.min(a.length, b.length) && a[start] === b[start]) start++;
  while (end < Math.min(a.length, b.length) - start && a[a.length - end - 1] === b[b.length - end - 1]) end++;
  return {
    prefix: a.slice(0, start).join(''), suffix: end ? a.slice(-end).join('') : '',
    removed: a.slice(start, a.length - end).join(''), added: b.slice(start, b.length - end).join(''),
    truncated: original.truncated || transformed.truncated,
  };
}
