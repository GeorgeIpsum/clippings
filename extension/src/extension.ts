// Composition root: builds each part of the extension and wires them to the
// server connection.

import * as vscode from 'vscode';
import { registerExpandCommands, resetExpansion } from './commands/expand';
import { needsRestart, registerServerCommands } from './commands/server';
import { affectsServer, readConfiguration } from './config/read';
import { ConfigurationSync, replacesTree } from './config/sync';
import { IconResolver } from './icons/resolver';
import type { StatusParams } from './protocol';
import { ServerConnection } from './server/connection';
import { ViewStateStore } from './state/viewState';
import type { ClippingsApi } from './testApi';
import { Expansion } from './tree/expansion';
import { NodeCache } from './tree/nodeCache';
import { TreeProvider } from './tree/provider';
import { testItem } from './tree/testItems';

let connection: ServerConnection | undefined;

export function activate(context: vscode.ExtensionContext): ClippingsApi {
  const log = vscode.window.createOutputChannel('Clippings', { log: true });
  const store = new ViewStateStore(context.workspaceState);
  const activeUri = () => vscode.window.activeTextEditor?.document.uri.toString() ?? null;
  const server = new ServerConnection({
    context,
    log,
    settings: () => sync.current,
    activeUri,
    error: (message, ...actions) => vscode.window.showErrorMessage(message, ...actions),
  });
  connection = server;
  const sync = new ConfigurationSync(
    () => readConfiguration(store.snapshot()),
    (settings) => server.configure(settings),
  );
  let lastStatus: StatusParams | undefined;

  const cache = new NodeCache();
  const expansion = new Expansion(store);
  const provider = new TreeProvider(server, cache, {
    icons: new IconResolver(),
    itemId: (id) => expansion.itemId(id),
    expanded: (node) => expansion.expanded(node),
  });
  const treeView = vscode.window.createTreeView('clippings-view', { treeDataProvider: provider });

  context.subscriptions.push(
    log,
    server,
    provider,
    treeView,
    ...registerServerCommands(server, log),
    ...registerExpandCommands({ store, sync, expansion, provider }),
    server.onStatus((s) => (lastStatus = s)),
    server.onNewInstance(() => provider.reset()),
    server.onTreeChanged((p) => {
      if (p.refresh.includes(null)) expansion.onRootRefresh();
      provider.refresh(p.refresh);
    }),
    treeView.onDidExpandElement((e) => expansion.set(e.element, true)),
    treeView.onDidCollapseElement((e) => expansion.set(e.element, false)),
    vscode.workspace.onDidChangeConfiguration((e) => {
      if (needsRestart(e)) {
        sync.push();
        void server.restart();
        return;
      }
      if (!affectsServer(e)) return;
      const push = sync.push();
      if (e.affectsConfiguration('clippings.tree.expanded')) {
        resetExpansion({ expansion, provider }, replacesTree(push));
      }
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
      tree: {
        view: treeView,
        onDidChange: provider.onDidChangeTreeData,
        items: async (parent) => {
          const ids = await provider.getChildren(parent);
          return ids.map((id) => testItem(id, provider.getTreeItem(id)));
        },
        get epoch() {
          return expansion.currentEpoch;
        },
      },
    },
  };
}

export async function deactivate(): Promise<void> {
  await connection?.stop();
  connection = undefined;
}
