// Expand Tree and Collapse Tree (spec 7.5).

import * as vscode from 'vscode';
import { replacesTree, type ConfigurationSync } from '../config/sync';
import type { ViewStateStore } from '../state/viewState';
import type { Expansion } from '../tree/expansion';
import type { TreeProvider } from '../tree/provider';

export interface ExpandDeps {
  store: ViewStateStore;
  sync: ConfigurationSync;
  expansion: Expansion;
  provider: TreeProvider;
}

/** Resets expansion after a push: at the server's root refresh if one is coming. */
export function resetExpansion(deps: Pick<ExpandDeps, 'expansion' | 'provider'>, replaced: boolean): void {
  deps.expansion.reset(replaced);
  if (!replaced) deps.provider.refreshAll();
}

export function registerExpandCommands(deps: ExpandDeps): vscode.Disposable[] {
  const setExpanded = async (expanded: boolean) => {
    await deps.store.setFlags({ expanded });
    resetExpansion(deps, replacesTree(deps.sync.push()));
  };
  return [
    vscode.commands.registerCommand('clippings.expand', () => setExpanded(true)),
    vscode.commands.registerCommand('clippings.collapse', () => setExpanded(false)),
  ];
}
