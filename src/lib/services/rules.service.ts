import { commands, type Rule } from '$lib/generated/bindings';
import { request } from './result';
export const rulesService = {
  getAll: () => request('Load rules', () => commands.getRules()),
  save: (rule: Rule) => request('Save rule', () => commands.saveRule(rule)),
  delete: (id: number) => request('Delete rule', () => commands.deleteRule(id)),
  reorder: (ids: number[]) => request('Reorder rules', () => commands.reorderRules(ids)),
  preview: (rule: Rule, input: string) => request('Preview rule', () => commands.previewRule(rule, input)),
};
