import * as assert from 'node:assert/strict';
import { buttons, DEFAULT_BUTTONS, statusBarClickBehaviour } from '../../config/clientSettings';
import { buildConfiguration } from '../../config/configuration';
import { changedValues, contextValues } from '../../context/keys';
import type { PersistedViewState } from '../../state/viewState';

const empty: PersistedViewState = { currentFilter: '', filtered: false, includeGlobs: [], excludeGlobs: [] };

function settings(tree: object = {}, viewState: Partial<PersistedViewState> = {}, general: object = {}) {
  return buildConfiguration({
    groups: {
      general,
      highlights: {},
      filtering: {},
      tree: {
        flat: false,
        tagsOnly: false,
        expanded: false,
        groupedByTag: false,
        groupedBySubTag: false,
        trackFile: true,
        hideTreeWhenEmpty: false,
        scanMode: 'workspace',
        ...tree,
      },
      regex: {},
    },
    filesExclude: {},
    searchExclude: {},
    explorerCompactFolders: true,
    viewState: { ...empty, ...viewState },
  });
}

describe('context keys', () => {
  it('reflects the default settings', () => {
    const v = contextValues(settings(), undefined);
    assert.equal(Object.keys(v).length, 22);
    assert.equal(v['clippings-show-reveal-button'], false, 'track file hides the reveal button');
    assert.equal(v['clippings-show-view-style-button'], true);
    assert.equal(v['clippings-show-export-button'], false);
    assert.equal(v['clippings-collapsible'], true);
    assert.equal(v['clippings-filtered'], false);
    assert.equal(v['clippings-global-filter-active'], '');
    assert.equal(v['clippings-can-toggle-compact-folders'], true);
    assert.equal(v['clippings-scan-mode'], 'workspace');
    assert.equal(v['clippings-has-sub-tags'], false);
    assert.equal(v['clippings-is-empty'], false);
  });

  it('reflects buttons, view state, filters and status', () => {
    const v = contextValues(
      settings(
        { trackFile: false, hideTreeWhenEmpty: true, buttons: { reveal: true, export: true } },
        { tagsOnly: true, currentFilter: 'fix', filtered: true, excludeGlobs: ['/w/**/*'] },
      ),
      { hasSubTags: true, isEmpty: true },
    );
    assert.equal(v['clippings-show-reveal-button'], true);
    assert.equal(v['clippings-show-export-button'], true);
    assert.equal(v['clippings-tags-only'], true);
    assert.equal(v['clippings-collapsible'], false, 'an ungrouped tags only view has nothing to collapse');
    assert.equal(v['clippings-filtered'], true);
    assert.equal(v['clippings-global-filter-active'], 'fix');
    assert.equal(v['clippings-folder-filter-active'], true);
    assert.equal(v['clippings-has-sub-tags'], true);
    assert.equal(v['clippings-is-empty'], true);
  });

  it('only reports keys whose values changed', () => {
    assert.deepEqual(changedValues({ a: true, b: 'x' }, { a: true, b: 'y', c: false }), { b: 'y', c: false });
  });
});

describe('client-only settings', () => {
  it('fills missing buttons with todo-tree defaults and ignores bad values', () => {
    assert.deepEqual(buttons(settings()), DEFAULT_BUTTONS);
    assert.equal(buttons(settings({ buttons: { scanMode: true, filter: 'no' } })).scanMode, true);
    assert.equal(buttons(settings({ buttons: { filter: 'no' } })).filter, true);
  });

  it('reads the status bar click behaviour', () => {
    assert.equal(statusBarClickBehaviour(settings()), 'reveal');
    assert.equal(statusBarClickBehaviour(settings({}, {}, { statusBarClickBehaviour: 'cycle' })), 'cycle');
    assert.equal(statusBarClickBehaviour(settings({}, {}, { statusBarClickBehaviour: 'bogus' })), 'reveal');
  });
});
