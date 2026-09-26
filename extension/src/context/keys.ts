// Context keys for menus and view visibility (spec 7.6). Pure.

import { buttons } from '../config/clientSettings';
import { effectiveView } from '../config/configuration';
import type { Settings, StatusParams } from '../protocol';

export type ContextValues = Record<string, boolean | string>;

export function contextValues(settings: Settings, status: Pick<StatusParams, 'hasSubTags' | 'isEmpty'> | undefined): ContextValues {
  const b = buttons(settings);
  const tree = settings.tree;
  const state = settings.viewState;
  const view = effectiveView(tree, state);
  return {
    'clippings-show-reveal-button': b.reveal && !tree.trackFile,
    'clippings-show-scan-mode-button': b.scanMode,
    'clippings-show-view-style-button': b.viewStyle,
    'clippings-show-group-by-tag-button': b.groupByTag,
    'clippings-show-group-by-sub-tag-button': b.groupBySubTag,
    'clippings-show-filter-button': b.filter,
    'clippings-show-refresh-button': b.refresh,
    'clippings-show-expand-button': b.expand,
    'clippings-show-export-button': b.export,
    'clippings-expanded': view.expanded,
    'clippings-flat': view.flat,
    'clippings-tags-only': view.tagsOnly,
    'clippings-grouped-by-tag': view.groupedByTag,
    'clippings-grouped-by-sub-tag': view.groupedBySubTag,
    'clippings-filtered': state.filter !== '',
    'clippings-collapsible': !view.tagsOnly || view.groupedByTag || view.groupedBySubTag,
    'clippings-folder-filter-active': state.includeGlobs.length + state.excludeGlobs.length > 0,
    'clippings-global-filter-active': state.filter,
    'clippings-can-toggle-compact-folders': settings.explorerCompactFolders,
    'clippings-has-sub-tags': status?.hasSubTags ?? false,
    'clippings-scan-mode': tree.scanMode,
    'clippings-is-empty': tree.hideTreeWhenEmpty && (status?.isEmpty ?? false),
  };
}

/** The keys whose values differ from `previous`. */
export function changedValues(previous: ContextValues, next: ContextValues): ContextValues {
  return Object.fromEntries(Object.entries(next).filter(([k, v]) => previous[k] !== v));
}
