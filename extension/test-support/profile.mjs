// Shared setup for the `.vscode-test*.mjs` configs: the locally built
// server's path, and a scratch directory with a quiet user profile and a
// fixture copy. `vscode-test` offers no after-run hook (see
// `@vscode/test-cli`'s configuration type — there is no teardown callback),
// so cleanup runs from a process `exit` handler here, once per config-file
// process, covering every scratch directory and link that process made.
// A cleanup failure is swallowed: `exit` handlers run synchronously and
// after every test result is already decided, so a leaked temp directory
// must never throw, and never fail or hide a test result.

import { cpSync, lstatSync, mkdirSync, mkdtempSync, realpathSync, rmSync, rmdirSync, symlinkSync, unlinkSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join, resolve } from 'node:path';

/** Quiet, predictable user settings for the test profile (spec 12.5). */
const PROFILE_SETTINGS = {
  'chat.disableAIFeatures': true,
  'workbench.startupEditor': 'none',
  'telemetry.telemetryLevel': 'off',
  'git.enabled': false,
  'update.mode': 'none',
};

/** The locally built server, or `CLIPPINGS_SERVER_PATH` when set. */
export function serverPath(here) {
  return (
    process.env.CLIPPINGS_SERVER_PATH ??
    resolve(here, '../target/debug/clippings' + (process.platform === 'win32' ? '.exe' : ''))
  );
}

const links = [];
const scratchDirs = [];

process.on('exit', () => {
  // Links first, and only ever as links: a Windows junction or directory
  // symlink is removed with the single-entry call that drops just the
  // reparse point, never by walking into it, and before its target
  // directory is removed by the ordinary recursive delete below.
  for (const link of links.splice(0)) removeLink(link);
  for (const dir of scratchDirs.splice(0)) {
    try {
      rmSync(dir, { recursive: true, force: true });
    } catch {
      // Best-effort: a leaked scratch directory must not fail the run.
    }
  }
});

function removeLink(path) {
  let stat;
  try {
    stat = lstatSync(path);
  } catch {
    return; // already gone
  }
  if (!stat.isSymbolicLink()) return;
  try {
    // Windows requires rmdir, not unlink, to remove a directory symlink or
    // junction without following it; POSIX always unlinks.
    if (process.platform === 'win32') rmdirSync(path);
    else unlinkSync(path);
  } catch {
    // Best-effort.
  }
}

/**
 * A fresh scratch directory (removed on exit) with a quiet user profile and
 * a copy of `fixture`, at `<dir>/workspace` or, under `parent`, at
 * `<dir>/<parent>/workspace`.
 */
export function scratchProfile(fixture, prefix, parent) {
  // The real path: on Windows the temporary directory can be an 8.3 short
  // name (`RUNNER~1`), and on macOS a symlink (`/tmp` to `/private/tmp`), so
  // tests and the server would otherwise spell the same file two ways.
  // Short: VS Code's IPC socket lives in the user data directory, and macOS
  // limits socket paths to 103 characters, so callers keep `prefix` short.
  const dir = mkdtempSync(join(realpathSync.native(tmpdir()), prefix));
  scratchDirs.push(dir);
  const copy = parent ? join(dir, parent, 'workspace') : join(dir, 'workspace');
  cpSync(fixture, copy, { recursive: true });
  mkdirSync(join(dir, 'user-data', 'User'), { recursive: true });
  writeFileSync(join(dir, 'user-data', 'User', 'settings.json'), JSON.stringify(PROFILE_SETTINGS));
  return { dir, copy };
}

/**
 * A link at `<dir>/alias/workspace` to `copy`: a directory symlink
 * (`type: 'dir'`) or an NTFS junction (`type: 'junction'`). Tracked for
 * removal, as a link and never by following it, before `dir` itself is
 * removed.
 */
export function linkedWorkspace(dir, copy, type) {
  const alias = join(dir, 'alias', 'workspace');
  mkdirSync(join(dir, 'alias'));
  symlinkSync(copy, alias, type);
  links.push(alias);
  return alias;
}
