import * as assert from 'node:assert/strict';
import { describe, it } from 'node:test';
import { cargoWorkspaceVersion, checkTag, isPreRelease, parseVersion } from './version.mjs';

describe('release versions', () => {
  it('parses plain major.minor.patch versions', () => {
    assert.deepEqual(parseVersion('0.1.0'), { major: 0, minor: 1, patch: 0 });
    assert.deepEqual(parseVersion('12.34.56'), { major: 12, minor: 34, patch: 56 });
  });

  it('rejects versions the Marketplace would reject or that are not plain', () => {
    for (const version of ['0.1.0-beta.1', '0.1.0+build', '0.1', '1.2.3.4', 'v0.1.0', '01.2.3', '', ' 0.1.0']) {
      assert.throws(() => parseVersion(version), /not plain/, version);
    }
  });

  it('makes odd minor versions pre-releases and even ones stable', () => {
    assert.equal(isPreRelease('0.1.0'), true);
    assert.equal(isPreRelease('0.3.7'), true);
    assert.equal(isPreRelease('1.11.0'), true);
    assert.equal(isPreRelease('0.0.1'), false);
    assert.equal(isPreRelease('0.2.0'), false);
    assert.equal(isPreRelease('2.10.4'), false);
  });

  it('accepts only the tag v<version>', () => {
    checkTag('v0.2.1', '0.2.1');
    for (const tag of ['0.2.1', 'v0.2.2', 'v0.2.1-rc1', 'refs/tags/v0.2.1']) {
      assert.throws(() => checkTag(tag, '0.2.1'), /does not match/, tag);
    }
  });

  it('reads the Cargo workspace version, not a dependency or package version', () => {
    const toml = [
      '[workspace]',
      'members = ["a"]',
      '',
      '[workspace.package]',
      'edition = "2021"',
      'version = "0.3.0"',
      '',
      '[workspace.dependencies]',
      'serde = { version = "1" }',
      'version = "9.9.9"',
    ].join('\n');
    assert.equal(cargoWorkspaceVersion(toml), '0.3.0');
    assert.equal(cargoWorkspaceVersion('[workspace.package]\nversion = "1.0.0"'), '1.0.0');
    assert.throws(() => cargoWorkspaceVersion('[package]\nversion = "1.0.0"\n'), /no \[workspace.package\]/);
  });
});
