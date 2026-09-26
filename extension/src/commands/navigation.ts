// Commands that open things: reveal in the tree, reveal in the file, open a
// sub-tag URL (spec 7.5, 7.6).

import * as vscode from 'vscode';
import type { Revealer } from '../tree/reveal';
import { revealInFile, type LineFlash } from '../tree/open';

export function registerNavigationCommands(revealer: Revealer, flash: LineFlash): vscode.Disposable[] {
  return [
    vscode.commands.registerCommand('clippings.reveal', () => revealer.revealActive()),
    vscode.commands.registerCommand('clippings.revealInFile', (uri: unknown, position: unknown) =>
      revealInFile(flash, uri, position),
    ),
    vscode.commands.registerCommand('clippings.openUrl', (url: unknown) =>
      typeof url === 'string' ? vscode.env.openExternal(vscode.Uri.parse(url)) : false,
    ),
  ];
}
