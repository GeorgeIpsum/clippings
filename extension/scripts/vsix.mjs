// Reads and checks a packaged VSIX (spec 8.1, 8.2), so every package is
// verified the way VS Code will see it: a platform package carries its
// target's server binary, executable, beside `bin/platform.ok`, and the
// universal package carries neither.
//
// Usage: node scripts/vsix.mjs <vsix> (--target <vscode-target> | --universal) [--pre-release true|false]
//
// --pre-release defaults to the channel of package.json's version.

import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';
import { parseArgs } from 'node:util';
import { inflateRawSync } from 'node:zlib';
import { isPreRelease } from './version.mjs';

export const TARGETS = [
  'win32-x64',
  'win32-arm64',
  'linux-x64',
  'linux-arm64',
  'linux-armhf',
  'alpine-x64',
  'alpine-arm64',
  'darwin-x64',
  'darwin-arm64',
];

/** The server's file name in a target's package. */
export function serverName(target) {
  return target.startsWith('win32-') ? 'clippings.exe' : 'clippings';
}

/**
 * The entries of a zip file: name, unix mode (0 when the archiver was not
 * unix), uncompressed size, and a function returning the contents.
 */
export function readZip(buffer) {
  let eocd = -1;
  for (let i = buffer.length - 22; i >= Math.max(0, buffer.length - 22 - 0xffff); i--) {
    if (buffer.readUInt32LE(i) === 0x06054b50) {
      eocd = i;
      break;
    }
  }
  if (eocd < 0) throw new Error('not a zip file: no end of central directory record');
  const count = buffer.readUInt16LE(eocd + 10);
  let offset = buffer.readUInt32LE(eocd + 16);
  const entries = [];
  for (let n = 0; n < count; n++) {
    if (buffer.readUInt32LE(offset) !== 0x02014b50) throw new Error('corrupt zip central directory');
    const madeBy = buffer.readUInt16LE(offset + 4);
    const method = buffer.readUInt16LE(offset + 10);
    const compressedSize = buffer.readUInt32LE(offset + 20);
    const size = buffer.readUInt32LE(offset + 24);
    const nameLength = buffer.readUInt16LE(offset + 28);
    const extraLength = buffer.readUInt16LE(offset + 30);
    const commentLength = buffer.readUInt16LE(offset + 32);
    const attributes = buffer.readUInt32LE(offset + 38);
    const local = buffer.readUInt32LE(offset + 42);
    const name = buffer.toString('utf8', offset + 46, offset + 46 + nameLength);
    const mode = madeBy >> 8 === 3 ? attributes >>> 16 : 0;
    const read = () => {
      if (buffer.readUInt32LE(local) !== 0x04034b50) throw new Error(`corrupt zip local header for ${name}`);
      const start = local + 30 + buffer.readUInt16LE(local + 26) + buffer.readUInt16LE(local + 28);
      const data = buffer.subarray(start, start + compressedSize);
      if (method === 0) return data;
      if (method === 8) return inflateRawSync(data);
      throw new Error(`${name}: unsupported zip compression method ${method}`);
    };
    entries.push({ name, mode, size, read });
    offset += 46 + nameLength + extraLength + commentLength;
  }
  return entries;
}

/** The format and architecture of an executable, from its headers. */
export function binaryKind(bytes) {
  if (bytes.length >= 20 && bytes.readUInt32BE(0) === 0x7f454c46) {
    const machine = bytes.readUInt16LE(18);
    const arch = { 0x3e: 'x64', 0xb7: 'arm64', 0x28: 'arm' }[machine] ?? `machine 0x${machine.toString(16)}`;
    return `elf-${arch}`;
  }
  if (bytes.length >= 8 && bytes.readUInt32LE(0) === 0xfeedfacf) {
    const cpu = bytes.readUInt32LE(4);
    const arch = { 0x01000007: 'x64', 0x0100000c: 'arm64' }[cpu] ?? `cpu 0x${cpu.toString(16)}`;
    return `macho-${arch}`;
  }
  if (bytes.length >= 64 && bytes.toString('latin1', 0, 2) === 'MZ') {
    const pe = bytes.readUInt32LE(0x3c);
    if (pe + 6 <= bytes.length && bytes.readUInt32BE(pe) === 0x50450000) {
      const machine = bytes.readUInt16LE(pe + 4);
      const arch = { 0x8664: 'x64', 0xaa64: 'arm64' }[machine] ?? `machine 0x${machine.toString(16)}`;
      return `pe-${arch}`;
    }
  }
  return 'unknown';
}

/** The binary kind each target's server must have. */
export function expectedKind(target) {
  const [os, arch] = target.split('-');
  const format = { win32: 'pe', linux: 'elf', alpine: 'elf', darwin: 'macho' }[os];
  return `${format}-${arch === 'armhf' ? 'arm' : arch}`;
}

// Everything a package may contain, under `extension/`, mirroring
// .vscodeignore; vsce renames README.md and LICENSE. Anything else means
// .vscodeignore let something through, such as a development bundle's
// source map.
const ALLOWED = [/^dist\/[^/]+\.js$/, /^bin\//, /^resources\//, /^(package\.json|readme\.md|LICENSE\.txt)$/];
const REQUIRED = ['package.json', 'readme.md', 'LICENSE.txt', 'dist/extension.js'];

/**
 * The problems with a package, empty when it is good. `target` is a VS Code
 * target, or undefined for the universal package.
 */
export function checkPackage(entries, { target, preRelease, version }) {
  const problems = [];
  const files = new Map();
  for (const entry of entries) {
    if (entry.name.endsWith('/')) continue;
    if (entry.name === '[Content_Types].xml' || entry.name === 'extension.vsixmanifest') {
      files.set(entry.name, entry);
      continue;
    }
    if (!entry.name.startsWith('extension/')) {
      problems.push(`unexpected entry ${entry.name}`);
      continue;
    }
    const path = entry.name.slice('extension/'.length);
    files.set(path, entry);
    if (!ALLOWED.some((pattern) => pattern.test(path))) problems.push(`unexpected file extension/${path}`);
  }
  for (const path of REQUIRED) {
    if (!files.has(path)) problems.push(`missing extension/${path}`);
  }

  const manifest = files.get('extension.vsixmanifest')?.read().toString('utf8');
  if (manifest === undefined) {
    problems.push('missing extension.vsixmanifest');
  } else {
    const platform = /TargetPlatform="([^"]*)"/.exec(manifest)?.[1];
    if (platform !== target) problems.push(`manifest target platform is ${platform ?? 'none'}, expected ${target ?? 'none'}`);
    const packagedPreRelease = manifest.includes('Id="Microsoft.VisualStudio.Code.PreRelease" Value="true"');
    if (packagedPreRelease !== preRelease) problems.push(`manifest pre-release is ${packagedPreRelease}, expected ${preRelease}`);
    const packagedVersion = /<Identity [^>]*Version="([^"]*)"/.exec(manifest)?.[1];
    if (packagedVersion !== version) problems.push(`manifest version is ${packagedVersion}, expected ${version}`);
  }

  const bin = [...files.keys()].filter((path) => path.startsWith('bin/')).sort();
  if (target === undefined) {
    if (bin.length > 0) problems.push(`the universal package must not bundle a server, but has ${bin.join(', ')}`);
    return problems;
  }
  const name = serverName(target);
  const expected = [`bin/${name}`, 'bin/platform.ok'].sort();
  if (bin.join() !== expected.join()) problems.push(`bin/ holds ${bin.join(', ') || 'nothing'}, expected ${expected.join(', ')}`);
  const server = files.get(`bin/${name}`);
  if (server) {
    const kind = binaryKind(server.read().subarray(0, 4096));
    if (kind !== expectedKind(target)) problems.push(`bin/${name} is ${kind}, expected ${expectedKind(target)}`);
    if (!target.startsWith('win32-') && (server.mode & 0o111) !== 0o111) {
      problems.push(`bin/${name} is not executable (mode ${(server.mode & 0o777).toString(8)})`);
    }
  }
  return problems;
}

function main() {
  const { values, positionals } = parseArgs({
    allowPositionals: true,
    options: { target: { type: 'string' }, universal: { type: 'boolean' }, 'pre-release': { type: 'string' } },
  });
  const [path] = positionals;
  if (!path || positionals.length > 1 || !!values.target === !!values.universal) {
    throw new Error('usage: vsix.mjs <vsix> (--target <vscode-target> | --universal) [--pre-release true|false]');
  }
  if (values.target !== undefined && !TARGETS.includes(values.target)) throw new Error(`unknown target ${values.target}`);
  const version = JSON.parse(readFileSync(resolve(import.meta.dirname, '../package.json'), 'utf8')).version;
  const preRelease = values['pre-release'] === undefined ? isPreRelease(version) : values['pre-release'] === 'true';
  const entries = readZip(readFileSync(path));
  const problems = checkPackage(entries, { target: values.target, preRelease, version });
  if (problems.length > 0) throw new Error(`${path}:\n  ${problems.join('\n  ')}`);
  const bin = entries.filter((e) => e.name.startsWith('extension/bin/'));
  const summary = bin.map((e) => `${e.name.slice('extension/'.length)} ${e.size} bytes, mode ${(e.mode & 0o777).toString(8)}`);
  console.log(`${path}: ok (${values.target ?? 'universal'}${preRelease ? ', pre-release' : ''})${summary.map((s) => `\n  ${s}`).join('')}`);
}

if (import.meta.filename === process.argv[1]) {
  try {
    main();
  } catch (error) {
    console.error(`error: ${error.message}`);
    process.exitCode = 1;
  }
}
