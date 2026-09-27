import * as assert from 'node:assert/strict';
import { describe, it } from 'node:test';
import { deflateRawSync } from 'node:zlib';
import { packageName, vsceArgs } from './package.mjs';
import { TARGETS, binaryKind, checkPackage, expectedKind, readZip, serverName } from './vsix.mjs';

/** A zip of `files` ({ name, data, mode }), stored or deflated, as yazl writes it. */
function zip(files, { deflate = false } = {}) {
  const locals = [];
  const centrals = [];
  let offset = 0;
  for (const { name, data, mode = 0o100644 } of files) {
    const body = deflate ? deflateRawSync(data) : data;
    const nameBytes = Buffer.from(name);
    const local = Buffer.alloc(30);
    local.writeUInt32LE(0x04034b50, 0);
    local.writeUInt16LE(deflate ? 8 : 0, 8);
    local.writeUInt32LE(body.length, 18);
    local.writeUInt32LE(data.length, 22);
    local.writeUInt16LE(nameBytes.length, 26);
    const central = Buffer.alloc(46);
    central.writeUInt32LE(0x02014b50, 0);
    central.writeUInt16LE((3 << 8) | 20, 4);
    central.writeUInt16LE(deflate ? 8 : 0, 10);
    central.writeUInt32LE(body.length, 20);
    central.writeUInt32LE(data.length, 24);
    central.writeUInt16LE(nameBytes.length, 28);
    central.writeUInt32LE((mode << 16) >>> 0, 38);
    central.writeUInt32LE(offset, 42);
    locals.push(local, nameBytes, body);
    centrals.push(central, nameBytes);
    offset += 30 + nameBytes.length + body.length;
  }
  const directory = Buffer.concat(centrals);
  const end = Buffer.alloc(22);
  end.writeUInt32LE(0x06054b50, 0);
  end.writeUInt16LE(files.length, 8);
  end.writeUInt16LE(files.length, 10);
  end.writeUInt32LE(directory.length, 12);
  end.writeUInt32LE(offset, 16);
  return Buffer.concat([...locals, directory, end]);
}

function elf(machine) {
  const bytes = Buffer.alloc(64);
  bytes.writeUInt32BE(0x7f454c46, 0);
  bytes.writeUInt16LE(machine, 18);
  return bytes;
}
function macho(cpu) {
  const bytes = Buffer.alloc(32);
  bytes.writeUInt32LE(0xfeedfacf, 0);
  bytes.writeUInt32LE(cpu, 4);
  return bytes;
}
function pe(machine) {
  const bytes = Buffer.alloc(256);
  bytes.write('MZ', 0, 'latin1');
  bytes.writeUInt32LE(0x80, 0x3c);
  bytes.writeUInt32BE(0x50450000, 0x80);
  bytes.writeUInt16LE(machine, 0x84);
  return bytes;
}
const SERVERS = {
  'win32-x64': pe(0x8664),
  'win32-arm64': pe(0xaa64),
  'linux-x64': elf(0x3e),
  'linux-arm64': elf(0xb7),
  'linux-armhf': elf(0x28),
  'alpine-x64': elf(0x3e),
  'alpine-arm64': elf(0xb7),
  'darwin-x64': macho(0x01000007),
  'darwin-arm64': macho(0x0100000c),
};

function manifest({ target, preRelease = true, version = '0.1.0' }) {
  const platform = target ? ` TargetPlatform="${target}"` : '';
  const property = preRelease ? '<Property Id="Microsoft.VisualStudio.Code.PreRelease" Value="true" />' : '';
  return `<PackageManifest><Metadata><Identity Language="en-US" Id="clippings" Version="${version}" Publisher="clippings-dev"${platform}/><Properties>${property}</Properties></Metadata></PackageManifest>`;
}

/** The entries of a well-formed package, read back through readZip. */
function vsix({ target, preRelease, extra = [], omit = [], serverMode = 0o100755 } = {}) {
  const files = [
    { name: '[Content_Types].xml', data: Buffer.from('<Types/>') },
    { name: 'extension.vsixmanifest', data: Buffer.from(manifest({ target, preRelease })) },
    { name: 'extension/package.json', data: Buffer.from('{}') },
    { name: 'extension/readme.md', data: Buffer.from('# Clippings') },
    { name: 'extension/LICENSE.txt', data: Buffer.from('MIT') },
    { name: 'extension/dist/extension.js', data: Buffer.from('module.exports = {};') },
    { name: 'extension/resources/clippings-container.svg', data: Buffer.from('<svg/>') },
  ];
  if (target) {
    files.push({ name: `extension/bin/${serverName(target)}`, data: SERVERS[target], mode: serverMode });
    files.push({ name: 'extension/bin/platform.ok', data: Buffer.from(`${target}\n`) });
  }
  const kept = files.filter((f) => !omit.includes(f.name));
  return readZip(zip([...kept, ...extra], { deflate: true }));
}

const check = (entries, options) => checkPackage(entries, { preRelease: true, version: '0.1.0', ...options });

describe('vsix', () => {
  it('reads names, modes, sizes and contents from stored and deflated zips', () => {
    for (const deflate of [false, true]) {
      const entries = readZip(
        zip(
          [
            { name: 'a.txt', data: Buffer.from('hello') },
            { name: 'bin/tool', data: Buffer.from('x'.repeat(1000)), mode: 0o100755 },
          ],
          { deflate },
        ),
      );
      assert.deepEqual(
        entries.map((e) => [e.name, e.mode & 0o777, e.size]),
        [
          ['a.txt', 0o644, 5],
          ['bin/tool', 0o755, 1000],
        ],
      );
      assert.equal(entries[0].read().toString(), 'hello');
      assert.equal(entries[1].read().toString(), 'x'.repeat(1000));
    }
    assert.throws(() => readZip(Buffer.from('not a zip at all, not even close')), /not a zip/);
  });

  it('tells executable formats and architectures apart', () => {
    for (const target of TARGETS) assert.equal(binaryKind(SERVERS[target]), expectedKind(target), target);
    assert.equal(binaryKind(Buffer.from('#!/bin/sh\necho hi\n')), 'unknown');
    assert.equal(expectedKind('linux-armhf'), 'elf-arm');
    assert.equal(expectedKind('alpine-arm64'), 'elf-arm64');
    assert.equal(expectedKind('win32-x64'), 'pe-x64');
  });

  it('accepts each well-formed platform package and the universal package', () => {
    for (const target of TARGETS) assert.deepEqual(check(vsix({ target }), { target }), [], target);
    assert.deepEqual(check(vsix(), { target: undefined }), []);
    assert.deepEqual(check(vsix({ target: 'linux-x64', preRelease: false }), { target: 'linux-x64', preRelease: false }), []);
  });

  it('rejects a universal package with a server, and a platform package without one', () => {
    assert.match(check(vsix({ target: 'linux-x64' }), { target: undefined }).join('\n'), /must not bundle a server/);
    const missing = check(vsix({ target: 'linux-x64', omit: ['extension/bin/clippings'] }), { target: 'linux-x64' });
    assert.match(missing.join('\n'), /bin\/ holds bin\/platform.ok, expected bin\/clippings, bin\/platform.ok/);
    const noMarker = check(vsix({ target: 'darwin-arm64', omit: ['extension/bin/platform.ok'] }), { target: 'darwin-arm64' });
    assert.match(noMarker.join('\n'), /bin\/ holds bin\/clippings, expected/);
  });

  it('rejects a server for the wrong target or without its executable bit', () => {
    assert.match(check(vsix({ target: 'linux-arm64' }), { target: 'linux-x64' }).join('\n'), /is elf-arm64, expected elf-x64/);
    const plain = check(vsix({ target: 'linux-x64', serverMode: 0o100644 }), { target: 'linux-x64' });
    assert.match(plain.join('\n'), /not executable \(mode 644\)/);
    // Windows does not use the executable bit.
    assert.deepEqual(check(vsix({ target: 'win32-x64', serverMode: 0o100644 }), { target: 'win32-x64' }), []);
  });

  it('rejects the wrong channel, version or target platform in the manifest', () => {
    const entries = vsix({ target: 'linux-x64', preRelease: true });
    assert.match(check(entries, { target: 'linux-x64', preRelease: false }).join('\n'), /pre-release is true, expected false/);
    assert.match(check(entries, { target: 'linux-x64', version: '0.2.0' }).join('\n'), /version is 0.1.0, expected 0.2.0/);
    assert.match(check(vsix(), { target: 'linux-x64' }).join('\n'), /target platform is none, expected linux-x64/);
  });

  it('rejects files .vscodeignore should have left out, and missing required files', () => {
    const extra = [
      { name: 'extension/dist/extension.js.map', data: Buffer.from('{}') },
      { name: 'extension/src/extension.ts', data: Buffer.from('') },
      { name: 'extension/node_modules/x/index.js', data: Buffer.from('') },
    ];
    const problems = check(vsix({ extra, omit: ['extension/LICENSE.txt'] }), { target: undefined });
    assert.deepEqual(problems, [
      'unexpected file extension/dist/extension.js.map',
      'unexpected file extension/src/extension.ts',
      'unexpected file extension/node_modules/x/index.js',
      'missing extension/LICENSE.txt',
    ]);
  });
});

describe('package', () => {
  it('names packages by target and version', () => {
    assert.equal(packageName('clippings', 'linux-armhf', '0.1.0'), 'clippings-linux-armhf-0.1.0.vsix');
    assert.equal(packageName('clippings', undefined, '0.2.3'), 'clippings-universal-0.2.3.vsix');
  });

  it('runs vsce without dependencies, with the target and channel', () => {
    assert.deepEqual(vsceArgs({ target: 'win32-arm64', preRelease: true, out: 'o.vsix' }), [
      'package',
      '--no-dependencies',
      '--target',
      'win32-arm64',
      '--pre-release',
      '--out',
      'o.vsix',
    ]);
    assert.deepEqual(vsceArgs({ target: undefined, preRelease: false, out: 'o.vsix' }), [
      'package',
      '--no-dependencies',
      '--out',
      'o.vsix',
    ]);
  });
});
