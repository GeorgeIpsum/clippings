import * as assert from 'node:assert/strict';
import * as vscode from 'vscode';
import type { Range } from '../../protocol';
import type { ClippingsApi } from '../../testApi';
import { getApi, nextEvent, setSetting, waitFor, whenIdle, workspacePath } from './helpers';

function r(line: number, start: number, end: number): Range {
  return { start: { line, character: start }, end: { line, character: end } };
}

describe('decorations', () => {
  let api: ClippingsApi;
  let editor: vscode.TextEditor;
  let uri: string;
  const entry = () => api.test.decorations.entry(uri);

  before(async () => {
    api = await getApi();
    await whenIdle(api);
  });

  beforeEach(async () => {
    editor = await vscode.window.showTextDocument(vscode.Uri.file(workspacePath('src', 'app.ts')));
    uri = editor.document.uri.toString();
  });

  afterEach(async () => {
    await vscode.commands.executeCommand('workbench.action.files.revert');
    await vscode.commands.executeCommand('workbench.action.closeAllEditors');
  });

  it('applies the server ranges per tag to an opened document', async () => {
    const applied = await waitFor('decorations', () => entry(), [api.test.decorations.onApplied]);
    assert.equal(applied.version, editor.document.version);
    assert.equal(applied.generation, api.test.decorations.generation);
    assert.deepEqual(applied.ranges, { TODO: [r(1, 5, 9)], FIXME: [r(3, 5, 10)], HACK: [r(7, 5, 9)] });
    for (const key of ['TODO', 'FIXME', 'HACK']) assert.ok(api.test.decorations.styleKeys.includes(key), key);
  });

  it('follows edits with the new document version', async () => {
    await waitFor('decorations', () => entry(), [api.test.decorations.onApplied]);
    await editor.edit((e) => e.insert(new vscode.Position(0, 0), '// BUG inserted at the top\n'));
    const version = editor.document.version;
    const applied = await waitFor(
      'decorations for the edit',
      () => (entry()?.version === version ? entry() : undefined),
      [api.test.decorations.onApplied],
    );
    assert.deepEqual(applied.ranges, { BUG: [r(0, 3, 6)], TODO: [r(2, 5, 9)], FIXME: [r(4, 5, 10)], HACK: [r(8, 5, 9)] });
  });

  it('clears when highlights are disabled and restyles when attributes change', async () => {
    await waitFor('decorations', () => entry(), [api.test.decorations.onApplied]);
    const generation = api.test.decorations.generation ?? -1;
    await setSetting('highlights.enabled', false);
    try {
      await waitFor('no decorations', () => entry() && Object.keys(entry()?.ranges ?? {}).length === 0, [
        api.test.decorations.onApplied,
      ]);
      assert.ok((api.test.decorations.generation ?? -1) > generation, 'a new style generation');
    } finally {
      await setSetting('highlights.enabled', undefined);
    }
    await waitFor('decorations again', () => Object.keys(entry()?.ranges ?? {}).length === 3, [
      api.test.decorations.onApplied,
    ]);
  });

  it('reapplies the cached decorations when the editor becomes visible again', async () => {
    await waitFor('decorations', () => entry(), [api.test.decorations.onApplied]);
    await vscode.window.showTextDocument(vscode.Uri.file(workspacePath('docs', 'plan.md')));
    const reapplied = nextEvent('a reapply from the cache', api.test.decorations.onApplied, (e) => e.uri === uri);
    await vscode.window.showTextDocument(editor.document);
    assert.equal((await reapplied).source, 'cache');
  });
});
