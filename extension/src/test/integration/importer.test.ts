import * as assert from 'node:assert/strict';
import { mkdirSync, rmSync, writeFileSync } from 'node:fs';
import * as vscode from 'vscode';
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
