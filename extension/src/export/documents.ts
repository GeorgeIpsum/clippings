// Export Tree (spec 5.15, 7.6): the server's export shown as a read-only
// virtual document under the `clippings-export` scheme, named by the
// formatted export path.

import * as vscode from 'vscode';
import type { ServerConnection } from '../server/connection';

export const EXPORT_SCHEME = 'clippings-export';

/** The virtual document URI for an export path. */
export function exportUri(path: string): vscode.Uri {
  const slashed = path.replaceAll('\\', '/');
  return vscode.Uri.from({ scheme: EXPORT_SCHEME, path: slashed.startsWith('/') ? slashed : `/${slashed}` });
}

export class ExportDocuments implements vscode.TextDocumentContentProvider, vscode.Disposable {
  private readonly contents = new Map<string, string>();
  private readonly changed = new vscode.EventEmitter<vscode.Uri>();
  readonly onDidChange = this.changed.event;

  provideTextDocumentContent(uri: vscode.Uri): string {
    return this.contents.get(uri.toString()) ?? '';
  }

  set(uri: vscode.Uri, content: string): void {
    this.contents.set(uri.toString(), content);
    this.changed.fire(uri);
  }

  dispose(): void {
    this.changed.dispose();
  }
}

export function registerExport(server: ServerConnection): vscode.Disposable[] {
  const documents = new ExportDocuments();
  return [
    documents,
    vscode.workspace.registerTextDocumentContentProvider(EXPORT_SCHEME, documents),
    vscode.commands.registerCommand('clippings.exportTree', async () => {
      const result = await server.export();
      if (!result) return;
      const uri = exportUri(result.path);
      documents.set(uri, result.content);
      const document = await vscode.workspace.openTextDocument(uri);
      await vscode.window.showTextDocument(document, { preview: true });
    }),
  ];
}
