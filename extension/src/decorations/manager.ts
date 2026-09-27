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
  private readonly renderOptions = new Map<string, vscode.DecorationRenderOptions>();
  /** The last applied decorations per document URI. */
  private readonly cache = new Map<string, DecorationsParams>();
  /**
   * Keys applied per editor, so a later message can clear the ones a given
   * editor no longer has (spec 7.7). Per editor, not per document: a split
   * view has one `TextEditor` per column, each needing its own clear diff -
   * a shared per-document set would only clear stale keys on the first
   * editor processed and leave the others stale. A `WeakMap` needs no
   * cleanup when an editor closes.
   */
  private applied = new WeakMap<vscode.TextEditor, Set<string>>();
  /** Style keys already warned about once (spec 7.7: an unknown key's ranges are ignored). */
  private readonly warnedUnknownKeys = new Set<string>();
  private readonly appliedEmitter = new vscode.EventEmitter<{ uri: string; source: ApplySource }>();
  /** Fires after decorations are applied to a document's editors. */
  readonly onApplied = this.appliedEmitter.event;

  constructor(
    private readonly icons: IconResolver,
    private readonly log: vscode.LogOutputChannel,
  ) {}

  get currentGeneration(): number | undefined {
    return this.generation;
  }

  get styleKeys(): string[] {
    return [...this.types.keys()];
  }

  /** The options a key's decoration type was created with, for the tests. */
  options(key: string): vscode.DecorationRenderOptions | undefined {
    return this.renderOptions.get(key);
  }

  entry(uri: string): DecorationsParams | undefined {
    return this.cache.get(uri);
  }

  /** The style keys last set on a specific editor (spec 12.5 test hook). */
  appliedKeys(editor: vscode.TextEditor): string[] | undefined {
    const keys = this.applied.get(editor);
    return keys ? [...keys] : undefined;
  }

  /** Creates each decoration type before returning, so later decorations find it. */
  onStyles(msg: StylesParams): void {
    const action = stylesAction(this.generation, msg);
    if (action === 'drop') return;
    if (action === 'reset') {
      this.disposeTypes();
      this.applied = new WeakMap();
      this.cache.clear();
      this.warnedUnknownKeys.clear();
      this.generation = msg.generation;
    }
    const replaced = new Set<string>();
    for (const [key, style] of Object.entries(msg.styles)) {
      if (this.types.has(key)) replaced.add(key);
      this.types.get(key)?.dispose();
      const options = renderOptions(style, {
        themeColor: (id) => new vscode.ThemeColor(id),
        gutterIcon: (s) => (s.gutterIcon ? this.icons.gutterIcon(s.gutterIcon) : undefined),
      });
      this.types.set(key, vscode.window.createTextEditorDecorationType(options));
      this.renderOptions.set(key, options);
    }
    // A key redefined within the current generation gets a fresh decoration
    // type; the old one's disposal cleared its highlights, so reapply the
    // cached ranges under the new type before anyone notices the gap.
    if (action === 'add') this.reapplyKeys(replaced);
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
    this.cache.delete(document.uri.toString());
    // `applied` is keyed by editor, not URI: its entries for this document's
    // editors fall out of the WeakMap on their own once nothing else holds
    // those `TextEditor` objects.
  }

  /** A new server instance: forget every type, generation and cached entry. */
  reset(): void {
    this.disposeTypes();
    this.generation = undefined;
    this.cache.clear();
    this.applied = new WeakMap();
    this.warnedUnknownKeys.clear();
  }

  /** Reapplies the cached ranges for `keys` to every editor showing them. */
  private reapplyKeys(keys: Set<string>): void {
    if (keys.size === 0) return;
    for (const [uri, entry] of this.cache) {
      if (![...keys].some((key) => key in entry.ranges)) continue;
      const editors = vscode.window.visibleTextEditors.filter((e) => e.document.uri.toString() === uri);
      for (const editor of editors) {
        for (const key of keys) {
          const type = this.types.get(key);
          if (type) editor.setDecorations(type, (entry.ranges[key] ?? []).map(toRange));
        }
      }
    }
  }

  private apply(editor: vscode.TextEditor, msg: DecorationsParams): void {
    const previous = this.applied.get(editor) ?? new Set<string>();
    for (const key of keysToSet(previous, msg.ranges)) {
      const type = this.types.get(key);
      if (type) {
        editor.setDecorations(type, (msg.ranges[key] ?? []).map(toRange));
      } else if (!this.warnedUnknownKeys.has(key)) {
        this.warnedUnknownKeys.add(key);
        this.log.warn(`No decoration style for tag "${key}"; its ranges are ignored.`);
      }
    }
    this.applied.set(editor, new Set(Object.keys(msg.ranges)));
  }

  private disposeTypes(): void {
    for (const type of this.types.values()) type.dispose();
    this.types.clear();
    this.renderOptions.clear();
  }

  dispose(): void {
    this.disposeTypes();
    this.appliedEmitter.dispose();
  }
}
