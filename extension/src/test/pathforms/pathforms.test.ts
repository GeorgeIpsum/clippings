// The essentials, with the workspace opened through another spelling of its
// path (.vscode-test.pathforms.mjs): the tree fills, the active file is
// revealed, an open file is decorated without a second tree node, and a
// change on disk reaches the tree.

import * as assert from 'node:assert/strict';
import { readFileSync, writeFileSync } from 'node:fs';
import * as vscode from 'vscode';
import type { ClippingsApi } from '../../testApi';
import { DEFAULT_TREE } from '../integration/fixture';
import { getApi, treeBecomes, waitFor, whenIdle, workspacePath } from '../integration/helpers';

const form = process.env['CLIPPINGS_TEST_PATH_FORM'] ?? 'unknown form';
const skip = process.env['CLIPPINGS_TEST_SKIP'];

describe(`workspace opened through a ${form} path`, () => {
  let api: ClippingsApi;
  let tree: string[];

  before(async function () {
    if (skip) {
      console.log(`  skipping the ${form} path form: ${skip}`);
      this.skip();
    }
    console.log(`  workspace: ${workspacePath()}`);
    api = await getApi();
    await whenIdle(api);
    // The root node is named after the folder as VS Code sees it, which for
    // an 8.3 short path is its short name.
    const name = vscode.workspace.workspaceFolders?.[0]?.name;
    assert.ok(name, 'a workspace folder');
    tree = DEFAULT_TREE.map((line) => (line === 'workspace' ? name : line));
  });

  afterEach(async () => {
    await vscode.commands.executeCommand('workbench.action.closeAllEditors');
  });

  it('fills the tree', async () => {
    assert.equal(vscode.workspace.workspaceFolders?.[0]?.uri.fsPath, vscode.Uri.file(workspacePath()).fsPath);
    await treeBecomes(api, tree);
  });

  it('reveals the active file', async () => {
    await vscode.commands.executeCommand('clippings-view.focus');
    await vscode.window.showTextDocument(vscode.Uri.file(workspacePath('src', 'util', 'strings.py')));
    await waitFor('strings.py selected', () => api.test.tree.view.selection[0]?.endsWith('/util/strings.py'), [
      api.test.tree.view.onDidChangeSelection,
    ]);
  });

  it('decorates an open file and keeps one tree node for it', async () => {
    const editor = await vscode.window.showTextDocument(vscode.Uri.file(workspacePath('src', 'app.ts')));
    const uri = editor.document.uri.toString();
    const applied = await waitFor('decorations', () => api.test.decorations.entry(uri), [api.test.decorations.onApplied]);
    assert.deepEqual(Object.keys(applied.ranges).sort(), ['FIXME', 'HACK', 'TODO']);
    // Buffer results replace the file's disk results under the same node.
    await whenIdle(api);
    await treeBecomes(api, tree);
  });

  it('reflects a change on disk', async () => {
    const path = workspacePath('lib', 'notes.rs');
    const original = readFileSync(path, 'utf8');
    try {
      writeFileSync(path, original + '// FIXME appended on disk\n');
      const expected = [...tree];
      expected.splice(expected.indexOf('      TODO first line of a long note') + 1, 0, '      FIXME appended on disk');
      await treeBecomes(api, expected);
    } finally {
      writeFileSync(path, original);
    }
    await treeBecomes(api, tree);
  });
});
