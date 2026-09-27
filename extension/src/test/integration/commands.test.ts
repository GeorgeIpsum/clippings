import * as assert from 'node:assert/strict';
import * as vscode from 'vscode';
import type { ClippingsApi } from '../../testApi';
import { DEFAULT_TREE } from './fixture';
import { getApi, itemAt, setSetting, treeBecomes, whenIdle, workspacePath } from './helpers';

const REGISTRATION_ONLY = ['clippings.openUrl', 'clippings.stopScan', 'clippings.onStatusBarClicked'];

describe('every command', function () {
  this.timeout(60_000);
  let api: ClippingsApi;
  let declared: string[];

  before(async () => {
    api = await getApi();
    await whenIdle(api);
    await treeBecomes(api, DEFAULT_TREE);
    const ext = vscode.extensions.getExtension('clippings-dev.clippings');
    const manifest = ext?.packageJSON as { contributes: { commands: { command: string }[] } };
    declared = manifest.contributes.commands.map((c) => c.command);
  });

  after(async () => {
    for (const key of ['tree.scanMode', 'tree.showCountsInTree', 'tree.showBadges', 'tree.disableCompactFolders']) {
      await setSetting(key, undefined, vscode.ConfigurationTarget.Workspace);
    }
    await vscode.commands.executeCommand('workbench.action.closeAllEditors');
    await vscode.commands.executeCommand('clippings.resetCache');
    await whenIdle(api);
    await treeBecomes(api, DEFAULT_TREE);
  });

  it('registers the 37 declared and 3 registration-only commands', async () => {
    assert.equal(declared.length, 37);
    const registered = new Set(await vscode.commands.getCommands(true));
    for (const id of [...declared, ...REGISTRATION_ONLY]) assert.ok(registered.has(id), id);
  });

  it('runs each command at least once without error', async () => {
    const ran = new Set<string>();
    const run = async (id: string, ...args: unknown[]) => {
      await vscode.commands.executeCommand(id, ...args);
      ran.add(id);
    };
    const src = (await itemAt(api, 'workspace', 'src')).id;
    const lib = (await itemAt(api, 'workspace', 'lib')).id;
    const plan = (await itemAt(api, 'workspace', 'docs', 'plan.md')).id;
    const editor = await vscode.window.showTextDocument(vscode.Uri.file(workspacePath('src', 'app.ts')));

    for (const id of ['showFlatView', 'showTagsOnlyView', 'showTreeView', 'groupByTag', 'ungroupByTag']) {
      await run(`clippings.${id}`);
    }
    for (const id of ['groupBySubTag', 'ungroupBySubTag', 'expand', 'collapse', 'refresh', 'stopScan']) {
      await run(`clippings.${id}`);
    }
    api.test.prompts.script(undefined);
    await run('clippings.filter');
    await run('clippings.filterClear');
    for (const id of ['scanOpenFilesOnly', 'scanCurrentFileOnly', 'scanWorkspaceOnly', 'scanWorkspaceAndOpenFiles']) {
      await run(`clippings.${id}`);
    }
    api.test.prompts.script(undefined, undefined);
    await run('clippings.addTag');
    await run('clippings.removeTag');
    await run('clippings.exportTree');
    await run('clippings.showOnlyThisFolder', src);
    await run('clippings.showOnlyThisFolderAndSubfolders', src);
    await run('clippings.excludeThisFolder', lib);
    await run('clippings.excludeThisFile', plan);
    api.test.prompts.script('OK', undefined);
    await run('clippings.switchScope');
    await run('clippings.removeFilter');
    await run('clippings.resetAllFilters');
    for (const id of ['toggleItemCounts', 'toggleBadges', 'toggleCompactFolders']) {
      await run(`clippings.${id}`);
      await run(`clippings.${id}`);
    }
    await vscode.window.showTextDocument(editor.document);
    await run('clippings.goToNext');
    await run('clippings.goToPrevious');
    await run('clippings.revealInFile');
    await run('clippings.revealInFile', editor.document.uri.toString(), { line: 3, character: 2 });
    await run('clippings.reveal');
    await run('clippings.openUrl');
    await run('clippings.onStatusBarClicked');
    // `importTodoTreeSettings` asks to overwrite when a Clippings value it
    // would write is already explicit (spec 7.3 fix round); script an answer
    // so the command completes whether or not that prompt appears here.
    api.test.prompts.script('Cancel');
    await run('clippings.importTodoTreeSettings');
    await run('clippings.resetCache');
    await run('clippings.showLog');
    await run('clippings.restartServer');
    await whenIdle(api);

    const missing = [...declared, ...REGISTRATION_ONLY].filter((id) => !ran.has(id));
    assert.deepEqual(missing, []);
  });
});
