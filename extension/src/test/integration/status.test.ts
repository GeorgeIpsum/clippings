import * as assert from 'node:assert/strict';
import * as vscode from 'vscode';
import type { ClippingsApi } from '../../testApi';
import { DEFAULT_TREE } from './fixture';
import { getApi, setSetting, treeBecomes, waitFor, whenIdle } from './helpers';

function inspect(key: string) {
  return vscode.workspace.getConfiguration('clippings').inspect(key);
}

describe('status bar, badge and title', () => {
  let api: ClippingsApi;
  const bar = () => api.test.statusBar();
  const badge = () => api.test.tree.view.badge;

  before(async () => {
    api = await getApi();
    await whenIdle(api);
    await treeBecomes(api, DEFAULT_TREE);
  });

  it('is hidden in the default none mode and shows scanning during a scan', async () => {
    assert.equal(bar()?.visible, false);
    const scanning = waitFor('a scanning status bar', () => (bar()?.text === 'Clippings: Scanning...' ? bar() : undefined), [
      api.test.server.onStatus,
    ]);
    await vscode.commands.executeCommand('clippings.refresh');
    const shown = await scanning;
    assert.equal(shown.command, 'clippings.stopScan');
    assert.equal(shown.visible, true);
    await whenIdle(api);
    assert.equal(bar()?.visible, false);
  });

  it('shows the total and tag counts', async () => {
    await setSetting('general.statusBar', 'total');
    await waitFor('the total', () => bar()?.text === '$(check) 8', [api.test.server.onStatus]);
    assert.equal(bar()?.tooltip, 'Clippings total');
    assert.equal(bar()?.command, 'clippings.onStatusBarClicked');
    await setSetting('general.statusBar', 'tags');
    await waitFor(
      'tag counts',
      // todo-tree's spacing: each item ends in a space and items are joined with another.
      () => bar()?.text === '$(check) BUG: 1  HACK: 1  FIXME: 1  TODO: 3  [ ]: 1  [x]: 1',
      [api.test.server.onStatus],
    );
    await setSetting('general.statusBar', undefined);
  });

  it('cycles the mode where it is set, with an information message', async () => {
    await setSetting('general.statusBarClickBehaviour', 'cycle');
    await setSetting('general.statusBar', 'total', vscode.ConfigurationTarget.Workspace);
    try {
      await vscode.commands.executeCommand('clippings.onStatusBarClicked');
      assert.equal(inspect('general.statusBar')?.workspaceValue, 'tags');
      assert.deepEqual(api.test.prompts.shown.at(-1), {
        kind: 'info',
        message: 'Clippings: Now showing tag counts',
        actions: [],
      });
      await vscode.commands.executeCommand('clippings.onStatusBarClicked');
      assert.equal(inspect('general.statusBar')?.workspaceValue, 'top three');
    } finally {
      await setSetting('general.statusBar', undefined, vscode.ConfigurationTarget.Workspace);
      await setSetting('general.statusBarClickBehaviour', undefined);
    }
  });

  it('toggles highlights on click', async () => {
    await setSetting('general.statusBarClickBehaviour', 'toggle highlights');
    try {
      await vscode.commands.executeCommand('clippings.onStatusBarClicked');
      assert.equal(inspect('highlights.enabled')?.globalValue, false);
      await vscode.commands.executeCommand('clippings.onStatusBarClicked');
      assert.equal(inspect('highlights.enabled')?.globalValue, true);
    } finally {
      await setSetting('highlights.enabled', undefined);
      await setSetting('general.statusBarClickBehaviour', undefined);
    }
  });

  it('reveals the view on click', async () => {
    await vscode.commands.executeCommand('workbench.view.explorer');
    await waitFor('a hidden view', () => !api.test.tree.view.visible, [api.test.tree.view.onDidChangeVisibility]);
    await vscode.commands.executeCommand('clippings.onStatusBarClicked');
    await waitFor('a visible view', () => api.test.tree.view.visible, [api.test.tree.view.onDidChangeVisibility]);
  });

  it('sets the badge and the view title', async () => {
    assert.equal(api.test.tree.view.title, 'Tree');
    assert.equal(badge(), undefined);
    await setSetting('general.showActivityBarBadge', true);
    await setSetting('tree.showCountsInTree', true);
    try {
      await waitFor('a badge', () => badge()?.value === 8, [api.test.server.onStatus]);
      assert.equal(badge()?.tooltip, '8 todos');
      await waitFor('a counted title', () => api.test.tree.view.title === 'Tree (8)', [api.test.server.onStatus]);
    } finally {
      await setSetting('general.showActivityBarBadge', undefined);
      await setSetting('tree.showCountsInTree', undefined);
    }
    await waitFor('no badge', () => badge() === undefined, [api.test.server.onStatus]);
  });

  it('shows each distinct configuration warning once', async () => {
    const warnings = () =>
      api.test.prompts.shown.filter((s) => (s.kind === 'warning' ? s.message.includes('notacolour') : false));
    await setSetting('highlights.customHighlight', { TODO: { foreground: 'notacolour' } });
    try {
      await waitFor('a colour warning', () => warnings().length === 1, [api.test.server.onStatus]);
      assert.deepEqual(warnings()[0], {
        kind: 'warning',
        message: 'Clippings: Invalid colour settings: customHighlight.TODO.foreground (notacolour)',
        actions: [],
      });
      await setSetting('tree.showCountsInTree', true);
      await waitFor('another status', () => api.test.tree.view.title === 'Tree (8)', [api.test.server.onStatus]);
      assert.equal(warnings().length, 1);
    } finally {
      await setSetting('tree.showCountsInTree', undefined);
      await setSetting('highlights.customHighlight', undefined);
    }
  });

  it('shows a regex error with an Open Settings action and keeps the last tree', async () => {
    api.test.prompts.script(undefined);
    await setSetting('regex.regex', '(unclosed');
    try {
      const shown = await waitFor(
        'an error',
        () => api.test.prompts.shown.find((s) => (s.kind === 'warning' ? s.message.includes('(unclosed') : false)),
        [api.test.server.onStatus],
      );
      assert.deepEqual(shown.kind === 'warning' && shown.actions, ['Open Settings']);
      assert.ok(api.test.server.status()?.error);
      await treeBecomes(api, DEFAULT_TREE);
    } finally {
      await setSetting('regex.regex', undefined);
    }
    await waitFor('no error', () => api.test.server.status()?.error === null, [api.test.server.onStatus]);
  });
});
