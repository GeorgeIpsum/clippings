// Whether to offer the todo-tree import and what it writes (spec 7.3). Pure.

import { CLIPPINGS_KEYS, mapSetting, TODO_TREE_KEYS } from './keys';

export interface Inspected {
  globalValue?: unknown;
  workspaceValue?: unknown;
  workspaceFolderValue?: unknown;
}

export type Scope = 'global' | 'workspace';

export interface Write {
  key: string;
  value: unknown;
  scope: Scope;
}

export interface ImportPlan {
  writes: Write[];
  /** Log lines for values the import leaves behind. */
  skipped: string[];
}

function explicit(i: Inspected | undefined): boolean {
  return i?.globalValue !== undefined || i?.workspaceValue !== undefined;
}

/**
 * Offer the import when some todo-tree key has a global or workspace value,
 * no Clippings key has one, and the user has not chosen Never.
 */
export function shouldOffer(
  inspect: (section: 'todo-tree' | 'clippings', key: string) => Inspected | undefined,
  decision: unknown,
): boolean {
  return (
    decision !== 'never' &&
    TODO_TREE_KEYS.some((k) => explicit(inspect('todo-tree', k))) &&
    !CLIPPINGS_KEYS.some((k) => explicit(inspect('clippings', k)))
  );
}

export function importPlan(inspect: (key: string) => Inspected | undefined): ImportPlan {
  const plan: ImportPlan = { writes: [], skipped: [] };
  for (const key of TODO_TREE_KEYS) {
    const i = inspect(key);
    if (!i) continue;
    if (i.workspaceFolderValue !== undefined) {
      plan.skipped.push(`todo-tree.${key}: skipped the workspace folder value, which todo-tree ignored`);
    }
    for (const [scope, value] of [
      ['global', i.globalValue],
      ['workspace', i.workspaceValue],
    ] as const) {
      if (value === undefined) continue;
      const mapped = mapSetting(key, value);
      if ('skip' in mapped) plan.skipped.push(`${mapped.skip}; skipped its ${scope} value`);
      else plan.writes.push({ ...mapped, scope });
    }
  }
  return plan;
}
