import * as assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';
import { CARRIED_KEYS, CLIPPINGS_KEYS, DROPPED_KEYS, mapSetting, NEW_KEYS } from '../../importer/keys';
import { applyWrites, importPlan, overwriteCount, shouldOffer, type Inspected, type Write } from '../../importer/plan';

const manifest = JSON.parse(readFileSync(resolve(__dirname, '../../../package.json'), 'utf8')) as {
  contributes: { configuration: { properties: Record<string, unknown> }[] };
};

describe('importer keys', () => {
  it('cover exactly the manifest settings: 61 carried and 4 new', () => {
    const declared = manifest.contributes.configuration.flatMap((g) => Object.keys(g.properties)).sort();
    assert.equal(CARRIED_KEYS.length, 61);
    assert.equal(NEW_KEYS.length, 4);
    assert.deepEqual(CLIPPINGS_KEYS.map((k) => `clippings.${k}`).sort(), declared);
    assert.equal(DROPPED_KEYS.length, 9);
  });

  it('maps carried keys unchanged, debug to a log level and drops the rest', () => {
    assert.deepEqual(mapSetting('general.tags', ['A']), { key: 'general.tags', value: ['A'] });
    assert.deepEqual(mapSetting('general.debug', true), { key: 'server.logLevel', value: 'debug' });
    assert.ok('skip' in mapSetting('general.debug', false));
    assert.deepEqual(mapSetting('ripgrep.ripgrepArgs', '--x'), {
      skip: 'todo-tree.ripgrep.ripgrepArgs has no Clippings equivalent',
    });
  });
});

describe('import offer', () => {
  const values = (map: Record<string, Inspected>) => (section: string, key: string) => map[`${section}.${key}`];

  it('needs a todo-tree value, no clippings value and no Never', () => {
    const todoTree = { 'todo-tree.general.tags': { globalValue: ['A'] } };
    assert.equal(shouldOffer(values(todoTree), undefined), true);
    assert.equal(shouldOffer(values(todoTree), 'never'), false);
    assert.equal(shouldOffer(values({}), undefined), false);
    assert.equal(
      shouldOffer(values({ ...todoTree, 'clippings.server.logLevel': { workspaceValue: 'info' } }), undefined),
      false,
    );
    assert.equal(
      shouldOffer(values({ 'todo-tree.general.tags': { workspaceFolderValue: ['A'] } }), undefined),
      false,
      'folder values do not count',
    );
  });
});

describe('import plan', () => {
  it('copies values to the same scope and skips what cannot move', () => {
    const plan = importPlan(
      (key) =>
        ({
          'general.tags': { globalValue: ['A'], workspaceValue: ['B'], workspaceFolderValue: ['C'] },
          'general.debug': { workspaceValue: true },
          'ripgrep.ripgrepArgs': { globalValue: '--foo' },
          'tree.buttons.export': { globalValue: true },
        })[key],
    );
    assert.deepEqual(plan.writes, [
      { key: 'general.tags', value: ['A'], scope: 'global' },
      { key: 'general.tags', value: ['B'], scope: 'workspace' },
      { key: 'tree.buttons.export', value: true, scope: 'global' },
      { key: 'server.logLevel', value: 'debug', scope: 'workspace' },
    ]);
    assert.deepEqual(plan.skipped, [
      'todo-tree.general.tags: skipped the workspace folder value, which todo-tree ignored',
      'todo-tree.ripgrep.ripgrepArgs has no Clippings equivalent; skipped its global value',
    ]);
  });
});

describe('overwrite count', () => {
  it('counts only the planned writes that already have a Clippings value at that same scope', () => {
    const writes: Write[] = [
      { key: 'general.tags', value: ['A'], scope: 'global' },
      { key: 'general.tags', value: ['B'], scope: 'workspace' },
      { key: 'tree.buttons.export', value: true, scope: 'global' },
    ];
    const values: Record<string, Inspected> = {
      'general.tags': { globalValue: ['X'] },
      'tree.buttons.export': { workspaceValue: true },
    };
    assert.equal(
      overwriteCount(writes, (key) => values[key]),
      1,
    );
    assert.equal(
      overwriteCount(writes, () => undefined),
      0,
    );
  });
});

describe('apply writes', () => {
  it('keeps writing after one fails (spec 10.2), and counts only the successes', async () => {
    const attempted: string[] = [];
    const write = async (key: string): Promise<boolean> => {
      attempted.push(key);
      return key !== 'general.tags';
    };
    const writes: Write[] = [
      { key: 'general.tags', value: ['A'], scope: 'workspace' },
      { key: 'tree.buttons.export', value: true, scope: 'global' },
    ];
    const written = await applyWrites(writes, write);
    assert.equal(written, 1);
    assert.deepEqual(attempted, ['general.tags', 'tree.buttons.export']);
  });
});
