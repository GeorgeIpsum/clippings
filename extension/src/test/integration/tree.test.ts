import * as assert from 'node:assert/strict';
import { writeFileSync, readFileSync } from 'node:fs';
import type { ClippingsApi } from '../../testApi';
import { DEFAULT_TREE } from './fixture';
import { getApi, outline, treeBecomes, whenIdle, workspacePath } from './helpers';


describe('tree provider', () => {
  let api: ClippingsApi;

  before(async () => {
    api = await getApi();
    await whenIdle(api);
  });

  it('shows the fixture workspace as a tree', async () => {
    await treeBecomes(api, DEFAULT_TREE);
  });

  it('maps node fields onto tree items', async () => {
    const [status, root] = await api.test.tree.items();
    assert.ok(status && root);
    assert.equal(status.id, 'status:scan-mode');
    assert.equal(status.icon, '$(search)');
    assert.equal(root.contextValue, 'folder');
    assert.equal(root.icon, '$(window)');
    assert.equal(root.state, 'collapsed');
    const [docs] = await api.test.tree.items(root.id);
    const [plan] = await api.test.tree.items(docs?.id);
    assert.equal(plan?.contextValue, 'file');
    const [todo] = await api.test.tree.items(plan?.id);
    assert.ok(todo);
    assert.equal(todo.state, 'none');
    assert.equal(todo.tooltip, `${workspacePath('docs', 'plan.md')}, line 3`);
    assert.equal(todo.command?.command, 'clippings.revealInFile');
    assert.deepEqual(todo.command?.arguments[1], { line: 2, character: 0 });
  });

  it('refreshes when a file changes on disk', async () => {
    const path = workspacePath('lib', 'notes.rs');
    const original = readFileSync(path, 'utf8');
    try {
      writeFileSync(path, original + '// FIXME appended on disk\n');
      const expected = [...DEFAULT_TREE];
      expected.splice(expected.indexOf('      TODO first line of a long note') + 1, 0, '      FIXME appended on disk');
      await treeBecomes(api, expected);
    } finally {
      writeFileSync(path, original);
    }
    await treeBecomes(api, DEFAULT_TREE);
    assert.ok((await outline(api, 1)).length === 2);
  });
});
