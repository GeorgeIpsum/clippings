import type * as vscode from 'vscode';

/** What `activate` returns: the extension's API, used by the tests. */
export interface ClippingsApi {
  readonly version: string;
}

export function activate(context: vscode.ExtensionContext): ClippingsApi {
  const manifest = context.extension.packageJSON as { version: string };
  return { version: manifest.version };
}

export function deactivate(): void {}
