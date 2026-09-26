import * as assert from 'node:assert/strict';
import * as vscode from 'vscode';
import type { ClippingsApi } from '../../testApi';
import { DEFAULT_TREE } from './fixture';
import { getApi, setSetting, treeBecomes, waitFor, whenIdle, workspacePath } from './helpers';

const APP_TODOS = [
  '      TODO (alice) wire up the router',
  '      FIXME handle the error path',
  '      HACK temporary shim until the router lands',
];

function inspect(key: string) {
  return vscode.workspace.getConfiguration('clippings').inspect(key);
}

describe('setting commands', () => {
  let api: ClippingsApi;

  before(async () => {
    api = await getApi();
    await whenIdle(api);
    await treeBecomes(api, DEFAULT_TREE);
  });

  afterEach(async () => {
    await vscode.commands.executeCommand('workbench.action.closeAllEditors');
  });

  it('switches scan modes in workspace settings', async () => {
    await vscode.window.showTextDocument(vscode.Uri.file(workspacePath('src', 'app.ts')));
    try {
      await vscode.commands.executeCommand('clippings.scanOpenFilesOnly');
      assert.equal(inspect('tree.scanMode')?.workspaceValue, 'open files');
      await treeBecomes(api, ['(Scan mode: open files)', 'workspace', '  src', '    app.ts', ...APP_TODOS]);

      await vscode.commands.executeCommand('clippings.scanCurrentFileOnly');
      await treeBecomes(api, ['(Scan mode: current file)', 'workspace', '  src', '    app.ts', ...APP_TODOS]);
      await vscode.window.showTextDocument(vscode.Uri.file(workspacePath('docs', 'plan.md')));
      await treeBecomes(api, [
        '(Scan mode: current file)',
        'workspace',
        '  docs',
        '    plan.md',
        '      [ ] write the guide',
        '      [x] pick a name',
      ]);

      await vscode.commands.executeCommand('clippings.scanWorkspaceOnly');
      await treeBecomes(api, ['(Scan mode: workspace only)', ...DEFAULT_TREE.slice(1)]);

      await vscode.commands.executeCommand('clippings.scanWorkspaceAndOpenFiles');
      assert.equal(inspect('tree.scanMode')?.workspaceValue, 'workspace');
      await treeBecomes(api, DEFAULT_TREE);
    } finally {
      await setSetting('tree.scanMode', undefined, vscode.ConfigurationTarget.Workspace);
    }
  });

  it('adds and removes tags where the tags are set', async () => {
    api.test.prompts.script('NOTE');
    await vscode.commands.executeCommand('clippings.addTag');
    assert.deepEqual(api.test.prompts.shown.at(-1), {
      kind: 'input',
      options: { prompt: 'New tag', placeHolder: 'e.g. FIXME' },
    });
    assert.deepEqual(inspect('general.tags')?.globalValue, ['BUG', 'HACK', 'FIXME', 'TODO', 'XXX', '[ ]', '[x]', 'NOTE']);

    await setSetting('general.tags', ['TODO'], vscode.ConfigurationTarget.Workspace);
    try {
      api.test.prompts.script('HACK');
      await vscode.commands.executeCommand('clippings.addTag');
      assert.deepEqual(inspect('general.tags')?.workspaceValue, ['TODO', 'HACK']);
      api.test.prompts.script(['TODO']);
      await vscode.commands.executeCommand('clippings.removeTag');
      assert.deepEqual(inspect('general.tags')?.workspaceValue, ['HACK']);
    } finally {
      await setSetting('general.tags', undefined, vscode.ConfigurationTarget.Workspace);
    }
    api.test.prompts.script(['NOTE']);
    await vscode.commands.executeCommand('clippings.removeTag');
    assert.deepEqual(inspect('general.tags')?.globalValue, ['BUG', 'HACK', 'FIXME', 'TODO', 'XXX', '[ ]', '[x]']);
    await setSetting('general.tags', undefined);
    await treeBecomes(api, DEFAULT_TREE);
  });

  it('toggles item counts, badges and compact folders in workspace settings', async () => {
    const root = async () => (await api.test.tree.items())[1];
    await vscode.commands.executeCommand('clippings.toggleItemCounts');
    assert.equal(inspect('tree.showCountsInTree')?.workspaceValue, true);
    await waitFor('counts', async () => (await root())?.description === '8', [api.test.tree.onDidChange]);

    assert.ok((await root())?.resourceUri);
    await vscode.commands.executeCommand('clippings.toggleBadges');
    assert.equal(inspect('tree.showBadges')?.workspaceValue, false);
    await waitFor('no badges', async () => (await root())?.resourceUri === undefined, [api.test.tree.onDidChange]);

    await vscode.commands.executeCommand('clippings.toggleCompactFolders');
    assert.equal(inspect('tree.disableCompactFolders')?.workspaceValue, true);

    for (const key of ['tree.showCountsInTree', 'tree.showBadges', 'tree.disableCompactFolders']) {
      await setSetting(key, undefined, vscode.ConfigurationTarget.Workspace);
    }
    await waitFor('defaults again', async () => (await root())?.description === undefined, [api.test.tree.onDidChange]);
  });

  it('goes to the next and previous todo without wrapping', async () => {
    const editor = await vscode.window.showTextDocument(vscode.Uri.file(workspacePath('src', 'app.ts')));
    const at = () => [editor.selection.active.line, editor.selection.active.character];
    editor.selection = new vscode.Selection(0, 0, 0, 0);
    await vscode.commands.executeCommand('clippings.goToNext');
    assert.deepEqual(at(), [1, 2]);
    await vscode.commands.executeCommand('clippings.goToNext');
    assert.deepEqual(at(), [3, 2]);
    await vscode.commands.executeCommand('clippings.goToNext');
    await vscode.commands.executeCommand('clippings.goToNext');
    assert.deepEqual(at(), [7, 2], 'stops at the last todo');
    await vscode.commands.executeCommand('clippings.goToPrevious');
    assert.deepEqual(at(), [3, 2]);
    await vscode.commands.executeCommand('clippings.goToPrevious');
    await vscode.commands.executeCommand('clippings.goToPrevious');
    assert.deepEqual(at(), [1, 2], 'stops at the first todo');
  });
});
