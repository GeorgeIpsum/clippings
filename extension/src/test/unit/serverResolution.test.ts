import * as assert from 'node:assert/strict';
import { chmodSync, mkdtempSync, rmSync, statSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { delimiter, join } from 'node:path';
import { ServerPathApprovals } from '../../server/approvals';
import {
  candidates,
  expandServerPath,
  findOnPath,
  serverPathSetting,
  type CandidateInputs,
} from '../../server/candidates';
import { ensureExecutable, probe, ProbeError } from '../../server/probe';
import { ResolutionError, resolveServer } from '../../server/resolve';
import { serverPath } from './lspProcess';
import { MemoryMemento } from './memento';

const base: CandidateInputs = {
  env: {},
  setting: undefined,
  home: '/home/u',
  workspaceFolder: '/w',
  extensionPath: '/ext',
  platform: 'linux',
  development: false,
  exists: () => false,
};

describe('server candidates', () => {
  it('orders environment, setting, bundled and PATH', () => {
    const list = candidates({
      ...base,
      env: { CLIPPINGS_SERVER_PATH: '/env/clippings', PATH: ['/a', '/b'].join(delimiter) },
      setting: { value: '~/bin/clippings', fromWorkspace: false },
      development: true,
      exists: (p) => ['/ext/bin/clippings', '/ext/bin/platform.ok', join('/b', 'clippings')].includes(p),
    });
    assert.deepEqual(list, [
      { source: 'environment', path: '/env/clippings' },
      { source: 'setting', path: '/home/u/bin/clippings', fromWorkspace: false },
      { source: 'bundled', path: join('/ext', 'bin', 'clippings') },
      { source: 'path', path: join('/b', 'clippings') },
    ]);
  });

  it('needs the platform marker next to the bundled binary', () => {
    const list = candidates({ ...base, exists: (p) => p === join('/ext', 'bin', 'clippings') });
    assert.equal(list.length, 1);
    assert.equal(list[0]?.unavailable, 'not bundled in this package');
  });

  it('only tries PATH in development builds and names the Windows binary', () => {
    assert.ok(!candidates(base).some((c) => c.source === 'path'));
    const dev = candidates({ ...base, platform: 'win32', development: true });
    assert.deepEqual(dev.at(-1), { source: 'path', path: 'clippings.exe', unavailable: 'not on PATH' });
  });

  it('marks a workspace setting and skips a blank one', () => {
    const list = candidates({ ...base, setting: { value: '${workspaceFolder}/target/clippings', fromWorkspace: true } });
    assert.deepEqual(list[0], { source: 'setting', path: '/w/target/clippings', fromWorkspace: true });
    assert.ok(!candidates({ ...base, setting: { value: '  ', fromWorkspace: true } }).some((c) => c.source === 'setting'));
  });

  it('ignores a workspace server.path in an untrusted workspace', () => {
    const values = { globalValue: '~/bin/clippings', workspaceValue: '/repo/tools/clippings' };
    assert.deepEqual(serverPathSetting(values, true), { value: '/repo/tools/clippings', fromWorkspace: true });
    assert.deepEqual(serverPathSetting(values, false), { value: '~/bin/clippings', fromWorkspace: false });
    assert.equal(serverPathSetting({ workspaceValue: '/repo/tools/clippings' }, false), undefined);
    assert.equal(serverPathSetting({ workspaceFolderValue: '/repo/tools/clippings' }, false), undefined);
    assert.equal(serverPathSetting(undefined, true), undefined);
    const setting = serverPathSetting({ workspaceValue: '/repo/tools/clippings' }, false);
    const untrusted = candidates({ ...base, setting });
    assert.ok(!untrusted.some((c) => c.source === 'setting'), 'no candidate from the workspace value');
  });

  it('expands ~ and ${workspaceFolder} only where they belong', () => {
    assert.equal(expandServerPath('~', '/h', undefined), '/h');
    assert.equal(expandServerPath('~/x', '/h', undefined), '/h/x');
    assert.equal(expandServerPath('/a/~b', '/h', undefined), '/a/~b');
    assert.equal(expandServerPath('${workspaceFolder}/x/${workspaceFolder}', '/h', '/w'), '/w/x//w');
  });

  it('finds the first match on PATH', () => {
    const env = { PATH: ['/one', '', '/two', '/three'].join(delimiter) };
    const found = findOnPath('clippings', env, (p) => p !== join('/one', 'clippings'));
    assert.equal(found, join('/two', 'clippings'));
    assert.equal(findOnPath('clippings', {}, () => true), undefined);
  });
});

describe('server resolution', () => {
  const info = { version: '1.0.0', target: 't', protocolVersion: 1 };

  it('returns the first candidate that passes the probe', async () => {
    const probed: string[] = [];
    const resolved = await resolveServer(
      [
        { source: 'environment', path: '/bad' },
        { source: 'setting', path: '/good', fromWorkspace: false },
        { source: 'bundled', path: '/never' },
      ],
      {
        probe: (p) => (probed.push(p), p === '/good' ? Promise.resolve(info) : Promise.reject(new ProbeError('boom'))),
        approve: () => Promise.resolve(true),
        log: () => {},
      },
    );
    assert.equal(resolved.candidate.path, '/good');
    assert.deepEqual(probed, ['/bad', '/good']);
  });

  it('asks before running a workspace path and aggregates every failure', async () => {
    const asked: string[] = [];
    await assert.rejects(
      resolveServer(
        [
          { source: 'setting', path: '/ws', fromWorkspace: true },
          { source: 'bundled', path: '/b', unavailable: 'not bundled in this package' },
        ],
        {
          probe: () => Promise.resolve(info),
          approve: (p) => (asked.push(p), Promise.resolve(false)),
          log: () => {},
        },
      ),
      (err: unknown) => {
        assert.ok(err instanceof ResolutionError);
        assert.deepEqual(
          err.failures.map((f) => f.reason),
          ['not allowed to run from workspace settings', 'not bundled in this package'],
        );
        assert.match(err.message, /clippings\.server\.path \(\/ws\): not allowed/);
        assert.match(err.message, /bundled server \(\/b\): not bundled/);
        return true;
      },
    );
    assert.deepEqual(asked, ['/ws']);
  });
});

describe('server path approvals', () => {
  it('asks once per path and remembers explicit choices', async () => {
    const memento = new MemoryMemento();
    const answers: ('allow' | 'deny' | undefined)[] = ['allow', undefined, 'deny'];
    const asked: string[] = [];
    const approvals = new ServerPathApprovals(memento, (p) => (asked.push(p), Promise.resolve(answers.shift())));
    assert.equal(await approvals.approve('/a'), true);
    assert.equal(await approvals.approve('/a'), true);
    assert.equal(await approvals.approve('/b'), false, 'dismissed denies');
    assert.equal(await approvals.approve('/b'), false, 'asked again, denied');
    assert.equal(await approvals.approve('/b'), false);
    assert.deepEqual(asked, ['/a', '/b', '/b']);
    assert.deepEqual(memento.get('serverPathDecisions'), { '/a': 'allow', '/b': 'deny' });
  });
});

describe('probe', function () {
  this.timeout(20_000);
  let dir: string;
  before(() => (dir = mkdtempSync(join(tmpdir(), 'clippings-probe-'))));
  after(() => rmSync(dir, { recursive: true, force: true }));

  function script(name: string, body: string): string {
    const path = join(dir, name);
    writeFileSync(path, `#!/bin/sh\n${body}\n`);
    chmodSync(path, 0o755);
    return path;
  }

  it('accepts the locally built server', async () => {
    const info = await probe(serverPath());
    assert.equal(info.protocolVersion, 1);
    assert.match(info.version, /^\d+\.\d+\.\d+/);
  });

  it('reports a missing binary, a crash, bad output, a wrong protocol and a hang', async function () {
    if (process.platform === 'win32') this.skip();
    await assert.rejects(probe(join(dir, 'missing')), /did not start \(ENOENT\)/);
    await assert.rejects(probe(script('crash', 'echo oops >&2; exit 3')), /exited with code 3: oops/);
    await assert.rejects(probe(script('text', 'echo hello')), /something other than JSON/);
    await assert.rejects(probe(script('shape', 'echo \'{"version":1}\'')), /unexpected JSON/);
    await assert.rejects(
      probe(script('old', 'echo \'{"version":"0.0.1","target":"x","protocolVersion":0}\'')),
      /protocol version 0, but this extension needs 1/,
    );
    await assert.rejects(probe(script('hang', 'exec sleep 5'), 300), /timed out after 0.3 s/);
  });

  it('makes a bundled binary executable', function () {
    if (process.platform === 'win32') this.skip();
    const path = join(dir, 'plain');
    writeFileSync(path, '');
    chmodSync(path, 0o644);
    ensureExecutable(path);
    assert.equal(statSync(path).mode & 0o777, 0o755);
  });
});
