import * as assert from 'node:assert/strict';
import * as vscode from 'vscode';
import type { ClippingsApi } from '../../testApi';
import {
  DEFAULT_TREE,
  FLAT_GROUPED_BY_TAG,
  FLAT_VIEW,
  TAGS_ONLY_GROUPED_BY_TAG,
  TAGS_ONLY_VIEW,
  TREE_GROUPED_BY_TAG,
} from './fixture';
import { getApi, treeBecomes, whenIdle } from './helpers';

async function run(...commands: string[]): Promise<void> {
  for (const command of commands) await vscode.commands.executeCommand(command);
}

describe('view modes', () => {
  let api: ClippingsApi;

  before(async () => {
    api = await getApi();
    await whenIdle(api);
    await treeBecomes(api, DEFAULT_TREE);
  });

  after(async () => {
    await run('clippings.resetCache');
    await treeBecomes(api, DEFAULT_TREE);
  });

  it('shows the flat view', async () => {
    await run('clippings.showFlatView');
    await treeBecomes(api, FLAT_VIEW);
    assert.equal(api.test.viewState().flat, true);
    assert.equal(api.test.viewState().tagsOnly, false);
  });

  it('shows the tags only view', async () => {
    await run('clippings.showTagsOnlyView');
    await treeBecomes(api, TAGS_ONLY_VIEW);
  });

  it('shows the tree view grouped by tag', async () => {
    await run('clippings.showTreeView', 'clippings.groupByTag');
    await treeBecomes(api, TREE_GROUPED_BY_TAG);
  });

  it('shows the flat view grouped by tag', async () => {
    await run('clippings.showFlatView');
    await treeBecomes(api, FLAT_GROUPED_BY_TAG);
  });

  it('shows the tags only view grouped by tag', async () => {
    await run('clippings.showTagsOnlyView');
    await treeBecomes(api, TAGS_ONLY_GROUPED_BY_TAG);
  });

  it('ungroups and returns to the tree view', async () => {
    await run('clippings.ungroupByTag', 'clippings.showTreeView');
    await treeBecomes(api, DEFAULT_TREE);
    assert.equal(api.test.viewState().groupedByTag, false);
  });

  it('groups and ungroups by sub tag', async () => {
    await vscode.workspace
      .getConfiguration('clippings.regex')
      .update('subTagRegex', '^\\s*\\((\\w+)\\)', vscode.ConfigurationTarget.Global);
    try {
      await run('clippings.groupBySubTag');
      await treeBecomes(api, [
        '(Scan mode: workspace and open files)',
        'workspace',
        '  alice',
        '    src',
        '      app.ts',
        '        TODO wire up the router',
        '  bob',
        '    src/util',
        '      strings.py',
        '        BUG drops leading tabs too',
        '  docs',
        '    plan.md',
        '      [ ] write the guide',
        '      [x] pick a name',
        '  lib',
        '    notes.rs',
        '      TODO first line of a long note',
        '  src',
        '    util',
        '      strings.py',
        '        TODO normalise unicode before comparing',
        '    app.ts',
        '      FIXME handle the error path',
        '      HACK temporary shim until the router lands',
      ]);
      await run('clippings.ungroupBySubTag');
      assert.equal(api.test.viewState().groupedBySubTag, false);
    } finally {
      await vscode.workspace
        .getConfiguration('clippings.regex')
        .update('subTagRegex', undefined, vscode.ConfigurationTarget.Global);
    }
    await treeBecomes(api, DEFAULT_TREE);
  });

  it('Reset Cache clears the clicked view state', async () => {
    await run('clippings.showFlatView', 'clippings.groupByTag');
    await treeBecomes(api, FLAT_GROUPED_BY_TAG);
    const epoch = api.test.tree.epoch;
    await run('clippings.resetCache');
    await treeBecomes(api, DEFAULT_TREE);
    assert.deepEqual(api.test.viewState(), { currentFilter: '', filtered: false, includeGlobs: [], excludeGlobs: [] });
    assert.ok(api.test.tree.epoch > epoch);
  });
});
