// Composition root: builds each part of the extension and wires them to the
// server connection.

import * as vscode from 'vscode';
import { registerExpandCommands, resetExpansion } from './commands/expand';
import { registerFilterCommands } from './commands/filters';
import { registerGoToCommands } from './commands/goTo';
import { registerNavigationCommands } from './commands/navigation';
import { registerScanCommands } from './commands/scan';
import { needsRestart, registerServerCommands } from './commands/server';
import { registerSettingCommands } from './commands/settings';
import { registerStatusBarCommand } from './commands/statusBar';
import { registerViewCommands } from './commands/view';
import { affectsServer, readConfiguration } from './config/read';
import { ConfigurationSync, replacesTree } from './config/sync';
import { ContextKeys } from './context/apply';
import { contextValues } from './context/keys';
import { SettingWriter } from './config/writes';
import { IconResolver } from './icons/resolver';
import type { StatusParams } from './protocol';
import { ServerConnection } from './server/connection';
import { ViewStateStore } from './state/viewState';
import { StatusController } from './status/controller';
import type { ClippingsApi } from './testApi';
import { Expansion } from './tree/expansion';
import { NodeCache } from './tree/nodeCache';
import { LineFlash } from './tree/open';
import { TreeProvider } from './tree/provider';
import { Revealer } from './tree/reveal';
import { Prompts } from './ui/prompts';
import { testItem } from './tree/testItems';

let connection: ServerConnection | undefined;

export function activate(context: vscode.ExtensionContext): ClippingsApi {
  const log = vscode.window.createOutputChannel('Clippings', { log: true });
  const store = new ViewStateStore(context.workspaceState);
  const prompts = new Prompts();
  const activeUri = () => vscode.window.activeTextEditor?.document.uri.toString() ?? null;
  const server = new ServerConnection({
    context,
    log,
    settings: () => sync.current,
    activeUri,
    error: (message, ...actions) => prompts.message('error', message, ...actions),
  });
  connection = server;
  const sync = new ConfigurationSync(
    () => readConfiguration(store.snapshot()),
    (settings) => server.configure(settings),
  );
  let lastStatus: StatusParams | undefined;
  const contextKeys = new ContextKeys();
  const updateContext = () => void contextKeys.apply(contextValues(sync.current, lastStatus));
  sync.onPush(updateContext);
  updateContext();

  const cache = new NodeCache();
  const expansion = new Expansion(store);
  const provider = new TreeProvider(server, cache, {
    icons: new IconResolver(),
    itemId: (id) => expansion.itemId(id),
    expanded: (node) => expansion.expanded(node),
  });
  const treeView = vscode.window.createTreeView('clippings-view', { treeDataProvider: provider });
  const revealer = new Revealer({
    find: (uri, line) => server.find(uri, line),
    cache,
    view: treeView,
    settings: () => sync.current,
  });
  const flash = new LineFlash();
  const writer = new SettingWriter(prompts);
  const statusController = new StatusController(treeView, prompts);

  context.subscriptions.push(
    log,
    server,
    provider,
    treeView,
    ...registerServerCommands(server, log),
    revealer,
    flash,
    ...registerExpandCommands({ store, sync, expansion, provider }),
    ...registerNavigationCommands(revealer, flash),
    ...registerViewCommands({ store, sync, expansion, provider, prompts }),
    ...registerFilterCommands({ store, sync, cache, prompts }),
    ...registerSettingCommands(writer, prompts),
    ...registerGoToCommands(server),
    statusController,
    registerStatusBarCommand({ sync, writer, prompts, view: treeView }),
    ...registerScanCommands(server),
    server.onStatus((s) => {
      lastStatus = s;
      updateContext();
      statusController.update(s);
    }),
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
    vscode.window.onDidChangeActiveTextEditor((editor) => {
      server.activeEditor(activeUri());
      revealer.onActiveEditor(editor);
    }),
  );
  void server.start();

  const manifest = context.extension.packageJSON as { version: string };
  return {
    version: manifest.version,
    test: {
      prompts,
      viewState: () => store.snapshot(),
      contextKeys: () => contextKeys.values,
      statusBar: () => statusController.shown,
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
        lastFlash: () => flash.last,
      },
    },
  };
}

export async function deactivate(): Promise<void> {
  await connection?.stop();
  connection = undefined;
}
