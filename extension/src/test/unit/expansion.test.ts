import * as assert from 'node:assert/strict';
import { buildConfiguration } from '../../config/configuration';
import { replacesTree } from '../../config/sync';
import type { Settings, ViewNode } from '../../protocol';
import { ViewStateStore, type PersistedViewState } from '../../state/viewState';
import { Expansion } from '../../tree/expansion';
import { MemoryMemento } from './memento';

function node(id: string, defaultExpanded: boolean): ViewNode {
  return {
    id,
    label: id,
    description: null,
    tooltip: null,
    icon: null,
    hasChildren: true,
    defaultExpanded,
    contextValue: null,
    resourceUri: null,
    command: null,
  };
}

describe('expansion state', () => {
  it('falls back to the node default and prefers the recorded state', () => {
    const expansion = new Expansion(new ViewStateStore(new MemoryMemento()));
    assert.equal(expansion.expanded(node('a', true)), true);
    expansion.set('a', false);
    assert.equal(expansion.expanded(node('a', true)), false);
    assert.equal(expansion.expanded(node('b', false)), false);
  });

  it('persists the map and the epoch across instances', async () => {
    const memento = new MemoryMemento();
    const first = new Expansion(new ViewStateStore(memento));
    first.set('a', true);
    first.reset(false);
    first.set('b', true);
    await Promise.resolve();
    const second = new Expansion(new ViewStateStore(memento));
    assert.equal(second.currentEpoch, 1);
    assert.equal(second.expanded(node('a', false)), false, 'reset forgot a');
    assert.equal(second.expanded(node('b', false)), true);
    assert.equal(second.itemId('x/y'), '1/x/y');
  });

  it('bumps the epoch at the next root refresh when the server re-renders', () => {
    const expansion = new Expansion(new ViewStateStore(new MemoryMemento()));
    expansion.set('a', true);
    expansion.reset(true);
    assert.equal(expansion.currentEpoch, 0);
    assert.equal(expansion.expanded(node('a', false)), false);
    expansion.onRootRefresh();
    assert.equal(expansion.currentEpoch, 1);
    expansion.onRootRefresh();
    assert.equal(expansion.currentEpoch, 1, 'ordinary root refreshes keep the epoch');
  });
});

describe('whole-tree changes', () => {
  const empty: PersistedViewState = { currentFilter: '', filtered: false, includeGlobs: [], excludeGlobs: [] };
  const settings = (tree: object, viewState: Partial<PersistedViewState> = {}): Settings =>
    buildConfiguration({
      groups: {
        general: {},
        highlights: {},
        filtering: {},
        tree: { flat: false, tagsOnly: false, expanded: false, groupedByTag: false, groupedBySubTag: false, scanMode: 'workspace', ...tree },
        regex: {},
      },
      filesExclude: {},
      searchExclude: {},
      explorerCompactFolders: false,
      viewState: { ...empty, ...viewState },
    });

  it('are view mode, grouping, expansion default and scan mode changes', () => {
    const base = settings({});
    assert.equal(replacesTree({ before: base, after: settings({}, { expanded: true }) }), true);
    assert.equal(replacesTree({ before: base, after: settings({ groupedByTag: true }) }), true);
    assert.equal(replacesTree({ before: base, after: settings({ scanMode: 'open files' }) }), true);
    assert.equal(replacesTree({ before: base, after: settings({ showBadges: false }) }), false);
  });

  it('ignore a setting that clicked view state overrides', () => {
    const before = settings({ expanded: false }, { expanded: true });
    const after = settings({ expanded: true }, { expanded: true });
    assert.equal(replacesTree({ before, after }), false);
  });
});
