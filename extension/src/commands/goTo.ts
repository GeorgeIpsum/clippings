// Go To Next and Go To Previous (spec 5.15): the server searches the open
// buffer with the same matcher as the scan.

import * as vscode from 'vscode';
import type { Direction } from '../protocol';
import type { ServerConnection } from '../server/connection';

async function goTo(server: ServerConnection, direction: Direction): Promise<void> {
  const editor = vscode.window.activeTextEditor;
  if (!editor) return;
  const positions = editor.selections.map((s) => ({ line: s.start.line, character: s.start.character }));
  const ranges = await server.navigate(editor.document.uri.toString(), positions, direction);
  if (!ranges || ranges.length === 0) return;
  editor.selections = ranges.map((r) => {
    const at = new vscode.Position(r.start.line, r.start.character);
    return new vscode.Selection(at, at);
  });
  const first = editor.selections[0];
  if (first) editor.revealRange(new vscode.Range(first.start, first.start));
}

export function registerGoToCommands(server: ServerConnection): vscode.Disposable[] {
  return [
    vscode.commands.registerCommand('clippings.goToNext', () => goTo(server, 'next')),
    vscode.commands.registerCommand('clippings.goToPrevious', () => goTo(server, 'previous')),
  ];
}
