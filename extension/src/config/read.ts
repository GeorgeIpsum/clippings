// Reads the configuration sources from VS Code at window level (spec 6.3).

import * as vscode from 'vscode';
import type { Settings } from '../protocol';
import type { PersistedViewState } from '../state/viewState';
import { buildConfiguration, GROUPS, type ConfigurationSources, type Group } from './configuration';

export function readSources(viewState: PersistedViewState): ConfigurationSources {
  const clippings = vscode.workspace.getConfiguration('clippings');
  const groups = Object.fromEntries(GROUPS.map((g) => [g, clippings.get(g)])) as Record<Group, unknown>;
  return {
    groups,
    filesExclude: vscode.workspace.getConfiguration('files').get('exclude'),
    searchExclude: vscode.workspace.getConfiguration('search').get('exclude'),
    explorerCompactFolders: vscode.workspace.getConfiguration('explorer').get('compactFolders'),
    viewState,
  };
}

export function readConfiguration(viewState: PersistedViewState): Settings {
  return buildConfiguration(readSources(viewState));
}

/** Whether a configuration change can change the object the server receives. */
export function affectsServer(e: vscode.ConfigurationChangeEvent): boolean {
  return ['clippings', 'files.exclude', 'search.exclude', 'explorer.compactFolders'].some((s) =>
    e.affectsConfiguration(s),
  );
}
