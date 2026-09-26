import * as assert from 'node:assert/strict';
import { chmodSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import * as vscode from 'vscode';
import { START_TIMEOUT_MS } from '../../server/connection';
import type { ClippingsApi } from '../../testApi';
import { getApi, waitFor, whenIdle, withTimeout } from './helpers';

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
    } finally {
      process.env['CLIPPINGS_SERVER_PATH'] = real;
      subscription.dispose();
      rmSync(dir, { recursive: true, force: true });
    }
    const restart = vscode.commands.executeCommand('clippings.restartServer');
    await withTimeout('Restart Server with the real server', restart);
    await whenIdle(api);
    assert.ok(s.running);
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
    } finally {
      process.env['CLIPPINGS_SERVER_PATH'] = real;
      s.setStartTimeout(START_TIMEOUT_MS);
      subscription.dispose();
      rmSync(dir, { recursive: true, force: true });
    }
    const restart = vscode.commands.executeCommand('clippings.restartServer');
    await withTimeout('Restart Server with the real server', restart);
    await whenIdle(api);
    assert.ok(s.running);
  });

  it('shows the log', async () => {
    await vscode.commands.executeCommand('clippings.showLog');
  });
});
