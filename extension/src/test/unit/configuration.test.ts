import * as assert from 'node:assert/strict';
import { buildConfiguration, effectiveView, trueKeys, viewStateOf } from '../../config/configuration';
import type { PersistedViewState } from '../../state/viewState';

const empty: PersistedViewState = { currentFilter: '', filtered: false, includeGlobs: [], excludeGlobs: [] };

describe('configuration builder', () => {
  it('keeps only exclude keys whose value is exactly true', () => {
    assert.deepEqual(trueKeys({ '**/.git': true, '**/x': false, '**/y': { when: '$(basename).ts' }, '**/z': 'true' }), [
      '**/.git',
    ]);
    assert.deepEqual(trueKeys(undefined), []);
    assert.deepEqual(trueKeys('nope'), []);
  });

  it('nests the five groups and adds the VS Code settings and view state', () => {
    const settings = buildConfiguration({
      groups: {
        general: { tags: ['TODO'], statusBarClickBehaviour: 'cycle' },
        highlights: { enabled: false },
        filtering: { excludeGlobs: [] },
        tree: { flat: true, buttons: { reveal: true } },
        regex: { regex: '($TAGS)' },
      },
      filesExclude: { '**/.git': true, '**/.DS_Store': false },
      searchExclude: { '**/dist': true },
      explorerCompactFolders: true,
      viewState: { ...empty, flat: false, currentFilter: 'fix', filtered: true, includeGlobs: ['/a/*'] },
    });
    assert.deepEqual(settings.general.tags, ['TODO']);
    assert.equal(settings.highlights.enabled, false);
    assert.equal(settings.tree.flat, true);
    assert.equal(settings.regex.regex, '($TAGS)');
    assert.deepEqual(settings.filesExclude, ['**/.git']);
    assert.deepEqual(settings.searchExclude, ['**/dist']);
    assert.equal(settings.explorerCompactFolders, true);
    assert.deepEqual(settings.viewState, { flat: false, filter: 'fix', includeGlobs: ['/a/*'], excludeGlobs: [] });
    // The object must survive JSON, which is how it reaches the server.
    assert.deepEqual(JSON.parse(JSON.stringify(settings)), settings);
  });

  it('treats a missing group as empty so the server fills defaults', () => {
    const settings = buildConfiguration({
      groups: { general: undefined, highlights: null, filtering: 3, tree: {}, regex: {} },
      filesExclude: undefined,
      searchExclude: undefined,
      explorerCompactFolders: 'yes',
      viewState: empty,
    });
    assert.deepEqual(settings.general, {});
    assert.deepEqual(settings.highlights, {});
    assert.deepEqual(settings.filtering, {});
    assert.equal(settings.explorerCompactFolders, false);
  });

  it('omits view flags the user never clicked', () => {
    const state = viewStateOf({ ...empty, tagsOnly: true });
    assert.deepEqual(state, { tagsOnly: true, filter: '', includeGlobs: [], excludeGlobs: [] });
    assert.ok(!('flat' in state));
  });

  it('sends the filter only while it is active', () => {
    assert.equal(viewStateOf({ ...empty, currentFilter: 'bug', filtered: false }).filter, '');
    assert.equal(viewStateOf({ ...empty, currentFilter: 'bug', filtered: true }).filter, 'bug');
  });

  it('applies clicked view state over the tree settings', () => {
    const tree = { flat: true, tagsOnly: false, expanded: false, groupedByTag: true, groupedBySubTag: false };
    const view = effectiveView(tree, { flat: false, expanded: true, filter: '', includeGlobs: [], excludeGlobs: [] });
    assert.deepEqual(view, { flat: false, tagsOnly: false, expanded: true, groupedByTag: true, groupedBySubTag: false });
  });
});
