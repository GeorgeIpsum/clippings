// Composition root: builds each part of the extension and wires them to the
// server connection.

import * as vscode from 'vscode';
import { needsRestart, registerServerCommands } from './commands/server';
import { affectsServer, readConfiguration } from './config/read';
import type { StatusParams } from './protocol';
import { ServerConnection } from './server/connection';
import { ViewStateStore } from './state/viewState';
import type { ClippingsApi } from './testApi';

let connection: ServerConnection | undefined;

export function activate(context: vscode.ExtensionContext): ClippingsApi {
  const log = vscode.window.createOutputChannel('Clippings', { log: true });
  const store = new ViewStateStore(context.workspaceState);
  const settings = () => readConfiguration(store.snapshot());
  const activeUri = () => vscode.window.activeTextEditor?.document.uri.toString() ?? null;
  connection = new ServerConnection({
    context,
    log,
    settings,
    activeUri,
    error: (message, ...actions) => vscode.window.showErrorMessage(message, ...actions),
  });
  const server = connection;
  let lastStatus: StatusParams | undefined;

  context.subscriptions.push(
    log,
    server,
    ...registerServerCommands(server, log),
    server.onStatus((s) => (lastStatus = s)),
    vscode.workspace.onDidChangeConfiguration((e) => {
      if (needsRestart(e)) void server.restart();
      else if (affectsServer(e)) server.configure(settings());
    }),
    vscode.window.onDidChangeActiveTextEditor(() => server.activeEditor(activeUri())),
  );
  void server.start();

  const manifest = context.extension.packageJSON as { version: string };
  return {
    version: manifest.version,
    test: {
      server: {
        get running() {
          return server.running;
        },
        get pid() {
          return server.pid;
        },
        onStatus: server.onStatus,
        onRunning: server.onRunning,
        onGaveUp: server.onGaveUp,
        setStartTimeout: (ms) => (server.startTimeoutMs = ms),
        status: () => lastStatus,
      },
    },
  };
}

export async function deactivate(): Promise<void> {
  await connection?.stop();
  connection = undefined;
}
