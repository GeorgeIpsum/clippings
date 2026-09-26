// The configuration object the client sends in `initialize` and
// `clippings/configure` (spec 6.3). Pure: the caller reads the settings.

import type { Filtering, General, Highlights, RegexSettings, Settings, Tree, ViewState } from '../protocol';
import type { PersistedViewState } from '../state/viewState';

/** The `clippings.*` groups the server reads, as VS Code resolves them at window level. */
export const GROUPS = ['general', 'highlights', 'filtering', 'tree', 'regex'] as const;
export type Group = (typeof GROUPS)[number];

export interface ConfigurationSources {
  /** `getConfiguration('clippings').get(group)` for each group. */
  groups: Record<Group, unknown>;
  /** `files.exclude`, `search.exclude` and `explorer.compactFolders`. */
  filesExclude: unknown;
  searchExclude: unknown;
  explorerCompactFolders: unknown;
  viewState: PersistedViewState;
}

/** Keys of a VS Code exclude object whose value is exactly `true`. */
export function trueKeys(value: unknown): string[] {
  if (!value || typeof value !== 'object') return [];
  return Object.entries(value as Record<string, unknown>)
    .filter(([, v]) => v === true)
    .map(([k]) => k);
}

function group<T>(value: unknown): T {
  // JSON boundary: VS Code validated the values against the manifest schema
  // and the server reports anything it cannot read as a warning.
  return (value && typeof value === 'object' ? { ...value } : {}) as T;
}

export function viewStateOf(persisted: PersistedViewState): ViewState {
  const state: ViewState = {
    filter: persisted.filtered ? persisted.currentFilter : '',
    includeGlobs: [...persisted.includeGlobs],
    excludeGlobs: [...persisted.excludeGlobs],
  };
  for (const key of ['flat', 'tagsOnly', 'expanded', 'groupedByTag', 'groupedBySubTag'] as const) {
    const value = persisted[key];
    if (value !== undefined) state[key] = value;
  }
  return state;
}

export function buildConfiguration(src: ConfigurationSources): Settings {
  return {
    general: group<General>(src.groups.general),
    highlights: group<Highlights>(src.groups.highlights),
    filtering: group<Filtering>(src.groups.filtering),
    tree: group<Tree>(src.groups.tree),
    regex: group<RegexSettings>(src.groups.regex),
    viewState: viewStateOf(src.viewState),
    filesExclude: trueKeys(src.filesExclude),
    searchExclude: trueKeys(src.searchExclude),
    explorerCompactFolders: src.explorerCompactFolders === true,
  };
}

/** The effective view options: clicked view state over the `tree.*` settings. */
export interface EffectiveView {
  flat: boolean;
  tagsOnly: boolean;
  expanded: boolean;
  groupedByTag: boolean;
  groupedBySubTag: boolean;
}

export function effectiveView(tree: Pick<Tree, keyof EffectiveView>, state: ViewState): EffectiveView {
  return {
    flat: state.flat ?? tree.flat,
    tagsOnly: state.tagsOnly ?? tree.tagsOnly,
    expanded: state.expanded ?? tree.expanded,
    groupedByTag: state.groupedByTag ?? tree.groupedByTag,
    groupedBySubTag: state.groupedBySubTag ?? tree.groupedBySubTag,
  };
}
