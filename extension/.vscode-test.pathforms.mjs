// Path-form tests: the fixture workspace opened through another spelling of
// its path, one configuration (label) per form. VS Code keeps the path it
// was given, while the filesystem, file watchers and the server may see
// another, so these check that the tree, reveal, decorations and disk
// changes still agree.
//
//   symlink   a directory symlink to the fixture copy (every OS; macOS's
//             /tmp -> /private/tmp is one)
//   junction  an NTFS junction to the fixture copy (Windows)
//   short     the fixture copy's 8.3 short path (Windows), under a directory
//             whose long name needs a short alias. Skipped, with the reason
//             in the test output, when the volume has 8.3 names disabled.
//
// Run one with `vscode-test --config .vscode-test.pathforms.mjs --label <form>`.
// `test-support/profile.mjs` removes every scratch directory and link on exit.
import { defineConfig } from '@vscode/test-cli';
import { execSync } from 'node:child_process';
import { join, resolve } from 'node:path';
import { linkedWorkspace, scratchProfile, serverPath } from './test-support/profile.mjs';

const here = import.meta.dirname;
const fixture = resolve(here, '../tests/fixtures/workspace');
const server = serverPath(here);
const windows = process.platform === 'win32';

/** A fresh scratch directory with a user profile and, under `parent`, a copy of the fixture. */
function scratch(form, parent = 'real') {
  return scratchProfile(fixture, `clp-${form.slice(0, 2)}-`, parent);
}

/** A link at <dir>/alias/workspace to the copy: a symlink or an NTFS junction. */
function linked(form, type) {
  const { dir, copy } = scratch(form);
  return { dir, workspace: linkedWorkspace(dir, copy, type) };
}

/** The copy's 8.3 short path, or a reason to skip when the volume has none. */
function short() {
  const { dir, copy } = scratch('short', 'clippings path forms');
  const shortPath = execSync(`for %I in ("${copy}") do @echo %~sI`, { encoding: 'utf8' }).trim();
  if (shortPath.toLowerCase() === copy.toLowerCase() || !shortPath.includes('~')) {
    return { dir, workspace: copy, skip: `8.3 short names are disabled on this volume (${copy} has no short form)` };
  }
  return { dir, workspace: shortPath };
}

const forms = { symlink: () => linked('symlink', 'dir') };
if (windows) {
  forms.junction = () => linked('junction', 'junction');
  forms.short = short;
}

export default defineConfig(
  Object.entries(forms).map(([label, make]) => {
    const { dir, workspace, skip } = make();
    return {
      label,
      files: 'out/test/pathforms/**/*.test.js',
      version: process.env.CLIPPINGS_TEST_VSCODE ?? 'stable',
      extensionDevelopmentPath: here,
      workspaceFolder: workspace,
      launchArgs: ['--disable-extensions', `--user-data-dir=${join(dir, 'user-data')}`, '--disable-workspace-trust'],
      env: {
        CLIPPINGS_SERVER_PATH: server,
        CLIPPINGS_TEST_WORKSPACE: workspace,
        CLIPPINGS_TEST_PATH_FORM: label,
        ...(skip ? { CLIPPINGS_TEST_SKIP: skip } : {}),
      },
      mocha: { ui: 'bdd', timeout: 20000, slow: 2000 },
    };
  }),
);
