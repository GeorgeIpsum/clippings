// Extension integration tests (spec 12.5). Each run copies the fixture
// workspace and uses a fresh user data directory, so tests can edit files
// and settings without touching the repository or the user's profile.
import { defineConfig } from '@vscode/test-cli';
import { cpSync, mkdirSync, mkdtempSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join, resolve } from 'node:path';

const here = import.meta.dirname;
const scratch = mkdtempSync(join(tmpdir(), 'clippings-test-'));
const workspace = join(scratch, 'workspace');
cpSync(resolve(here, '../tests/fixtures/workspace'), workspace, { recursive: true });
// Quiet, predictable user settings for the test profile.
mkdirSync(join(scratch, 'user-data', 'User'), { recursive: true });
writeFileSync(
  join(scratch, 'user-data', 'User', 'settings.json'),
  JSON.stringify({
    'chat.disableAIFeatures': true,
    'workbench.startupEditor': 'none',
    'telemetry.telemetryLevel': 'off',
    'git.enabled': false,
    'update.mode': 'none',
  }),
);
const server =
  process.env.CLIPPINGS_SERVER_PATH ?? resolve(here, '../target/debug/clippings' + (process.platform === 'win32' ? '.exe' : ''));

export default defineConfig({
  // `index.js` registers a global `afterEach` (spec 12.5); listed
  // explicitly since it doesn't match the `*.test.js` glob.
  files: ['out/test/integration/**/*.test.js', 'out/test/integration/index.js'],
  version: process.env.CLIPPINGS_TEST_VSCODE ?? 'stable',
  extensionDevelopmentPath: here,
  workspaceFolder: workspace,
  launchArgs: ['--disable-extensions', `--user-data-dir=${join(scratch, 'user-data')}`, '--disable-workspace-trust'],
  env: { CLIPPINGS_SERVER_PATH: server, CLIPPINGS_TEST_WORKSPACE: workspace },
  mocha: { ui: 'bdd', timeout: 20000, slow: 2000 },
});
