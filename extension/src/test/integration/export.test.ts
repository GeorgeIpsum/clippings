import * as assert from 'node:assert/strict';
import { homedir } from 'node:os';
import * as vscode from 'vscode';
import type { ClippingsApi } from '../../testApi';
import { DEFAULT_TREE } from './fixture';
import { getApi, setSetting, treeBecomes, whenIdle } from './helpers';

async function exported(): Promise<vscode.TextDocument> {
  await vscode.commands.executeCommand('clippings.exportTree');
  const editor = vscode.window.activeTextEditor;
  assert.ok(editor, 'an export editor');
  assert.equal(editor.document.uri.scheme, 'clippings-export');
  return editor.document;
}

describe('export', () => {
  let api: ClippingsApi;

  before(async () => {
    api = await getApi();
    await whenIdle(api);
    await treeBecomes(api, DEFAULT_TREE);
  });

  afterEach(async () => {
    await vscode.commands.executeCommand('workbench.action.closeAllEditors');
  });

  it('opens the visible tree as a text tree named by the formatted export path', async () => {
    const document = await exported();
    const path = document.uri.path;
    assert.ok(path.startsWith(homedir().replaceAll('\\', '/')) || path.startsWith('/' + homedir()), path);
    assert.match(path, /\/todo-tree-\d{8}-\d{4}\.txt$/);
    const text = document.getText();
    assert.match(text, /^└─ workspace\n/);
    assert.ok(text.includes('line 2: TODO (alice) wire up the router'), text);
    assert.ok(!text.includes('Scan mode'), 'status nodes are not exported');
  });

  it('exports JSON when the path ends in .json', async () => {
    await setSetting('general.exportPath', '${HOME}/clippings-export.json');
    try {
      const document = await exported();
      assert.equal(document.uri.path.endsWith('/clippings-export.json'), true);
      assert.equal(document.languageId, 'json');
      const tree = JSON.parse(document.getText()) as Record<string, Record<string, unknown>>;
      assert.deepEqual(Object.keys(tree), ['workspace']);
      assert.deepEqual(Object.keys(tree['workspace'] ?? {}), ['docs', 'lib', 'src']);
    } finally {
      await setSetting('general.exportPath', undefined);
    }
  });
});
