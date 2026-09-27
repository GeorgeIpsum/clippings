// The server's lifecycle (spec 7.4): resolve the binary, spawn `clippings
// lsp` through the language client, relay its notifications, and restart on
// demand or after a crash. Everything else talks to the server through this
// class.

import { spawn, type ChildProcess } from 'node:child_process';
import * as vscode from 'vscode';
import {
  LanguageClient,
  State,
  type LanguageClientOptions,
  type ServerOptions,
} from 'vscode-languageclient/node';
import { documentSelector } from '../config/schemes';
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

/**
 * The language client, with its notices routed to the log: Clippings shows
 * its own notice when a server fails to start or keeps crashing.
 */
class Client extends LanguageClient {
  override error(message: string, data?: unknown, _showNotification?: boolean | 'force'): void {
    super.error(message, data, false);
  }

  /**
   * A client that never finished starting has nothing to stop. The library
   * throws instead, even from its own unawaited `stop` after a failed
   * `initialize`; Clippings ends the server process itself.
   */
  override stop(timeout?: number): Promise<void> {
    if (this.state === State.Starting || this.state === State.StartFailed) return Promise.resolve();
    return super.stop(timeout);
  }
}

/** One language client and the server processes it spawns. */
interface Session {
  readonly client: LanguageClient;
  readonly policy: CrashPolicy;
  /** The latest server process: the client spawns a new one after each crash. */
  child: ChildProcess | undefined;
  /** Set once the connection lets go of this session; it may not spawn or report again. */
  abandoned: boolean;
  /** Settles once the client is disposed and its server process is gone. */
  retired?: Promise<void>;
}

export class ServerConnection implements vscode.Disposable {
  /** The current session: the only one that spawns servers and relays their messages. */
  private session: Session | undefined;
  /**
   * Every start, stop and restart runs on this chain, one at a time, so only
   * one of them ever creates or ends a session.
   */
  private lifecycle: Promise<void> = Promise.resolve();
  /** Bumped by each stop and restart; work queued under an older one is skipped. */
  private generation = 0;
  /** Aborts when `generation` is bumped, so a pending start gives up at once. */
  private cancellation = new AbortController();
  /** Sessions being retired; `stop` settles once they are gone. */
  private readonly retiring = new Set<Promise<void>>();
  private disposed = false;
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
    instance: new vscode.EventEmitter<string>(),
  };
  private instance: string | undefined;
  readonly onStatus = this.emitters.status.event;
  readonly onTreeChanged = this.emitters.treeChanged.event;
  readonly onStyles = this.emitters.styles.event;
  readonly onDecorations = this.emitters.decorations.event;
  /** Fires with the notice when repeated crashes stop the automatic restarts. */
  readonly onGaveUp = this.emitters.gaveUp.event;
  /** Fires each time a server, new or restarted, is ready for requests. */
  readonly onRunning = this.emitters.running.event;
  /**
   * Fires before the first status of a new server instance, so listeners
   * discard their node, decoration and style state (spec 6.2).
   */
  readonly onNewInstance = this.emitters.instance.event;

  private readonly trust: vscode.Disposable;

  constructor(private readonly host: ConnectionHost) {
    // A workspace `server.path` counts only once the workspace is trusted.
    this.trust = vscode.workspace.onDidGrantWorkspaceTrust(() => {
      if (vscode.workspace.getConfiguration('clippings').inspect('server.path')?.workspaceValue) void this.restart();
    });
  }

  private get client(): LanguageClient | undefined {
    return this.session?.client;
  }

  get running(): boolean {
    return this.client?.state === State.Running;
  }

  /** The server process ID, for tests. */
  get pid(): number | undefined {
    return this.session?.child?.pid;
  }

  /** Starts a server unless one is running or starting. Does nothing once disposed. */
  start(): Promise<void> {
    const generation = this.generation;
    return this.enqueue(() => this.startSession(generation));
  }

  /** Restarts on demand: cancels any pending start, forgets past crashes, then starts a fresh client. */
  restart(): Promise<void> {
    const generation = this.cancel();
    return this.enqueue(async () => {
      await this.retirements();
      if (generation !== this.generation) return;
      this.crashes.clear();
      await this.startSession(generation);
    });
  }

  /**
   * Cancels any pending start and ends the server. Once this settles, no
   * server runs or starts until the next `start` or `restart`.
   */
  stop(): Promise<void> {
    this.cancel();
    return this.enqueue(() => this.retirements());
  }

  /** Invalidates queued and pending work and retires the current session. */
  private cancel(): number {
    this.generation++;
    this.cancellation.abort();
    this.cancellation = new AbortController();
    if (this.session) void this.retire(this.session);
    return this.generation;
  }

  private enqueue(work: () => Promise<void>): Promise<void> {
    this.lifecycle = this.lifecycle
      .then(work)
      .catch((err: unknown) => this.host.log.error(`The server lifecycle failed: ${String(err)}`));
    return this.lifecycle;
  }

  private async retirements(): Promise<void> {
    await Promise.all(this.retiring);
  }

  /**
   * Starts a client, and a fresh one after each start failure until the
   * crash limit. Settles once a server runs, resolution fails, the limit is
   * reached, or a stop or restart cancels it.
   */
  private async startSession(generation: number): Promise<void> {
    if (this.disposed || generation !== this.generation) return;
    // Taken now: a cancellation during the awaits below must still reach this start.
    const signal = this.cancellation.signal;
    if (this.session) {
      const state = this.session.client.state;
      if (state === State.Running || state === State.Starting) return;
      await this.retire(this.session);
    }
    while ((await this.startOnce(signal)) === 'failed') {
      if (this.crashes.record(Date.now()) === 'give up') {
        void this.showCrash(GIVE_UP_MESSAGE);
        return;
      }
    }
  }

  private async startOnce(signal: AbortSignal): Promise<'started' | 'failed' | 'error' | 'cancelled'> {
    const { log, context } = this.host;
    let serverPath: string;
    try {
      const located = await unlessAborted(locateServer(context, (msg) => log.info(msg)), signal);
      if (located === 'cancelled') return 'cancelled';
      serverPath = located.candidate.path;
    } catch (err) {
      const message = err instanceof Error ? err.message : String(err);
      log.error(message);
      void this.showError(message, err instanceof ResolutionError);
      return 'error';
    }
    // `startSession` retired any earlier session; never replace a live one.
    if (this.session) await this.retire(this.session);
    if (signal.aborted) return 'cancelled';
    const settings = this.host.settings();
    const env = {
      ...process.env,
      RUST_BACKTRACE: '1',
      CLIPPINGS_LOG: vscode.workspace.getConfiguration('clippings').get<string>('server.logLevel', 'info'),
    };
    // Called for the first start and for each restart after a crash.
    const serverOptions: ServerOptions = async () => {
      if (session.abandoned) throw new Error('The server connection was stopped.');
      const previous = session.child;
      void killProcess(previous);
      const child = spawn(serverPath, ['lsp'], { env, windowsHide: true });
      session.child = child;
      child.on('error', (err) => log.error(`The server process failed: ${err.message}`));
      // A spawn that fails (ENOENT, EACCES) fails this start.
      await new Promise<void>((resolve, reject) => {
        child.once('spawn', resolve);
        child.once('error', reject);
      });
      return child;
    };
    // Set while `startOnce` waits for this client; a start failure after that
    // comes from the client restarting a crashed server.
    let waiting: ((outcome: 'failed') => void) | undefined;
    const failed = new Promise<'failed'>((resolve) => (waiting = resolve));
    // Each start, first or after a crash, must reach the running state in time.
    let watchdog: NodeJS.Timeout | undefined;
    let settled = false;
    const startFailed = (reason: string) => {
      clearTimeout(watchdog);
      if (settled || session.abandoned) return;
      settled = true;
      log.error(reason);
      if (waiting) waiting('failed');
      else this.afterStartFailure(session);
    };
    const policy = new CrashPolicy(this.crashes, {
      startFailed: () => startFailed('The server exited while starting.'),
      gaveUp: (message) => void this.showCrash(message),
    });
    const clientOptions: LanguageClientOptions = {
      // `vscode-notebook-cell` in the list covers notebook cells (spec 6.1).
      documentSelector: documentSelector(settings.general.schemes),
      initializationOptions: () => ({ protocolVersion: PROTOCOL_VERSION, settings: this.host.settings() }),
      // Clippings reports a failed start itself, once.
      initializationFailedHandler: () => false,
      outputChannel: log,
      errorHandler: policy,
    };
    const client = new Client('clippings', 'Clippings', serverOptions, clientOptions);
    const session: Session = { client, policy, child: undefined, abandoned: false };
    const relay =
      <P>(emitter: vscode.EventEmitter<P>) =>
      (params: P) => {
        if (this.session === session) emitter.fire(params);
      };
    client.onNotification(m.Status, (p) => {
      if (this.session !== session) return;
      if (p.instance !== this.instance) {
        this.instance = p.instance;
        this.emitters.instance.fire(p.instance);
      }
      this.emitters.status.fire(p);
    });
    client.onNotification(m.TreeChanged, relay(this.emitters.treeChanged));
    client.onNotification(m.Styles, relay(this.emitters.styles));
    client.onNotification(m.Decorations, relay(this.emitters.decorations));
    client.onDidChangeState((e) => {
      clearTimeout(watchdog);
      // While starting, `start()` returns the client's own start promise. A
      // connection that closes during `initialize` makes the client drop that
      // promise and then reject it; handling it here keeps that quiet.
      if (e.newState === State.Starting) client.start().catch(() => undefined);
      if (session.abandoned) return;
      if (e.newState === State.Starting) {
        settled = false;
        const seconds = this.startTimeoutMs / 1000;
        watchdog = setTimeout(
          () => startFailed(`The server did not finish starting within ${seconds} s.`),
          this.startTimeoutMs,
        );
      }
      if (e.newState === State.StartFailed) startFailed('The server failed to start.');
      policy.setRunning(e.newState === State.Running);
      if (e.newState === State.Running && this.session === session) this.onClientRunning(client);
    });
    this.session = session;
    // When the server exits or hangs during `initialize`, `start()` may never
    // settle; a start failure or a cancellation settles the race instead.
    const started = client.start().then(() => 'started' as const);
    started.catch(() => undefined);
    try {
      const outcome = await unlessAborted(Promise.race([started, failed]), signal);
      if (outcome !== 'started') await this.retire(session);
      return outcome;
    } catch (err) {
      // `initialize` failed or the connection closed under it: a start failure.
      settled = true;
      clearTimeout(watchdog);
      log.error(`The server failed to start: ${String(err)}`);
      await this.retire(session);
      return 'failed';
    } finally {
      waiting = undefined;
    }
  }

  /**
   * A crashed server that the client restarted, then failed to start: counts
   * the failure and starts a fresh client, unless a stop or restart came first.
   */
  private afterStartFailure(session: Session): void {
    if (this.session !== session) return;
    const generation = this.generation;
    void this.retire(session);
    void this.enqueue(async () => {
      await this.retirements();
      if (generation !== this.generation) return;
      if (this.crashes.record(Date.now()) === 'give up') void this.showCrash(GIVE_UP_MESSAGE);
      else await this.startSession(generation);
    });
  }

  /**
   * Lets go of a session for good: its events stop counting, its client
   * cannot start again, and its server process is ended. Idempotent.
   */
  private retire(session: Session): Promise<void> {
    if (session.retired) return session.retired;
    session.abandoned = true;
    session.policy.abandon();
    if (this.session === session) this.session = undefined;
    const retired = (async () => {
      try {
        // Asks a running server to shut down; resolves at once for any other.
        await session.client.dispose(2000);
      } catch (err) {
        this.host.log.warn(`Stopping the server failed: ${String(err)}`);
      }
      await killProcess(session.child);
    })();
    session.retired = retired;
    this.retiring.add(retired);
    void retired.finally(() => this.retiring.delete(retired));
    return retired;
  }

  private onClientRunning(client: LanguageClient): void {
    // Settings may have changed between `initialize` and now.
    this.notified(client.sendNotification(m.Configure, this.host.settings()));
    this.notified(client.sendNotification(m.ActiveEditor, { uri: this.host.activeUri() }));
    this.emitters.running.fire();
  }

  /** Settles a notification: one lost to a server that just died is only logged. */
  private notified(sending: Promise<void>): void {
    sending.catch((err: unknown) => this.host.log.debug(`Sending a notification failed: ${String(err)}`));
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
    if (this.running && this.client) this.notified(this.client.sendNotification(m.Configure, settings));
  }

  activeEditor(uri: string | null): void {
    if (this.running && this.client) this.notified(this.client.sendNotification(m.ActiveEditor, { uri }));
  }

  rescan(): void {
    if (this.running && this.client) this.notified(this.client.sendNotification(m.Rescan, {}));
  }

  stopScan(): void {
    if (this.running && this.client) this.notified(this.client.sendNotification(m.StopScan, {}));
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
    this.disposed = true;
    this.trust.dispose();
    void this.stop();
    for (const e of Object.values(this.emitters)) e.dispose();
  }
}

/** Settles like `work`, or with `'cancelled'` as soon as `signal` aborts. */
function unlessAborted<T>(work: Promise<T>, signal: AbortSignal): Promise<T | 'cancelled'> {
  if (signal.aborted) {
    work.catch(() => undefined);
    return Promise.resolve('cancelled');
  }
  return new Promise((resolve, reject) => {
    const cancel = () => resolve('cancelled');
    signal.addEventListener('abort', cancel, { once: true });
    work.then(resolve, reject).finally(() => signal.removeEventListener('abort', cancel));
  });
}

/** Ends a server process that did not exit by itself: SIGTERM, then SIGKILL. */
async function killProcess(child: ChildProcess | undefined): Promise<void> {
  // A process that never spawned has no ID and nothing to end.
  if (child?.pid === undefined) return;
  const exited = () => child.exitCode !== null || child.signalCode !== null;
  for (const signal of ['SIGTERM', 'SIGKILL'] as const) {
    if (exited()) return;
    let onExit: (() => void) | undefined;
    const exit = new Promise<void>((resolve) => child.once('exit', (onExit = () => resolve())));
    child.kill(signal);
    let timer: NodeJS.Timeout | undefined;
    await Promise.race([exit, new Promise<void>((resolve) => (timer = setTimeout(resolve, KILL_WAIT_MS)))]);
    clearTimeout(timer);
    if (onExit) child.off('exit', onExit);
  }
}
