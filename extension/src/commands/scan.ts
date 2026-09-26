// Refresh and stop scan (spec 5.9, 7.7).

import * as vscode from 'vscode';
import type { ServerConnection } from '../server/connection';

export function registerScanCommands(server: ServerConnection): vscode.Disposable[] {
  return [
    vscode.commands.registerCommand('clippings.refresh', () => server.rescan()),
    vscode.commands.registerCommand('clippings.stopScan', () => server.stopScan()),
  ];
}

export const NEEDS_SCAN_MESSAGE = 'Click the refresh button to scan...';
