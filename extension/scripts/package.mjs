// Packages one VSIX (spec 8.2) from the bundle `pnpm build --production`
// left in dist/.
// A platform package copies the target's server binary and a `platform.ok`
// marker into bin/ and runs `vsce package --target`; the universal package
// has an empty bin/ and no target. Odd minor versions are packaged with
// `--pre-release`. The package is then checked with vsix.mjs, and bin/ is
// removed again.
//
// Usage: node scripts/package.mjs --target <vscode-target> --server <binary> [--out <dir>]
//        node scripts/package.mjs --universal [--out <dir>]
//
// --out defaults to ../dist/vsix. Prints the package's path.

import { spawnSync } from 'node:child_process';
import { chmodSync, copyFileSync, existsSync, mkdirSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { createRequire } from 'node:module';
import { join, resolve } from 'node:path';
import { parseArgs } from 'node:util';
import { isPreRelease } from './version.mjs';
import { TARGETS, checkPackage, readZip, serverName } from './vsix.mjs';

const extension = resolve(import.meta.dirname, '..');

/** The package's file name: `clippings-<target or universal>-<version>.vsix`. */
export function packageName(name, target, version) {
  return `${name}-${target ?? 'universal'}-${version}.vsix`;
}

/** The arguments to `vsce package`. */
export function vsceArgs({ target, preRelease, out }) {
  return [
    'package',
    '--no-dependencies',
    ...(target === undefined ? [] : ['--target', target]),
    ...(preRelease ? ['--pre-release'] : []),
    '--out',
    out,
  ];
}

function main() {
  const { values } = parseArgs({
    options: {
      target: { type: 'string' },
      server: { type: 'string' },
      universal: { type: 'boolean' },
      out: { type: 'string' },
    },
  });
  const { target, server, universal } = values;
  if (universal ? target !== undefined || server !== undefined : target === undefined || server === undefined) {
    throw new Error('usage: package.mjs --target <vscode-target> --server <binary> | --universal [--out <dir>]');
  }
  if (target !== undefined && !TARGETS.includes(target)) throw new Error(`unknown target ${target}`);
  if (!existsSync(join(extension, 'dist', 'extension.js'))) {
    throw new Error('dist/extension.js is missing: run pnpm build --production first');
  }
  if (existsSync(join(extension, 'dist', 'extension.js.map'))) {
    throw new Error('dist/ holds a development bundle: run pnpm build --production first');
  }

  const manifest = JSON.parse(readFileSync(join(extension, 'package.json'), 'utf8'));
  const preRelease = isPreRelease(manifest.version);
  const outDir = resolve(values.out ?? resolve(extension, '../dist/vsix'));
  const out = join(outDir, packageName(manifest.name, target, manifest.version));
  mkdirSync(outDir, { recursive: true });

  const bin = join(extension, 'bin');
  rmSync(bin, { recursive: true, force: true });
  try {
    if (target !== undefined) {
      mkdirSync(bin);
      const binary = join(bin, serverName(target));
      copyFileSync(resolve(server), binary);
      chmodSync(binary, 0o755);
      writeFileSync(join(bin, 'platform.ok'), `${target}\n`);
    }
    const vsce = createRequire(import.meta.url).resolve('@vscode/vsce/vsce');
    const result = spawnSync(process.execPath, [vsce, ...vsceArgs({ target, preRelease, out })], {
      cwd: extension,
      stdio: ['ignore', 'inherit', 'inherit'],
    });
    if (result.status !== 0) throw new Error(`vsce package failed with ${result.error ?? `exit code ${result.status}`}`);
  } finally {
    rmSync(bin, { recursive: true, force: true });
  }

  const problems = checkPackage(readZip(readFileSync(out)), { target, preRelease, version: manifest.version });
  if (problems.length > 0) throw new Error(`${out}:\n  ${problems.join('\n  ')}`);
  console.log(out);
}

if (import.meta.filename === process.argv[1]) {
  try {
    main();
  } catch (error) {
    console.error(`error: ${error.message}`);
    process.exitCode = 1;
  }
}
