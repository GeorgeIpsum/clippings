import * as assert from 'node:assert/strict';
import { mkdirSync, rmSync, writeFileSync } from 'node:fs';
import * as vscode from 'vscode';
import { SettingWriter } from '../../config/writes';
import type { ClippingsApi } from '../../testApi';
import { DEFAULT_TREE } from './fixture';
import { getApi, nextEvent, treeBecomes, whenIdle, workspacePath } from './helpers';

/** Replaces the workspace settings file and waits for VS Code to read it. */
async function writeWorkspaceSettings(settings: Record<string, unknown> | undefined, section: string): Promise<void> {
  const changed = nextEvent('a settings change', vscode.workspace.onDidChangeConfiguration, (e) =>
    e.affectsConfiguration(section),
  );
  const path = workspacePath('.vscode', 'settings.json');
  if (settings) {
    mkdirSync(workspacePath('.vscode'), { recursive: true });
    writeFileSync(path, JSON.stringify(settings, null, 2));
  } else {
    rmSync(path, { force: true });
  }
  await changed;
}

const TODO_TREE_SETTINGS = {
  'todo-tree.general.tags': ['BUG', 'TODO'],
  'todo-tree.general.debug': true,
  'todo-tree.ripgrep.ripgrepArgs': '--max-columns=1000',
  'todo-tree.tree.buttons.export': true,
};

function offers(api: ClippingsApi): number {
  return api.test.prompts.shown.filter((s) => s.kind === 'info' && s.message.includes('found Todo Tree settings')).length;
}

describe('todo-tree settings import', () => {
  let api: ClippingsApi;

  before(async () => {
    api = await getApi();
    await whenIdle(api);
    await writeWorkspaceSettings(TODO_TREE_SETTINGS, 'todo-tree');
  });

  after(async () => {
    await writeWorkspaceSettings(undefined, 'clippings');
    await whenIdle(api);
    await treeBecomes(api, DEFAULT_TREE);
  });

  it('offers the import until the user chooses Never', async () => {
    const before = offers(api);
    api.test.prompts.script('Not Now');
    await api.test.offerImport();
    assert.equal(offers(api), before + 1);
    assert.equal(vscode.workspace.getConfiguration('clippings').inspect('general.tags')?.workspaceValue, undefined);
    api.test.prompts.script('Never');
    await api.test.offerImport();
    assert.equal(offers(api), before + 2);
    await api.test.offerImport();
    assert.equal(offers(api), before + 2, 'no offer after Never');
  });

  it('imports carried values to the same scope and maps debug to a log level', async () => {
    await vscode.commands.executeCommand('clippings.importTodoTreeSettings');
    const clippings = vscode.workspace.getConfiguration('clippings');
    assert.deepEqual(clippings.inspect('general.tags')?.workspaceValue, ['BUG', 'TODO']);
    assert.equal(clippings.inspect('tree.buttons.export')?.workspaceValue, true);
    assert.equal(clippings.inspect('server.logLevel')?.workspaceValue, 'debug');
    const done = api.test.prompts.shown.at(-1);
    assert.deepEqual(done, { kind: 'info', message: 'Clippings: imported 3 settings from Todo Tree.', actions: [] });
    assert.equal(api.test.contextKeys()['clippings-show-export-button'], true);
  });
});

describe('todo-tree import: write failures', () => {
  it('shows a warning for a write that fails, and still writes the next one', async () => {
    const api = await getApi();
    // The real `SettingWriter` the importer now writes through (spec 10.2):
    // an unregistered key is refused by VS Code before any file is touched,
    // giving a write failure with no side effect on the real settings file.
    const writer = new SettingWriter(api.test.prompts);
    const before = api.test.prompts.shown.length;

    const failed = await writer.write('thisKeyIsNotRegistered', 'x', 'workspace');
    const succeeded = await writer.write('tree.buttons.export', true, 'workspace');

    assert.equal(failed, false, 'the unregistered write is reported as failed');
    assert.equal(succeeded, true, 'the next write still ran');
    const warning = api.test.prompts.shown
      .slice(before)
      .find((s) => s.kind === 'warning' && s.message.startsWith('Clippings: could not update clippings.thisKeyIsNotRegistered:'));
    assert.ok(warning, 'a warning was shown for the failed write');
    assert.equal(
      vscode.workspace.getConfiguration('clippings').inspect('tree.buttons.export')?.workspaceValue,
      true,
    );

    await vscode.workspace
      .getConfiguration('clippings')
      .update('tree.buttons.export', undefined, vscode.ConfigurationTarget.Workspace);
  });
});

describe('todo-tree import: overwrite confirmation', () => {
  let api: ClippingsApi;

  before(async () => {
    api = await getApi();
    await whenIdle(api);
    await writeWorkspaceSettings(
      { 'todo-tree.general.tags': ['BUG', 'TODO'], 'clippings.general.tags': ['EXISTING'] },
      'clippings',
    );
  });

  after(async () => {
    await writeWorkspaceSettings(undefined, 'clippings');
    await whenIdle(api);
    await treeBecomes(api, DEFAULT_TREE);
  });

  it('does nothing when the user cancels', async () => {
    api.test.prompts.script('Cancel');
    await vscode.commands.executeCommand('clippings.importTodoTreeSettings');
    assert.deepEqual(vscode.workspace.getConfiguration('clippings').inspect('general.tags')?.workspaceValue, [
      'EXISTING',
    ]);
    assert.deepEqual(api.test.prompts.shown.at(-1), {
      kind: 'warning',
      message: 'Overwrite 1 Clippings setting with your Todo Tree settings?',
      actions: ['Overwrite', 'Cancel'],
    });
  });

  it('replaces the existing values when the user overwrites', async () => {
    api.test.prompts.script('Overwrite');
    const changed = nextEvent('the overwrite', vscode.workspace.onDidChangeConfiguration, (e) =>
      e.affectsConfiguration('clippings.general.tags'),
    );
    await vscode.commands.executeCommand('clippings.importTodoTreeSettings');
    await changed;
    assert.deepEqual(vscode.workspace.getConfiguration('clippings').inspect('general.tags')?.workspaceValue, [
      'BUG',
      'TODO',
    ]);
    assert.deepEqual(api.test.prompts.shown.at(-1), {
      kind: 'info',
      message: 'Clippings: imported 1 setting from Todo Tree.',
      actions: [],
    });
  });
});
