import * as assert from 'node:assert/strict';
import { chmodSync, mkdtempSync, readFileSync, rmSync, watch, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import * as vscode from 'vscode';
import { readConfiguration } from '../../config/read';
import { GIVE_UP_MESSAGE } from '../../server/crashHistory';
import { START_TIMEOUT_MS, ServerConnection, type ConnectionHost } from '../../server/connection';
import { ViewStateStore } from '../../state/viewState';
import type { ClippingsApi } from '../../testApi';
import { MemoryMemento } from '../unit/memento';
import { DEFAULT_TREE } from './fixture';
import { getApi, setSetting, treeBecomes, waitFor, whenIdle, withTimeout } from './helpers';

describe('server lifecycle', () => {
  let api: ClippingsApi;

  before(async () => {
    api = await getApi();
    await whenIdle(api);
  });

  it('starts the locally built server and receives its status', () => {
    const status = api.test.server.status();
    assert.ok(status);
    assert.match(status.instance, /^[0-9a-f]+-[0-9a-f]+$/);
    assert.equal(status.error, null);
    assert.ok(api.test.server.pid);
  });

  it('restarts on demand with a new server instance', async () => {
    const before = api.test.server.status()?.instance;
    await vscode.commands.executeCommand('clippings.restartServer');
    const s = api.test.server;
    await waitFor('a new instance', () => s.running && s.status()?.instance !== before, [s.onStatus, s.onRunning]);
    await whenIdle(api);
  });

  it('restarts a crashed server', async () => {
    const s = api.test.server;
    const instance = s.status()?.instance;
    const pid = s.pid;
    assert.ok(pid);
    process.kill(pid, 'SIGKILL');
    await waitFor(
      'a restarted server',
      () => s.running && s.pid !== pid && s.status()?.instance !== instance,
      [s.onStatus, s.onRunning],
    );
    await whenIdle(api);
  });

  it('restarts when a setting fixed at start changes', async () => {
    const s = api.test.server;
    const instance = s.status()?.instance;
    await vscode.workspace
      .getConfiguration('clippings')
      .update('server.logLevel', 'debug', vscode.ConfigurationTarget.Global);
    try {
      await waitFor('a restart', () => s.running && s.status()?.instance !== instance, [s.onStatus, s.onRunning]);
    } finally {
      await vscode.workspace
        .getConfiguration('clippings')
        .update('server.logLevel', undefined, vscode.ConfigurationTarget.Global);
    }
    await whenIdle(api);
  });

  it('stops restarting after five crashes in three minutes and restarts on demand', async function () {
    this.timeout(60_000);
    const s = api.test.server;
    await vscode.commands.executeCommand('clippings.restartServer');
    await whenIdle(api);
    let notice: string | undefined;
    const subscription = s.onGaveUp((message) => (notice = message));
    try {
      for (let crash = 1; crash <= 5; crash++) {
        const pid = s.pid;
        assert.ok(pid, `a running server before crash ${crash}`);
        process.kill(pid, 'SIGKILL');
        if (crash < 5) {
          await waitFor(`a restart after crash ${crash}`, () => s.running && s.pid !== pid, [s.onStatus, s.onRunning]);
        }
      }
      const message = await waitFor('the crash notice', () => notice, [s.onGaveUp]);
      assert.match(message, /crashed 5 times in the last 3 minutes/);
      assert.equal(s.running, false);
    } finally {
      subscription.dispose();
    }
    await vscode.commands.executeCommand('clippings.restartServer');
    await whenIdle(api);
    assert.ok(s.running);
  });

  it('counts a server that exits while starting as a crash and restarts on demand', async function () {
    if (process.platform === 'win32') this.skip();
    this.timeout(60_000);
    const s = api.test.server;
    // Passes the probe, then exits as soon as `initialize` arrives.
    const dir = mkdtempSync(join(tmpdir(), 'clippings-fake-'));
    const fake = join(dir, 'clippings');
    writeFileSync(
      fake,
      [
        '#!/bin/sh',
        'if [ "$1" = probe ]; then',
        '  echo \'{"version":"0.0.0","target":"fake","protocolVersion":1}\'',
        '  exit 0',
        'fi',
        'head -c 1 > /dev/null',
        'exit 1',
        '',
      ].join('\n'),
    );
    chmodSync(fake, 0o755);
    const real = process.env['CLIPPINGS_SERVER_PATH'];
    let notice: string | undefined;
    const subscription = s.onGaveUp((message) => (notice = message));
    try {
      process.env['CLIPPINGS_SERVER_PATH'] = fake;
      const restart = vscode.commands.executeCommand('clippings.restartServer');
      await withTimeout('Restart Server with a server that exits', restart);
      const message = await waitFor('the crash notice', () => notice, [s.onGaveUp]);
      assert.match(message, /crashed 5 times in the last 3 minutes/);
      assert.equal(s.running, false);
      const shown = api.test.prompts.shown.at(-1);
      assert.deepEqual(shown, { kind: 'error', message, actions: ['Restart', 'Show Log'] });
    } finally {
      setServerPath(real);
      subscription.dispose();
      rmSync(dir, { recursive: true, force: true });
    }
    const restart = vscode.commands.executeCommand('clippings.restartServer');
    await withTimeout('Restart Server with the real server', restart);
    await whenIdle(api);
    assert.ok(s.running);
    await treeBecomes(api, DEFAULT_TREE);
  });

  it('counts a server that does not finish starting in time as a crash and kills it', async function () {
    if (process.platform === 'win32') this.skip();
    this.timeout(60_000);
    const s = api.test.server;
    // Passes the probe, then reads `initialize` and never answers.
    const dir = mkdtempSync(join(tmpdir(), 'clippings-fake-'));
    const fake = join(dir, 'clippings');
    const pids = join(dir, 'pids');
    writeFileSync(
      fake,
      [
        '#!/bin/sh',
        'if [ "$1" = probe ]; then',
        '  echo \'{"version":"0.0.0","target":"fake","protocolVersion":1}\'',
        '  exit 0',
        'fi',
        `echo $$ >> '${pids}'`,
        // Keeps stdout open on fd 3 while discarding the requests.
        'exec cat 3>&1 > /dev/null',
        '',
      ].join('\n'),
    );
    chmodSync(fake, 0o755);
    const real = process.env['CLIPPINGS_SERVER_PATH'];
    let notice: string | undefined;
    const subscription = s.onGaveUp((message) => (notice = message));
    try {
      process.env['CLIPPINGS_SERVER_PATH'] = fake;
      s.setStartTimeout(500);
      const restart = vscode.commands.executeCommand('clippings.restartServer');
      await withTimeout('Restart Server with a server that hangs', restart);
      const message = await waitFor('the crash notice', () => notice, [s.onGaveUp]);
      assert.match(message, /crashed 5 times in the last 3 minutes/);
      assert.equal(s.running, false);
      const started = readFileSync(pids, 'utf8').trim().split('\n').map(Number);
      assert.equal(started.length, 5, 'five attempts');
      for (const pid of started) assert.throws(() => process.kill(pid, 0), /ESRCH/, `server process ${pid} is gone`);
      const shown = api.test.prompts.shown.at(-1);
      assert.deepEqual(shown, { kind: 'error', message, actions: ['Restart', 'Show Log'] });
    } finally {
      setServerPath(real);
      s.setStartTimeout(START_TIMEOUT_MS);
      subscription.dispose();
      rmSync(dir, { recursive: true, force: true });
    }
    const restart = vscode.commands.executeCommand('clippings.restartServer');
    await withTimeout('Restart Server with the real server', restart);
    await whenIdle(api);
    assert.ok(s.running);
    await treeBecomes(api, DEFAULT_TREE);
  });

  it('ends two overlapping restarts with one server', async function () {
    if (process.platform === 'win32') this.skip();
    this.timeout(60_000);
    const s = api.test.server;
    // Answers `initialize` but never `shutdown`, so each stop takes its full timeout.
    const dir = mkdtempSync(join(tmpdir(), 'clippings-fake-'));
    const pids = join(dir, 'pids');
    const script = join(dir, 'server.js');
    writeFileSync(script, SLOW_TO_STOP_SERVER);
    const fake = fakeServer(dir, [
      `echo $$ >> '${pids}'`,
      `ELECTRON_RUN_AS_NODE=1 exec '${process.execPath}' '${script}'`,
    ]);
    const real = process.env['CLIPPINGS_SERVER_PATH'];
    let started: number[] = [];
    try {
      process.env['CLIPPINGS_SERVER_PATH'] = fake;
      const restart = vscode.commands.executeCommand('clippings.restartServer');
      await withTimeout('Restart Server with the fake server', restart);
      await waitFor('the fake server', () => s.running, [s.onRunning]);
      const first = vscode.commands.executeCommand('clippings.restartServer');
      const second = vscode.commands.executeCommand('clippings.restartServer');
      await withTimeout('two restarts', Promise.all([first, second]));
      await waitFor('a running server', () => s.running, [s.onRunning]);
      started = startedPids(pids);
      assert.deepEqual(started.filter(isAlive), [s.pid], 'exactly one live server, the current one');
    } finally {
      setServerPath(real);
      rmSync(dir, { recursive: true, force: true });
    }
    const restart = vscode.commands.executeCommand('clippings.restartServer');
    await withTimeout('Restart Server with the real server', restart);
    await whenIdle(api);
    assert.deepEqual(started.filter(isAlive), [], 'every fake server is gone');
  });

  it('cancels a pending start on restart and restarts promptly with a fresh crash budget', async function () {
    if (process.platform === 'win32') this.skip();
    this.timeout(60_000);
    const s = api.test.server;
    const timeout = 2000;
    const dir = mkdtempSync(join(tmpdir(), 'clippings-fake-'));
    const pids = join(dir, 'pids');
    const fake = fakeServer(dir, [`echo $$ >> '${pids}'`, 'exec cat 3>&1 > /dev/null']);
    const real = process.env['CLIPPINGS_SERVER_PATH'];
    let notices = 0;
    const subscription = s.onGaveUp(() => notices++);
    try {
      process.env['CLIPPINGS_SERVER_PATH'] = fake;
      s.setStartTimeout(timeout);
      const first = vscode.commands.executeCommand('clippings.restartServer');
      await waitFor('the first attempt', () => startedPids(pids).length === 1, [onFilesChanged(dir)]);
      const since = Date.now();
      const second = vscode.commands.executeCommand('clippings.restartServer');
      await waitFor('the next attempt', () => startedPids(pids).length === 2, [onFilesChanged(dir)]);
      assert.ok(Date.now() - since < timeout / 2, 'the restart did not wait for the pending start');
      await withTimeout('the first restart', first);
      await withTimeout('the second restart', second, 30_000);
      assert.equal(notices, 1, 'one crash notice');
      const started = startedPids(pids);
      assert.equal(started.length, 6, 'one cancelled attempt, then five');
      for (const pid of started) assert.ok(!isAlive(pid), `server process ${pid} is gone`);
    } finally {
      setServerPath(real);
      s.setStartTimeout(START_TIMEOUT_MS);
      subscription.dispose();
      rmSync(dir, { recursive: true, force: true });
    }
    const restart = vscode.commands.executeCommand('clippings.restartServer');
    await withTimeout('Restart Server with the real server', restart);
    await whenIdle(api);
  });

  for (const end of ['stop', 'dispose'] as const) {
    it(`spawns no server after ${end} during a start`, async function () {
      if (process.platform === 'win32') this.skip();
      this.timeout(60_000);
      const dir = mkdtempSync(join(tmpdir(), 'clippings-fake-'));
      const pids = join(dir, 'pids');
      const fake = fakeServer(dir, [`echo $$ >> '${pids}'`, 'exec cat 3>&1 > /dev/null']);
      const real = process.env['CLIPPINGS_SERVER_PATH'];
      const host = testHost();
      const connection = new ServerConnection(host);
      try {
        process.env['CLIPPINGS_SERVER_PATH'] = fake;
        connection.startTimeoutMs = 500;
        const start = connection.start();
        await waitFor('the first attempt', () => startedPids(pids).length === 1, [onFilesChanged(dir)]);
        if (end === 'stop') await connection.stop();
        else connection.dispose();
        await withTimeout('the start', start);
        await withTimeout('the stop', connection.stop());
        assert.equal(connection.running, false);
        assert.equal(startedPids(pids).length, 1, 'no server started after the first');
        assert.ok(!isAlive(startedPids(pids)[0] ?? 0), 'the first server is gone');
        assert.deepEqual(host.errors, []);
      } finally {
        setServerPath(real);
        connection.dispose();
        rmSync(dir, { recursive: true, force: true });
      }
    });
  }

  it('shows only its own notice when the server fails to start', async function () {
    if (process.platform === 'win32') this.skip();
    this.timeout(60_000);
    const dir = mkdtempSync(join(tmpdir(), 'clippings-fake-'));
    const fake = fakeServer(dir, ['head -c 1 > /dev/null', 'exit 1']);
    const real = process.env['CLIPPINGS_SERVER_PATH'];
    const host = testHost();
    const connection = new ServerConnection(host);
    // Every error toast from this extension, the language client included.
    const window = vscode.window as { showErrorMessage: typeof vscode.window.showErrorMessage };
    const showErrorMessage = window.showErrorMessage;
    const toasts: string[] = [];
    window.showErrorMessage = (message: string) => (toasts.push(message), Promise.resolve(undefined));
    try {
      process.env['CLIPPINGS_SERVER_PATH'] = fake;
      await withTimeout('the start', connection.start());
      assert.deepEqual(host.errors, [GIVE_UP_MESSAGE]);
      assert.deepEqual(toasts, []);
    } finally {
      window.showErrorMessage = showErrorMessage;
      setServerPath(real);
      connection.dispose();
      rmSync(dir, { recursive: true, force: true });
    }
  });

  it('clears the start watchdog when a pending start is retired', async function () {
    if (process.platform === 'win32') this.skip();
    this.timeout(20_000);
    // Never responds to `initialize`, so the session stays in the `Starting`
    // state: disposing it then never goes through a state change, which is
    // the case that leaves the watchdog timer dangling without the fix.
    const dir = mkdtempSync(join(tmpdir(), 'clippings-fake-'));
    const fake = fakeServer(dir, ['exec cat 3>&1 > /dev/null']);
    const real = process.env['CLIPPINGS_SERVER_PATH'];
    const host = testHost();
    const connection = new ServerConnection(host);
    const realSetTimeout = global.setTimeout;
    const realClearTimeout = global.clearTimeout;
    // A distinctive delay so only the watchdog's own `setTimeout` call matches.
    const WATCHDOG_MS = 424_242;
    let watchdogHandle: NodeJS.Timeout | undefined;
    let watchdogCleared = false;
    global.setTimeout = ((handler: (...args: unknown[]) => void, ms?: number, ...args: unknown[]) => {
      const handle = realSetTimeout(handler, ms, ...args);
      if (ms === WATCHDOG_MS) watchdogHandle = handle;
      return handle;
    }) as typeof setTimeout;
    global.clearTimeout = ((handle?: NodeJS.Timeout | string | number) => {
      if (handle !== undefined && handle === watchdogHandle) watchdogCleared = true;
      return realClearTimeout(handle as NodeJS.Timeout);
    }) as typeof clearTimeout;
    const delay = (ms: number) => new Promise<void>((resolve) => realSetTimeout(resolve, ms));
    try {
      process.env['CLIPPINGS_SERVER_PATH'] = fake;
      connection.startTimeoutMs = WATCHDOG_MS;
      const started = connection.start();
      started.catch(() => undefined);
      const deadline = Date.now() + 5000;
      while (watchdogHandle === undefined) {
        assert.ok(Date.now() < deadline, 'the watchdog timer was never scheduled');
        await delay(10);
      }
      // `retire` clears the watchdog synchronously, from `stop`'s own call to
      // `cancel`, before anything async (killing the process, which closes
      // the client's connection and would otherwise clear it too, later and
      // for an unrelated reason) has had a chance to run.
      const stopping = connection.stop();
      assert.equal(watchdogCleared, true, 'retire must clear the pending watchdog timer synchronously');
      await withTimeout('the stop', stopping);
    } finally {
      global.setTimeout = realSetTimeout;
      global.clearTimeout = realClearTimeout;
      setServerPath(real);
      connection.dispose();
      rmSync(dir, { recursive: true, force: true });
    }
  });

  it('settles a pending request when a server that ignores shutdown is stopped', async function () {
    if (process.platform === 'win32') this.skip();
    this.timeout(20_000);
    // Answers `initialize` only, so the request stays pending and the stop
    // times out. The language client then never disposes its connection,
    // which is what rejects pending requests; a request left pending kept
    // VS Code's tree view waiting on it for good.
    const dir = mkdtempSync(join(tmpdir(), 'clippings-fake-'));
    const received = join(dir, 'received');
    const script = join(dir, 'server.js');
    writeFileSync(script, SLOW_TO_STOP_SERVER);
    const fake = fakeServer(dir, [`ELECTRON_RUN_AS_NODE=1 exec '${process.execPath}' '${script}' '${received}'`]);
    const real = process.env['CLIPPINGS_SERVER_PATH'];
    const connection = new ServerConnection(testHost());
    try {
      process.env['CLIPPINGS_SERVER_PATH'] = fake;
      await withTimeout('the start', connection.start());
      assert.equal(connection.running, true);
      const children = connection.children(null).then(
        () => 'resolved',
        () => 'rejected',
      );
      await waitFor('the request at the server', () => receivedMethods(received).includes('clippings/children'), [
        onFilesChanged(dir),
      ]);
      await withTimeout('the stop', connection.stop());
      assert.equal(await withTimeout('the pending request', children, 5000), 'rejected');
    } finally {
      setServerPath(real);
      connection.dispose();
      rmSync(dir, { recursive: true, force: true });
    }
  });

  it('starts on the defaults with a warning when a setting has the wrong type', async () => {
    const s = api.test.server;
    const instance = s.status()?.instance;
    await setSetting('general.schemes', 'file');
    try {
      const status = await waitFor(
        'a new server with a warning',
        () => {
          const current = s.status();
          return current && current.instance !== instance && current.warnings.length > 0 ? current : undefined;
        },
        [s.onStatus],
      );
      assert.match(status.warnings[0] ?? '', /^Invalid configuration, using the defaults: general\.schemes: invalid type/);
      const warned = api.test.prompts.shown.some(
        (p) => p.kind === 'warning' && p.message.includes('Invalid configuration'),
      );
      assert.ok(warned);
    } finally {
      await setSetting('general.schemes', undefined);
    }
    await waitFor('a server without warnings', () => s.running && s.status()?.warnings.length === 0, [
      s.onStatus,
      s.onRunning,
    ]);
    await whenIdle(api);
  });

  it('shows the log', async () => {
    await vscode.commands.executeCommand('clippings.showLog');
  });
});

/**
 * A server that answers `initialize` and ignores everything else, `shutdown`
 * included. Given a file path, it appends each method it receives there.
 */
const SLOW_TO_STOP_SERVER = `
const received = process.argv[2];
let buffer = Buffer.alloc(0);
process.stdin.on('data', (chunk) => {
  buffer = Buffer.concat([buffer, chunk]);
  for (;;) {
    const end = buffer.indexOf('\\r\\n\\r\\n');
    if (end < 0) return;
    const length = Number(/Content-Length: *(\\d+)/i.exec(buffer.subarray(0, end).toString())[1]);
    if (buffer.length < end + 4 + length) return;
    const message = JSON.parse(buffer.subarray(end + 4, end + 4 + length).toString());
    buffer = buffer.subarray(end + 4 + length);
    if (received) require('fs').appendFileSync(received, message.method + '\\n');
    if (message.method === 'initialize') {
      const body = JSON.stringify({ jsonrpc: '2.0', id: message.id, result: { capabilities: {} } });
      process.stdout.write('Content-Length: ' + Buffer.byteLength(body) + '\\r\\n\\r\\n' + body);
    }
  }
});
`;

/** Writes an executable fake server that passes the probe, then runs `lsp` for `clippings lsp`. */
function fakeServer(dir: string, lsp: string[]): string {
  const path = join(dir, 'clippings');
  writeFileSync(
    path,
    [
      '#!/bin/sh',
      'if [ "$1" = probe ]; then',
      '  echo \'{"version":"0.0.0","target":"fake","protocolVersion":1}\'',
      '  exit 0',
      'fi',
      ...lsp,
      '',
    ].join('\n'),
  );
  chmodSync(path, 0o755);
  return path;
}

/** Restores `CLIPPINGS_SERVER_PATH`; assigning `undefined` would set the string "undefined". */
function setServerPath(path: string | undefined): void {
  if (path === undefined) delete process.env['CLIPPINGS_SERVER_PATH'];
  else process.env['CLIPPINGS_SERVER_PATH'] = path;
}

/** The process IDs a fake server recorded, in start order. */
function startedPids(file: string): number[] {
  try {
    return readFileSync(file, 'utf8').trim().split('\n').filter(Boolean).map(Number);
  } catch {
    return [];
  }
}

/** The methods a fake server recorded, in arrival order. */
function receivedMethods(file: string): string[] {
  try {
    return readFileSync(file, 'utf8').split('\n').filter(Boolean);
  } catch {
    return [];
  }
}

function isAlive(pid: number): boolean {
  try {
    process.kill(pid, 0);
    return true;
  } catch {
    return false;
  }
}

/** Fires when a file in `dir` changes. */
function onFilesChanged(dir: string): vscode.Event<void> {
  return (listener) => {
    const watcher = watch(dir, () => listener());
    return { dispose: () => watcher.close() };
  };
}

/**
 * The log for connections the tests create. It stays open: a connection's
 * client can still log a closed connection after `stop` settles.
 */
let testLog: vscode.LogOutputChannel | undefined;

/** A connection host, apart from the extension's, that records the errors it would show. */
function testHost(): ConnectionHost & { readonly errors: string[] } {
  const log = (testLog ??= vscode.window.createOutputChannel('Clippings test', { log: true }));
  const store = new ViewStateStore(new MemoryMemento());
  const ext = vscode.extensions.getExtension('clippings-dev.clippings');
  assert.ok(ext);
  const context = {
    extensionPath: ext.extensionPath,
    extensionMode: vscode.ExtensionMode.Test,
    globalState: new MemoryMemento(),
  } as unknown as vscode.ExtensionContext;
  const errors: string[] = [];
  return {
    context,
    log,
    errors,
    settings: () => readConfiguration(store.snapshot()),
    activeUri: () => null,
    error: (message) => (errors.push(message), Promise.resolve(undefined)),
  };
}
