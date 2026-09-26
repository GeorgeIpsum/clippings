import * as assert from 'node:assert/strict';
import { rmSync, writeFileSync } from 'node:fs';
import * as vscode from 'vscode';
import type { ClippingsApi } from '../../testApi';
import { DEFAULT_TREE } from './fixture';
import { getApi, treeBecomes, waitFor, whenIdle, workspacePath } from './helpers';

// Notebook cells reach the server through the `vscode-notebook-cell` scheme
// (spec 6.1), are indexed per cell and grouped under their notebook (spec
// 11.2), and are decorated in each cell's editor (spec 7.7).
const NOTEBOOK = {
  cells: [
    { cell_type: 'code', execution_count: null, metadata: {}, outputs: [], source: ['# TODO cell one\n', 'x = 1\n'] },
    { cell_type: 'markdown', metadata: {}, source: ['FIXME in markdown'] },
  ],
  metadata: { language_info: { name: 'python' } },
  nbformat: 4,
  nbformat_minor: 5,
};

describe('notebook cells', () => {
  let api: ClippingsApi;
  const path = workspacePath('notes.ipynb');

  before(async () => {
    api = await getApi();
    await whenIdle(api);
  });

  after(async () => {
    rmSync(path, { force: true });
    await treeBecomes(api, DEFAULT_TREE);
  });

  it('lists and decorates the todos in notebook cells', async () => {
    writeFileSync(path, JSON.stringify(NOTEBOOK));
    const notebook = await vscode.workspace.openNotebookDocument(vscode.Uri.file(path));
    await vscode.window.showNotebookDocument(notebook);
    const cell = notebook.cellAt(0).document.uri;
    assert.equal(cell.scheme, 'vscode-notebook-cell');
    const applied = await waitFor('decorations in the first cell', () => api.test.decorations.entry(cell.toString()), [
      api.test.decorations.onApplied,
    ]);
    assert.deepEqual(applied.ranges, {
      TODO: [{ start: { line: 0, character: 2 }, end: { line: 0, character: 6 } }],
    });
    await treeBecomes(api, [...DEFAULT_TREE, '  notes.ipynb', '    TODO cell one', '    FIXME in markdown']);

    // Empty the cells so the still-open cell buffers hold no todos, then save
    // and close; `after` deletes the file.
    const edit = new vscode.WorkspaceEdit();
    for (const c of notebook.getCells()) {
      edit.replace(c.document.uri, new vscode.Range(0, 0, c.document.lineCount, 0), '');
    }
    assert.ok(await vscode.workspace.applyEdit(edit));
    assert.ok(await notebook.save());
    await vscode.commands.executeCommand('workbench.action.closeAllEditors');
  });
});
