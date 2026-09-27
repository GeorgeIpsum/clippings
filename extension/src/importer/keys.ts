// The todo-tree settings the importer knows (spec 7.2, 7.3). Pure.

/** Settings carried unchanged in name, type and default: 61 keys. */
export const CARRIED_KEYS = [
  'general.automaticGitRefreshInterval',
  'general.periodicRefreshInterval',
  'general.revealBehaviour',
  'general.exportPath',
  'general.rootFolder',
  'general.schemes',
  'general.statusBar',
  'general.showIconsInsteadOfTagsInStatusBar',
  'general.statusBarClickBehaviour',
  'general.tagGroups',
  'general.tags',
  'general.showActivityBarBadge',
  'highlights.customHighlight',
  'highlights.defaultHighlight',
  'highlights.enabled',
  'highlights.highlightDelay',
  'highlights.useColourScheme',
  'highlights.foregroundColourScheme',
  'highlights.backgroundColourScheme',
  'filtering.excludedWorkspaces',
  'filtering.excludeGlobs',
  'filtering.ignoreGitSubmodules',
  'filtering.includedWorkspaces',
  'filtering.includeGlobs',
  'filtering.includeHiddenFiles',
  'filtering.scopes',
  'filtering.useBuiltInExcludes',
  'tree.autoRefresh',
  'tree.disableCompactFolders',
  'tree.expanded',
  'tree.filterCaseSensitive',
  'tree.flat',
  'tree.groupedByTag',
  'tree.groupedBySubTag',
  'tree.hideIconsWhenGroupedByTag',
  'tree.hideTreeWhenEmpty',
  'tree.labelFormat',
  'tree.scanAtStartup',
  'tree.scanMode',
  'tree.showBadges',
  'tree.showCountsInTree',
  'tree.showCurrentScanMode',
  'tree.subTagClickUrl',
  'tree.sortTagsOnlyViewAlphabetically',
  'tree.sort',
  'tree.tagsOnly',
  'tree.tooltipFormat',
  'tree.trackFile',
  'tree.buttons.reveal',
  'tree.buttons.scanMode',
  'tree.buttons.viewStyle',
  'tree.buttons.groupByTag',
  'tree.buttons.groupBySubTag',
  'tree.buttons.filter',
  'tree.buttons.refresh',
  'tree.buttons.expand',
  'tree.buttons.export',
  'regex.regex',
  'regex.regexCaseSensitive',
  'regex.subTagRegex',
  'regex.enableMultiLine',
] as const;

/** Clippings' own settings, which todo-tree never had. */
export const NEW_KEYS = ['filtering.builtInExcludes', 'server.path', 'server.logLevel', 'trace.server'] as const;

/** todo-tree settings with no Clippings equivalent; `general.debug` maps to a log level. */
export const DROPPED_KEYS = [
  'ripgrep.ripgrep',
  'ripgrep.ripgrepArgs',
  'ripgrep.ripgrepMaxBuffer',
  'ripgrep.usePatternFile',
  'filtering.passGlobsToRipgrep',
  'tree.showInExplorer',
  'tree.showScanOpenFilesOrWorkspaceButton',
  'tree.showTagsFromOpenFilesOnly',
  'general.debug',
] as const;

export const TODO_TREE_KEYS: readonly string[] = [...CARRIED_KEYS, ...DROPPED_KEYS];
export const CLIPPINGS_KEYS: readonly string[] = [...CARRIED_KEYS, ...NEW_KEYS];

const carried = new Set<string>(CARRIED_KEYS);

/** The Clippings setting for a todo-tree key and value, or why there is none. */
export function mapSetting(key: string, value: unknown): { key: string; value: unknown } | { skip: string } {
  if (carried.has(key)) return { key, value };
  if (key === 'general.debug') {
    return value === true ? { key: 'server.logLevel', value: 'debug' } : { skip: 'general.debug is off' };
  }
  return { skip: `todo-tree.${key} has no Clippings equivalent` };
}
