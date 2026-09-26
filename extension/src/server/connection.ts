// The server's lifecycle (spec 7.4): resolve the binary, spawn `clippings
// lsp` through the language client, relay its notifications, and restart on
// demand or after a crash. Everything else talks to the server through this
// class.

import { spawn, type ChildProcess } from 'node:child_process';
import * as vscode from 'vscode';
import { LanguageClient, State, type LanguageClientOptions, type ServerOptions } from 'vscode-languageclient/node';
import {
  PROTOCOL_VERSION,
  type DecorationsParams,
  type Direction,
  type ExportResult,
  type Position,
  type Range,
  type Settings,
  type StatusParams,
  type StylesParams,
  type TreeChangedParams,
  type ViewNode,
} from '../protocol';
import { CrashHistory, GIVE_UP_MESSAGE } from './crashHistory';
import { CrashPolicy } from './crashPolicy';
import { locateServer } from './locate';
import * as m from './messages';
import { ResolutionError } from './resolve';

/** How long a server may take to finish starting before it counts as a crash. */
export const START_TIMEOUT_MS = 10_000;
/** How long to wait for a killed server process to exit, per signal. */
const KILL_WAIT_MS = 2000;

export interface ConnectionHost {
  readonly context: vscode.ExtensionContext;
  readonly log: vscode.LogOutputChannel;
  /** The configuration object to send, read fresh on each call. */
  settings(): Settings;
  /** The active editor's document URI, for `clippings/activeEditor`. */
  activeUri(): string | null;
  /** Shows an error with action buttons and resolves with the chosen one. */
  error(message: string, ...actions: string[]): PromiseLike<string | undefined>;
}

export class ServerConnection implements vscode.Disposable {
  private client: LanguageClient | undefined;
  /**
   * The current client's server process. Clippings spawns it itself: the
   * language client forgets its process once the connection closes, and a
   * server that closed its output can still be alive and must be killed.
   */
  private process: ChildProcess | undefined;
  private starting: Promise<void> | undefined;
  /** Crashes and start failures across every client of this connection. */
  private readonly crashes = new CrashHistory();
  /** `START_TIMEOUT_MS`; tests shorten it. */
  startTimeoutMs = START_TIMEOUT_MS;
  private readonly emitters = {
    gaveUp: new vscode.EventEmitter<string>(),
    status: new vscode.EventEmitter<StatusParams>(),
    treeChanged: new vscode.EventEmitter<TreeChangedParams>(),
    styles: new vscode.EventEmitter<StylesParams>(),
    decorations: new vscode.EventEmitter<DecorationsParams>(),
    running: new vscode.EventEmitter<void>(),
  };
  readonly onStatus = this.emitters.status.event;
  readonly onTreeChanged = this.emitters.treeChanged.event;
  readonly onStyles = this.emitters.styles.event;
  readonly onDecorations = this.emitters.decorations.event;
  /** Fires with the notice when repeated crashes stop the automatic restarts. */
  readonly onGaveUp = this.emitters.gaveUp.event;
  /** Fires each time a server, new or restarted, is ready for requests. */
  readonly onRunning = this.emitters.running.event;

  private readonly trust: vscode.Disposable;

  constructor(private readonly host: ConnectionHost) {
    // A workspace `server.path` counts only once the workspace is trusted.
    this.trust = vscode.workspace.onDidGrantWorkspaceTrust(() => {
      if (vscode.workspace.getConfiguration('clippings').inspect('server.path')?.workspaceValue) void this.restart();
    });
  }

  get running(): boolean {
    return this.client?.state === State.Running;
  }

  /** The server process ID, for tests. */
  get pid(): number | undefined {
    return this.client ? this.process?.pid : undefined;
  }

  start(): Promise<void> {
    this.starting ??= this.doStart().finally(() => (this.starting = undefined));
    return this.starting;
  }

  /** Restarts on demand: forgets past crashes, then starts a fresh client. */
  async restart(): Promise<void> {
    this.crashes.clear();
    await this.starting;
    await this.stop();
    await this.start();
  }

  async stop(): Promise<void> {
    const client = this.client;
    const child = this.process;
    this.client = undefined;
    this.process = undefined;
    if (client) {
      try {
        await client.dispose(2000);
      } catch (err) {
        this.host.log.warn(`Stopping the server failed: ${String(err)}`);
      }
    }
    await killProcess(child);
  }

  /**
   * Starts a client, and starts a fresh one after each start failure until
   * the crash limit. Settles once a server runs, resolution fails, or the
   * limit is reached, so `restart` never waits on a server that died.
   */
  private async doStart(): Promise<void> {
    while ((await this.startOnce()) === 'failed') {
      if (this.crashes.record(Date.now()) === 'give up') {
        void this.showCrash(GIVE_UP_MESSAGE);
        return;
      }
    }
  }

  private async startOnce(): Promise<'started' | 'failed' | 'error'> {
    const { log, context } = this.host;
    let serverPath: string;
    try {
      serverPath = (await locateServer(context, (msg) => log.info(msg))).candidate.path;
    } catch (err) {
      const message = err instanceof Error ? err.message : String(err);
      log.error(message);
      void this.showError(message, err instanceof ResolutionError);
      return 'error';
    }
    const settings = this.host.settings();
    const env = {
      ...process.env,
      RUST_BACKTRACE: '1',
      CLIPPINGS_LOG: vscode.workspace.getConfiguration('clippings').get<string>('server.logLevel', 'info'),
    };
    // Called for the first start and for each restart after a crash.
    let child: ChildProcess | undefined;
    const serverOptions: ServerOptions = () => {
      child = spawn(serverPath, ['lsp'], { env, windowsHide: true });
      if (this.client === client) this.process = child;
      return Promise.resolve(child);
    };
    // Set while `startOnce` waits for this client; a start failure after that
    // comes from the client restarting a crashed server.
    let waiting: ((outcome: 'failed') => void) | undefined;
    const failed = new Promise<'failed'>((resolve) => (waiting = resolve));
    // Each start, first or after a crash, must reach the running state in time.
    let watchdog: NodeJS.Timeout | undefined;
    let abandoned = false;
    const startFailed = (reason: string) => {
      if (abandoned) return;
      abandoned = true;
      clearTimeout(watchdog);
      log.error(reason);
      if (waiting) waiting('failed');
      else void this.afterStartFailure(client, child);
    };
    const policy = new CrashPolicy(this.crashes, {
      startFailed: () => startFailed('The server exited while starting.'),
      gaveUp: (message) => void this.showCrash(message),
    });
    const clientOptions: LanguageClientOptions = {
      documentSelector: settings.general.schemes.map((scheme) => ({ scheme })),
      initializationOptions: () => ({ protocolVersion: PROTOCOL_VERSION, settings: this.host.settings() }),
      outputChannel: log,
      errorHandler: policy,
    };
    const client = new LanguageClient('clippings', 'Clippings', serverOptions, clientOptions);
    client.onNotification(m.Status, (p) => this.emitters.status.fire(p));
    client.onNotification(m.TreeChanged, (p) => this.emitters.treeChanged.fire(p));
    client.onNotification(m.Styles, (p) => this.emitters.styles.fire(p));
    client.onNotification(m.Decorations, (p) => this.emitters.decorations.fire(p));
    client.onDidChangeState((e) => {
      clearTimeout(watchdog);
      if (e.newState === State.Starting) {
        const seconds = this.startTimeoutMs / 1000;
        watchdog = setTimeout(
          () => startFailed(`The server did not finish starting within ${seconds} s.`),
          this.startTimeoutMs,
        );
      }
      policy.setRunning(e.newState === State.Running);
      if (e.newState === State.Running && this.client === client) this.onClientRunning(client);
    });
    this.client = client;
    // When the server exits or hangs during `initialize`, `start()` may never
    // settle; a start failure settles the race instead.
    const started = client.start().then(() => 'started' as const);
    started.catch(() => undefined);
    try {
      const outcome = await Promise.race([started, failed]);
      if (outcome === 'failed') await this.discard(client, child);
      return outcome;
    } catch (err) {
      // `initialize` failed or the connection closed under it: a start failure.
      abandoned = true;
      clearTimeout(watchdog);
      log.error(`The server failed to start: ${String(err)}`);
      await this.discard(client, child);
      return 'failed';
    } finally {
      waiting = undefined;
    }
  }

  /** A crashed server that the client restarted, then failed to start. */
  private async afterStartFailure(client: LanguageClient, child: ChildProcess | undefined): Promise<void> {
    await this.discard(client, child);
    if (this.crashes.record(Date.now()) === 'give up') void this.showCrash(GIVE_UP_MESSAGE);
    else void this.start();
  }

  /** Forgets a client whose server failed; it cannot be started again. */
  private async discard(client: LanguageClient, child: ChildProcess | undefined): Promise<void> {
    if (this.client === client) {
      this.client = undefined;
      this.process = undefined;
    }
    await killProcess(child);
    try {
      await client.dispose(2000);
    } catch {
      // A client that never ran cannot be stopped; there is nothing to stop.
    }
  }

  private onClientRunning(client: LanguageClient): void {
    // Settings may have changed between `initialize` and now.
    void client.sendNotification(m.Configure, this.host.settings());
    void client.sendNotification(m.ActiveEditor, { uri: this.host.activeUri() });
    this.emitters.running.fire();
  }

  private async showError(message: string, offerSetting: boolean): Promise<void> {
    const actions = offerSetting ? ['Show Log', 'Open Setting'] : ['Show Log'];
    const choice = await this.host.error(message, ...actions);
    if (choice === 'Show Log') this.host.log.show(true);
    if (choice === 'Open Setting') {
      await vscode.commands.executeCommand('workbench.action.openSettings', 'clippings.server.path');
    }
  }

  private async showCrash(message: string): Promise<void> {
    this.host.log.error(message);
    this.emitters.gaveUp.fire(message);
    const choice = await this.host.error(message, 'Restart', 'Show Log');
    if (choice === 'Restart') await this.restart();
    if (choice === 'Show Log') this.host.log.show(true);
  }

  // ---- client to server ----

  configure(settings: Settings): void {
    if (this.running) void this.client?.sendNotification(m.Configure, settings);
  }

  activeEditor(uri: string | null): void {
    if (this.running) void this.client?.sendNotification(m.ActiveEditor, { uri });
  }

  rescan(): void {
    if (this.running) void this.client?.sendNotification(m.Rescan, {});
  }

  stopScan(): void {
    if (this.running) void this.client?.sendNotification(m.StopScan, {});
  }

  async children(parent: string | null): Promise<ViewNode[]> {
    if (!this.running || !this.client) return [];
    return (await this.client.sendRequest(m.Children, { parent })).nodes;
  }

  async find(uri: string, line: number | null): Promise<ViewNode[][]> {
    if (!this.running || !this.client) return [];
    return (await this.client.sendRequest(m.Find, { uri, line })).paths;
  }

  async navigate(uri: string, positions: Position[], direction: Direction): Promise<Range[] | null> {
    if (!this.running || !this.client) return null;
    return (await this.client.sendRequest(m.Navigate, { uri, positions, direction })).ranges;
  }

  async export(): Promise<ExportResult | undefined> {
    if (!this.running || !this.client) return undefined;
    return this.client.sendRequest(m.Export, {});
  }

  dispose(): void {
    this.trust.dispose();
    void this.stop();
    for (const e of Object.values(this.emitters)) e.dispose();
  }
}

/** Ends a server process that did not exit by itself: SIGTERM, then SIGKILL. */
async function killProcess(child: ChildProcess | undefined): Promise<void> {
  if (!child) return;
  const exited = () => child.exitCode !== null || child.signalCode !== null;
  for (const signal of ['SIGTERM', 'SIGKILL'] as const) {
    if (exited()) return;
    const exit = new Promise<void>((resolve) => child.once('exit', () => resolve()));
    child.kill(signal);
    let timer: NodeJS.Timeout | undefined;
    await Promise.race([exit, new Promise<void>((resolve) => (timer = setTimeout(resolve, KILL_WAIT_MS)))]);
    clearTimeout(timer);
  }
}
