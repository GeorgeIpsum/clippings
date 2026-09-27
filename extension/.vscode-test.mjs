// Extension integration tests (spec 12.5). Each run copies the fixture
// workspace and uses a fresh user data directory, so tests can edit files
// and settings without touching the repository or the user's profile.
// `test-support/profile.mjs` removes the scratch directory on exit.
import { defineConfig } from '@vscode/test-cli';
import { join, resolve } from 'node:path';
import { scratchProfile, serverPath } from './test-support/profile.mjs';

const here = import.meta.dirname;
const { dir: scratch, copy: workspace } = scratchProfile(resolve(here, '../tests/fixtures/workspace'), 'clippings-test-');

export default defineConfig({
  // `index.js` registers a global `afterEach` (spec 12.5); listed
  // explicitly since it doesn't match the `*.test.js` glob.
  files: ['out/test/integration/**/*.test.js', 'out/test/integration/index.js'],
  version: process.env.CLIPPINGS_TEST_VSCODE ?? 'stable',
  extensionDevelopmentPath: here,
  workspaceFolder: workspace,
  launchArgs: ['--disable-extensions', `--user-data-dir=${join(scratch, 'user-data')}`, '--disable-workspace-trust'],
  env: { CLIPPINGS_SERVER_PATH: serverPath(here), CLIPPINGS_TEST_WORKSPACE: workspace },
  mocha: { ui: 'bdd', timeout: 20000, slow: 2000 },
});
