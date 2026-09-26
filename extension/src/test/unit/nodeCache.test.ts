import * as assert from 'node:assert/strict';
import type { ViewNode } from '../../protocol';
import { NodeCache } from '../../tree/nodeCache';

function node(id: string): ViewNode {
  return {
    id,
    label: id,
    description: null,
    tooltip: null,
    icon: null,
    hasChildren: false,
    defaultExpanded: false,
    contextValue: null,
    resourceUri: null,
    command: null,
  };
}

describe('node cache', () => {
  it('records children and their parents', () => {
    const cache = new NodeCache();
    cache.record(null, [node('w:file:///w')]);
    cache.record('w:file:///w', [node('w:file:///w/d:/w/src')]);
    assert.equal(cache.parent('w:file:///w'), null);
    assert.equal(cache.parent('w:file:///w/d:/w/src'), 'w:file:///w');
    assert.equal(cache.parent('unknown'), undefined);
    assert.equal(cache.size, 2);
  });

  it('records a find path top down', () => {
    const cache = new NodeCache();
    cache.recordPath([node('a'), node('a/b'), node('a/b/c')]);
    assert.equal(cache.parent('a'), null);
    assert.equal(cache.parent('a/b/c'), 'a/b');
  });

  it('derives own keys that contain slashes from the recorded parent', () => {
    const cache = new NodeCache();
    const root = 'w:file:///w';
    const folder = `${root}/d:/w/app/[slug]`;
    const file = `${folder}/f:/w/app/[slug]/page.ts`;
    cache.recordPath([node(root), node(folder), node(file)]);
    assert.equal(cache.ownKey(root), root);
    assert.equal(cache.ownKey(folder), 'd:/w/app/[slug]');
    assert.equal(cache.ownKey(file), 'f:/w/app/[slug]/page.ts');
    assert.equal(cache.ownKey('never-seen'), 'never-seen');
  });

  it('clears everything', () => {
    const cache = new NodeCache();
    cache.record(null, [node('a')]);
    cache.clear();
    assert.equal(cache.size, 0);
    assert.equal(cache.get('a'), undefined);
  });
});
