// Folder and file filters, scopes, Remove Filter and Reset All Filters
// (spec 5.3, 7.6; inventory 6.2).

import * as vscode from 'vscode';
import type { ConfigurationSync } from '../config/sync';
import {
  fileGlob,
  folderGlob,
  isCurrentScope,
  nodePath,
  removalChoices,
  scopesOf,
} from '../filters/globs';
import type { ViewStateStore } from '../state/viewState';
import type { NodeCache } from '../tree/nodeCache';
import type { Prompts } from '../ui/prompts';

export interface FilterDeps {
  store: ViewStateStore;
  sync: ConfigurationSync;
  cache: NodeCache;
  prompts: Prompts;
}

export function registerFilterCommands(deps: FilterDeps): vscode.Disposable[] {
  const { store, sync, cache, prompts } = deps;
  const pathOf = (element: unknown) => (typeof element === 'string' ? nodePath(cache.ownKey(element)) : undefined);
  const setGlobs = async (include: string[], exclude: string[]) => {
    await store.setGlobs(include, exclude);
    sync.push();
  };
  const onlyFolder = (recursive: boolean) => async (element: unknown) => {
    const path = pathOf(element);
    if (path === undefined) return;
    await setGlobs([folderGlob(path, recursive)], store.snapshot().excludeGlobs);
  };
  const exclude = (glob: (path: string) => string) => async (element: unknown) => {
    const path = pathOf(element);
    if (path === undefined) return;
    const { includeGlobs, excludeGlobs } = store.snapshot();
    const g = glob(path);
    if (!excludeGlobs.includes(g)) await setGlobs(includeGlobs, [...excludeGlobs, g]);
  };

  const commands: Record<string, (...args: unknown[]) => unknown> = {
    'clippings.showOnlyThisFolder': onlyFolder(false),
    'clippings.showOnlyThisFolderAndSubfolders': onlyFolder(true),
    'clippings.excludeThisFolder': exclude((p) => folderGlob(p, true)),
    'clippings.excludeThisFile': exclude(fileGlob),
    'clippings.switchScope': async () => {
      const scopes = scopesOf(vscode.workspace.getConfiguration('clippings.filtering').get('scopes'));
      if (scopes.length === 0) {
        const choice = await prompts.message(
          'warning',
          'Clippings: No scopes configured (see clippings.filtering.scopes setting)',
          'Open Settings',
          'OK',
        );
        if (choice === 'Open Settings') {
          await vscode.workspace
            .getConfiguration('clippings.filtering')
            .update('scopes', [], vscode.ConfigurationTarget.Global);
          await vscode.commands.executeCommand('workbench.action.openSettingsJson', 'clippings.filtering.scopes');
        }
        return;
      }
      const { includeGlobs, excludeGlobs } = store.snapshot();
      const name = await prompts.pickItem(
        scopes.map((s) => ({
          label: s.name,
          description: isCurrentScope(s, includeGlobs, excludeGlobs) ? '$(check)' : undefined,
        })),
        { placeHolder: 'Select scope...' },
      );
      const scope = scopes.find((s) => s.name === name);
      if (scope) await setGlobs(scope.includeGlobs, scope.excludeGlobs);
    },
    'clippings.removeFilter': async () => {
      const state = store.snapshot();
      const filter = state.filtered ? state.currentFilter : '';
      const choices = removalChoices(filter, state.includeGlobs, state.excludeGlobs);
      const picked = await prompts.pick([...choices.keys()], {
        canPickMany: true,
        matchOnDescription: true,
        matchOnDetail: true,
        placeHolder: 'Select filters to remove',
      });
      if (!picked || picked.length === 0) return;
      const removals = picked.map((label) => choices.get(label));
      if (removals.some((r) => r?.kind === 'clear')) await store.setFilter(undefined);
      const include = state.includeGlobs.filter((g) => !removals.some((r) => r?.kind === 'include' && r.glob === g));
      const exclude = state.excludeGlobs.filter((g) => !removals.some((r) => r?.kind === 'exclude' && r.glob === g));
      await setGlobs(include, exclude);
    },
    'clippings.resetAllFilters': async () => {
      await store.setFilter(undefined);
      await setGlobs([], []);
    },
  };
  return Object.entries(commands).map(([id, run]) => vscode.commands.registerCommand(id, run));
}
