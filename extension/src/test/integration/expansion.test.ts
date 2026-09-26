import * as assert from 'node:assert/strict';
import { readFileSync, writeFileSync } from 'node:fs';
import * as vscode from 'vscode';
import type { ClippingsApi } from '../../testApi';
import type { TestItem } from '../../tree/testItems';
import { getApi, treeBecomes, waitFor, whenIdle, workspacePath } from './helpers';
import { DEFAULT_TREE } from './fixture';

/** Every item with children, depth first. */
async function containers(api: ClippingsApi, parent?: string): Promise<TestItem[]> {
  const out: TestItem[] = [];
  for (const item of await api.test.tree.items(parent)) {
    if (item.state === 'none') continue;
    out.push(item, ...(await containers(api, item.id)));
  }
  return out;
}

async function allContainers(api: ClippingsApi, state: 'expanded' | 'collapsed'): Promise<TestItem[]> {
  return waitFor(
    `every container ${state}`,
    async () => {
      const items = await containers(api);
      return items.length > 0 && items.every((i) => i.state === state) ? items : undefined;
    },
    [api.test.tree.onDidChange],
  );
}

describe('expand and collapse', () => {
  let api: ClippingsApi;

  before(async () => {
    api = await getApi();
    await whenIdle(api);
    await treeBecomes(api, DEFAULT_TREE);
    await vscode.commands.executeCommand('clippings-view.focus');
  });

  after(async () => {
    await vscode.commands.executeCommand('clippings.collapse');
    await allContainers(api, 'collapsed');
  });

  it('Expand Tree expands every node under a new epoch', async () => {
    const epoch = api.test.tree.epoch;
    await vscode.commands.executeCommand('clippings.expand');
    const items = await allContainers(api, 'expanded');
    assert.ok(api.test.tree.epoch > epoch);
    for (const item of items) assert.equal(item.itemId, `${api.test.tree.epoch}/${item.id}`);
  });

  it('Collapse Tree collapses every node under a new epoch', async () => {
    const epoch = api.test.tree.epoch;
    await vscode.commands.executeCommand('clippings.collapse');
    await allContainers(api, 'collapsed');
    assert.ok(api.test.tree.epoch > epoch);
  });

  it('records expanding and collapsing one node on the client', async () => {
    const [, root] = await api.test.tree.items();
    assert.ok(root);
    await api.test.tree.view.reveal(root.id, { expand: true, focus: false, select: false });
    const [docs, lib] = await waitFor(
      'the root expanded',
      async () => {
        const [, r] = await api.test.tree.items();
        return r?.state === 'expanded' ? api.test.tree.items(r.id) : undefined;
      },
      [api.test.tree.view.onDidExpandElement],
    );
    assert.ok(docs && lib);
    assert.equal(docs.state, 'collapsed', 'siblings keep the default');
    await api.test.tree.view.reveal(docs.id, { expand: true, focus: false, select: false });
    await waitFor(
      'docs expanded',
      async () => (await api.test.tree.items(root.id))[0]?.state === 'expanded',
      [api.test.tree.view.onDidExpandElement],
    );
  });

  it('keeps the epoch across ordinary changes', async () => {
    const epoch = api.test.tree.epoch;
    const path = workspacePath('docs', 'plan.md');
    const original = readFileSync(path, 'utf8');
    const expected = [...DEFAULT_TREE];
    expected.splice(expected.indexOf('      [x] pick a name') + 1, 0, '      [ ] another item');
    try {
      writeFileSync(path, original + '- [ ] another item\n');
      await treeBecomes(api, expected);
      assert.equal(api.test.tree.epoch, epoch);
    } finally {
      writeFileSync(path, original);
    }
    await treeBecomes(api, DEFAULT_TREE);
  });
});
