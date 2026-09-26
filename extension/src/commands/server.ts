// Server commands (spec 7.4, 10.3): restart on demand and show the log.

import * as vscode from 'vscode';
import type { ServerConnection } from '../server/connection';

export function registerServerCommands(connection: ServerConnection, log: vscode.LogOutputChannel): vscode.Disposable[] {
  return [
    vscode.commands.registerCommand('clippings.restartServer', () => connection.restart()),
    vscode.commands.registerCommand('clippings.showLog', () => log.show(true)),
  ];
}

/** Settings fixed when the client starts: changing one restarts it (spec 7.4). */
const RESTART_SETTINGS = [
  'clippings.general.schemes',
  'clippings.server.path',
  'clippings.server.logLevel',
  'clippings.trace.server',
];

export function needsRestart(e: vscode.ConfigurationChangeEvent): boolean {
  return RESTART_SETTINGS.some((s) => e.affectsConfiguration(s));
}
