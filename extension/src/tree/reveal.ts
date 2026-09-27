// Reveal Current File In Tree and track file (spec 7.5).

import * as vscode from 'vscode';
import { schemeList } from '../config/schemes';
import type { Settings, ViewNode } from '../protocol';
import type { NodeCache } from './nodeCache';
import { dbgLog } from '../server/connection';

export interface RevealDeps {
  find(uri: string, line: number | null): Promise<ViewNode[][]>;
  cache: NodeCache;
  view: vscode.TreeView<string>;
  settings(): Settings;
  log: vscode.LogOutputChannel;
}

export const TRACK_DELAY_MS = 500;

export class Revealer implements vscode.Disposable {
  private timer: NodeJS.Timeout | undefined;

  constructor(private readonly deps: RevealDeps) {}

  /** Reveals the document's file node; false when the tree has none. */
  async reveal(uri: vscode.Uri, focus: boolean): Promise<boolean> {
    const paths = await this.deps.find(uri.toString(), null);
    const first = paths[0];
    const target = first?.at(-1);
    if (!target) return false;
    for (const path of paths) this.deps.cache.recordPath(path);
    dbgLog(`DBG reveal start ${target.id.slice(-60)}`);
    await this.deps.view.reveal(target.id, { select: true, focus, expand: false });
    dbgLog(`DBG reveal done ${target.id.slice(-60)}`);
    return true;
  }

  /** The `clippings.reveal` command: the active editor's file, if the view is visible. */
  async revealActive(): Promise<void> {
    const editor = vscode.window.activeTextEditor;
    if (editor && this.deps.view.visible) await this.reveal(editor.document.uri, false);
  }

  /** Track file: reveal the new active editor's file after a pause. */
  onActiveEditor(editor: vscode.TextEditor | undefined): void {
    clearTimeout(this.timer);
    if (!editor) return;
    const uri = editor.document.uri;
    this.timer = setTimeout(() => {
      const { tree, general } = this.deps.settings();
      if (tree.autoRefresh && tree.trackFile && schemeList(general.schemes).includes(uri.scheme) && this.deps.view.visible) {
        this.reveal(uri, false).catch((err: unknown) => this.deps.log.debug(`Track file reveal failed: ${String(err)}`));
      }
    }, TRACK_DELAY_MS);
  }

  dispose(): void {
    clearTimeout(this.timer);
  }
}
