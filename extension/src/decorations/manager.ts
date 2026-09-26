// Applies `clippings/styles` and `clippings/decorations` (spec 6.2, 7.7):
// decoration types per key and generation, the last decorations per
// document, and reapplication when an editor becomes visible.

import * as vscode from 'vscode';
import type { DecorationsParams, Range, StylesParams } from '../protocol';
import type { IconResolver } from '../icons/resolver';
import { renderOptions } from './renderOptions';
import { acceptDecorations, keysToSet, reusable, stylesAction } from './rules';

export type ApplySource = 'message' | 'cache';

function toRange(r: Range): vscode.Range {
  return new vscode.Range(r.start.line, r.start.character, r.end.line, r.end.character);
}

export class DecorationManager implements vscode.Disposable {
  private generation: number | undefined;
  private readonly types = new Map<string, vscode.TextEditorDecorationType>();
  /** The last applied decorations per document URI. */
  private readonly cache = new Map<string, DecorationsParams>();
  /** Keys applied per document, so a later message can clear the absent ones. */
  private readonly applied = new Map<string, Set<string>>();
  private readonly appliedEmitter = new vscode.EventEmitter<{ uri: string; source: ApplySource }>();
  /** Fires after decorations are applied to a document's editors. */
  readonly onApplied = this.appliedEmitter.event;

  constructor(private readonly icons: IconResolver) {}

  get currentGeneration(): number | undefined {
    return this.generation;
  }

  get styleKeys(): string[] {
    return [...this.types.keys()];
  }

  entry(uri: string): DecorationsParams | undefined {
    return this.cache.get(uri);
  }

  /** Creates each decoration type before returning, so later decorations find it. */
  onStyles(msg: StylesParams): void {
    const action = stylesAction(this.generation, msg);
    if (action === 'drop') return;
    if (action === 'reset') {
      this.disposeTypes();
      this.applied.clear();
      this.cache.clear();
      this.generation = msg.generation;
    }
    for (const [key, style] of Object.entries(msg.styles)) {
      this.types.get(key)?.dispose();
      const options = renderOptions(style, {
        themeColor: (id) => new vscode.ThemeColor(id),
        gutterIcon: (s) => (s.gutterIcon ? this.icons.gutterIcon(s.gutterIcon) : undefined),
      });
      this.types.set(key, vscode.window.createTextEditorDecorationType(options));
    }
  }

  onDecorations(msg: DecorationsParams): void {
    const document = vscode.workspace.textDocuments.find((d) => d.uri.toString() === msg.uri);
    if (!acceptDecorations(this.generation, msg, document?.version)) return;
    this.cache.set(msg.uri, msg);
    const editors = vscode.window.visibleTextEditors.filter((e) => e.document.uri.toString() === msg.uri);
    for (const editor of editors) this.apply(editor, msg);
    this.appliedEmitter.fire({ uri: msg.uri, source: 'message' });
  }

  /** Reapplies cached decorations to editors that became visible. */
  onVisibleEditors(editors: readonly vscode.TextEditor[]): void {
    for (const editor of editors) {
      const uri = editor.document.uri.toString();
      const entry = this.cache.get(uri);
      if (!entry || !reusable(this.generation, entry, editor.document.version)) continue;
      this.apply(editor, entry);
      this.appliedEmitter.fire({ uri, source: 'cache' });
    }
  }

  onDocumentClosed(document: vscode.TextDocument): void {
    const uri = document.uri.toString();
    this.cache.delete(uri);
    this.applied.delete(uri);
  }

  /** A new server instance: forget every type, generation and cached entry. */
  reset(): void {
    this.disposeTypes();
    this.generation = undefined;
    this.cache.clear();
    this.applied.clear();
  }

  private apply(editor: vscode.TextEditor, msg: DecorationsParams): void {
    const previous = this.applied.get(msg.uri) ?? new Set<string>();
    for (const key of keysToSet(previous, msg.ranges)) {
      const type = this.types.get(key);
      if (type) editor.setDecorations(type, (msg.ranges[key] ?? []).map(toRange));
    }
    this.applied.set(msg.uri, new Set(Object.keys(msg.ranges)));
  }

  private disposeTypes(): void {
    for (const type of this.types.values()) type.dispose();
    this.types.clear();
  }

  dispose(): void {
    this.disposeTypes();
    this.appliedEmitter.dispose();
  }
}
