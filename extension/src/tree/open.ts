// Todo clicks (spec 7.5): open the document at the node's position and
// flash the line for 150 ms with one reused decoration type.

import * as vscode from 'vscode';
import type { Position } from '../protocol';

export const FLASH_MS = 150;

export class LineFlash implements vscode.Disposable {
  private readonly type = vscode.window.createTextEditorDecorationType({
    isWholeLine: true,
    backgroundColor: new vscode.ThemeColor('editor.rangeHighlightBackground'),
  });
  private timer: NodeJS.Timeout | undefined;
  /** The last flashed line, for the tests. */
  last: { uri: string; line: number } | undefined;

  flash(editor: vscode.TextEditor): void {
    const line = editor.selection.active.line;
    clearTimeout(this.timer);
    editor.setDecorations(this.type, [editor.document.lineAt(line).range]);
    this.last = { uri: editor.document.uri.toString(), line };
    this.timer = setTimeout(() => editor.setDecorations(this.type, []), FLASH_MS);
  }

  dispose(): void {
    clearTimeout(this.timer);
    this.type.dispose();
  }
}

export async function revealInFile(flash: LineFlash, uri: unknown, position: unknown): Promise<void> {
  const target = typeof uri === 'string' ? vscode.Uri.parse(uri) : uri instanceof vscode.Uri ? uri : undefined;
  if (!target) return;
  const p = position as Partial<Position> | undefined;
  const options: vscode.TextDocumentShowOptions = {};
  if (typeof p?.line === 'number' && typeof p.character === 'number') {
    const at = new vscode.Position(p.line, p.character);
    options.selection = new vscode.Range(at, at);
  }
  await vscode.commands.executeCommand('vscode.open', target, options);
  const editor = vscode.window.activeTextEditor;
  if (editor && editor.document.uri.toString() === target.toString()) flash.flash(editor);
}
