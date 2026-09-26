import * as assert from 'node:assert/strict';
import { ViewStateStore } from '../../state/viewState';
import { MemoryMemento } from './memento';

describe('view state store', () => {
  it('starts empty', () => {
    const store = new ViewStateStore(new MemoryMemento());
    assert.deepEqual(store.snapshot(), { currentFilter: '', filtered: false, includeGlobs: [], excludeGlobs: [] });
    assert.deepEqual(store.expandedNodes(), {});
    assert.equal(store.epoch(), 0);
  });

  it('persists flags, filter and globs under todo-tree’s keys', async () => {
    const memento = new MemoryMemento();
    const store = new ViewStateStore(memento);
    await store.setFlags({ flat: true, tagsOnly: false });
    await store.setFilter('fix');
    await store.setGlobs(['/w/src/*'], ['/w/lib/**/*']);
    assert.deepEqual(store.snapshot(), {
      flat: true,
      tagsOnly: false,
      currentFilter: 'fix',
      filtered: true,
      includeGlobs: ['/w/src/*'],
      excludeGlobs: ['/w/lib/**/*'],
    });
    assert.equal(memento.get('currentFilter'), 'fix');
    assert.equal(memento.get('filtered'), true);
    await store.setFilter(undefined);
    assert.equal(store.snapshot().filtered, false);
  });

  it('ignores values of the wrong type', () => {
    const memento = new MemoryMemento();
    memento.values.set('flat', 'yes');
    memento.values.set('includeGlobs', ['a', 3]);
    memento.values.set('epoch', 'x');
    const store = new ViewStateStore(memento);
    assert.equal(store.snapshot().flat, undefined);
    assert.deepEqual(store.snapshot().includeGlobs, ['a']);
    assert.equal(store.epoch(), 0);
  });

  it('reset clears everything but keeps the epoch growing', async () => {
    const memento = new MemoryMemento();
    const store = new ViewStateStore(memento);
    await store.setFlags({ expanded: true });
    await store.setExpandedNodes({ a: true });
    assert.equal(await store.bumpEpoch(), 1);
    await store.reset();
    assert.deepEqual([...memento.values.keys()], ['epoch']);
    assert.equal(await store.bumpEpoch(), 2);
  });
});
