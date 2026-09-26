// Settings only the client reads. They travel inside the configuration
// object's groups, which the server ignores. Pure.

import type { Settings } from '../protocol';

export interface Buttons {
  reveal: boolean;
  scanMode: boolean;
  viewStyle: boolean;
  groupByTag: boolean;
  groupBySubTag: boolean;
  filter: boolean;
  refresh: boolean;
  expand: boolean;
  export: boolean;
}

/** todo-tree's defaults for `tree.buttons.*`. */
export const DEFAULT_BUTTONS: Buttons = {
  reveal: true,
  scanMode: false,
  viewStyle: true,
  groupByTag: true,
  groupBySubTag: false,
  filter: true,
  refresh: true,
  expand: true,
  export: false,
};

export function buttons(settings: Settings): Buttons {
  const raw = (settings.tree as unknown as { buttons?: Partial<Record<keyof Buttons, unknown>> }).buttons ?? {};
  const out = { ...DEFAULT_BUTTONS };
  for (const key of Object.keys(out) as (keyof Buttons)[]) {
    if (typeof raw[key] === 'boolean') out[key] = raw[key];
  }
  return out;
}

export type ClickBehaviour = 'cycle' | 'reveal' | 'toggle highlights';

export function statusBarClickBehaviour(settings: Settings): ClickBehaviour {
  const value = (settings.general as unknown as { statusBarClickBehaviour?: unknown }).statusBarClickBehaviour;
  return value === 'cycle' || value === 'toggle highlights' ? value : 'reveal';
}
