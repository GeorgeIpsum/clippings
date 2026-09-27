import * as assert from 'node:assert/strict';
import * as vscode from 'vscode';
import type { ClippingsApi } from '../../testApi';
import { DEFAULT_TREE } from './fixture';
import { getApi, itemAt, setSetting, treeBecomes, waitFor, whenIdle } from './helpers';

describe('context keys', () => {
  let api: ClippingsApi;
  const key = (name: string) => api.test.contextKeys()[`clippings-${name}`];

  before(async () => {
    api = await getApi();
    await whenIdle(api);
    await treeBecomes(api, DEFAULT_TREE);
  });

  after(async () => {
    await vscode.commands.executeCommand('clippings.resetCache');
    await treeBecomes(api, DEFAULT_TREE);
  });

  it('start from the settings and view state', () => {
    assert.equal(key('flat'), false);
    assert.equal(key('tags-only'), false);
    assert.equal(key('expanded'), false);
    assert.equal(key('collapsible'), true);
    assert.equal(key('filtered'), false);
    assert.equal(key('scan-mode'), 'workspace');
    assert.equal(key('show-reveal-button'), false);
    assert.equal(key('show-refresh-button'), true);
    assert.equal(key('can-toggle-compact-folders'), true);
    assert.equal(key('is-empty'), false);
  });

  it('follow the view commands', async () => {
    await vscode.commands.executeCommand('clippings.showTagsOnlyView');
    assert.equal(key('tags-only'), true);
    assert.equal(key('collapsible'), false);
    await vscode.commands.executeCommand('clippings.groupByTag');
    assert.equal(key('grouped-by-tag'), true);
    assert.equal(key('collapsible'), true);
    await vscode.commands.executeCommand('clippings.expand');
    assert.equal(key('expanded'), true);
    await vscode.commands.executeCommand('clippings.resetCache');
    assert.equal(key('tags-only'), false);
    assert.equal(key('grouped-by-tag'), false);
  });

  it('follow the filters', async () => {
    // The text filter must keep `lib` visible: the server applies it
    // asynchronously, so `lib` is looked up either before or after it lands.
    api.test.prompts.script('long note');
    try {
      await vscode.commands.executeCommand('clippings.filter');
      assert.equal(key('filtered'), true);
      assert.equal(key('global-filter-active'), 'long note');
      const lib = await itemAt(api, 'workspace', 'lib');
      await vscode.commands.executeCommand('clippings.excludeThisFolder', lib.id);
      assert.equal(key('folder-filter-active'), true);
    } finally {
      // A filter left behind would hide the sub-tags the next test waits for.
      await vscode.commands.executeCommand('clippings.resetAllFilters');
    }
    assert.equal(key('filtered'), false);
    assert.equal(key('folder-filter-active'), false);
  });

  it('follow settings and the server status', async () => {
    await setSetting('tree.trackFile', false);
    await setSetting('tree.buttons.export', true);
    try {
      assert.equal(key('show-reveal-button'), true);
      assert.equal(key('show-export-button'), true);
    } finally {
      await setSetting('tree.buttons.export', undefined);
      await setSetting('tree.trackFile', undefined);
    }
    assert.equal(key('show-reveal-button'), false);

    await setSetting('regex.subTagRegex', '^\\s*\\((\\w+)\\)');
    try {
      await waitFor('sub-tags', () => key('has-sub-tags') === true, [api.test.server.onStatus]);
    } finally {
      await setSetting('regex.subTagRegex', undefined);
    }
    await waitFor('no sub-tags', () => key('has-sub-tags') === false, [api.test.server.onStatus]);

    await setSetting('tree.hideTreeWhenEmpty', true);
    try {
      api.test.prompts.script('matches nothing at all');
      await vscode.commands.executeCommand('clippings.filter');
      await waitFor('an empty view', () => key('is-empty') === true, [api.test.server.onStatus]);
      await vscode.commands.executeCommand('clippings.filterClear');
      await waitFor('a view again', () => key('is-empty') === false, [api.test.server.onStatus]);
    } finally {
      await setSetting('tree.hideTreeWhenEmpty', undefined);
    }
  });
});
