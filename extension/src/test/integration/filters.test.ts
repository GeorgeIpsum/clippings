import * as assert from 'node:assert/strict';
import { mkdirSync, rmSync, writeFileSync } from 'node:fs';
import * as vscode from 'vscode';
import type { ClippingsApi } from '../../testApi';
import { DEFAULT_TREE } from './fixture';
import { getApi, itemAt, setSetting, treeBecomes, whenIdle, slashPath, workspacePath } from './helpers';

describe('filters', () => {
  let api: ClippingsApi;

  before(async () => {
    api = await getApi();
    await whenIdle(api);
    await treeBecomes(api, DEFAULT_TREE);
  });

  afterEach(async () => {
    await vscode.commands.executeCommand('clippings.resetAllFilters');
    await treeBecomes(api, DEFAULT_TREE);
  });

  it('filters the tree by text and clears the filter', async () => {
    api.test.prompts.script('router');
    await vscode.commands.executeCommand('clippings.filter');
    assert.deepEqual(api.test.prompts.shown.at(-1), { kind: 'input', options: { prompt: 'Filter tree' } });
    await treeBecomes(api, [
      '(Scan mode: workspace and open files)',
      '(1 filter active)',
      'workspace',
      '  src',
      '    app.ts',
      '      TODO (alice) wire up the router',
      '      HACK temporary shim until the router lands',
    ]);
    assert.equal(api.test.viewState().currentFilter, 'router');
    assert.equal(api.test.viewState().filtered, true);
    await vscode.commands.executeCommand('clippings.filterClear');
    await treeBecomes(api, DEFAULT_TREE);
    assert.equal(api.test.viewState().filtered, false);
  });

  it('ignores a cancelled filter prompt', async () => {
    api.test.prompts.script(undefined);
    await vscode.commands.executeCommand('clippings.filter');
    assert.equal(api.test.viewState().filtered, false);
  });

  it('hides a folder and removes that filter', async () => {
    const src = await itemAt(api, 'workspace', 'src');
    await vscode.commands.executeCommand('clippings.excludeThisFolder', src.id);
    await treeBecomes(api, DEFAULT_TREE.slice(0, 9).toSpliced(1, 0, '(1 filter active)'));
    assert.deepEqual(api.test.viewState().excludeGlobs, [`${slashPath('src')}/**/*`]);
    api.test.prompts.script([`Exclude Folder: ${slashPath('src')}`]);
    await vscode.commands.executeCommand('clippings.removeFilter');
    await treeBecomes(api, DEFAULT_TREE);
  });

  it('hides a file', async () => {
    const plan = await itemAt(api, 'workspace', 'docs', 'plan.md');
    await vscode.commands.executeCommand('clippings.excludeThisFile', plan.id);
    await treeBecomes(api, [
      '(Scan mode: workspace and open files)',
      '(1 filter active)',
      'workspace',
      ...DEFAULT_TREE.slice(6),
    ]);
    assert.deepEqual(api.test.viewState().excludeGlobs, [slashPath('docs', 'plan.md')]);
  });

  it('shows only a folder, then only a folder and its subfolders', async () => {
    const src = await itemAt(api, 'workspace', 'src');
    await vscode.commands.executeCommand('clippings.showOnlyThisFolder', src.id);
    await treeBecomes(api, [
      '(Scan mode: workspace and open files)',
      '(1 filter active)',
      'workspace',
      '  src',
      '    app.ts',
      '      TODO (alice) wire up the router',
      '      FIXME handle the error path',
      '      HACK temporary shim until the router lands',
    ]);
    const srcAgain = await itemAt(api, 'workspace', 'src');
    await vscode.commands.executeCommand('clippings.showOnlyThisFolderAndSubfolders', srcAgain.id);
    await treeBecomes(api, [
      '(Scan mode: workspace and open files)',
      '(1 filter active)',
      'workspace',
      ...DEFAULT_TREE.slice(9),
    ]);
    assert.deepEqual(api.test.viewState().includeGlobs, [`${slashPath('src')}/**/*`]);
  });

  it('filters a folder whose name has spaces and brackets', async () => {
    const folder = 'app [slug] (old)';
    mkdirSync(workspacePath(folder), { recursive: true });
    writeFileSync(workspacePath(folder, 'page.ts'), '// TODO render the page\n');
    try {
      const withFolder = [
        ...DEFAULT_TREE.slice(0, 2),
        `  ${folder}`,
        '    page.ts',
        '      TODO render the page',
        ...DEFAULT_TREE.slice(2),
      ];
      await treeBecomes(api, withFolder);
      const app = await itemAt(api, 'workspace', folder);
      await vscode.commands.executeCommand('clippings.showOnlyThisFolder', app.id);
      await treeBecomes(api, [
        '(Scan mode: workspace and open files)',
        '(1 filter active)',
        'workspace',
        `  ${folder}`,
        '    page.ts',
        '      TODO render the page',
      ]);
      assert.deepEqual(api.test.viewState().includeGlobs, [`${slashPath('app [[]slug[]] (old)')}/*`]);
      await vscode.commands.executeCommand('clippings.resetAllFilters');
      await treeBecomes(api, withFolder);
      const appAgain = await itemAt(api, 'workspace', folder);
      await vscode.commands.executeCommand('clippings.excludeThisFolder', appAgain.id);
      await treeBecomes(api, [...withFolder.slice(0, 1), '(1 filter active)', ...DEFAULT_TREE.slice(1)]);
    } finally {
      rmSync(workspacePath(folder), { recursive: true, force: true });
    }
  });

  it('warns when no scopes exist and switches to a configured scope', async () => {
    api.test.prompts.script('OK');
    await vscode.commands.executeCommand('clippings.switchScope');
    const warning = api.test.prompts.shown.at(-1);
    assert.equal(warning?.kind, 'warning');
    await setSetting('filtering.scopes', [{ name: 'lib only', includeGlobs: '**/lib/**' }]);
    try {
      api.test.prompts.script('lib only');
      await vscode.commands.executeCommand('clippings.switchScope');
      await treeBecomes(api, [
        '(Scan mode: workspace and open files)',
        '(1 filter active)',
        'workspace',
        '  lib',
        '    notes.rs',
        '      TODO first line of a long note',
      ]);
      api.test.prompts.script(undefined);
      await vscode.commands.executeCommand('clippings.switchScope');
      const pick = api.test.prompts.shown.at(-1);
      assert.deepEqual(pick?.kind === 'pick' && pick.items, ['lib only']);
    } finally {
      await setSetting('filtering.scopes', undefined);
    }
  });

  it('resets filtering.scopes globally through the setting writer when Open Settings is chosen', async () => {
    assert.equal(vscode.workspace.getConfiguration('clippings.filtering').inspect('scopes')?.globalValue, undefined);
    try {
      api.test.prompts.script('Open Settings');
      await vscode.commands.executeCommand('clippings.switchScope');
      assert.deepEqual(
        vscode.workspace.getConfiguration('clippings.filtering').inspect('scopes')?.globalValue,
        [],
      );
    } finally {
      await vscode.commands.executeCommand('workbench.action.closeAllEditors');
      await setSetting('filtering.scopes', undefined);
    }
  });

  it('resets all filters, including the text filter', async () => {
    // Before the text filter, which hides `lib`.
    const lib = await itemAt(api, 'workspace', 'lib');
    await vscode.commands.executeCommand('clippings.excludeThisFolder', lib.id);
    api.test.prompts.script('guide');
    await vscode.commands.executeCommand('clippings.filter');
    await treeBecomes(api, [
      '(Scan mode: workspace and open files)',
      '(2 filters active)',
      'workspace',
      '  docs',
      '    plan.md',
      '      [ ] write the guide',
    ]);
    await vscode.commands.executeCommand('clippings.resetAllFilters');
    await treeBecomes(api, DEFAULT_TREE);
    assert.deepEqual(api.test.viewState(), { currentFilter: '', filtered: false, includeGlobs: [], excludeGlobs: [] });
  });
});
