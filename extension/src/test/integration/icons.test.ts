import * as assert from 'node:assert/strict';
import { existsSync, readFileSync } from 'node:fs';
import { basename } from 'node:path';
import * as vscode from 'vscode';
import type { ClippingsApi } from '../../testApi';
import { DEFAULT_TREE } from './fixture';
import { getApi, itemAt, setSetting, treeBecomes, waitFor, whenIdle, workspacePath } from './helpers';

describe('icons', () => {
  let api: ClippingsApi;
  const todoIcon = async () => (await itemAt(api, 'workspace', 'lib', 'notes.rs', 'TODO first line of a long note')).icon;

  before(async () => {
    api = await getApi();
    await whenIdle(api);
    await treeBecomes(api, DEFAULT_TREE);
  });

  afterEach(async () => {
    await setSetting('highlights.customHighlight', undefined);
    await vscode.commands.executeCommand('workbench.action.closeAllEditors');
  });

  it('renders octicons and the check-circle to SVG files in the icon colour', async () => {
    const bug = await itemAt(api, 'workspace', 'src', 'util', 'strings.py', 'BUG (bob) drops leading tabs too');
    assert.ok(bug.icon);
    assert.match(basename(bug.icon), /^octicon-bug-green-[0-9a-f]{8}\.svg$/);
    assert.match(readFileSync(bug.icon, 'utf8'), /octicon-bug.*fill="green"|fill="green".*octicon-bug/);
    const todo = await todoIcon();
    assert.ok(todo);
    assert.match(basename(todo), /^check-check-green-/);
    assert.ok(existsSync(todo));
  });

  it('renders the todo-tree icon names and codicons', async () => {
    await setSetting('highlights.customHighlight', { TODO: { icon: 'todo-tree-filled', iconColour: '#123456' } });
    const filled = await waitFor('a filled todo icon', async () => ((await todoIcon())?.includes('todoTree-filled') ? todoIcon() : undefined), [
      api.test.tree.onDidChange,
    ]);
    assert.match(readFileSync(filled, 'utf8'), /fill="#123456"/);

    await setSetting('highlights.customHighlight', { TODO: { icon: '$(beaker)' } });
    await waitFor('a codicon', async () => (await todoIcon()) === '$(beaker)', [api.test.tree.onDidChange]);
  });

  it('gives decorations a gutter icon file when asked', async () => {
    await setSetting('highlights.customHighlight', { TODO: { gutterIcon: true, icon: 'flame' } });
    const editor = await vscode.window.showTextDocument(vscode.Uri.file(workspacePath('src', 'app.ts')));
    const uri = editor.document.uri.toString();
    await waitFor('decorations with a gutter icon', () => api.test.decorations.options('TODO')?.gutterIconPath && api.test.decorations.entry(uri), [
      api.test.decorations.onApplied,
    ]);
    const gutter = api.test.decorations.options('TODO')?.gutterIconPath;
    assert.ok(gutter instanceof vscode.Uri);
    assert.match(basename(gutter.fsPath), /^octicon-flame-green-/);
    assert.ok(existsSync(gutter.fsPath));
    assert.equal(api.test.decorations.options('FIXME')?.gutterIconPath, undefined);
  });

  it('warns once about icon names it does not know', async () => {
    const shown = () => api.test.prompts.shown.filter((s) => s.kind === 'warning' && s.message.includes('Invalid icons'));
    const before = shown().length;
    await setSetting('highlights.customHighlight', { TODO: { icon: 'not-an-octicon' } });
    await waitFor('an icon warning', () => shown().length === before + 1, [api.test.server.onStatus]);
    const last = shown().at(-1);
    assert.equal(last?.kind === 'warning' ? last.message : '', 'Clippings: Invalid icons: not-an-octicon');
  });
});
