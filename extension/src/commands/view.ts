// View style, grouping, the tree filter and Reset Cache (spec 7.6). Each
// updates workspace storage and sends the configuration.

import * as vscode from 'vscode';
import { replacesTree, type ConfigurationSync } from '../config/sync';
import type { ViewFlags, ViewStateStore } from '../state/viewState';
import type { Expansion } from '../tree/expansion';
import type { TreeProvider } from '../tree/provider';
import type { Prompts } from '../ui/prompts';
import { resetExpansion } from './expand';

export interface ViewDeps {
  store: ViewStateStore;
  sync: ConfigurationSync;
  expansion: Expansion;
  provider: TreeProvider;
  prompts: Prompts;
}

export function registerViewCommands(deps: ViewDeps): vscode.Disposable[] {
  const { store, sync, prompts } = deps;
  const flags = (value: ViewFlags) => async () => {
    await store.setFlags(value);
    sync.push();
  };
  const commands: Record<string, (...args: unknown[]) => unknown> = {
    'clippings.showFlatView': flags({ tagsOnly: false, flat: true }),
    'clippings.showTagsOnlyView': flags({ flat: false, tagsOnly: true }),
    'clippings.showTreeView': flags({ flat: false, tagsOnly: false }),
    'clippings.groupByTag': flags({ groupedByTag: true }),
    'clippings.ungroupByTag': flags({ groupedByTag: false }),
    'clippings.groupBySubTag': flags({ groupedBySubTag: true }),
    'clippings.ungroupBySubTag': flags({ groupedBySubTag: false }),
    'clippings.filter': async () => {
      const text = await prompts.input({ prompt: 'Filter tree' });
      if (!text) return;
      await store.setFilter(text);
      sync.push();
    },
    'clippings.filterClear': async () => {
      await store.setFilter(undefined);
      sync.push();
    },
    'clippings.resetCache': async () => {
      await store.reset();
      resetExpansion(deps, replacesTree(sync.push()));
    },
  };
  return Object.entries(commands).map(([id, run]) => vscode.commands.registerCommand(id, run));
}
