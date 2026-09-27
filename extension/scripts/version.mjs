// Release versions (spec 8.2). Versions are plain `major.minor.patch`,
// because the Marketplace rejects semver pre-release versions. An odd minor
// version is a pre-release, packaged and published with `--pre-release`; an
// even minor version is stable.
//
// Usage: node scripts/version.mjs [--tag <tag>]
//
// Prints `version=<x.y.z>` and `pre-release=<true|false>`, and appends them
// to $GITHUB_OUTPUT when it is set. Fails if the extension's version is not
// plain, differs from the Cargo workspace version, or, with --tag, differs
// from the tag.

import { appendFileSync, readFileSync } from 'node:fs';
import { resolve } from 'node:path';
import { parseArgs } from 'node:util';

const PLAIN = /^(0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*)$/;

/** Parses a plain `major.minor.patch` version, or throws. */
export function parseVersion(version) {
  const match = PLAIN.exec(version);
  if (!match) throw new Error(`version ${JSON.stringify(version)} is not plain major.minor.patch`);
  return { major: Number(match[1]), minor: Number(match[2]), patch: Number(match[3]) };
}

/** True when `version` is a pre-release: its minor version is odd. */
export function isPreRelease(version) {
  return parseVersion(version).minor % 2 === 1;
}

/** Throws unless `tag` is `v` followed by `version`. */
export function checkTag(tag, version) {
  if (tag !== `v${version}`) throw new Error(`tag ${tag} does not match package.json version ${version}`);
}

/** The `version` under `[workspace.package]` in the root Cargo.toml. */
export function cargoWorkspaceVersion(toml) {
  const section = /^\[workspace\.package\]\s*$([\s\S]*?)(?=^\[|(?![\s\S]))/m.exec(toml);
  const version = section && /^version\s*=\s*"([^"]*)"\s*$/m.exec(section[1]);
  if (!version) throw new Error('Cargo.toml has no [workspace.package] version');
  return version[1];
}

function main() {
  const { values } = parseArgs({ options: { tag: { type: 'string' } } });
  const here = import.meta.dirname;
  const version = JSON.parse(readFileSync(resolve(here, '../package.json'), 'utf8')).version;
  const preRelease = isPreRelease(version);
  const cargo = cargoWorkspaceVersion(readFileSync(resolve(here, '../../Cargo.toml'), 'utf8'));
  if (cargo !== version) throw new Error(`Cargo workspace version ${cargo} differs from package.json version ${version}`);
  if (values.tag !== undefined) checkTag(values.tag, version);

  const output = `version=${version}\npre-release=${preRelease}\n`;
  process.stdout.write(output);
  if (process.env.GITHUB_OUTPUT) appendFileSync(process.env.GITHUB_OUTPUT, output);
}

if (import.meta.filename === process.argv[1]) {
  try {
    main();
  } catch (error) {
    console.error(`error: ${error.message}`);
    process.exitCode = 1;
  }
}
