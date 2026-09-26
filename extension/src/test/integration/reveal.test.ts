import * as assert from 'node:assert/strict';
import * as vscode from 'vscode';
import { NEEDS_SCAN_MESSAGE } from '../../commands/scan';
import type { ClippingsApi } from '../../testApi';
import { DEFAULT_TREE } from './fixture';
import { getApi, itemAt, setSetting, treeBecomes, waitFor, whenIdle, workspacePath } from './helpers';

async function open(path: string): Promise<vscode.TextEditor> {
  return vscode.window.showTextDocument(vscode.Uri.file(path));
}

function selected(api: ClippingsApi): string | undefined {
  return api.test.tree.view.selection[0];
}

describe('reveal, track file and todo clicks', () => {
  let api: ClippingsApi;

  before(async () => {
    api = await getApi();
    await whenIdle(api);
    await treeBecomes(api, DEFAULT_TREE);
    await vscode.commands.executeCommand('clippings-view.focus');
  });

  afterEach(async () => {
    await vscode.commands.executeCommand('workbench.action.closeAllEditors');
  });

  it('opens a clicked todo at its position and flashes the line', async () => {
    const todo = await itemAt(api, 'workspace', 'src', 'app.ts', 'FIXME handle the error path');
    assert.ok(todo.command);
    await vscode.commands.executeCommand(todo.command.command, ...todo.command.arguments);
    const editor = vscode.window.activeTextEditor;
    assert.ok(editor);
    assert.equal(editor.document.uri.fsPath, vscode.Uri.file(workspacePath('src', 'app.ts')).fsPath);
    assert.deepEqual([editor.selection.active.line, editor.selection.active.character], [3, 2]);
    assert.deepEqual(api.test.tree.lastFlash(), { uri: editor.document.uri.toString(), line: 3 });
  });

  it('tracks the active file in the tree', async () => {
    const path = workspacePath('src', 'util', 'strings.py');
    await open(path);
    await waitFor('strings.py selected', () => selected(api)?.endsWith(`/f:${path}`), [
      api.test.tree.view.onDidChangeSelection,
    ]);
  });

  it('reveals the current file on demand when tracking is off', async () => {
    await setSetting('tree.trackFile', false);
    try {
      const path = workspacePath('docs', 'plan.md');
      await open(path);
      assert.ok(!selected(api)?.endsWith(`/f:${path}`));
      await vscode.commands.executeCommand('clippings.reveal');
      await waitFor('plan.md selected', () => selected(api)?.endsWith(`/f:${path}`), [
        api.test.tree.view.onDidChangeSelection,
      ]);
    } finally {
      await setSetting('tree.trackFile', undefined);
    }
  });

  it('asks for a refresh when the startup scan is off', async () => {
    await setSetting('tree.scanAtStartup', false);
    try {
      await vscode.commands.executeCommand('clippings.restartServer');
      const s = api.test.server;
      await waitFor('needsScan', () => s.running && s.status()?.needsScan, [s.onStatus, s.onRunning]);
      assert.equal(api.test.tree.view.message, NEEDS_SCAN_MESSAGE);
      await vscode.commands.executeCommand('clippings.refresh');
      await waitFor('a scan', () => s.status()?.needsScan === false, [s.onStatus]);
      assert.equal(api.test.tree.view.message, undefined);
    } finally {
      await setSetting('tree.scanAtStartup', undefined);
    }
    await whenIdle(api);
    await treeBecomes(api, DEFAULT_TREE);
  });
});
