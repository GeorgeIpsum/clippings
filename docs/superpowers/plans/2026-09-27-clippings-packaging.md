# Clippings Packaging Implementation Plan (Plan 4 of 6)

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Ship Clippings: build the server for the nine VS Code targets with separate debug symbols, check and probe every binary, package nine platform VSIX files and a universal one and verify each, run the extension tests on Linux, macOS and Windows in CI, and release tagged versions to GitHub with Marketplace and Open VSX publishing and code signing that stay off until configured.

**Architecture:** Shell scripts in `scripts/dist/` build one target (`build.sh`, over a new `dist` Cargo profile), check it (`check.sh`: glibc floor, symbol table, `clippings probe` natively or in a container under qemu) and optionally sign it (`sign-macos.sh`); `target.sh` maps a VS Code target to its Rust triple. Node scripts in `extension/scripts/`, tested with `node:test`, decide the release channel from the version (`version.mjs`), package one VSIX (`package.mjs`) and verify a VSIX by reading the zip itself (`vsix.mjs`). Three workflows use them: `ci.yml` runs the Rust and extension tests on three operating systems and calls `build.yml` on pushes to `main`; `build.yml` is a reusable workflow with a nine-target server matrix, an optional Windows signing job, a Windows Arm64 probe and a packaging job; `release.yml` decides what a run may do in a `plan` job, calls `build.yml`, and creates the GitHub release and publishes only for a `v*` tag with the credentials configured. A manual run is a dry run.

**Tech Stack:** Rust stable through `dtolnay/rust-toolchain@stable` (1.98.1 when the prototype ran) with `Swatinem/rust-cache@v2`; mimalloc 0.1.52 (`default-features = false`, resolving libmimalloc-sys 0.1.49) on Windows only; cargo-zigbuild 0.23.4 with ziglang 0.15.2, installed with pip into a venv; `llvm-objcopy` from the rustup `llvm-tools` component; Docker images `debian:10` (glibc 2.28) and `alpine:3.22`, with `docker/setup-qemu-action@v3`; `readelf` and `jq` on the Linux runners. Node 22 with `node:test` for the script tests, pnpm 10.27.0, @vscode/vsce 4.0.0 (already in the lockfile) and `ovsx@1.2.0` through `npx`. GitHub Actions: `actions/checkout@v5`, `actions/setup-node@v5`, `pnpm/action-setup@v4`, `actions/cache@v4`, `actions/upload-artifact@v4`, `actions/download-artifact@v5`, `azure/login@v3`, `azure/artifact-signing-action@v2`; runners `ubuntu-latest`, `macos-latest`, `windows-latest` and `windows-11-arm`. VS Code 1.91.0 (the `engines.vscode` floor) and `stable` in CI. Locally: actionlint 1.7.12, Docker, the `gh` CLI, and an Apple silicon Mac, where the prototype ran.

**Spec:** `docs/superpowers/specs/2026-09-23-clippings-design.md`. This plan implements section 8 (packaging and release) and the CI half of 12.5 (the extension tests on every OS, and path-form tests). The last task records this plan's rulings in the spec, and moves `clippings bench` out of milestone 4 (section 14).

**Builds on:** plan 3, merged on `main` at `ab1e46b`. The server, the extension and their tests are used as they are, with one product change (Task 6: flat-view directory labels use `/` on Windows) and two test fixes (Tasks 1 and 6).

## Global Constraints

- **No tags, releases or publishing.** Do not create or push a tag, do not create a GitHub release, and do not run the release workflow with `dry-run` off. Do not configure any secret, variable or environment in the repository. Publishing and signing stay off.
- **Pushing is allowed.** The user approved pushing the feature branch to `origin` and running workflows on it. The CI workflow runs on pull requests, so Task 7 opens one draft pull request from the feature branch to `main`; never merge it or mark it ready. The release workflow runs on the branch through `gh workflow run`.
- **The proof is a `workflow_dispatch` dry run** of the release workflow on the feature branch (Task 12): every build, check and packaging job green, the release and publishing jobs skipped, and all ten downloaded packages passing `vsix.mjs`.
- **Rust gates.** For every task that changes Rust (Tasks 2 and 6), `cargo fmt --all --check` must pass, `cargo clippy --all-targets -- -D warnings` must pass, and `INSTA_UPDATE=no cargo test --all` must pass (204 passed, 2 ignored) before the commit. Code under `cfg(windows)` is only compiled by CI's Windows jobs.
- **Extension gates.** `pnpm -C extension typecheck` must pass before any commit that touches `extension/src/`. Unit tests run from the esbuild output, so build before `pnpm -C extension test:unit`. `pnpm -C extension test` runs the whole chain: `dev`, `typecheck`, `build`, `test:unit`, `test:scripts` (from Task 4), `test:integration` and `test:pathforms` (from Task 8).
- **Commit messages:** the subject line (and body, where given) from the task, a blank line, then two trailer lines, `Co-Authored-By: <model attribution>` with your model attribution, and `Claude-Session: https://claude.ai/code/session_01EuAdHjqQeFrieAZqvsQ325`. Use the heredoc shown in each task so the blank line is kept.
- **VS Code test windows appear** on screen during every integration and path-form run and close by themselves. Do not click in them or type into them while tests run.
- **Lockfiles.** No task changes the extension's dependencies: `extension/pnpm-lock.yaml` stays as it is; restore dependencies with `pnpm -C extension install --frozen-lockfile`. Task 2 adds mimalloc to `Cargo.lock` through the diff it gives; if that diff does not apply, run `cargo update -p mimalloc --precise 0.1.52` after the `Cargo.toml` edits to produce the same entries. Every build uses `--locked`.
- **The host.** Commands that build or package for "this machine" are written for an Apple silicon Mac (`darwin-arm64`). On another machine use its own target (`darwin-x64`, `win32-x64`, and so on); Linux targets need cargo-zigbuild and zig. Task 10's glibc check needs Docker.
- **Build output** goes to `dist/` at the repository root, which Task 2 adds to `.gitignore`. `extension/bin/` exists only while `package.mjs` runs; it must never be committed.
- **Style.** Shell scripts are bash with `set -euo pipefail`, quote every expansion, and are sourced or run from any directory. Node scripts are ES modules (`.mjs`) that use only Node built-ins, export their logic for tests, and run `main()` only when executed directly. Workflow files pin action majors and tool versions exactly, give each job the least `permissions` it needs, and pass `${{ }}` values into `run:` scripts through `env:`, not inline, except matrix values that are fixed strings.

## Review Focus

These inputs are implied by the spec but easy to miss, and each would bite a real user. Each has a test in the task that owns the code.

1. **A workspace opened through a symlink, an NTFS junction or an 8.3 short path** (`C:\Users\RUNNER~1\...\WORKSP~1`), which VS Code keeps as given while the server's file watcher sees the real path. The tree must fill, the active file must be revealed, an open file must be decorated without a second tree node, and a change on disk must reach the tree. Tests `fills the tree`, `reveals the active file`, `decorates an open file and keeps one tree node for it` and `reflects a change on disk` in `src/test/pathforms/pathforms.test.ts`, run once per form by `.vscode-test.pathforms.mjs` (Task 8; the Windows forms run in CI).
2. **A VSIX with the wrong server or none**: an arm64 binary in an x64 package, a server in the universal package, a platform package missing its server or `platform.ok`, or a server without its executable bit. VS Code would install it and the extension would fail at start on that platform only. Tests `rejects a universal package with a server, and a platform package without one` and `rejects a server for the wrong target or without its executable bit` in `scripts/vsix.test.mjs`, and a check of a real darwin-arm64 package against `linux-x64` (Task 5).
3. **A tag that does not match the version**, such as `v0.2.0` pushed while `package.json` still says `0.1.0`, or a Cargo workspace version that differs, which would release packages whose manifest and `clippings probe` disagree with the tag. Test `accepts only the tag v<version>` in `scripts/version.test.mjs` and the `version.mjs --tag v0.2.0` check (Task 4); the release `plan` job runs it before anything builds (Task 12).
4. **The pre-release parity rule**: an odd minor version must be packaged and published as a pre-release and an even one as stable, or users on the stable channel get pre-release builds. Tests `makes odd minor versions pre-releases and even ones stable` (Task 4) and `rejects the wrong channel, version or target platform in the manifest` (Task 5).
5. **A glibc floor regression**: a Linux binary built against a newer glibc, which would fail to start on the older distributions the 2.28 floor promises. `check.sh` must reject it. The check in Task 10 builds the server with plain cargo on Debian bookworm and expects `linux-x64: needs GLIBC_2.34, above the 2.28 floor`.

## Rulings made while prototyping this plan

Each was taken where the spec was silent, unworkable or at odds with what the tools allow. Task 14 writes them into the spec where they change or clarify behaviour.

1. **Shipped binaries use a `dist` profile**: `release` plus `debug = "full"` and `strip = "none"`, after which `scripts/dist/build.sh` moves the debug info into a separate file. `release` keeps `strip = "symbols"` unchanged. One profile cannot do both jobs: `strip = "symbols"` makes MSVC link with `/DEBUG:NONE`, which suppresses the PDB, and on Linux it deletes the debug info before it can be split out (Task 2, spec 8.2).
2. **Only debug info is stripped, so backtraces name functions.** macOS builds with the environment overrides `CARGO_PROFILE_DIST_SPLIT_DEBUGINFO=packed` and `CARGO_PROFILE_DIST_STRIP=debuginfo` (rustc runs dsymutil before stripping; the dSYM's UUID matches the binary). Linux and Alpine binaries go through `llvm-objcopy --only-keep-debug`, then `--strip-debug --add-gnu-debuglink`. A panic with `RUST_BACKTRACE=1` then names its functions with no symbols file present, which the prototype proved on macOS arm64, linux-x64 and alpine-arm64. The cost is 15 to 27 % more binary and 5 to 9 % more VSIX. **Windows cannot do this**: an MSVC executable has no symbol table, its names live only in the PDB, the PDB adds about 55 MB per package, and `/PDBSTRIPPED` keeps almost nothing after fat LTO. Windows backtraces show `<unknown>` frames unless the release's PDB is placed beside `clippings.exe` (Tasks 2 and 10, spec 8.2).
3. **mimalloc 0.1.52 on Windows only**, with default features off, as rust-analyzer does; the system allocator elsewhere (Task 2, spec 8.2).
4. **The armhf `debian:10` fallback is kept but unused.** zigbuild builds armv7 against glibc 2.28, so CI builds all five Linux and Alpine targets with it. `CLIPPINGS_BUILD_TOOL=cargo` in `build.sh` builds with plain cargo for the container fallback; the prototype proved it in Docker, where a plain bookworm build was correctly rejected by the glibc check at `GLIBC_2.34` (Tasks 2 and 10, spec 8.2).
5. **Every binary is probed, not only the Arm Linux ones**, and must report the extension's `PROTOCOL_VERSION`: Linux binaries in `debian:10`, which also proves the 2.28 floor at run time, Alpine binaries in `alpine:3.22`, the Arm ones under qemu, `darwin-arm64` natively and `darwin-x64` under Rosetta on the Apple silicon runner, `win32-x64` natively and `win32-arm64` on a `windows-11-arm` runner. `check.sh` also requires a `clippings::main` symbol and, on Linux, no `.debug_info` section (Task 10, spec 8.2).
6. **The package bundle is minified**: `pnpm build --production`, 1.4 MB with no source map, instead of the 2 MB development bundle and its 3 MB map. `package.mjs` refuses a development bundle, and `build.mjs` now clears `dist/` so a stale map cannot linger (Task 5, spec 8.2).
7. **`.vscodeignore` is unchanged.** It already whitelists `resources`, `package.json`, `LICENSE` and `README.md` besides `dist/**` and `bin/**`. vsce lets a negation win regardless of order, so a `dist/**/*.map` exclusion cannot sit beside `!dist/**`; the verifier rejects maps instead (Task 5, spec 8.2).
8. **Every package is verified after packaging**, more strictly than the spec asks: `vsix.mjs` reads the zip itself and requires exactly the whitelisted files, `bin/` holding exactly the server and `platform.ok`, the server's executable format and architecture from its ELF, Mach-O or PE header, mode 755 outside Windows, and the manifest's target platform, version and pre-release flag; the universal package must have no `bin/`. `platform.ok` holds the target name and a newline, which helps debugging (Task 5, spec 8.1 and 8.2).
9. **The Cargo workspace version must equal `package.json`'s**, so `clippings probe` reports the extension's version. `version.mjs` enforces it, and with `--tag` also requires the tag to be `v` plus the version (Tasks 4 and 12, spec 8.2).
10. **Pre-release parity**: an odd minor version is packaged with `--pre-release` and published with `--pre-release`; vsce accepts `--pre-release` with `--packagePath` only for a package built with it, which these are. The GitHub release is marked pre-release too (Tasks 4, 5 and 12, spec 8.2).
11. **Asset names**: `clippings-<target>-<version>.vsix`, `clippings-universal-<version>.vsix`, `clippings-darwin-<arch>.dSYM.zip`, `clippings-win32-<arch>.pdb` and `clippings-<linux|alpine>-<arch>.debug`, the last named by the binary's `.gnu_debuglink` (Tasks 2, 5 and 12).
12. **The flat-view directory label uses `/` on Windows**, like folder labels and node IDs. The one product change in this plan, found when the extension tests first ran on Windows (Task 6, spec 5.12 rendering).
13. **The Windows test fixes are test-side.** `.vscode-test.mjs` copies the fixture under the real path of the temporary directory, because on Windows it can be an 8.3 short name (`RUNNER~1`) and on macOS a symlink (`/var` to `/private/var`); tests compare node IDs and globs through `slashPath`, and the export test compares the home prefix case-insensitively. Path forms are covered on purpose by Task 8 instead (Task 6).
14. **The flaky macOS context-key test was a race in the test.** It applied the text filter `guide` and then looked up `lib`, which that filter hides; the filter command sets its context keys synchronously but the server rebuilds the tree asynchronously, so the lookup passed only while it ran first. On a slower runner it lost, and its leftover filter then hid the sub-tags the next test waited for. A forced 2-second wait reproduces both failures. The fix filters by `long note`, which keeps `lib` visible, and resets the filters in `finally`. Filters and context keys behave as designed (Task 1, spec 12.5).
15. **VS Code in CI is pinned to the `engines.vscode` floor, 1.91.0**, on Linux, macOS and Windows, with `.vscode-test` cached per OS, architecture and version, plus one Linux job on `stable`. The floor makes runs deterministic and cacheable and proves the declared minimum; the stable job catches upstream breakage. Local runs keep `stable` unless `CLIPPINGS_TEST_VSCODE` says otherwise (Task 7, spec 12.5).
16. **Path-form tests** open the fixture through a directory symlink on every OS, and through an NTFS junction and the 8.3 short path on Windows, each in its own VS Code profile. The short form is used only when `%~sI` returns a different path containing `~`; otherwise the suite is skipped with the reason. Scratch names stay short (`clp-<form>-`), because macOS limits VS Code's IPC socket path to 103 characters. No form failed, because the server echoes the URI it receives and its watcher rebases canonical event paths onto the configured root (Task 8, spec 12.5).
17. **CI builds the packages only on pushes to `main`** (the `dist` job), not on pull requests, because nine fat-LTO builds take far longer than the tests. The release workflow builds again from its tag (Task 9, spec 8.2).
18. **Dry-run semantics.** A `v*` tag push releases. A manual run releases only with `dry-run` off, and only from a tag: with `dry-run` off on a branch the `plan` job fails. Anything else is a dry run that builds, checks and keeps the packages as workflow artifacts. The release job runs `gh release create --verify-tag --generate-notes`, titled `Clippings X.Y.Z`, with `--prerelease` for odd minors; `concurrency` is per ref. `release.yml` needs no branch trigger: `gh workflow run release.yml --ref <branch>` works once the file is pushed (Task 12, spec 8.2).
19. **Marketplace publishing prefers Microsoft Entra ID.** The `plan` job outputs `marketplace` as `entra` when the repository variables `AZURE_CLIENT_ID` and `AZURE_TENANT_ID` are both set, `pat` when only the `VSCE_PAT` secret exists, else `none`, which skips the job. With `entra` the job runs `azure/login@v3` and `vsce publish --azure-credential`, in the `marketplace` environment with `id-token: write`, so the identity's federated credential has a stable subject. The PAT path is a fallback because Azure DevOps global personal access tokens are retired on 1 December 2026. The variables must be at repository level, because the `plan` job has no environment. Open VSX uses `npx ovsx@1.2.0` with an `OVSX_PAT` secret. Both publish with `--no-dependencies` and `--skip-duplicate` (Tasks 12 and 13, spec 8.2).
20. **Signing is optional and off by default.** macOS binaries are signed with a Developer ID and notarized by `scripts/dist/sign-macos.sh` when the `MACOS_CERTIFICATE_P12` secret exists; otherwise the step logs that the binary stays ad-hoc signed. Windows binaries are signed with Azure Artifact Signing in a separate `sign-windows` job, gated on the `WINDOWS_SIGNING_ENDPOINT` variable, in the `signing` environment for a stable federated-credential subject; the Arm64 probe and the packages tolerate it being skipped. Packages need no signing, because files VS Code extracts from a VSIX carry no macOS quarantine attribute and no Windows Mark-of-the-Web, so Gatekeeper and SmartScreen never inspect the server; signing matters for binaries downloaded from the GitHub release and for managed machines. Neither path has run with real credentials (Tasks 11 and 13, spec 8.2).
21. **The publisher stays the placeholder `clippings-dev`** (spec 7.1). `docs/release.md` lists every file to change when a real publisher exists (Task 13).
22. **Tool and action pins**: cargo-zigbuild 0.23.4 with ziglang 0.15.2 (pip in a venv), `alpine:3.22`, `debian:10`, mimalloc 0.1.52; action majors `checkout@v5`, `setup-node@v5`, `pnpm/action-setup@v4`, `cache@v4`, `upload-artifact@v4`, `download-artifact@v5`, `setup-qemu-action@v3`, `azure/login@v3` and `azure/artifact-signing-action@v2`. These majors were proven in CI; newer ones exist (checkout v7, upload-artifact v7, download-artifact v8) whose input changes were not validated (Tasks 7 and 9 to 12).
23. **Script tests use Node's own runner.** `pnpm test:scripts` runs `node --test "scripts/**/*.test.mjs"`, with no new dependency, and `pnpm test` includes it (Task 4).
24. **`clippings bench` and the first tilliX results move out of this plan** to a later benchmarks-and-performance plan (user decision, 2026-09-27). Milestone 4 no longer lists them; a new milestone 6, Benchmarks and performance, takes them with the final benchmark run and the view rebuild optimisation that section 13 assigned to "plan 4" (Task 14, spec 13 and 14).

---

### Task 1: Fix the context-key test race

The macOS extension job failed now and then in two context-key tests, which would make every CI run of this plan flaky (spec 12.5). The cause is in the test (ruling 14): `follow the filters` applied the text filter `guide`, which hides the `lib` folder, and then looked `lib` up. The filter command sets its context keys at once, but the server rebuilds the tree asynchronously, so the lookup passed only while it won that race; losing it also left the filter active, which hid the sub-tags the next test waits for. This task reproduces both failures deterministically, then fixes the test.

**Files:**
- Test: `extension/src/test/integration/contextKeys.test.ts` (modified)

**Interfaces:**
- None. The test's behaviour is unchanged; only its filter text and cleanup change.

- [ ] **Step 1: Reproduce the race**

Force the losing order: let the filtered tree land before the lookup, as a slow runner does.

Apply this diff to `extension/src/test/integration/contextKeys.test.ts` with `git apply`:

```diff
diff --git a/extension/src/test/integration/contextKeys.test.ts b/extension/src/test/integration/contextKeys.test.ts
index ca8db20..8feeb4e 100644
--- a/extension/src/test/integration/contextKeys.test.ts
+++ b/extension/src/test/integration/contextKeys.test.ts
@@ -49,6 +49,8 @@ describe('context keys', () => {
   it('follow the filters', async () => {
     api.test.prompts.script('guide');
     await vscode.commands.executeCommand('clippings.filter');
+    // Let the filtered tree land before the lookup, as a slow runner does.
+    await new Promise((resolve) => setTimeout(resolve, 2000));
     assert.equal(key('filtered'), true);
     assert.equal(key('global-filter-active'), 'guide');
     const lib = await itemAt(api, 'workspace', 'lib');
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `pnpm -C extension dev && pnpm -C extension typecheck && pnpm -C extension build && pnpm -C extension test:integration`
Expected: FAIL. `81 passing`, `2 failing`: `no tree item workspace > lib` and `timed out waiting for sub-tags`. A VS Code test window opens, runs the suite and closes.

- [ ] **Step 3: Fix the test**

Filter by `long note`, which keeps `lib` visible whichever way the race goes, and reset the filters in `finally` so a failure cannot leak into the next test. This replaces the whole file, which also removes the forced wait.

Replace `extension/src/test/integration/contextKeys.test.ts` with:

```ts
import * as assert from 'node:assert/strict';
import * as vscode from 'vscode';
import type { ClippingsApi } from '../../testApi';
import { DEFAULT_TREE } from './fixture';
import { getApi, itemAt, setSetting, treeBecomes, waitFor, whenIdle } from './helpers';

describe('context keys', () => {
  let api: ClippingsApi;
  const key = (name: string) => api.test.contextKeys()[`clippings-${name}`];

  before(async () => {
    api = await getApi();
    await whenIdle(api);
    await treeBecomes(api, DEFAULT_TREE);
  });

  after(async () => {
    await vscode.commands.executeCommand('clippings.resetCache');
    await treeBecomes(api, DEFAULT_TREE);
  });

  it('start from the settings and view state', () => {
    assert.equal(key('flat'), false);
    assert.equal(key('tags-only'), false);
    assert.equal(key('expanded'), false);
    assert.equal(key('collapsible'), true);
    assert.equal(key('filtered'), false);
    assert.equal(key('scan-mode'), 'workspace');
    assert.equal(key('show-reveal-button'), false);
    assert.equal(key('show-refresh-button'), true);
    assert.equal(key('can-toggle-compact-folders'), true);
    assert.equal(key('is-empty'), false);
  });

  it('follow the view commands', async () => {
    await vscode.commands.executeCommand('clippings.showTagsOnlyView');
    assert.equal(key('tags-only'), true);
    assert.equal(key('collapsible'), false);
    await vscode.commands.executeCommand('clippings.groupByTag');
    assert.equal(key('grouped-by-tag'), true);
    assert.equal(key('collapsible'), true);
    await vscode.commands.executeCommand('clippings.expand');
    assert.equal(key('expanded'), true);
    await vscode.commands.executeCommand('clippings.resetCache');
    assert.equal(key('tags-only'), false);
    assert.equal(key('grouped-by-tag'), false);
  });

  it('follow the filters', async () => {
    // The text filter must keep `lib` visible: the server applies it
    // asynchronously, so `lib` is looked up either before or after it lands.
    api.test.prompts.script('long note');
    try {
      await vscode.commands.executeCommand('clippings.filter');
      assert.equal(key('filtered'), true);
      assert.equal(key('global-filter-active'), 'long note');
      const lib = await itemAt(api, 'workspace', 'lib');
      await vscode.commands.executeCommand('clippings.excludeThisFolder', lib.id);
      assert.equal(key('folder-filter-active'), true);
    } finally {
      // A filter left behind would hide the sub-tags the next test waits for.
      await vscode.commands.executeCommand('clippings.resetAllFilters');
    }
    assert.equal(key('filtered'), false);
    assert.equal(key('folder-filter-active'), false);
  });

  it('follow settings and the server status', async () => {
    await setSetting('tree.trackFile', false);
    await setSetting('tree.buttons.export', true);
    try {
      assert.equal(key('show-reveal-button'), true);
      assert.equal(key('show-export-button'), true);
    } finally {
      await setSetting('tree.buttons.export', undefined);
      await setSetting('tree.trackFile', undefined);
    }
    assert.equal(key('show-reveal-button'), false);

    await setSetting('regex.subTagRegex', '^\\s*\\((\\w+)\\)');
    try {
      await waitFor('sub-tags', () => key('has-sub-tags') === true, [api.test.server.onStatus]);
    } finally {
      await setSetting('regex.subTagRegex', undefined);
    }
    await waitFor('no sub-tags', () => key('has-sub-tags') === false, [api.test.server.onStatus]);

    await setSetting('tree.hideTreeWhenEmpty', true);
    try {
      api.test.prompts.script('matches nothing at all');
      await vscode.commands.executeCommand('clippings.filter');
      await waitFor('an empty view', () => key('is-empty') === true, [api.test.server.onStatus]);
      await vscode.commands.executeCommand('clippings.filterClear');
      await waitFor('a view again', () => key('is-empty') === false, [api.test.server.onStatus]);
    } finally {
      await setSetting('tree.hideTreeWhenEmpty', undefined);
    }
  });
});
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `pnpm -C extension typecheck && pnpm -C extension build && pnpm -C extension test:integration`
Expected: `83 passing`. A VS Code test window opens, runs the suite and closes.

To prove the fix holds in the losing order too, you may re-add the 2-second wait after the `clippings.filter` command in the fixed test, see the suite pass, and remove it again.

- [ ] **Step 5: Commit**

```bash
git add -A
git commit -F - <<'EOF'
test(extension): pick a text filter that keeps the folder visible in the context key test

The test looked up the lib folder after applying the text filter "guide",
which hides it. The server applies a filter asynchronously, so the lookup
passed only when it won that race. On macOS CI it sometimes lost, and the
filter it left behind then hid the sub-tags the next test waited for.

Co-Authored-By: <model attribution>
Claude-Session: https://claude.ai/code/session_01EuAdHjqQeFrieAZqvsQ325
EOF
```

### Task 2: The dist profile, mimalloc on Windows and the per-target build script

Spec 8.2's release build: fat LTO, one codegen unit, `panic = "unwind"`, debug info kept as a separate asset, and mimalloc on Windows. A new `dist` profile inherits `release` and adds full debug info (ruling 1), and `scripts/dist/build.sh` builds one VS Code target (spec 8.1) with it, then moves the debug info into a dSYM, PDB or `.debug` file and strips only debug info from the binary, so panics still name their functions (ruling 2). Linux and Alpine targets build with cargo-zigbuild against glibc 2.28, or with plain cargo for the `debian:10` fallback (ruling 4). `target.sh` holds the target table the later scripts share.

**Files:**
- Create: `scripts/dist/target.sh`, `scripts/dist/build.sh`
- Modify: `Cargo.toml`, `crates/clippings/Cargo.toml`, `Cargo.lock` (diff), `crates/clippings/src/main.rs`, `.gitignore`

**Interfaces:**
- Produces: `[profile.dist]` in the root `Cargo.toml`.
- Produces: `scripts/dist/target.sh`, sourced, defining `resolve_target <vscode-target>`, which sets `triple` (the Rust triple), `glibc` (`2.28` for `linux-*`, empty otherwise) and `exe` (`clippings.exe` for `win32-*`, else `clippings`), and fails for an unknown target.
- Produces: `scripts/dist/build.sh <vscode-target> [out-dir]` (out-dir defaults to `dist`), writing `<out>/<target>/clippings[.exe]` (mode 755) and `<out>/symbols/clippings-<target>.dSYM.zip`, `.pdb` or `.debug`. Environment: `CLIPPINGS_BUILD_TOOL=cargo` for plain cargo on Linux, `OBJCOPY` to name llvm-objcopy. Tasks 9 and 10 call it from CI.

- [ ] **Step 1: Check the profile does not exist yet**

Run: `cargo build --locked --profile dist -p clippings`
Expected: FAIL with `is not defined`: cargo reports that profile `dist` is not defined.

- [ ] **Step 2: Add the profile, mimalloc and the scripts**

The `Cargo.lock` diff pins the versions the prototype resolved; see Global Constraints if it does not apply. `build.sh` must be executable (`chmod 755 scripts/dist/build.sh`); `target.sh` is only sourced.

Replace `Cargo.toml` with:

```toml
[workspace]
resolver = "2"
members = ["crates/clippings-core", "crates/clippings"]

[workspace.package]
version = "0.1.0"
edition = "2021"
license = "MIT"
rust-version = "1.85"

[workspace.dependencies]
clippings-core = { path = "crates/clippings-core" }
anyhow = "1"
chrono = { version = "0.4.45", default-features = false, features = ["clock", "std"] }
clap = { version = "4.6", features = ["derive"] }
crossbeam-channel = "0.5.17"
dunce = "1.0"
encoding_rs = "0.8.41"
fancy-regex = "0.19"
globset = "0.4.20"
grep-matcher = "0.1.9"
grep-regex = "0.1.14"
grep-searcher = "0.1.17"
ignore = "0.4.33"
insta = { version = "1.48", features = ["json"] }
lsp-server = "0.10"
memchr = "2.8"
mimalloc = { version = "0.1.52", default-features = false }
notify = "8.2"
regex = "1.13"
regex-syntax = "0.8.11"
serde = { version = "1", features = ["derive"] }
serde_json = { version = "1", features = ["preserve_order"] }
serde_path_to_error = "0.1"
tempfile = "3"
thiserror = "2"
tracing = "0.1"
tracing-subscriber = { version = "0.3", default-features = false, features = ["fmt", "std"] }

[profile.release]
lto = "fat"
codegen-units = 1
panic = "unwind"
strip = "symbols"

# The shipped binaries (spec 8.2): `release`, plus full debug info that
# `scripts/dist/build.sh` moves out of the binary into a separate symbols
# file (dSYM, PDB or `.debug`). The binary keeps its symbol table, so
# backtraces name functions.
[profile.dist]
inherits = "release"
debug = "full"
strip = "none"
```

Replace `crates/clippings/Cargo.toml` with:

```toml
[package]
name = "clippings"
version.workspace = true
edition.workspace = true
license.workspace = true
rust-version.workspace = true

[dependencies]
anyhow.workspace = true
clap.workspace = true
clippings-core.workspace = true
dunce.workspace = true
serde_json.workspace = true
tracing-subscriber.workspace = true

# rust-analyzer's choice: the Windows system allocator is slow under the
# many small allocations of a scan; elsewhere the system allocator is fine.
[target.'cfg(windows)'.dependencies]
mimalloc.workspace = true

[dev-dependencies]
insta.workspace = true
lsp-server.workspace = true
tempfile.workspace = true
```

Apply this diff to `Cargo.lock` with `git apply`:

```diff
diff --git a/Cargo.lock b/Cargo.lock
index 801f662..40cc492 100644
--- a/Cargo.lock
+++ b/Cargo.lock
@@ -197,6 +197,7 @@ dependencies = [
  "dunce",
  "insta",
  "lsp-server",
+ "mimalloc",
  "serde_json",
  "tempfile",
  "tracing-subscriber",
@@ -612,6 +613,15 @@ version = "0.2.189"
 source = "registry+https://github.com/rust-lang/crates.io-index"
 checksum = "3eaf3ede3fee6db1a4c2ee091bf8a8b4dccdc6d17f656fb07896ee72867612f2"
 
+[[package]]
+name = "libmimalloc-sys"
+version = "0.1.49"
+source = "registry+https://github.com/rust-lang/crates.io-index"
+checksum = "6a45a52f43e1c16f667ccfe4dd8c85b7f7c204fd5e3bf46c5b0db9a5c3c0b8e9"
+dependencies = [
+ "cc",
+]
+
 [[package]]
 name = "linux-raw-sys"
 version = "0.12.1"
@@ -652,6 +662,15 @@ dependencies = [
  "libc",
 ]
 
+[[package]]
+name = "mimalloc"
+version = "0.1.52"
+source = "registry+https://github.com/rust-lang/crates.io-index"
+checksum = "2d4139bb28d14ad1facf21d5eb8825051b326e172d216b39f6d31df53cc97862"
+dependencies = [
+ "libmimalloc-sys",
+]
+
 [[package]]
 name = "mio"
 version = "1.2.3"
```

Replace `crates/clippings/src/main.rs` with:

```rust
use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use clippings_core::config::CoreConfig;
use clippings_core::fs::NativeFs;
use clippings_core::report::scan_report;
use std::io::Write;
use std::path::PathBuf;
use std::sync::Arc;
use tracing_subscriber::filter::LevelFilter;

mod watch;

/// mimalloc on Windows, the system allocator elsewhere (spec 8.2).
#[cfg(windows)]
#[global_allocator]
static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;

/// Writes `line` to `out`, terminated with a newline. Standard Unix
/// behaviour for a pipe whose reader has gone away (`head`, a killed
/// watcher client): stop quietly with exit code 0 instead of reporting a
/// broken pipe as an error.
fn write_line(out: &mut impl Write, line: &str) -> Result<()> {
    if let Err(e) = writeln!(out, "{line}") {
        if e.kind() == std::io::ErrorKind::BrokenPipe {
            std::process::exit(0);
        }
        return Err(e.into());
    }
    Ok(())
}

/// Flushes `out`, with the same broken-pipe handling as [`write_line`].
fn flush(out: &mut impl Write) -> Result<()> {
    if let Err(e) = out.flush() {
        if e.kind() == std::io::ErrorKind::BrokenPipe {
            std::process::exit(0);
        }
        return Err(e.into());
    }
    Ok(())
}

#[derive(Parser)]
#[command(name = "clippings", version, about = "Fast TODO scanning for VS Code")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Scan directories and print the todos found.
    Scan {
        /// Directories to scan.
        #[arg(required = true)]
        roots: Vec<PathBuf>,
        /// JSON file with `clippings.*` settings in the extension's configuration shape.
        #[arg(long)]
        config: Option<PathBuf>,
        /// Include hidden files and directories.
        #[arg(long)]
        hidden: bool,
        /// Do not honour .gitignore, .ignore or .rgignore files.
        #[arg(long)]
        no_ignore: bool,
        /// Print a JSON report instead of one line per todo.
        #[arg(long)]
        json: bool,
    },
    /// Print version, target and protocol version as JSON, for the extension's binary check.
    Probe,
    /// Run the language server over stdin and stdout.
    Lsp,
    /// Scan, then print a JSON line for every file whose todos change.
    Watch {
        /// Directories to watch.
        #[arg(required = true)]
        roots: Vec<PathBuf>,
        /// JSON file with `clippings.*` settings in the extension's configuration shape.
        #[arg(long)]
        config: Option<PathBuf>,
        /// Include hidden files and directories.
        #[arg(long)]
        hidden: bool,
        /// Do not honour .gitignore, .ignore or .rgignore files.
        #[arg(long)]
        no_ignore: bool,
    },
}

fn load_config(config: Option<PathBuf>, hidden: bool, no_ignore: bool) -> Result<CoreConfig> {
    let mut cfg: CoreConfig = match config {
        Some(p) => serde_json::from_slice(
            &std::fs::read(&p).with_context(|| format!("reading {}", p.display()))?,
        )?,
        None => CoreConfig::default(),
    };
    cfg.include_hidden_files |= hidden;
    cfg.respect_ignore_files &= !no_ignore;
    Ok(cfg)
}

fn canonical(roots: &[PathBuf]) -> Result<Vec<PathBuf>> {
    roots
        .iter()
        .map(|r| dunce::canonicalize(r).with_context(|| format!("root {}", r.display())))
        .collect()
}

/// The level named by a `CLIPPINGS_LOG` value: `error`, `warn`, `info`,
/// `debug`, `trace` or `off`, case-insensitively. Anything else, including
/// an empty or whitespace-only value, is `info` — note that
/// `LevelFilter::from_str("")` returns `Ok(ERROR)`, not a default, so this
/// cannot delegate to `FromStr`.
fn parse_log_level(v: &str) -> LevelFilter {
    match v.trim().to_ascii_lowercase().as_str() {
        "error" => LevelFilter::ERROR,
        "warn" => LevelFilter::WARN,
        "info" => LevelFilter::INFO,
        "debug" => LevelFilter::DEBUG,
        "trace" => LevelFilter::TRACE,
        "off" => LevelFilter::OFF,
        _ => LevelFilter::INFO,
    }
}

/// Sends log records to stderr at the level named by `CLIPPINGS_LOG`
/// (`error`, `warn`, `info`, `debug`, `trace` or `off`), `info` by default.
/// The extension shows stderr in its output channel (spec 10.3); stdout
/// carries JSON-RPC only.
fn init_logging() {
    let level = std::env::var("CLIPPINGS_LOG")
        .map(|v| parse_log_level(&v))
        .unwrap_or(LevelFilter::INFO);
    tracing_subscriber::fmt()
        .with_writer(std::io::stderr)
        .with_max_level(level)
        .with_target(false)
        .init();
}

fn main() -> Result<()> {
    init_logging();
    match Cli::parse().command {
        Command::Scan {
            roots,
            config,
            hidden,
            no_ignore,
            json,
        } => {
            let cfg = load_config(config, hidden, no_ignore)?;
            let roots = canonical(&roots)?;
            let report = scan_report(&roots, &cfg, Arc::new(NativeFs))?;
            let mut out = std::io::stdout();
            if json {
                write_line(&mut out, &serde_json::to_string_pretty(&report)?)?;
            } else {
                for f in &report.files {
                    for t in &f.todos {
                        write_line(
                            &mut out,
                            &format!(
                                "{}:{}:{}: {} {}",
                                f.path,
                                t.start.line + 1,
                                t.start.character + 1,
                                t.tag,
                                t.after
                            ),
                        )?;
                    }
                }
            }
            flush(&mut out)?;
        }
        Command::Probe => {
            let info = serde_json::json!({
                "version": env!("CARGO_PKG_VERSION"),
                "target": format!("{}-{}", std::env::consts::ARCH, std::env::consts::OS),
                "protocolVersion": clippings_core::PROTOCOL_VERSION,
            });
            println!("{info}");
        }
        Command::Lsp => {
            clippings_core::server::main_loop::run_stdio().map_err(anyhow::Error::msg)?
        }
        Command::Watch {
            roots,
            config,
            hidden,
            no_ignore,
        } => watch::run(canonical(&roots)?, load_config(config, hidden, no_ignore)?)?,
    }
    Ok(())
}
```

Create `scripts/dist/target.sh`:

```bash
# Sourced by the other scripts in this directory. Maps a VS Code target
# (spec 8.1) to its Rust triple and build facts:
#
#   triple   the Rust target triple
#   glibc    the glibc floor for `-gnu` targets, empty otherwise
#   exe      the binary's file name
#
# Usage: resolve_target <vscode-target>

resolve_target() {
  glibc=""
  exe=clippings
  case "$1" in
    win32-x64) triple=x86_64-pc-windows-msvc exe=clippings.exe ;;
    win32-arm64) triple=aarch64-pc-windows-msvc exe=clippings.exe ;;
    linux-x64) triple=x86_64-unknown-linux-gnu glibc=2.28 ;;
    linux-arm64) triple=aarch64-unknown-linux-gnu glibc=2.28 ;;
    linux-armhf) triple=armv7-unknown-linux-gnueabihf glibc=2.28 ;;
    alpine-x64) triple=x86_64-unknown-linux-musl ;;
    alpine-arm64) triple=aarch64-unknown-linux-musl ;;
    darwin-x64) triple=x86_64-apple-darwin ;;
    darwin-arm64) triple=aarch64-apple-darwin ;;
    *)
      echo "unknown VS Code target: $1" >&2
      return 1
      ;;
  esac
}
```

Create `scripts/dist/build.sh`:

```bash
#!/usr/bin/env bash
# Builds the shipped server for one VS Code target (spec 8.1, 8.2) with the
# `dist` profile, then moves its debug info into a separate symbols file:
#
#   <out>/<target>/clippings[.exe]              the binary, without debug info
#   <out>/symbols/clippings-<target>.dSYM.zip   macOS
#   <out>/symbols/clippings-<target>.pdb        Windows
#   <out>/symbols/clippings-<target>.debug      Linux and Alpine
#
# macOS and Windows targets build with cargo on their own runners. Linux and
# Alpine targets build with cargo-zigbuild, using the glibc-suffixed target
# for the 2.28 floor; set CLIPPINGS_BUILD_TOOL=cargo to build a Linux target
# with plain cargo instead, as the debian:10 fallback does. Linux symbols are
# split with llvm-objcopy, found in the rustup `llvm-tools` component or on
# PATH, or named by $OBJCOPY.
#
# macOS and Linux binaries keep their symbol table, so a panic's backtrace
# (the client sets RUST_BACKTRACE=1) names its functions; file and line
# numbers need the symbols file. MSVC binaries never carry a symbol table:
# Windows backtraces name functions only when the PDB is beside the binary.
#
# Usage: scripts/dist/build.sh <vscode-target> [out-dir]   (out-dir: dist)
set -euo pipefail

here=$(cd "$(dirname "$0")" && pwd)
root=$(cd "$here/../.." && pwd)
# shellcheck source=target.sh
source "$here/target.sh"

target=${1:?usage: build.sh <vscode-target> [out-dir]}
out=${2:-dist}
resolve_target "$target"
mkdir -p "$out/$target" "$out/symbols"
out=$(cd "$out" && pwd)
built=$root/target/$triple/dist

cargo_args=(--locked --profile dist -p clippings --manifest-path "$root/Cargo.toml")

case "$target" in
  darwin-*)
    # rustc runs dsymutil on the linked binary, then strips its debug info.
    CARGO_PROFILE_DIST_SPLIT_DEBUGINFO=packed CARGO_PROFILE_DIST_STRIP=debuginfo \
      cargo build "${cargo_args[@]}" --target "$triple"
    cp "$built/$exe" "$out/$target/$exe"
    # target/<triple>/dist/clippings.dSYM is a symlink into deps/.
    staging=$(mktemp -d)
    cp -RL "$built/$exe.dSYM" "$staging/$exe.dSYM"
    rm -f "$out/symbols/clippings-$target.dSYM.zip"
    (cd "$staging" && ditto -c -k --norsrc --noextattr --noacl --keepParent "$exe.dSYM" "$out/symbols/clippings-$target.dSYM.zip")
    rm -rf "$staging"
    ;;
  win32-*)
    # MSVC always writes debug info to a PDB beside the binary, so the
    # binary carries none; `strip` would suppress the PDB instead.
    cargo build "${cargo_args[@]}" --target "$triple"
    cp "$built/$exe" "$out/$target/$exe"
    cp "$built/clippings.pdb" "$out/symbols/clippings-$target.pdb"
    ;;
  linux-* | alpine-*)
    if [[ ${CLIPPINGS_BUILD_TOOL:-zigbuild} == cargo ]]; then
      cargo build "${cargo_args[@]}" --target "$triple"
    else
      cargo zigbuild "${cargo_args[@]}" --target "$triple${glibc:+.$glibc}"
    fi
    if [[ -z ${OBJCOPY:-} ]]; then
      host=$(rustc -vV | sed -n 's/^host: //p')
      OBJCOPY=$(rustc --print sysroot)/lib/rustlib/$host/bin/llvm-objcopy
      [[ -x $OBJCOPY ]] || OBJCOPY=llvm-objcopy
    fi
    debug=$out/symbols/clippings-$target.debug
    cp "$built/$exe" "$out/$target/$exe"
    "$OBJCOPY" --only-keep-debug "$out/$target/$exe" "$debug"
    # The debug link names the symbols file by its base name, so gdb and
    # lldb find it when it sits beside the binary or in a debug directory.
    (cd "$out/symbols" && "$OBJCOPY" --strip-debug --add-gnu-debuglink="clippings-$target.debug" "$out/$target/$exe")
    ;;
esac

chmod 755 "$out/$target/$exe"
ls -l "$out/$target/$exe" "$out/symbols/clippings-$target".*
```

Replace `.gitignore` with:

```gitignore
/target
node_modules/
*.vsix
/extension/bin/
/extension/dist/
*.pending-snap
/.superpowers/
!/tests/fixtures/workspace/node_modules/
/dist/
```

Run: `chmod 755 scripts/dist/build.sh`
Expected: no output.

- [ ] **Step 3: Build this machine's target and inspect it**

Run: `scripts/dist/build.sh darwin-arm64`
Expected: cargo finishes the `profile [optimized + debuginfo]` build, and the script lists `dist/darwin-arm64/clippings` and `dist/symbols/clippings-darwin-arm64.dSYM.zip`.

Run: `nm dist/darwin-arm64/clippings | grep -c clippings4main && dist/darwin-arm64/clippings probe`
Expected: a count of at least `1` (the binary keeps clippings::main in its symbol table), then the probe JSON with `"protocolVersion":1`.

Run: `unzip -l dist/symbols/clippings-darwin-arm64.dSYM.zip`
Expected: the listing includes `clippings.dSYM/Contents/Resources/DWARF/clippings`.

`git status --short` must not list `dist/`.

- [ ] **Step 4: Run the Rust gates**

Run: `cargo fmt --all --check && cargo clippy --all-targets -- -D warnings && INSTA_UPDATE=no cargo test --all`
Expected: no diffs, no warnings, and every test passes (204 passed, 2 ignored).

- [ ] **Step 5: Commit**

```bash
git add -A
git commit -F - <<'EOF'
build: add the dist profile, Windows mimalloc and the per-target build script

Co-Authored-By: <model attribution>
Claude-Session: https://claude.ai/code/session_01EuAdHjqQeFrieAZqvsQ325
EOF
```

### Task 3: README, LICENSE and repository fields

`vsce package` prompts, or fails in CI, without a `repository` field, a README and a LICENSE, and the Marketplace page shows the README (spec 8.2). The README claims only what the spec provides. A manifest test pins the fields and files, so packaging never prompts.

**Files:**
- Create: `extension/README.md`, `extension/LICENSE`
- Modify: `extension/package.json` (diff)
- Test: `extension/src/test/unit/manifest.test.ts` (modified)

**Interfaces:**
- Produces: `repository` (`https://github.com/GeorgeIpsum/clippings.git`, directory `extension`), `homepage` and `bugs` in `extension/package.json`; `extension/README.md` and `extension/LICENSE` (MIT), which vsce packages as `readme.md` and `LICENSE.txt` (Task 5's verifier expects those names).

- [ ] **Step 1: Write the failing test**

Replace `extension/src/test/unit/manifest.test.ts` with:

```ts
import * as assert from 'node:assert/strict';
import { existsSync, readFileSync } from 'node:fs';
import { resolve } from 'node:path';

interface Command {
  command: string;
  title: string;
  category: string;
}
interface MenuItem {
  command: string;
  when?: string;
}
interface Setting {
  scope?: string;
  default?: unknown;
}
interface Manifest {
  publisher: string;
  license: string;
  repository: { type: string; url: string };
  scripts: Record<string, string>;
  engines: { vscode: string };
  extensionKind: string[];
  activationEvents: string[];
  capabilities: {
    untrustedWorkspaces: { supported: string; restrictedConfigurations: string[] };
    virtualWorkspaces: boolean;
  };
  contributes: {
    commands: Command[];
    menus: Record<string, MenuItem[]>;
    configuration: { title: string; properties: Record<string, Setting> }[];
    views: Record<string, { id: string; when: string }[]>;
  };
}

// Tests run from out/test/unit; the manifest sits three levels up.
const manifest = JSON.parse(readFileSync(resolve(__dirname, '../../../package.json'), 'utf8')) as Manifest;
const settings = Object.assign({}, ...manifest.contributes.configuration.map((g) => g.properties)) as Record<
  string,
  Setting
>;

describe('manifest', () => {
  it('declares the platform facts from spec 7.1', () => {
    assert.equal(manifest.engines.vscode, '^1.91.0');
    assert.deepEqual(manifest.extensionKind, ['workspace']);
    assert.deepEqual(manifest.activationEvents, ['onStartupFinished']);
    assert.equal(manifest.capabilities.untrustedWorkspaces.supported, 'limited');
    assert.deepEqual(manifest.capabilities.untrustedWorkspaces.restrictedConfigurations, ['clippings.server.path']);
    assert.equal(manifest.capabilities.virtualWorkspaces, false);
    assert.deepEqual(manifest.contributes.views['clippings'], [
      { id: 'clippings-view', name: 'TODOs', when: '!clippings-is-empty' },
    ]);
  });

  it('has what vsce and the Marketplace ask for, so packaging never prompts', () => {
    assert.equal(manifest.publisher, 'clippings-dev');
    assert.equal(manifest.license, 'MIT');
    assert.equal(manifest.repository.type, 'git');
    assert.match(manifest.repository.url, /^https:\/\/github\.com\//);
    for (const file of ['README.md', 'LICENSE']) {
      assert.ok(existsSync(resolve(__dirname, '../../..', file)), `extension/${file}`);
    }
  });

  it('declares 65 settings, all window scoped except server.path', () => {
    const keys = Object.keys(settings);
    assert.equal(keys.length, 65);
    for (const key of keys) {
      assert.ok(key.startsWith('clippings.'), key);
      const expected = key === 'clippings.server.path' ? 'machine-overridable' : 'window';
      assert.equal(settings[key]?.scope, expected, key);
    }
    assert.equal((settings['clippings.filtering.builtInExcludes']?.default as string[]).length, 25);
  });

  it('declares 37 commands in the Clippings category', () => {
    const commands = manifest.contributes.commands;
    assert.equal(commands.length, 37);
    assert.equal(new Set(commands.map((c) => c.command)).size, 37);
    for (const c of commands) {
      assert.ok(c.command.startsWith('clippings.'), c.command);
      assert.equal(c.category, 'Clippings', c.command);
    }
  });

  it('menus reference declared commands and the clippings view only', () => {
    const declared = new Set(manifest.contributes.commands.map((c) => c.command));
    for (const [menu, items] of Object.entries(manifest.contributes.menus)) {
      for (const item of items) {
        assert.ok(declared.has(item.command), `${menu}: ${item.command}`);
        assert.ok(!item.when?.includes('todo-tree'), `${menu}: ${item.when}`);
        if (menu !== 'commandPalette') assert.match(item.when ?? '', /^view == clippings-view( && |$)/);
      }
    }
  });

  it('guards every disjunct of a menu when clause with the view', () => {
    for (const items of Object.values(manifest.contributes.menus)) {
      for (const item of items) {
        const when = item.when ?? '';
        // An `||` outside parentheses would escape the `view ==` guard.
        let depth = 0;
        for (let i = 0; i < when.length; i++) {
          if (when[i] === '(') depth++;
          if (when[i] === ')') depth--;
          assert.ok(!(depth === 0 && when.startsWith('||', i)), when);
        }
      }
    }
  });
});
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `pnpm -C extension build && pnpm -C extension test:unit`
Expected: FAIL. `87 passing`, `1 failing`: `has what vsce and the Marketplace ask for, so packaging never prompts` with `Cannot read properties of undefined (reading 'type')`.

- [ ] **Step 3: Add the fields and files**

Create `extension/README.md`:

```markdown
# Clippings

Clippings is a fast alternative to [Todo Tree](https://github.com/Gruntfuggly/todo-tree). It finds `TODO`, `FIXME` and other tags in your workspace, shows them in a tree and highlights them in the editor, with Todo Tree's features and settings.

A Rust language server does the scanning, indexing and tree building in its own process, so typing, saving and rescanning stay off the editor's extension host even in large monorepos. The server ships inside the extension: there is nothing else to install.

## Features

- A **TODOs** view in the activity bar, as a tree of folders and files, a flat list of files, or tags only. Group by tag or sub-tag, filter by text, and hide or show only a folder.
- **Highlights** for tags in the editor, with custom colours, icons, gutter icons and overview ruler marks per tag.
- A **status bar** count: the total, per tag, the top three tags, or the current file.
- **Scan modes**: the whole workspace and open files, the workspace only, open files only, or the current file only.
- **File watching**: changes on disk update the tree as they happen, including changes made outside the editor.
- **Never-index list**: build output and dependency folders such as `node_modules`, `target` and `.next` are never scanned (`clippings.filtering.builtInExcludes`).
- Go to next and previous todo, reveal the current file in the tree, and export the tree to text or JSON.
- Multi-root workspaces, Remote-SSH, WSL and dev containers: the server runs where the files are.

## Coming from Todo Tree

Settings live under `clippings.*` with Todo Tree's names and defaults below the prefix, so `todo-tree.general.tags` becomes `clippings.general.tags`. When Clippings finds Todo Tree settings it offers to import them; run **Clippings: Import Settings from Todo Tree** to import them at any time.

Todo Tree's `ripgrep.*` settings have no equivalent, because Clippings does not use ripgrep, and `general.debug` is replaced by `clippings.server.logLevel`. Disable Todo Tree while you use Clippings, or both will highlight the same tags.

## Settings

The most used settings:

| Setting | Default | Meaning |
|---|---|---|
| `clippings.general.tags` | `BUG`, `HACK`, `FIXME`, `TODO`, `XXX`, `[ ]`, `[x]` | the tags to find |
| `clippings.regex.regex` | comment prefixes followed by `($TAGS)` | the regular expression that finds a tag |
| `clippings.highlights.customHighlight` | icons per tag | colours, icons and highlight type per tag |
| `clippings.filtering.excludeGlobs` | `**/node_modules/*/**` | globs of files to leave out |
| `clippings.filtering.builtInExcludes` | 25 build and dependency folders | folder names never scanned |
| `clippings.tree.scanMode` | `workspace` | which files to scan |
| `clippings.general.statusBar` | `none` | what the status bar shows |
| `clippings.server.path` | empty | a server binary to use instead of the bundled one |
| `clippings.server.logLevel` | `info` | the server's log level in the Clippings output channel |

Every setting is described in the Settings editor under **Extensions > Clippings**.

## Platforms

The extension is published per platform, each with its own server: Windows x64 and Arm64, Linux x64, Arm64 and Armhf (glibc 2.28 or later), Alpine Linux x64 and Arm64, and macOS Intel and Apple silicon. VS Code installs the right one.

A universal package without a server also exists. It works only when `clippings.server.path` points at a server you built yourself from the [Clippings repository](https://github.com/GeorgeIpsum/clippings) with `cargo build --release -p clippings`.

## Troubleshooting

Run **Clippings: Show Log** to open the Clippings output channel, and **Clippings: Restart Server** to restart the server. Set `clippings.server.logLevel` to `debug` for more detail.

## License

MIT
```

Create `extension/LICENSE`:

```text
MIT License

Copyright (c) 2026 Ibrahim Saberi

Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the "Software"), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all
copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
SOFTWARE.
```

Apply this diff to `extension/package.json` with `git apply`:

```diff
diff --git a/extension/package.json b/extension/package.json
index f4eef49..ae4f8aa 100644
--- a/extension/package.json
+++ b/extension/package.json
@@ -5,6 +5,15 @@
   "version": "0.1.0",
   "publisher": "clippings-dev",
   "license": "MIT",
+  "repository": {
+    "type": "git",
+    "url": "https://github.com/GeorgeIpsum/clippings.git",
+    "directory": "extension"
+  },
+  "homepage": "https://github.com/GeorgeIpsum/clippings/tree/main/extension#readme",
+  "bugs": {
+    "url": "https://github.com/GeorgeIpsum/clippings/issues"
+  },
   "private": true,
   "categories": [
     "Other"
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `pnpm -C extension typecheck && pnpm -C extension build && pnpm -C extension test:unit`
Expected: `88 passing`.

- [ ] **Step 5: Commit**

```bash
git add -A
git commit -F - <<'EOF'
feat(extension): add the README, LICENSE and repository fields packaging needs

Co-Authored-By: <model attribution>
Claude-Session: https://claude.ai/code/session_01EuAdHjqQeFrieAZqvsQ325
EOF
```

### Task 4: Release channel from the version

Spec 8.2: versions are plain `major.minor.patch`, odd minor versions are pre-releases and even ones stable. `version.mjs` decides the channel, checks that the Cargo workspace version equals the extension's (ruling 9), and with `--tag` that the tag is `v` plus the version (Review Focus 3 and 4). It prints `version=` and `pre-release=` and appends them to `$GITHUB_OUTPUT` for the workflows. The script tests run under `node:test` through a new `test:scripts` script, which `pnpm test` includes (ruling 23).

**Files:**
- Create: `extension/scripts/version.mjs`
- Modify: `extension/package.json` (diff)
- Test: `extension/scripts/version.test.mjs`

**Interfaces:**
- Produces, in `extension/scripts/version.mjs`: `parseVersion(version: string): { major: number; minor: number; patch: number }` (throws `... is not plain major.minor.patch`), `isPreRelease(version: string): boolean`, `checkTag(tag: string, version: string): void` (throws `tag <tag> does not match package.json version <version>`), `cargoWorkspaceVersion(toml: string): string`. Task 5 imports `isPreRelease`.
- Produces: the CLI `node extension/scripts/version.mjs [--tag <tag>]`, which prints `version=<x.y.z>` and `pre-release=<true|false>`, appends both to `$GITHUB_OUTPUT` when set, and exits 1 with `error: <reason>`. Tasks 9 and 12 run it.
- Produces: the `test:scripts` script in `extension/package.json`.

- [ ] **Step 1: Write the failing tests**

Create `extension/scripts/version.test.mjs`:

```js
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
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `node --test extension/scripts/version.test.mjs`
Expected: FAIL with `ERR_MODULE_NOT_FOUND` for `scripts/version.mjs'`.

- [ ] **Step 3: Write the script and the test script entry**

Create `extension/scripts/version.mjs`:

```js
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
```

Apply this diff to `extension/package.json` with `git apply`:

```diff
diff --git a/extension/package.json b/extension/package.json
index ae4f8aa..fb91626 100644
--- a/extension/package.json
+++ b/extension/package.json
@@ -1301,8 +1301,9 @@
     "typecheck": "tsc --noEmit -p .",
     "dev": "cargo build --manifest-path ../Cargo.toml -p clippings",
     "test:unit": "mocha --ui bdd \"out/test/unit/**/*.test.js\"",
+    "test:scripts": "node --test \"scripts/**/*.test.mjs\"",
     "test:integration": "vscode-test",
-    "test": "pnpm dev && pnpm typecheck && pnpm build && pnpm test:unit && pnpm test:integration",
+    "test": "pnpm dev && pnpm typecheck && pnpm build && pnpm test:unit && pnpm test:scripts && pnpm test:integration",
     "package": "vsce package --no-dependencies"
   },
   "packageManager": "pnpm@10.27.0",
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `pnpm -C extension test:scripts`
Expected: 5 tests pass: `pass 5`, `fail 0`.

Run: `node extension/scripts/version.mjs`
Expected: `version=0.1.0` and `pre-release=true`.

Run: `node extension/scripts/version.mjs --tag v0.2.0`
Expected: FAIL with `error: tag v0.2.0 does not match package.json version 0.1.0`.

- [ ] **Step 5: Commit**

```bash
git add -A
git commit -F - <<'EOF'
feat(extension): decide the release channel from the version, with tests

Co-Authored-By: <model attribution>
Claude-Session: https://claude.ai/code/session_01EuAdHjqQeFrieAZqvsQ325
EOF
```

### Task 5: Package and verify a VSIX per target and a universal VSIX

Spec 8.1 and 8.2: each target's package carries its server and `platform.ok` in `extension/bin/`, built by `vsce package --no-dependencies --target <target>` from one bundle, and the universal package carries neither. `package.mjs` stages `bin/`, runs vsce (with `--pre-release` for odd minors), verifies the result and removes `bin/` again, so local development still resolves the server through `CLIPPINGS_SERVER_PATH`. `vsix.mjs` reads the zip itself and checks what VS Code will see (ruling 8, Review Focus 2 and 4). The bundle is the minified production build (ruling 6), and `.vscodeignore` stays as it is (ruling 7).

**Files:**
- Create: `extension/scripts/vsix.mjs`, `extension/scripts/package.mjs`
- Modify: `extension/build.mjs`, `extension/package.json` (diff)
- Test: `extension/scripts/vsix.test.mjs`

**Interfaces:**
- Consumes: `isPreRelease` from `extension/scripts/version.mjs` (Task 4).
- Produces, in `extension/scripts/vsix.mjs`: `TARGETS` (the nine VS Code targets of spec 8.1), `serverName(target: string): string`, `readZip(buffer: Buffer): { name: string; mode: number; size: number; read(): Buffer }[]`, `binaryKind(bytes: Buffer): string` (`elf-x64`, `macho-arm64`, `pe-arm64`, ... or `unknown`), `expectedKind(target: string): string`, and `checkPackage(entries, { target, preRelease, version }): string[]`, empty for a good package, where `target` is undefined for the universal package.
- Produces: the CLI `node extension/scripts/vsix.mjs <vsix> (--target <vscode-target> | --universal) [--pre-release true|false]`, printing `<vsix>: ok (...)` or exiting 1 with each problem.
- Produces, in `extension/scripts/package.mjs`: `packageName(name, target, version): string` (`clippings-<target or universal>-<version>.vsix`) and `vsceArgs({ target, preRelease, out }): string[]`; the CLI `node extension/scripts/package.mjs --target <vscode-target> --server <binary> [--out <dir>]` or `--universal [--out <dir>]`, with `--out` defaulting to `dist/vsix` at the repository root, printing the package's path. `pnpm -C extension package` runs it. Task 9's package job calls it for all ten packages.
- Produces: `node build.mjs --production` (`pnpm -C extension build --production`) writes only a minified `dist/extension.js`, and every build first clears `dist/` and `out/`.

- [ ] **Step 1: Write the failing tests**

Create `extension/scripts/vsix.test.mjs`:

```js
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
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `pnpm -C extension test:scripts`
Expected: FAIL. `pass 5`, `fail 1`: `vsix.test.mjs` fails with `ERR_MODULE_NOT_FOUND` for `scripts/package.mjs'`.

- [ ] **Step 3: Write the verifier, the packager and the build change**

Create `extension/scripts/vsix.mjs`:

```js
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
```

Create `extension/scripts/package.mjs`:

```js
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
```

Replace `extension/build.mjs` with:

```js
// Bundles the extension into dist/ and the tests into out/test/.
// esbuild only transpiles; `pnpm typecheck` runs the type checker.
import * as esbuild from 'esbuild';
import { readdirSync, rmSync, statSync } from 'node:fs';
import { join } from 'node:path';

const watch = process.argv.includes('--watch');
const production = process.argv.includes('--production');

function testEntries(dir) {
  return readdirSync(dir).flatMap((name) => {
    const path = join(dir, name);
    if (statSync(path).isDirectory()) return testEntries(path);
    return name.endsWith('.test.ts') || name === 'index.ts' ? [path] : [];
  });
}

const common = {
  bundle: true,
  platform: 'node',
  format: 'cjs',
  target: 'node20',
  external: ['vscode', 'mocha'],
  sourcemap: !production,
  minify: production,
  logLevel: 'info',
};

// A stale source map from a development build must not reach a package.
rmSync('dist', { recursive: true, force: true });
rmSync('out', { recursive: true, force: true });
const contexts = [
  await esbuild.context({ ...common, entryPoints: ['src/extension.ts'], outfile: 'dist/extension.js' }),
];
if (!production) {
  contexts.push(
    await esbuild.context({
      ...common,
      entryPoints: testEntries('src/test'),
      outdir: 'out/test',
      outbase: 'src/test',
    }),
  );
}

if (watch) {
  await Promise.all(contexts.map((c) => c.watch()));
} else {
  await Promise.all(contexts.map((c) => c.rebuild()));
  await Promise.all(contexts.map((c) => c.dispose()));
}
```

Apply this diff to `extension/package.json` with `git apply`:

```diff
diff --git a/extension/package.json b/extension/package.json
index fb91626..6aab4c4 100644
--- a/extension/package.json
+++ b/extension/package.json
@@ -1304,7 +1304,7 @@
     "test:scripts": "node --test \"scripts/**/*.test.mjs\"",
     "test:integration": "vscode-test",
     "test": "pnpm dev && pnpm typecheck && pnpm build && pnpm test:unit && pnpm test:scripts && pnpm test:integration",
-    "package": "vsce package --no-dependencies"
+    "package": "node scripts/package.mjs"
   },
   "packageManager": "pnpm@10.27.0",
   "dependencies": {
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `pnpm -C extension test:scripts`
Expected: 14 tests pass: `pass 14`, `fail 0`.

- [ ] **Step 5: Package this machine's target and the universal package**

The debug server is enough to exercise packaging; the release packages use `build.sh`'s binaries.

Run: `pnpm -C extension dev && pnpm -C extension build --production && node extension/scripts/package.mjs --target darwin-arm64 --server target/debug/clippings && node extension/scripts/package.mjs --universal`
Expected: vsce reports `DONE  Packaged:` twice, and the script prints the paths of `clippings-darwin-arm64-0.1.0.vsix` and `clippings-universal-0.1.0.vsix` under `dist/vsix/`.

Run: `node extension/scripts/vsix.mjs dist/vsix/clippings-darwin-arm64-0.1.0.vsix --target darwin-arm64 && node extension/scripts/vsix.mjs dist/vsix/clippings-universal-0.1.0.vsix --universal && test ! -e extension/bin`
Expected: `ok (darwin-arm64, pre-release)` with `bin/clippings` at `mode 755`, then `ok (universal, pre-release)`, and extension/bin is gone.

Check that a package for the wrong target is caught (Review Focus 2):

Run: `node extension/scripts/vsix.mjs dist/vsix/clippings-darwin-arm64-0.1.0.vsix --target linux-x64`
Expected: FAIL with `manifest target platform is darwin-arm64, expected linux-x64` and `bin/clippings is macho-arm64, expected elf-x64`.

Check that a development bundle is refused, which also puts the development build back for the unit tests:

Run: `pnpm -C extension build && node extension/scripts/package.mjs --universal`
Expected: FAIL with `dist/ holds a development bundle: run pnpm build --production first`.

Run: `pnpm -C extension test:unit`
Expected: `88 passing`.

- [ ] **Step 6: Commit**

```bash
git add -A
git commit -F - <<'EOF'
feat(extension): package and verify a VSIX per target and a universal VSIX

Co-Authored-By: <model attribution>
Claude-Session: https://claude.ai/code/session_01EuAdHjqQeFrieAZqvsQ325
EOF
```

### Task 6: Pass the extension tests on Windows, with slash-separated flat view directories

Task 7 runs the extension tests on Windows for the first time (spec 12.5), where they fail in three ways. The tests built expected paths with `/` while node IDs, globs and `path.join` disagree, and compared the export path's home prefix case-sensitively; the fixture lived under the 8.3 short temporary directory (`RUNNER~1`); and the flat view's directory label, `strings.py (src\util)`, used backslashes, unlike folder labels and node IDs, which spec 5.12 renders with `/`. The label is fixed in Rust (ruling 12); the rest is test-side (ruling 13): `workspacePath` now uses the platform's separators, `slashPath` gives the form node IDs and globs use, and `.vscode-test.mjs` copies the fixture under the real temporary path. Path forms get their own suite in Task 8.

**Files:**
- Modify: `crates/clippings-core/src/view/place.rs`, `extension/.vscode-test.mjs`
- Test: `extension/src/test/integration/helpers.ts` (modified), `extension/src/test/integration/export.test.ts` (modified), `extension/src/test/integration/filters.test.ts` (modified), `extension/src/test/integration/reveal.test.ts` (modified), `extension/src/test/unit/serverResolution.test.ts` (modified)

**Interfaces:**
- Produces, in `test/integration/helpers.ts`: `workspacePath(...parts: string[]): string` now joins with the platform's separators; new `slashPath(...parts: string[]): string`, the same path with `/` on Windows, as node IDs and filter globs spell it. Task 8's path-form suite uses `workspacePath`.
- Produces: flat-view file labels `<name> (<dir>)` with `/` separators on every platform.

- [ ] **Step 1: Update the tests**

The Windows failures cannot be reproduced on macOS or Linux, where both separators are `/`; Task 7's Windows CI job is their proof. Here the changed tests must keep passing.

Replace `extension/src/test/integration/helpers.ts` with:

```ts
// Shared helpers for the integration tests. Waits are driven by real
// signals from the extension, never by fixed sleeps.

import { join } from 'node:path';
import * as vscode from 'vscode';
import type { ClippingsApi } from '../../testApi';
import type { TestItem } from '../../tree/testItems';

export async function getApi(): Promise<ClippingsApi> {
  const ext = vscode.extensions.getExtension<ClippingsApi>('clippings-dev.clippings');
  if (!ext) throw new Error('extension not installed');
  return ext.activate();
}

/**
 * Resolves with the first truthy value of `check`, evaluated now and again
 * after each event from `events`.
 */
export function waitFor<T>(
  what: string,
  check: () => T | Promise<T>,
  events: vscode.Event<unknown>[],
  timeoutMs = 15_000,
): Promise<NonNullable<T>> {
  return new Promise((resolve, reject) => {
    let done = false;
    let running = false;
    let again = false;
    const subscriptions: vscode.Disposable[] = [];
    const finish = (error: Error | undefined, value?: NonNullable<T>) => {
      if (done) return;
      done = true;
      clearTimeout(timer);
      subscriptions.forEach((s) => s.dispose());
      if (error) reject(error);
      else resolve(value as NonNullable<T>);
    };
    const timer = setTimeout(() => finish(new Error(`timed out waiting for ${what}`)), timeoutMs);
    const evaluate = async (): Promise<void> => {
      if (running) {
        again = true;
        return;
      }
      running = true;
      try {
        do {
          again = false;
          const value = await check();
          if (value) return finish(undefined, value as NonNullable<T>);
        } while (again && !done);
      } catch (err) {
        finish(err instanceof Error ? err : new Error(String(err)));
      } finally {
        running = false;
      }
    };
    for (const event of events) subscriptions.push(event(() => void evaluate()));
    void evaluate();
  });
}

/** Resolves like `work`, or rejects if it takes longer than `timeoutMs`. */
export function withTimeout<T>(what: string, work: Thenable<T>, timeoutMs = 20_000): Promise<T> {
  return new Promise((resolve, reject) => {
    const timer = setTimeout(() => reject(new Error(`timed out waiting for ${what}`)), timeoutMs);
    work.then(
      (value) => (clearTimeout(timer), resolve(value)),
      (err: unknown) => (clearTimeout(timer), reject(err instanceof Error ? err : new Error(String(err)))),
    );
  });
}

/** Waits until the server is running and has finished scanning. */
export async function whenIdle(api: ClippingsApi): Promise<void> {
  const s = api.test.server;
  await waitFor('an idle server', () => s.running && s.status()?.scanning === false, [s.onStatus, s.onRunning]);
}

/** A path in the test workspace, with the platform's separators. */
export function workspacePath(...parts: string[]): string {
  const root = process.env['CLIPPINGS_TEST_WORKSPACE'];
  if (!root) throw new Error('CLIPPINGS_TEST_WORKSPACE is not set');
  return join(root, ...parts);
}

/** A workspace path with `/` separators, as node IDs and filter globs spell it. */
export function slashPath(...parts: string[]): string {
  const path = workspacePath(...parts);
  return process.platform === 'win32' ? path.replaceAll('\\', '/') : path;
}

/**
 * The tree as indented lines, `label` or `label  description` for status
 * nodes, walking every node with children down to `depth` levels.
 */
export async function outline(api: ClippingsApi, depth = 10, parent?: string, indent = ''): Promise<string[]> {
  const lines: string[] = [];
  for (const item of await api.test.tree.items(parent)) {
    lines.push(indent + (item.label || `(${item.description ?? ''})`));
    if (item.state !== 'none' && depth > 1) lines.push(...(await outline(api, depth - 1, item.id, indent + '  ')));
  }
  return lines;
}

/** Waits until the tree's outline equals `expected`. */
export async function treeBecomes(api: ClippingsApi, expected: string[], depth = 10): Promise<void> {
  let last: string[] = [];
  try {
    await waitFor(
      'the expected tree',
      async () => {
        last = await outline(api, depth);
        return last.join('\n') === expected.join('\n');
      },
      [api.test.tree.onDidChange, api.test.server.onStatus],
    );
  } catch (err) {
    throw new Error(`${(err as Error).message}\nexpected:\n${expected.join('\n')}\nactual:\n${last.join('\n')}`);
  }
}

/** Finds a tree item by the labels on the path to it. */
export async function itemAt(api: ClippingsApi, ...labels: string[]): Promise<TestItem> {
  let parent: string | undefined;
  let found: TestItem | undefined;
  for (const label of labels) {
    found = (await api.test.tree.items(parent)).find((i) => i.label === label);
    if (!found) throw new Error(`no tree item ${labels.join(' > ')}`);
    parent = found.id;
  }
  if (!found) throw new Error('no labels');
  return found;
}

/** Updates a `clippings.*` setting and waits until VS Code reports the change. */
export async function setSetting(
  key: string,
  value: unknown,
  target = vscode.ConfigurationTarget.Global,
): Promise<void> {
  const changed = new Promise<void>((resolve) => {
    const sub = vscode.workspace.onDidChangeConfiguration((e) => {
      if (e.affectsConfiguration(`clippings.${key}`)) {
        sub.dispose();
        resolve();
      }
    });
  });
  const current = vscode.workspace.getConfiguration('clippings').inspect(key);
  const existing =
    target === vscode.ConfigurationTarget.Global ? current?.globalValue : current?.workspaceValue;
  if (JSON.stringify(existing) === JSON.stringify(value)) return;
  await vscode.workspace.getConfiguration('clippings').update(key, value, target);
  await changed;
}

/** Resolves with the next event that matches, or rejects after a timeout. */
export function nextEvent<T>(
  what: string,
  event: vscode.Event<T>,
  matches: (e: T) => boolean = () => true,
  timeoutMs = 15_000,
): Promise<T> {
  return new Promise((resolve, reject) => {
    const timer = setTimeout(() => {
      sub.dispose();
      reject(new Error(`timed out waiting for ${what}`));
    }, timeoutMs);
    const sub = event((e) => {
      if (!matches(e)) return;
      clearTimeout(timer);
      sub.dispose();
      resolve(e);
    });
  });
}
```

Replace `extension/src/test/integration/export.test.ts` with:

```ts
import * as assert from 'node:assert/strict';
import { homedir } from 'node:os';
import * as vscode from 'vscode';
import { ExportDocuments, exportUri } from '../../export/documents';
import type { ClippingsApi } from '../../testApi';
import { DEFAULT_TREE } from './fixture';
import { getApi, setSetting, treeBecomes, whenIdle } from './helpers';

async function exported(): Promise<vscode.TextDocument> {
  await vscode.commands.executeCommand('clippings.exportTree');
  const editor = vscode.window.activeTextEditor;
  assert.ok(editor, 'an export editor');
  assert.equal(editor.document.uri.scheme, 'clippings-export');
  return editor.document;
}

describe('export', () => {
  let api: ClippingsApi;

  before(async () => {
    api = await getApi();
    await whenIdle(api);
    await treeBecomes(api, DEFAULT_TREE);
  });

  afterEach(async () => {
    await vscode.commands.executeCommand('workbench.action.closeAllEditors');
  });

  it('opens the visible tree as a text tree named by the formatted export path', async () => {
    const document = await exported();
    const path = document.uri.path;
    // On Windows the URI path is `/c:/Users/...` for a home of `C:\Users\...`.
    const home = homedir().replaceAll('\\', '/').replace(/^(?!\/)/, '/');
    assert.ok(path.toLowerCase().startsWith(home.toLowerCase()), path);
    assert.match(path, /\/todo-tree-\d{8}-\d{4}\.txt$/);
    const text = document.getText();
    assert.match(text, /^└─ workspace\n/);
    assert.ok(text.includes('line 2: TODO (alice) wire up the router'), text);
    assert.ok(!text.includes('Scan mode'), 'status nodes are not exported');
  });

  it('releases a closed export document\'s stored content, keeping others', () => {
    const documents = new ExportDocuments();
    const closedUri = exportUri('/tmp/todo-tree-closed.txt');
    const otherUri = exportUri('/tmp/todo-tree-still-open.txt');
    documents.set(closedUri, 'closed content');
    documents.set(otherUri, 'still open content');
    documents.release(closedUri);
    assert.equal(documents.provideTextDocumentContent(closedUri), '', 'the closed document’s content is dropped');
    assert.equal(
      documents.provideTextDocumentContent(otherUri),
      'still open content',
      'an unrelated document keeps its content',
    );
  });

  it('exports JSON when the path ends in .json', async () => {
    await setSetting('general.exportPath', '${HOME}/clippings-export.json');
    try {
      const document = await exported();
      assert.equal(document.uri.path.endsWith('/clippings-export.json'), true);
      assert.equal(document.languageId, 'json');
      const tree = JSON.parse(document.getText()) as Record<string, Record<string, unknown>>;
      assert.deepEqual(Object.keys(tree), ['workspace']);
      assert.deepEqual(Object.keys(tree['workspace'] ?? {}), ['docs', 'lib', 'src']);
    } finally {
      await setSetting('general.exportPath', undefined);
    }
  });
});
```

Replace `extension/src/test/integration/filters.test.ts` with:

```ts
import * as assert from 'node:assert/strict';
import { mkdirSync, rmSync, writeFileSync } from 'node:fs';
import * as vscode from 'vscode';
import type { ClippingsApi } from '../../testApi';
import { DEFAULT_TREE } from './fixture';
import { getApi, itemAt, setSetting, treeBecomes, whenIdle, slashPath, workspacePath } from './helpers';

describe('filters', () => {
  let api: ClippingsApi;

  before(async () => {
    api = await getApi();
    await whenIdle(api);
    await treeBecomes(api, DEFAULT_TREE);
  });

  afterEach(async () => {
    await vscode.commands.executeCommand('clippings.resetAllFilters');
    await treeBecomes(api, DEFAULT_TREE);
  });

  it('filters the tree by text and clears the filter', async () => {
    api.test.prompts.script('router');
    await vscode.commands.executeCommand('clippings.filter');
    assert.deepEqual(api.test.prompts.shown.at(-1), { kind: 'input', options: { prompt: 'Filter tree' } });
    await treeBecomes(api, [
      '(Scan mode: workspace and open files)',
      '(1 filter active)',
      'workspace',
      '  src',
      '    app.ts',
      '      TODO (alice) wire up the router',
      '      HACK temporary shim until the router lands',
    ]);
    assert.equal(api.test.viewState().currentFilter, 'router');
    assert.equal(api.test.viewState().filtered, true);
    await vscode.commands.executeCommand('clippings.filterClear');
    await treeBecomes(api, DEFAULT_TREE);
    assert.equal(api.test.viewState().filtered, false);
  });

  it('ignores a cancelled filter prompt', async () => {
    api.test.prompts.script(undefined);
    await vscode.commands.executeCommand('clippings.filter');
    assert.equal(api.test.viewState().filtered, false);
  });

  it('hides a folder and removes that filter', async () => {
    const src = await itemAt(api, 'workspace', 'src');
    await vscode.commands.executeCommand('clippings.excludeThisFolder', src.id);
    await treeBecomes(api, DEFAULT_TREE.slice(0, 9).toSpliced(1, 0, '(1 filter active)'));
    assert.deepEqual(api.test.viewState().excludeGlobs, [`${slashPath('src')}/**/*`]);
    api.test.prompts.script([`Exclude Folder: ${slashPath('src')}`]);
    await vscode.commands.executeCommand('clippings.removeFilter');
    await treeBecomes(api, DEFAULT_TREE);
  });

  it('hides a file', async () => {
    const plan = await itemAt(api, 'workspace', 'docs', 'plan.md');
    await vscode.commands.executeCommand('clippings.excludeThisFile', plan.id);
    await treeBecomes(api, [
      '(Scan mode: workspace and open files)',
      '(1 filter active)',
      'workspace',
      ...DEFAULT_TREE.slice(6),
    ]);
    assert.deepEqual(api.test.viewState().excludeGlobs, [slashPath('docs', 'plan.md')]);
  });

  it('shows only a folder, then only a folder and its subfolders', async () => {
    const src = await itemAt(api, 'workspace', 'src');
    await vscode.commands.executeCommand('clippings.showOnlyThisFolder', src.id);
    await treeBecomes(api, [
      '(Scan mode: workspace and open files)',
      '(1 filter active)',
      'workspace',
      '  src',
      '    app.ts',
      '      TODO (alice) wire up the router',
      '      FIXME handle the error path',
      '      HACK temporary shim until the router lands',
    ]);
    const srcAgain = await itemAt(api, 'workspace', 'src');
    await vscode.commands.executeCommand('clippings.showOnlyThisFolderAndSubfolders', srcAgain.id);
    await treeBecomes(api, [
      '(Scan mode: workspace and open files)',
      '(1 filter active)',
      'workspace',
      ...DEFAULT_TREE.slice(9),
    ]);
    assert.deepEqual(api.test.viewState().includeGlobs, [`${slashPath('src')}/**/*`]);
  });

  it('filters a folder whose name has spaces and brackets', async () => {
    const folder = 'app [slug] (old)';
    mkdirSync(workspacePath(folder), { recursive: true });
    writeFileSync(workspacePath(folder, 'page.ts'), '// TODO render the page\n');
    try {
      const withFolder = [
        ...DEFAULT_TREE.slice(0, 2),
        `  ${folder}`,
        '    page.ts',
        '      TODO render the page',
        ...DEFAULT_TREE.slice(2),
      ];
      await treeBecomes(api, withFolder);
      const app = await itemAt(api, 'workspace', folder);
      await vscode.commands.executeCommand('clippings.showOnlyThisFolder', app.id);
      await treeBecomes(api, [
        '(Scan mode: workspace and open files)',
        '(1 filter active)',
        'workspace',
        `  ${folder}`,
        '    page.ts',
        '      TODO render the page',
      ]);
      assert.deepEqual(api.test.viewState().includeGlobs, [`${slashPath('app [[]slug[]] (old)')}/*`]);
      await vscode.commands.executeCommand('clippings.resetAllFilters');
      await treeBecomes(api, withFolder);
      const appAgain = await itemAt(api, 'workspace', folder);
      await vscode.commands.executeCommand('clippings.excludeThisFolder', appAgain.id);
      await treeBecomes(api, [...withFolder.slice(0, 1), '(1 filter active)', ...DEFAULT_TREE.slice(1)]);
    } finally {
      rmSync(workspacePath(folder), { recursive: true, force: true });
    }
  });

  it('warns when no scopes exist and switches to a configured scope', async () => {
    api.test.prompts.script('OK');
    await vscode.commands.executeCommand('clippings.switchScope');
    const warning = api.test.prompts.shown.at(-1);
    assert.equal(warning?.kind, 'warning');
    await setSetting('filtering.scopes', [{ name: 'lib only', includeGlobs: '**/lib/**' }]);
    try {
      api.test.prompts.script('lib only');
      await vscode.commands.executeCommand('clippings.switchScope');
      await treeBecomes(api, [
        '(Scan mode: workspace and open files)',
        '(1 filter active)',
        'workspace',
        '  lib',
        '    notes.rs',
        '      TODO first line of a long note',
      ]);
      api.test.prompts.script(undefined);
      await vscode.commands.executeCommand('clippings.switchScope');
      const pick = api.test.prompts.shown.at(-1);
      assert.deepEqual(pick?.kind === 'pick' && pick.items, ['lib only']);
    } finally {
      await setSetting('filtering.scopes', undefined);
    }
  });

  it('resets filtering.scopes globally through the setting writer when Open Settings is chosen', async () => {
    assert.equal(vscode.workspace.getConfiguration('clippings.filtering').inspect('scopes')?.globalValue, undefined);
    try {
      api.test.prompts.script('Open Settings');
      await vscode.commands.executeCommand('clippings.switchScope');
      assert.deepEqual(
        vscode.workspace.getConfiguration('clippings.filtering').inspect('scopes')?.globalValue,
        [],
      );
    } finally {
      await vscode.commands.executeCommand('workbench.action.closeAllEditors');
      await setSetting('filtering.scopes', undefined);
    }
  });

  it('resets all filters, including the text filter', async () => {
    api.test.prompts.script('guide');
    await vscode.commands.executeCommand('clippings.filter');
    const lib = await itemAt(api, 'workspace', 'lib');
    await vscode.commands.executeCommand('clippings.excludeThisFolder', lib.id);
    await treeBecomes(api, [
      '(Scan mode: workspace and open files)',
      '(2 filters active)',
      'workspace',
      '  docs',
      '    plan.md',
      '      [ ] write the guide',
    ]);
    await vscode.commands.executeCommand('clippings.resetAllFilters');
    await treeBecomes(api, DEFAULT_TREE);
    assert.deepEqual(api.test.viewState(), { currentFilter: '', filtered: false, includeGlobs: [], excludeGlobs: [] });
  });
});
```

Replace `extension/src/test/integration/reveal.test.ts` with:

```ts
import * as assert from 'node:assert/strict';
import * as vscode from 'vscode';
import { NEEDS_SCAN_MESSAGE } from '../../commands/scan';
import type { ClippingsApi } from '../../testApi';
import { DEFAULT_TREE } from './fixture';
import { getApi, itemAt, setSetting, treeBecomes, waitFor, whenIdle, workspacePath } from './helpers';

async function open(path: string): Promise<vscode.TextEditor> {
  return vscode.window.showTextDocument(vscode.Uri.file(path));
}

/** A path as node IDs spell it, with `/` separators. */
function slashed(path: string): string {
  return process.platform === 'win32' ? path.replaceAll('\\', '/') : path;
}

function selected(api: ClippingsApi): string | undefined {
  return api.test.tree.view.selection[0];
}

describe('reveal, track file and todo clicks', () => {
  let api: ClippingsApi;

  before(async () => {
    api = await getApi();
    await whenIdle(api);
    await treeBecomes(api, DEFAULT_TREE);
    await vscode.commands.executeCommand('clippings-view.focus');
  });

  afterEach(async () => {
    await vscode.commands.executeCommand('workbench.action.closeAllEditors');
  });

  it('opens a clicked todo at its position and flashes the line', async () => {
    const todo = await itemAt(api, 'workspace', 'src', 'app.ts', 'FIXME handle the error path');
    assert.ok(todo.command);
    await vscode.commands.executeCommand(todo.command.command, ...todo.command.arguments);
    const editor = vscode.window.activeTextEditor;
    assert.ok(editor);
    assert.equal(editor.document.uri.fsPath, vscode.Uri.file(workspacePath('src', 'app.ts')).fsPath);
    assert.deepEqual([editor.selection.active.line, editor.selection.active.character], [3, 2]);
    assert.deepEqual(api.test.tree.lastFlash(), { uri: editor.document.uri.toString(), line: 3 });
  });

  it('tracks the active file in the tree', async () => {
    const path = workspacePath('src', 'util', 'strings.py');
    await open(path);
    await waitFor('strings.py selected', () => selected(api)?.endsWith(`/f:${slashed(path)}`), [
      api.test.tree.view.onDidChangeSelection,
    ]);
  });

  it('never leaves an unhandled rejection when the track-file reveal fails', async () => {
    const view = api.test.tree.view;
    const originalReveal = view.reveal.bind(view);
    let notifyCalled: (() => void) | undefined;
    const called = new Promise<void>((resolve) => (notifyCalled = resolve));
    view.reveal = (() => {
      notifyCalled?.();
      return Promise.reject(new Error('injected reveal failure'));
    }) as typeof originalReveal;
    const rejections: unknown[] = [];
    const onUnhandledRejection = (reason: unknown) => rejections.push(reason);
    process.on('unhandledRejection', onUnhandledRejection);
    try {
      await open(workspacePath('lib', 'notes.rs'));
      await called;
      // Node flags an unhandled rejection on a later tick; give it room to do so.
      await new Promise((resolve) => setTimeout(resolve, 200));
      assert.deepEqual(rejections, [], 'a failed track-file reveal must not become an unhandled rejection');
    } finally {
      process.off('unhandledRejection', onUnhandledRejection);
      view.reveal = originalReveal;
    }
  });

  it('reveals the current file on demand when tracking is off', async () => {
    await setSetting('tree.trackFile', false);
    try {
      const path = workspacePath('docs', 'plan.md');
      await open(path);
      assert.ok(!selected(api)?.endsWith(`/f:${slashed(path)}`));
      await vscode.commands.executeCommand('clippings.reveal');
      await waitFor('plan.md selected', () => selected(api)?.endsWith(`/f:${slashed(path)}`), [
        api.test.tree.view.onDidChangeSelection,
      ]);
    } finally {
      await setSetting('tree.trackFile', undefined);
    }
  });

  it('asks for a refresh when the startup scan is off', async () => {
    await setSetting('tree.scanAtStartup', false);
    try {
      await vscode.commands.executeCommand('clippings.restartServer');
      const s = api.test.server;
      await waitFor('needsScan', () => s.running && s.status()?.needsScan, [s.onStatus, s.onRunning]);
      assert.equal(api.test.tree.view.message, NEEDS_SCAN_MESSAGE);
      await vscode.commands.executeCommand('clippings.refresh');
      await waitFor('a scan', () => s.status()?.needsScan === false, [s.onStatus]);
      assert.equal(api.test.tree.view.message, undefined);
    } finally {
      await setSetting('tree.scanAtStartup', undefined);
    }
    await whenIdle(api);
    await treeBecomes(api, DEFAULT_TREE);
  });
});
```

Replace `extension/src/test/unit/serverResolution.test.ts` with:

```ts
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
      exists: (p) => [join('/ext', 'bin', 'clippings'), join('/ext', 'bin', 'platform.ok'), join('/b', 'clippings')].includes(p),
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
```

- [ ] **Step 2: Fix the flat view label and the test workspace path**

Replace `crates/clippings-core/src/view/place.rs` with:

```rust
//! Placement (spec section 5.12): where each todo goes in the tree, flat
//! and tags-only views, and each node's ID.

use super::{Arena, Kind, Node, TodoData};
use crate::globs::slash_path;
use crate::index::EffectiveFile;
use crate::roots::deepest_root;
use crate::settings::Settings;
use crate::styles::Resolver;
use crate::uri::{file_uri, uri_basename};
use std::path::{Path, PathBuf};

fn name_of(path: &Path) -> String {
    path.file_name().map_or_else(
        || path.to_string_lossy().into_owned(),
        |n| n.to_string_lossy().into_owned(),
    )
}

/// The todo's key for decorations, icons and hide flags: its group or tag,
/// or its raw label when untagged.
pub fn todo_key(settings: &Settings, tag: &str, raw_label: &str) -> String {
    if tag.is_empty() {
        raw_label.to_string()
    } else {
        settings.key_of(tag).to_string()
    }
}

/// Raw label (spec section 5.12): `after`, or `line N`, prefixed with the
/// tag unless grouped by tag; a multi-line todo's raw label is its tag.
pub fn raw_label(
    tag: &str,
    after: &str,
    line: u32,
    multi_line: bool,
    grouped_by_tag: bool,
) -> String {
    if multi_line {
        return tag.to_string();
    }
    let text = if after.is_empty() {
        format!("line {}", line + 1)
    } else {
        after.to_string()
    };
    if grouped_by_tag || tag.is_empty() {
        text
    } else {
        format!("{tag} {text}")
    }
}

pub fn place(settings: &Settings, files: &[EffectiveFile], tree_roots: &[PathBuf]) -> Arena {
    let view = settings.view();
    let resolver = Resolver::new(settings);
    let mut a = Arena::default();
    let mut hide_from_tree: std::collections::HashMap<String, bool> =
        std::collections::HashMap::new();
    // Container chains depend only on the file, key and sub-tag; build each once.
    let mut chains: std::collections::HashMap<(usize, String, Option<String>), Option<usize>> =
        std::collections::HashMap::new();
    for (file_i, file) in files.iter().enumerate() {
        let doc_uri = file.uri.map(str::to_string);
        let disk_uri = file.path.map(file_uri);
        for sourced in &file.todos {
            let t = sourced.todo;
            let uri = sourced
                .buffer_uri
                .map(str::to_string)
                .or_else(|| disk_uri.clone())
                .or_else(|| doc_uri.clone())
                .unwrap_or_default();
            let sub = t.sub_tag.clone().filter(|s| !s.is_empty());
            let multi = !t.extra_lines.is_empty();
            let raw = raw_label(&t.tag, &t.after, t.start.line, multi, view.grouped_by_tag);
            let key = todo_key(settings, &t.tag, &raw);

            let chain_key = (file_i, key.clone(), sub.clone());
            let parent = if let Some(p) = chains.get(&chain_key) {
                *p
            } else {
                let p = if view.tags_only {
                    if view.grouped_by_tag {
                        let label = match &sub {
                            Some(s) => format!("{key} ({s})"),
                            None => key.clone(),
                        };
                        Some(a.get_or_add(None, &format!("g:{label}"), || {
                            let mut n = Node::new(Kind::Tag, label.clone());
                            n.key = Some(key.clone());
                            n
                        }))
                    } else if let (true, Some(s)) = (view.grouped_by_sub_tag, &sub) {
                        Some(a.get_or_add(None, &format!("s:{s}"), || {
                            let mut n = Node::new(Kind::SubTag, s.clone());
                            n.sub_tag = Some(s.clone());
                            n
                        }))
                    } else {
                        None
                    }
                } else {
                    let root = file.path.and_then(|p| deepest_root(p, tree_roots));
                    let mut parent = root.map(|r| {
                        a.get_or_add(None, &format!("w:{}", file_uri(r)), || {
                            let mut n = Node::new(Kind::Root, name_of(r));
                            n.path = Some(r.to_path_buf());
                            n
                        })
                    });
                    if view.grouped_by_tag {
                        parent = Some(a.get_or_add(parent, &format!("g:{key}"), || {
                            let mut n = Node::new(Kind::Tag, key.clone());
                            n.key = Some(key.clone());
                            n
                        }));
                    } else if let (true, Some(s)) = (view.grouped_by_sub_tag, &sub) {
                        parent = Some(a.get_or_add(parent, &format!("s:{s}"), || {
                            let mut n = Node::new(Kind::SubTag, s.clone());
                            n.sub_tag = Some(s.clone());
                            n
                        }));
                    }
                    if let (Some(r), Some(path), false) = (root, file.path, view.flat) {
                        let mut dir = r.to_path_buf();
                        if let Ok(rel) = path.parent().unwrap_or(path).strip_prefix(r) {
                            for c in rel.components() {
                                dir.push(c);
                                let d = dir.clone();
                                parent = Some(a.get_or_add(
                                    parent,
                                    &format!("d:{}", slash_path(&d)),
                                    || {
                                        let mut n = Node::new(Kind::Folder, name_of(&d));
                                        n.path = Some(d.clone());
                                        n
                                    },
                                ));
                            }
                        }
                    }
                    let file_key = match file.path {
                        Some(p) => format!("f:{}", slash_path(p)),
                        None => format!("f:{}", doc_uri.clone().unwrap_or_default()),
                    };
                    let file_idx = a.get_or_add(parent, &file_key, || {
                        let mut n = Node::new(
                            Kind::File,
                            file.path.map(name_of).unwrap_or_else(|| uri_basename(&uri)),
                        );
                        n.path = file.path.map(Path::to_path_buf);
                        n.uri = Some(disk_uri.clone().unwrap_or_else(|| uri.clone()));
                        // `/` separators on every platform, as in folder labels.
                        let dir = file.path.and_then(|p| p.parent()).map(|d| match root {
                            Some(r) => slash_path(d.strip_prefix(r).unwrap_or(d)),
                            None => slash_path(d),
                        });
                        if view.flat || root.is_none() {
                            n.path_label = dir.filter(|d| !d.is_empty()).map(|d| format!("({d})"));
                        }
                        n
                    });
                    let mut parent = Some(file_idx);
                    if let (Some(s), false) = (&sub, view.grouped_by_sub_tag) {
                        parent = Some(a.get_or_add(parent, &format!("s:{s}"), || {
                            let mut n = Node::new(Kind::SubTag, s.clone());
                            n.sub_tag = Some(s.clone());
                            n
                        }));
                    }
                    parent
                };
                chains.insert(chain_key, p);
                p
            };

            let file_parent = parent.and_then(|p| match a.nodes[p].kind {
                Kind::File => Some(p),
                Kind::SubTag => a.nodes[p].parent.filter(|&g| a.nodes[g].kind == Kind::File),
                _ => None,
            });
            // A notebook cell's todos share their file with the other
            // cells', so line and column alone would collide.
            let cell = disk_uri.as_ref().or(doc_uri.as_ref()) != Some(&uri);
            let todo_key = if file_parent.is_some() && !cell {
                format!("t:{}:{}", t.start.line, t.start.character)
            } else {
                format!("t:{uri}:{}:{}", t.start.line, t.start.character)
            };
            let data = TodoData {
                uri: uri.clone(),
                start: t.start,
                text_end: t.text_end,
                tag: t.tag.clone(),
                sub_tag: sub.clone(),
                before: t.before.clone(),
                after: t.after.clone(),
                multi_line: multi,
                text: raw.clone(),
                cell,
            };
            let hidden = match hide_from_tree.get(&key) {
                Some(h) => *h,
                None => {
                    let h = resolver.flag(&key, |x| x.hide_from_tree);
                    hide_from_tree.insert(key.clone(), h);
                    h
                }
            };
            let extra_base = (!t.extra_lines.is_empty()).then(|| data.clone());
            let todo_idx = a.get_or_add(parent, &todo_key, || {
                let mut n = Node::new(Kind::Todo, raw);
                n.path = file.path.map(Path::to_path_buf);
                n.uri = Some(uri.clone());
                n.key = Some(key);
                n.sub_tag = sub;
                n.todo = Some(data);
                n.hidden = hidden;
                n
            });
            for (i, extra) in t.extra_lines.iter().enumerate() {
                let mut d = extra_base.clone().expect("extra lines imply a base");
                d.text = extra.text.clone();
                d.start = crate::position::Position {
                    line: extra.line,
                    character: 0,
                };
                d.multi_line = false;
                a.get_or_add(Some(todo_idx), &format!("x:{i}"), || {
                    let mut n = Node::new(Kind::Extra, extra.text.clone());
                    n.path = file.path.map(Path::to_path_buf);
                    n.uri = Some(uri.clone());
                    n.todo = Some(d);
                    n
                });
            }
        }
    }
    a
}
```

Replace `extension/.vscode-test.mjs` with:

```js
// Extension integration tests (spec 12.5). Each run copies the fixture
// workspace and uses a fresh user data directory, so tests can edit files
// and settings without touching the repository or the user's profile.
import { defineConfig } from '@vscode/test-cli';
import { cpSync, mkdirSync, mkdtempSync, realpathSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join, resolve } from 'node:path';

const here = import.meta.dirname;
// The real path: on Windows the temporary directory can be an 8.3 short
// name (`RUNNER~1`), and on macOS a symlink (`/var` to `/private/var`), so
// tests and the server would otherwise spell the same file two ways.
const scratch = mkdtempSync(join(realpathSync.native(tmpdir()), 'clippings-test-'));
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
```

- [ ] **Step 3: Run the tests to verify they pass**

Run: `cargo fmt --all --check && cargo clippy --all-targets -- -D warnings && INSTA_UPDATE=no cargo test --all`
Expected: no diffs, no warnings, and every test passes (204 passed, 2 ignored).

Run: `pnpm -C extension dev && pnpm -C extension typecheck && pnpm -C extension build && pnpm -C extension test:unit`
Expected: `88 passing`.

Run: `pnpm -C extension test:integration`
Expected: `83 passing`. A VS Code test window opens, runs the suite and closes.

- [ ] **Step 4: Commit**

```bash
git add -A
git commit -F - <<'EOF'
fix: pass the extension tests on Windows, with slash-separated flat view directories

Co-Authored-By: <model attribution>
Claude-Session: https://claude.ai/code/session_01EuAdHjqQeFrieAZqvsQ325
EOF
```

### Task 7: Run the extension tests on Linux, macOS and Windows

Spec 12.5's extension tests join CI. A new `extension` job runs `pnpm -C extension test` (the debug server, type check, bundle, unit, script and integration tests) on Ubuntu, macOS and Windows with VS Code pinned to the `engines.vscode` floor 1.91.0, whose download is cached, plus Ubuntu on `stable` (ruling 15). Linux runs under `xvfb-run`. The workflow gets read-only `contents` permissions, and checkout moves to v5 (ruling 22).

**Files:**
- Modify: `.github/workflows/ci.yml`

**Interfaces:**
- Produces: the CI jobs `extension (<os>, VS Code <version>)`, which set `CLIPPINGS_TEST_VSCODE` from the matrix. Task 8's path-form suite runs inside them through `pnpm test`.

- [ ] **Step 1: Write the workflow**

Replace `.github/workflows/ci.yml` with:

```yaml
name: CI

on:
  push:
    branches: [main]
  pull_request:

permissions:
  contents: read

jobs:
  rust:
    strategy:
      fail-fast: false
      matrix:
        os: [ubuntu-latest, macos-latest, windows-latest]
    runs-on: ${{ matrix.os }}
    steps:
      - uses: actions/checkout@v5
      - uses: dtolnay/rust-toolchain@stable
        with:
          components: rustfmt, clippy
      - uses: Swatinem/rust-cache@v2
      - run: git config --global init.defaultBranch main
      - run: cargo fmt --all --check
      - run: cargo clippy --locked --all-targets -- -D warnings
      - run: cargo test --all --locked
        env:
          INSTA_UPDATE: "no"

  # Extension unit, script and integration tests (spec 12.5) against the
  # debug server that `pnpm test` builds. VS Code is pinned to the
  # `engines.vscode` floor on every OS, so the suite is deterministic, its
  # download is cached, and the manifest's minimum version is proven; one
  # `stable` job catches upstream changes before users meet them.
  extension:
    name: extension (${{ matrix.os }}, VS Code ${{ matrix.vscode }})
    strategy:
      fail-fast: false
      matrix:
        os: [ubuntu-latest, macos-latest, windows-latest]
        vscode: ["1.91.0"]
        include:
          - os: ubuntu-latest
            vscode: stable
    runs-on: ${{ matrix.os }}
    env:
      CLIPPINGS_TEST_VSCODE: ${{ matrix.vscode }}
    steps:
      - uses: actions/checkout@v5
      - uses: dtolnay/rust-toolchain@stable
      - uses: Swatinem/rust-cache@v2
      - uses: pnpm/action-setup@v4
        with:
          package_json_file: extension/package.json
      - uses: actions/setup-node@v5
        with:
          node-version: 22
          cache: pnpm
          cache-dependency-path: extension/pnpm-lock.yaml
      - name: Cache the VS Code download
        if: matrix.vscode != 'stable'
        uses: actions/cache@v4
        with:
          path: extension/.vscode-test
          key: vscode-test-${{ runner.os }}-${{ runner.arch }}-${{ matrix.vscode }}
      - run: pnpm -C extension install --frozen-lockfile
      - name: pnpm -C extension test
        if: runner.os == 'Linux'
        run: xvfb-run -a pnpm -C extension test
      - name: pnpm -C extension test
        if: runner.os != 'Linux'
        run: pnpm -C extension test
```

- [ ] **Step 2: Check it locally**

Run: `actionlint .github/workflows/*.yml`
Expected: no output and exit code 0.

Without actionlint (`brew install actionlint`, or `go install github.com/rhysd/actionlint/cmd/actionlint@v1.7.12`), at least check that the YAML parses with `ruby -ryaml -e 'ARGV.each { |f| YAML.load_file(f) }' .github/workflows/*.yml`, and rely on the CI run below.

Run: `pnpm -C extension test`
Expected: `88 passing` (unit), `pass 14` (scripts) and `83 passing` (integration). A VS Code test window opens, runs the suite and closes.

To run the suite as CI's pinned jobs do, set `CLIPPINGS_TEST_VSCODE=1.91.0`; the first run downloads that version.

- [ ] **Step 3: Commit**

```bash
git add -A
git commit -F - <<'EOF'
ci: run the extension tests on Linux, macOS and Windows

Co-Authored-By: <model attribution>
Claude-Session: https://claude.ai/code/session_01EuAdHjqQeFrieAZqvsQ325
EOF
```

- [ ] **Step 4: Push and run CI on a draft pull request**

CI runs on pull requests, so open one draft pull request from the feature branch now; later tasks push to it. Never merge it or mark it ready (Global Constraints).

```bash
git push -u origin HEAD
gh pr create --draft --base main --title "Packaging, CI and release (plan 4)" --body "Executes docs/superpowers/plans/2026-09-27-clippings-packaging.md. Draft: do not merge."
sleep 10
id=$(gh run list --workflow ci.yml --branch "$(git branch --show-current)" --limit 1 --json databaseId --jq '.[0].databaseId')
gh run watch "$id" --exit-status
```

Expected: `gh run watch` exits 0, and `gh run view "$id"` lists these jobs:

- `rust (ubuntu-latest)`, `rust (macos-latest)`, `rust (windows-latest)`: success
- `extension (ubuntu-latest, VS Code 1.91.0)`, `extension (macos-latest, VS Code 1.91.0)`, `extension (windows-latest, VS Code 1.91.0)`, `extension (ubuntu-latest, VS Code stable)`: success

The Windows job reports the fake-server tests as pending, by design: they need a POSIX shell script as the server. If a job fails, read it with `gh run view "$id" --log-failed`. A failure in `extension (windows-latest, ...)` that compares a path means a Task 6 fix is missing; a macOS failure in `context keys` means Task 1's fix is missing. Fix the owning task's files, commit, push, and watch the new run.

### Task 8: Open the workspace through a symlink, a junction and an 8.3 short path

Review Focus 1: a workspace opened through another spelling of its path, which VS Code keeps while the file watcher and the server may see the real one (spec 12.5, ruling 16). `.vscode-test.pathforms.mjs` defines one test-cli configuration per form, each with its own fixture copy and profile: `symlink` on every OS, `junction` and `short` on Windows. The short form is skipped, with the reason, when the volume has no 8.3 names. The suite checks the four essentials: the tree fills, the active file is revealed, an open file is decorated without a second tree node, and a disk change reaches the tree. `pnpm test` runs it, so Task 7's CI jobs pick it up.

**Files:**
- Modify: `extension/package.json` (diff)
- Test: `extension/src/test/pathforms/pathforms.test.ts`, `extension/.vscode-test.pathforms.mjs`

**Interfaces:**
- Consumes: `getApi`, `treeBecomes`, `waitFor`, `whenIdle` and `workspacePath` from `test/integration/helpers.ts` (Task 6 for `workspacePath`'s separators); `DEFAULT_TREE` from `test/integration/fixture.ts`.
- Produces: the `test:pathforms` script (`vscode-test --config .vscode-test.pathforms.mjs`), run by `pnpm test` after `test:integration`; configurations labelled `symlink`, `junction` and `short`, runnable one at a time with `--label <form>`; the environment variables `CLIPPINGS_TEST_PATH_FORM` and `CLIPPINGS_TEST_SKIP`.

- [ ] **Step 1: Write the path-form suite and its configuration**

Create `extension/src/test/pathforms/pathforms.test.ts`:

```ts
// The essentials, with the workspace opened through another spelling of its
// path (.vscode-test.pathforms.mjs): the tree fills, the active file is
// revealed, an open file is decorated without a second tree node, and a
// change on disk reaches the tree.

import * as assert from 'node:assert/strict';
import { readFileSync, writeFileSync } from 'node:fs';
import * as vscode from 'vscode';
import type { ClippingsApi } from '../../testApi';
import { DEFAULT_TREE } from '../integration/fixture';
import { getApi, treeBecomes, waitFor, whenIdle, workspacePath } from '../integration/helpers';

const form = process.env['CLIPPINGS_TEST_PATH_FORM'] ?? 'unknown form';
const skip = process.env['CLIPPINGS_TEST_SKIP'];

describe(`workspace opened through a ${form} path`, () => {
  let api: ClippingsApi;
  let tree: string[];

  before(async function () {
    if (skip) {
      console.log(`  skipping the ${form} path form: ${skip}`);
      this.skip();
    }
    console.log(`  workspace: ${workspacePath()}`);
    api = await getApi();
    await whenIdle(api);
    // The root node is named after the folder as VS Code sees it, which for
    // an 8.3 short path is its short name.
    const name = vscode.workspace.workspaceFolders?.[0]?.name;
    assert.ok(name, 'a workspace folder');
    tree = DEFAULT_TREE.map((line) => (line === 'workspace' ? name : line));
  });

  afterEach(async () => {
    await vscode.commands.executeCommand('workbench.action.closeAllEditors');
  });

  it('fills the tree', async () => {
    assert.equal(vscode.workspace.workspaceFolders?.[0]?.uri.fsPath, vscode.Uri.file(workspacePath()).fsPath);
    await treeBecomes(api, tree);
  });

  it('reveals the active file', async () => {
    await vscode.commands.executeCommand('clippings-view.focus');
    await vscode.window.showTextDocument(vscode.Uri.file(workspacePath('src', 'util', 'strings.py')));
    await waitFor('strings.py selected', () => api.test.tree.view.selection[0]?.endsWith('/util/strings.py'), [
      api.test.tree.view.onDidChangeSelection,
    ]);
  });

  it('decorates an open file and keeps one tree node for it', async () => {
    const editor = await vscode.window.showTextDocument(vscode.Uri.file(workspacePath('src', 'app.ts')));
    const uri = editor.document.uri.toString();
    const applied = await waitFor('decorations', () => api.test.decorations.entry(uri), [api.test.decorations.onApplied]);
    assert.deepEqual(Object.keys(applied.ranges).sort(), ['FIXME', 'HACK', 'TODO']);
    // Buffer results replace the file's disk results under the same node.
    await whenIdle(api);
    await treeBecomes(api, tree);
  });

  it('reflects a change on disk', async () => {
    const path = workspacePath('lib', 'notes.rs');
    const original = readFileSync(path, 'utf8');
    try {
      writeFileSync(path, original + '// FIXME appended on disk\n');
      const expected = [...tree];
      expected.splice(expected.indexOf('      TODO first line of a long note') + 1, 0, '      FIXME appended on disk');
      await treeBecomes(api, expected);
    } finally {
      writeFileSync(path, original);
    }
    await treeBecomes(api, tree);
  });
});
```

Create `extension/.vscode-test.pathforms.mjs`:

```js
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
import { defineConfig } from '@vscode/test-cli';
import { execSync } from 'node:child_process';
import { cpSync, mkdirSync, mkdtempSync, realpathSync, symlinkSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join, resolve } from 'node:path';

const here = import.meta.dirname;
const fixture = resolve(here, '../tests/fixtures/workspace');
const server =
  process.env.CLIPPINGS_SERVER_PATH ?? resolve(here, '../target/debug/clippings' + (process.platform === 'win32' ? '.exe' : ''));
const windows = process.platform === 'win32';

/** A fresh scratch directory with a user profile and, under `parent`, a copy of the fixture. */
function scratch(form, parent = 'real') {
  // Short: VS Code's IPC socket lives in the user data directory, and macOS
  // limits socket paths to 103 characters.
  const dir = mkdtempSync(join(realpathSync.native(tmpdir()), `clp-${form.slice(0, 2)}-`));
  const copy = join(dir, parent, 'workspace');
  cpSync(fixture, copy, { recursive: true });
  mkdirSync(join(dir, 'user-data', 'User'), { recursive: true });
  writeFileSync(
    join(dir, 'user-data', 'User', 'settings.json'),
    JSON.stringify({
      'chat.disableAIFeatures': true,
      'workbench.startupEditor': 'none',
      'telemetry.telemetryLevel': 'off',
      'git.enabled': false,
      'update.mode': 'none',
    }),
  );
  return { dir, copy };
}

/** A link at <dir>/alias/workspace to the copy: a symlink or an NTFS junction. */
function linked(form, type) {
  const { dir, copy } = scratch(form);
  const alias = join(dir, 'alias', 'workspace');
  mkdirSync(join(dir, 'alias'));
  symlinkSync(copy, alias, type);
  return { dir, workspace: alias };
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
```

- [ ] **Step 2: Run the suite to verify it is not wired up yet**

Run: `pnpm -C extension run test:pathforms`
Expected: FAIL with `Missing script: test:pathforms`.

- [ ] **Step 3: Add the script**

Apply this diff to `extension/package.json` with `git apply`:

```diff
diff --git a/extension/package.json b/extension/package.json
index 6aab4c4..1bc3dc7 100644
--- a/extension/package.json
+++ b/extension/package.json
@@ -1303,7 +1303,8 @@
     "test:unit": "mocha --ui bdd \"out/test/unit/**/*.test.js\"",
     "test:scripts": "node --test \"scripts/**/*.test.mjs\"",
     "test:integration": "vscode-test",
-    "test": "pnpm dev && pnpm typecheck && pnpm build && pnpm test:unit && pnpm test:scripts && pnpm test:integration",
+    "test:pathforms": "vscode-test --config .vscode-test.pathforms.mjs",
+    "test": "pnpm dev && pnpm typecheck && pnpm build && pnpm test:unit && pnpm test:scripts && pnpm test:integration && pnpm test:pathforms",
     "package": "node scripts/package.mjs"
   },
   "packageManager": "pnpm@10.27.0",
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `pnpm -C extension typecheck && pnpm -C extension build && pnpm -C extension test:pathforms`
Expected: `workspace opened through a symlink path`, then `4 passing`. On macOS and Linux only the symlink form runs. A VS Code test window opens, runs the suite and closes.

Run: `pnpm -C extension test`
Expected: `88 passing`, `pass 14`, `83 passing`, then the path-form suite's `4 passing`. A VS Code test window opens, runs the suite and closes.

- [ ] **Step 5: Commit**

```bash
git add -A
git commit -F - <<'EOF'
test(extension): open the workspace through a symlink, a junction and an 8.3 short path

Co-Authored-By: <model attribution>
Claude-Session: https://claude.ai/code/session_01EuAdHjqQeFrieAZqvsQ325
EOF
```

- [ ] **Step 6: Push and check the Windows path forms in CI**

Push, then wait for the pull request's CI run (opened in Task 7):

```bash
git push
sleep 10
id=$(gh run list --workflow ci.yml --branch "$(git branch --show-current)" --limit 1 --json databaseId --jq '.[0].databaseId')
gh run watch "$id" --exit-status
```

Expected: `gh run watch` exits 0, and `gh run view "$id"` lists these jobs:

- `rust (ubuntu-latest)`, `rust (macos-latest)`, `rust (windows-latest)`: success
- `extension (ubuntu-latest, VS Code 1.91.0)`, `extension (macos-latest, VS Code 1.91.0)`, `extension (windows-latest, VS Code 1.91.0)`, `extension (ubuntu-latest, VS Code stable)`: success

Then read the Windows job's path-form lines:

```bash
job=$(gh run view "$id" --json jobs --jq '.jobs[] | select(.name | startswith("extension (windows")) | .databaseId')
gh run view "$id" --job "$job" --log | grep -E 'workspace opened through|workspace: |passing|pending'
```

Expected: three suites, `workspace opened through a symlink path`, `... a junction path` and `... a short path`, each with `4 passing`. The short form's `workspace:` line is an 8.3 path such as `C:\Users\RUNNER~1\AppData\Local\Temp\CLP-SH~1\CLIPPI~1\WORKSP~1`. If it says instead `skipping the short path form: 8.3 short names are disabled on this volume`, the runner image changed and the form did not run: report it rather than treating the job as proof.

If a job fails, read its log with `gh run view "$id" --log-failed`, fix the cause in this task's files, amend the commit (`git commit --amend --no-edit`), push with `git push --force-with-lease`, and watch the new run.

### Task 9: Build the server for the nine targets and package the ten VSIX files

Spec 8.1 and 8.2 in CI. `build.yml` is a reusable workflow: a `server` matrix builds each target with Task 2's `build.sh` on its native runner (Linux and Alpine on Ubuntu with cargo-zigbuild 0.23.4 and ziglang 0.15.2, ruling 22) and uploads `server-<target>` and `symbols-<target>`; a `package` job bundles the extension once and runs Task 5's `package.mjs` for all ten packages, uploading them as `vsix`. `ci.yml` calls it from a `dist` job on pushes to `main` only (ruling 17). Task 10 adds the checks and Task 11 the signing.

**Files:**
- Create: `.github/workflows/build.yml`
- Modify: `.github/workflows/ci.yml`

**Interfaces:**
- Consumes: `scripts/dist/build.sh` (Task 2); `extension/scripts/version.mjs` (Task 4); `extension/scripts/package.mjs` (Task 5).
- Produces: the reusable workflow `.github/workflows/build.yml` (`on: workflow_call`), with outputs `version` and `pre-release` and the artifacts `server-<target>` (the binary), `symbols-<target>` (its symbols file) and `vsix` (the ten packages). Task 12 calls it.
- Produces: the `dist` job in `ci.yml`.

- [ ] **Step 1: Write the build workflow and call it from CI**

Create `.github/workflows/build.yml`:

```yaml
# Builds the server for the nine VS Code targets and the ten VSIX packages
# (spec 8.1, 8.2). Called by the release workflow and by CI on main.
#
# Artifacts:
#   server-<target>    the stripped server binary
#   symbols-<target>   its debug symbols (dSYM zip, PDB or .debug file)
#   vsix               the nine platform packages and the universal package
name: Build

on:
  workflow_call:
    outputs:
      version:
        description: The extension version, from package.json.
        value: ${{ jobs.package.outputs.version }}
      pre-release:
        description: "'true' when the version is a pre-release (odd minor)."
        value: ${{ jobs.package.outputs.pre-release }}

permissions:
  contents: read

jobs:
  server:
    name: server (${{ matrix.target }})
    strategy:
      fail-fast: false
      matrix:
        include:
          - { target: win32-x64, triple: x86_64-pc-windows-msvc, os: windows-latest }
          - { target: win32-arm64, triple: aarch64-pc-windows-msvc, os: windows-latest }
          - { target: darwin-x64, triple: x86_64-apple-darwin, os: macos-latest }
          - { target: darwin-arm64, triple: aarch64-apple-darwin, os: macos-latest }
          - { target: linux-x64, triple: x86_64-unknown-linux-gnu, os: ubuntu-latest }
          - { target: linux-arm64, triple: aarch64-unknown-linux-gnu, os: ubuntu-latest }
          - { target: linux-armhf, triple: armv7-unknown-linux-gnueabihf, os: ubuntu-latest }
          - { target: alpine-x64, triple: x86_64-unknown-linux-musl, os: ubuntu-latest }
          - { target: alpine-arm64, triple: aarch64-unknown-linux-musl, os: ubuntu-latest }
    runs-on: ${{ matrix.os }}
    defaults:
      run:
        shell: bash
    steps:
      - uses: actions/checkout@v5
      - uses: dtolnay/rust-toolchain@stable
        with:
          targets: ${{ matrix.triple }}
          components: ${{ runner.os == 'Linux' && 'llvm-tools' || '' }}
      - uses: Swatinem/rust-cache@v2
        with:
          key: ${{ matrix.target }}
      - name: Install cargo-zigbuild
        if: runner.os == 'Linux'
        run: |
          python3 -m venv "$RUNNER_TEMP/zig"
          "$RUNNER_TEMP/zig/bin/pip" install cargo-zigbuild==0.23.4 ziglang==0.15.2
          echo "$RUNNER_TEMP/zig/bin" >> "$GITHUB_PATH"
      - name: Build
        run: scripts/dist/build.sh ${{ matrix.target }} dist
      - uses: actions/upload-artifact@v4
        with:
          name: server-${{ matrix.target }}
          path: dist/${{ matrix.target }}/
          if-no-files-found: error
      - uses: actions/upload-artifact@v4
        with:
          name: symbols-${{ matrix.target }}
          path: dist/symbols/
          if-no-files-found: error

  package:
    needs: server
    runs-on: ubuntu-latest
    outputs:
      version: ${{ steps.version.outputs.version }}
      pre-release: ${{ steps.version.outputs.pre-release }}
    steps:
      - uses: actions/checkout@v5
      - uses: pnpm/action-setup@v4
        with:
          package_json_file: extension/package.json
      - uses: actions/setup-node@v5
        with:
          node-version: 22
          cache: pnpm
          cache-dependency-path: extension/pnpm-lock.yaml
      - run: pnpm -C extension install --frozen-lockfile
      - id: version
        run: node extension/scripts/version.mjs
      # One bundle serves all ten packages.
      - run: pnpm -C extension build --production
      - uses: actions/download-artifact@v5
        with:
          pattern: server-*
          path: dist/server
      - name: Package
        run: |
          for target in win32-x64 win32-arm64 linux-x64 linux-arm64 linux-armhf alpine-x64 alpine-arm64 darwin-x64 darwin-arm64; do
            exe=clippings
            [[ $target == win32-* ]] && exe=clippings.exe
            node extension/scripts/package.mjs --target "$target" --server "dist/server/server-$target/$exe" --out dist/vsix
          done
          node extension/scripts/package.mjs --universal --out dist/vsix
          ls -l dist/vsix
      - uses: actions/upload-artifact@v4
        with:
          name: vsix
          path: dist/vsix/*.vsix
          if-no-files-found: error
```

Replace `.github/workflows/ci.yml` with:

```yaml
name: CI

on:
  push:
    branches: [main]
  pull_request:

permissions:
  contents: read

jobs:
  rust:
    strategy:
      fail-fast: false
      matrix:
        os: [ubuntu-latest, macos-latest, windows-latest]
    runs-on: ${{ matrix.os }}
    steps:
      - uses: actions/checkout@v5
      - uses: dtolnay/rust-toolchain@stable
        with:
          components: rustfmt, clippy
      - uses: Swatinem/rust-cache@v2
      - run: git config --global init.defaultBranch main
      - run: cargo fmt --all --check
      - run: cargo clippy --locked --all-targets -- -D warnings
      - run: cargo test --all --locked
        env:
          INSTA_UPDATE: "no"

  # Extension unit, script and integration tests (spec 12.5) against the
  # debug server that `pnpm test` builds. VS Code is pinned to the
  # `engines.vscode` floor on every OS, so the suite is deterministic, its
  # download is cached, and the manifest's minimum version is proven; one
  # `stable` job catches upstream changes before users meet them.
  extension:
    name: extension (${{ matrix.os }}, VS Code ${{ matrix.vscode }})
    strategy:
      fail-fast: false
      matrix:
        os: [ubuntu-latest, macos-latest, windows-latest]
        vscode: ["1.91.0"]
        include:
          - os: ubuntu-latest
            vscode: stable
    runs-on: ${{ matrix.os }}
    env:
      CLIPPINGS_TEST_VSCODE: ${{ matrix.vscode }}
    steps:
      - uses: actions/checkout@v5
      - uses: dtolnay/rust-toolchain@stable
      - uses: Swatinem/rust-cache@v2
      - uses: pnpm/action-setup@v4
        with:
          package_json_file: extension/package.json
      - uses: actions/setup-node@v5
        with:
          node-version: 22
          cache: pnpm
          cache-dependency-path: extension/pnpm-lock.yaml
      - name: Cache the VS Code download
        if: matrix.vscode != 'stable'
        uses: actions/cache@v4
        with:
          path: extension/.vscode-test
          key: vscode-test-${{ runner.os }}-${{ runner.arch }}-${{ matrix.vscode }}
      - run: pnpm -C extension install --frozen-lockfile
      - name: pnpm -C extension test
        if: runner.os == 'Linux'
        run: xvfb-run -a pnpm -C extension test
      - name: pnpm -C extension test
        if: runner.os != 'Linux'
        run: pnpm -C extension test

  # The nine server builds and ten packages, on main only: they take longer
  # than the tests, and the release workflow builds them again from its tag.
  dist:
    if: github.event_name == 'push' && github.ref == 'refs/heads/main'
    uses: ./.github/workflows/build.yml
```

- [ ] **Step 2: Check it locally**

Run: `actionlint .github/workflows/*.yml`
Expected: no output and exit code 0.

Without actionlint (`brew install actionlint`, or `go install github.com/rhysd/actionlint/cmd/actionlint@v1.7.12`), at least check that the YAML parses with `ruby -ryaml -e 'ARGV.each { |f| YAML.load_file(f) }' .github/workflows/*.yml`, and rely on the CI run below.

The workflow's steps are Task 2's and Task 5's scripts, already run locally. `build.yml` has no trigger of its own, so its first CI run is Task 12's dry run; a failure there is fixed in this task's file.

- [ ] **Step 3: Commit**

```bash
git add -A
git commit -F - <<'EOF'
ci: build the server for the nine targets and package the ten VSIX files

Co-Authored-By: <model attribution>
Claude-Session: https://claude.ai/code/session_01EuAdHjqQeFrieAZqvsQ325
EOF
```

- [ ] **Step 4: Push and run CI**

Push, then wait for the pull request's CI run (opened in Task 7):

```bash
git push
sleep 10
id=$(gh run list --workflow ci.yml --branch "$(git branch --show-current)" --limit 1 --json databaseId --jq '.[0].databaseId')
gh run watch "$id" --exit-status
```

Expected: `gh run watch` exits 0, and `gh run view "$id"` lists these jobs:

- `rust (ubuntu-latest)`, `rust (macos-latest)`, `rust (windows-latest)`: success
- `extension (ubuntu-latest, VS Code 1.91.0)`, `extension (macos-latest, VS Code 1.91.0)`, `extension (windows-latest, VS Code 1.91.0)`, `extension (ubuntu-latest, VS Code stable)`: success
- `dist`: skipped, because it runs only on pushes to `main`

If a job fails, read its log with `gh run view "$id" --log-failed`, fix the cause in this task's files, amend the commit (`git commit --amend --no-edit`), push with `git push --force-with-lease`, and watch the new run.

### Task 10: Check each binary's glibc floor and probe it natively or under qemu

Spec 8.2: CI checks that no glibc binary needs a `GLIBC_` symbol version above 2.28, and probes the Arm Linux binaries under qemu. `check.sh` does that for every target (ruling 5): the glibc floor, a kept symbol table without debug info (ruling 2), and `clippings probe`, whose `protocolVersion` must equal the extension's `PROTOCOL_VERSION`, run in `debian:10` or `alpine:3.22` for Linux targets. `build.yml` runs it after each build, sets up qemu on Linux, and adds a `probe-win32-arm64` job on a `windows-11-arm` runner, because the x64 runner cannot run the Arm64 binary (Review Focus 5).

**Files:**
- Create: `scripts/dist/check.sh`
- Modify: `.github/workflows/build.yml`

**Interfaces:**
- Consumes: `resolve_target` from `scripts/dist/target.sh` (Task 2); `PROTOCOL_VERSION` in `extension/src/protocol.ts`, read with `sed`.
- Produces: `scripts/dist/check.sh <vscode-target> <binary> [--no-probe]`, exiting 1 with `<target>: needs GLIBC_<v>, above the 2.28 floor`, `<target>: no symbol for clippings::main; ...`, `<target>: the binary still carries debug info` or `<target>: expected protocolVersion <n>`. Task 11's `sign-windows` job reuses it.
- Produces: the `probe-win32-arm64` job in `build.yml`, which `package` waits for.

- [ ] **Step 1: Write the check script**

Create `scripts/dist/check.sh`:

```bash
#!/usr/bin/env bash
# Checks a server binary built by build.sh before it is packaged (spec 8.2):
#
# - a glibc binary may need no `GLIBC_` symbol version above its floor;
# - a macOS, Linux or Alpine binary keeps its symbol table, so backtraces
#   name functions, and carries no debug info, which ships separately;
# - `clippings probe` must print JSON whose protocolVersion is the one the
#   extension speaks (extension/src/protocol.ts).
#
# Linux binaries probe in a container of their own architecture: debian:10,
# whose glibc is exactly 2.28, for `linux-*`, and Alpine for `alpine-*`. On
# an x64 host the arm64 and armv7 containers run under qemu, so the host
# needs binfmt handlers (docker/setup-qemu-action). macOS and Windows
# binaries run directly; pass --no-probe where the host cannot run them.
#
# Usage: scripts/dist/check.sh <vscode-target> <binary> [--no-probe]
set -euo pipefail

here=$(cd "$(dirname "$0")" && pwd)
root=$(cd "$here/../.." && pwd)
# shellcheck source=target.sh
source "$here/target.sh"

target=${1:?usage: check.sh <vscode-target> <binary> [--no-probe]}
binary=${2:?usage: check.sh <vscode-target> <binary> [--no-probe]}
probe=${3:-}
resolve_target "$target"
[[ -f $binary ]] || { echo "$binary: not found" >&2; exit 1; }

if [[ -n $glibc ]]; then
  highest=$(readelf -V --wide "$binary" | grep -o 'GLIBC_[0-9][0-9.]*' | sed 's/^GLIBC_//' | sort -uV | tail -n 1)
  if [[ -z $highest ]]; then
    echo "$target: no GLIBC_ symbol versions found; is this a glibc binary?" >&2
    exit 1
  fi
  if [[ $(printf '%s\n%s\n' "$highest" "$glibc" | sort -V | tail -n 1) != "$glibc" ]]; then
    echo "$target: needs GLIBC_$highest, above the $glibc floor" >&2
    exit 1
  fi
  echo "$target: highest glibc symbol version $highest (floor $glibc)"
fi

case "$target" in
  linux-* | alpine-*)
    sections=$(readelf -S --wide "$binary")
    symbols=$(readelf -s --wide "$binary")
    ;;
  darwin-*)
    sections=""
    symbols=$(nm "$binary")
    ;;
  *)
    sections="" symbols=clippings4main
    ;;
esac
if [[ $symbols != *clippings4main* ]]; then
  echo "$target: no symbol for clippings::main; backtraces would not name functions" >&2
  exit 1
fi
if [[ $sections == *" .debug_info "* ]]; then
  echo "$target: the binary still carries debug info" >&2
  exit 1
fi
[[ $target == win32-* ]] || echo "$target: symbol table kept, no debug info"

if [[ $probe == --no-probe ]]; then
  echo "$target: probe skipped"
  exit 0
fi

case "$target" in
  linux-* | alpine-*)
    case "$target" in
      *-x64) platform=linux/amd64 ;;
      *-arm64) platform=linux/arm64 ;;
      *-armhf) platform=linux/arm/v7 ;;
    esac
    case "$target" in
      linux-*) image=debian:10 ;;
      alpine-*) image=alpine:3.22 ;;
    esac
    dir=$(cd "$(dirname "$binary")" && pwd)
    json=$(docker run --rm --platform "$platform" -v "$dir:/probe:ro" "$image" "/probe/$(basename "$binary")" probe)
    ;;
  *)
    json=$("$binary" probe)
    ;;
esac

expected=$(sed -n 's/^export const PROTOCOL_VERSION = \([0-9][0-9]*\);$/\1/p' "$root/extension/src/protocol.ts")
[[ -n $expected ]] || { echo "PROTOCOL_VERSION not found in extension/src/protocol.ts" >&2; exit 1; }
echo "$target: probe: $json"
if ! jq -e --argjson v "$expected" '.protocolVersion == $v' <<<"$json" >/dev/null; then
  echo "$target: expected protocolVersion $expected" >&2
  exit 1
fi
```

Run: `chmod 755 scripts/dist/check.sh`
Expected: no output.

- [ ] **Step 2: Check a good binary and two bad ones**

Run: `scripts/dist/build.sh darwin-arm64 && scripts/dist/check.sh darwin-arm64 dist/darwin-arm64/clippings`
Expected: `darwin-arm64: symbol table kept, no debug info` and a probe line with `"protocolVersion":1`.

A fully stripped binary, as the `release` profile builds, must be refused:

Run: `cargo build --locked --release -p clippings && scripts/dist/check.sh darwin-arm64 target/release/clippings --no-probe`
Expected: FAIL with `darwin-arm64: no symbol for clippings::main; backtraces would not name functions`.

A binary built against a newer glibc must be refused (Review Focus 5). This builds the debug server with plain cargo on Debian bookworm, whose glibc is 2.36, in a throwaway container:

Run: `docker run --rm -v "$PWD":/src:ro -w /src rust:1-bookworm bash -c 'CARGO_TARGET_DIR=/tmp/target cargo build --locked -q -p clippings && scripts/dist/check.sh linux-x64 /tmp/target/debug/clippings --no-probe'`
Expected: FAIL with `linux-x64: needs GLIBC_2.34, above the 2.28 floor`.

- [ ] **Step 3: Run the checks in the build workflow**

Replace `.github/workflows/build.yml` with:

```yaml
# Builds the server for the nine VS Code targets and the ten VSIX packages
# (spec 8.1, 8.2). Called by the release workflow and by CI on main.
#
# Artifacts:
#   server-<target>    the stripped server binary
#   symbols-<target>   its debug symbols (dSYM zip, PDB or .debug file)
#   vsix               the nine platform packages and the universal package
name: Build

on:
  workflow_call:
    outputs:
      version:
        description: The extension version, from package.json.
        value: ${{ jobs.package.outputs.version }}
      pre-release:
        description: "'true' when the version is a pre-release (odd minor)."
        value: ${{ jobs.package.outputs.pre-release }}

permissions:
  contents: read

jobs:
  server:
    name: server (${{ matrix.target }})
    strategy:
      fail-fast: false
      matrix:
        include:
          - { target: win32-x64, triple: x86_64-pc-windows-msvc, os: windows-latest }
          # The x64 runner cannot run an Arm64 binary; `probe-win32-arm64` does.
          - { target: win32-arm64, triple: aarch64-pc-windows-msvc, os: windows-latest, probe: --no-probe }
          - { target: darwin-x64, triple: x86_64-apple-darwin, os: macos-latest }
          - { target: darwin-arm64, triple: aarch64-apple-darwin, os: macos-latest }
          - { target: linux-x64, triple: x86_64-unknown-linux-gnu, os: ubuntu-latest }
          - { target: linux-arm64, triple: aarch64-unknown-linux-gnu, os: ubuntu-latest }
          - { target: linux-armhf, triple: armv7-unknown-linux-gnueabihf, os: ubuntu-latest }
          - { target: alpine-x64, triple: x86_64-unknown-linux-musl, os: ubuntu-latest }
          - { target: alpine-arm64, triple: aarch64-unknown-linux-musl, os: ubuntu-latest }
    runs-on: ${{ matrix.os }}
    defaults:
      run:
        shell: bash
    steps:
      - uses: actions/checkout@v5
      - uses: dtolnay/rust-toolchain@stable
        with:
          targets: ${{ matrix.triple }}
          components: ${{ runner.os == 'Linux' && 'llvm-tools' || '' }}
      - uses: Swatinem/rust-cache@v2
        with:
          key: ${{ matrix.target }}
      - name: Install cargo-zigbuild
        if: runner.os == 'Linux'
        run: |
          python3 -m venv "$RUNNER_TEMP/zig"
          "$RUNNER_TEMP/zig/bin/pip" install cargo-zigbuild==0.23.4 ziglang==0.15.2
          echo "$RUNNER_TEMP/zig/bin" >> "$GITHUB_PATH"
      - name: Set up qemu for the Arm probes
        if: runner.os == 'Linux'
        uses: docker/setup-qemu-action@v3
        with:
          platforms: arm64,arm
      - name: Build
        run: scripts/dist/build.sh ${{ matrix.target }} dist
      - name: Check glibc floor and probe
        run: scripts/dist/check.sh ${{ matrix.target }} dist/${{ matrix.target }}/clippings${{ startsWith(matrix.target, 'win32-') && '.exe' || '' }} ${{ matrix.probe }}
      - uses: actions/upload-artifact@v4
        with:
          name: server-${{ matrix.target }}
          path: dist/${{ matrix.target }}/
          if-no-files-found: error
      - uses: actions/upload-artifact@v4
        with:
          name: symbols-${{ matrix.target }}
          path: dist/symbols/
          if-no-files-found: error

  probe-win32-arm64:
    needs: server
    runs-on: windows-11-arm
    defaults:
      run:
        shell: bash
    steps:
      - uses: actions/checkout@v5
      - uses: actions/download-artifact@v5
        with:
          name: server-win32-arm64
          path: dist/win32-arm64
      - run: scripts/dist/check.sh win32-arm64 dist/win32-arm64/clippings.exe

  package:
    needs: [server, probe-win32-arm64]
    runs-on: ubuntu-latest
    outputs:
      version: ${{ steps.version.outputs.version }}
      pre-release: ${{ steps.version.outputs.pre-release }}
    steps:
      - uses: actions/checkout@v5
      - uses: pnpm/action-setup@v4
        with:
          package_json_file: extension/package.json
      - uses: actions/setup-node@v5
        with:
          node-version: 22
          cache: pnpm
          cache-dependency-path: extension/pnpm-lock.yaml
      - run: pnpm -C extension install --frozen-lockfile
      - id: version
        run: node extension/scripts/version.mjs
      # One bundle serves all ten packages.
      - run: pnpm -C extension build --production
      - uses: actions/download-artifact@v5
        with:
          pattern: server-*
          path: dist/server
      - name: Package
        run: |
          for target in win32-x64 win32-arm64 linux-x64 linux-arm64 linux-armhf alpine-x64 alpine-arm64 darwin-x64 darwin-arm64; do
            exe=clippings
            [[ $target == win32-* ]] && exe=clippings.exe
            node extension/scripts/package.mjs --target "$target" --server "dist/server/server-$target/$exe" --out dist/vsix
          done
          node extension/scripts/package.mjs --universal --out dist/vsix
          ls -l dist/vsix
      - uses: actions/upload-artifact@v4
        with:
          name: vsix
          path: dist/vsix/*.vsix
          if-no-files-found: error
```

- [ ] **Step 4: Check the workflow locally**

Run: `actionlint .github/workflows/*.yml`
Expected: no output and exit code 0.

Without actionlint (`brew install actionlint`, or `go install github.com/rhysd/actionlint/cmd/actionlint@v1.7.12`), at least check that the YAML parses with `ruby -ryaml -e 'ARGV.each { |f| YAML.load_file(f) }' .github/workflows/*.yml`, and rely on the CI run below.

- [ ] **Step 5: Commit**

```bash
git add -A
git commit -F - <<'EOF'
ci: check each binary's glibc floor and probe it natively or under qemu

Co-Authored-By: <model attribution>
Claude-Session: https://claude.ai/code/session_01EuAdHjqQeFrieAZqvsQ325
EOF
```

- [ ] **Step 6: Push and run CI**

Push, then wait for the pull request's CI run (opened in Task 7):

```bash
git push
sleep 10
id=$(gh run list --workflow ci.yml --branch "$(git branch --show-current)" --limit 1 --json databaseId --jq '.[0].databaseId')
gh run watch "$id" --exit-status
```

Expected: `gh run watch` exits 0, and `gh run view "$id"` lists these jobs:

- `rust (ubuntu-latest)`, `rust (macos-latest)`, `rust (windows-latest)`: success
- `extension (ubuntu-latest, VS Code 1.91.0)`, `extension (macos-latest, VS Code 1.91.0)`, `extension (windows-latest, VS Code 1.91.0)`, `extension (ubuntu-latest, VS Code stable)`: success
- `dist`: skipped, because it runs only on pushes to `main`

The build workflow's checks run first in Task 12's dry run, where every `build / server (<target>)` log shows `symbol table kept, no debug info` (not on Windows), the three `linux-*` logs show `highest glibc symbol version 2.28 (floor 2.28)`, and every probe line shows `"protocolVersion":1`.

If a job fails, read its log with `gh run view "$id" --log-failed`, fix the cause in this task's files, amend the commit (`git commit --amend --no-edit`), push with `git push --force-with-lease`, and watch the new run.

### Task 11: Sign and notarize macOS binaries and sign Windows binaries when configured

Signing is optional and off until configured (ruling 20, spec 8.2). `sign-macos.sh` signs a binary with a Developer ID in a throwaway keychain, with the hardened runtime and a secure timestamp, and notarizes it; the build workflow runs it before the check, so the signed binary is probed, and logs that it is skipped when the `MACOS_CERTIFICATE_P12` secret is absent. A `sign-windows` job, gated on the `WINDOWS_SIGNING_ENDPOINT` variable, signs both Windows binaries with Azure Artifact Signing, re-checks them and replaces their artifacts; `probe-win32-arm64` and `package` run whether it ran or was skipped. The callers pass `secrets: inherit` and grant `id-token: write`.

**Files:**
- Create: `scripts/dist/sign-macos.sh`
- Modify: `.github/workflows/build.yml`, `.github/workflows/ci.yml`

**Interfaces:**
- Consumes: `scripts/dist/check.sh` (Task 10).
- Produces: `scripts/dist/sign-macos.sh <binary>`, reading the secrets `MACOS_CERTIFICATE_P12`, `MACOS_CERTIFICATE_PASSWORD`, `MACOS_NOTARY_APPLE_ID` and `MACOS_NOTARY_PASSWORD` and the variables `MACOS_SIGNING_IDENTITY` and `MACOS_NOTARY_TEAM_ID`.
- Produces: the `sign-windows` job in `build.yml`, in the `signing` environment, reading the variables `AZURE_CLIENT_ID`, `AZURE_TENANT_ID`, `WINDOWS_SIGNING_ENDPOINT`, `WINDOWS_SIGNING_ACCOUNT` and `WINDOWS_SIGNING_PROFILE`. Task 12's `build` call passes `secrets: inherit` and `id-token: write` the same way `ci.yml`'s `dist` job now does.

- [ ] **Step 1: Write the macOS signing script**

Create `scripts/dist/sign-macos.sh`:

```bash
#!/usr/bin/env bash
# Signs a macOS server binary with a Developer ID Application certificate
# and notarizes it (docs/release.md, "Signing"). Optional: the build
# workflow runs this only when the certificate secret is configured.
#
# A bare binary cannot carry a stapled ticket, so notarization only records
# it with Apple; Gatekeeper looks the ticket up online. notarytool takes a
# zip of the binary.
#
# Environment:
#   MACOS_CERTIFICATE_P12        base64 of the .p12 holding the certificate and key
#   MACOS_CERTIFICATE_PASSWORD   the .p12's password
#   MACOS_SIGNING_IDENTITY       e.g. "Developer ID Application: Name (TEAMID)"
#   MACOS_NOTARY_APPLE_ID        Apple ID used for notarization
#   MACOS_NOTARY_PASSWORD        an app-specific password for that Apple ID
#   MACOS_NOTARY_TEAM_ID         the team ID
#
# Usage: scripts/dist/sign-macos.sh <binary>
set -euo pipefail

binary=${1:?usage: sign-macos.sh <binary>}
work=$(mktemp -d)
keychain=$work/signing.keychain-db
keychain_password=$(uuidgen)
cleanup() {
  security delete-keychain "$keychain" 2>/dev/null || true
  rm -rf "$work"
}
trap cleanup EXIT

# A throwaway keychain holding only the signing identity.
base64 --decode <<<"$MACOS_CERTIFICATE_P12" >"$work/certificate.p12"
security create-keychain -p "$keychain_password" "$keychain"
security set-keychain-settings -lut 21600 "$keychain"
security unlock-keychain -p "$keychain_password" "$keychain"
security import "$work/certificate.p12" -k "$keychain" -P "$MACOS_CERTIFICATE_PASSWORD" -T /usr/bin/codesign
security set-key-partition-list -S apple-tool:,apple: -s -k "$keychain_password" "$keychain" >/dev/null
security list-keychains -d user -s "$keychain" $(security list-keychains -d user | tr -d '"')

# The hardened runtime and a secure timestamp are required for notarization.
codesign --force --options runtime --timestamp --keychain "$keychain" --sign "$MACOS_SIGNING_IDENTITY" "$binary"
codesign --verify --strict --verbose=2 "$binary"

ditto -c -k --keepParent "$binary" "$work/notarize.zip"
credentials=(--apple-id "$MACOS_NOTARY_APPLE_ID" --password "$MACOS_NOTARY_PASSWORD" --team-id "$MACOS_NOTARY_TEAM_ID")
result=$(xcrun notarytool submit "$work/notarize.zip" "${credentials[@]}" --wait --output-format json)
echo "$result"
if [[ $(jq -r .status <<<"$result") != Accepted ]]; then
  # The log says why Apple rejected the binary.
  xcrun notarytool log "$(jq -r .id <<<"$result")" "${credentials[@]}" || true
  echo "notarization failed" >&2
  exit 1
fi
```

Run: `chmod 755 scripts/dist/sign-macos.sh`
Expected: no output.

- [ ] **Step 2: Add the signing steps to the workflows**

Replace `.github/workflows/build.yml` with:

```yaml
# Builds the server for the nine VS Code targets and the ten VSIX packages
# (spec 8.1, 8.2). Called by the release workflow and by CI on main.
#
# Artifacts:
#   server-<target>    the stripped server binary
#   symbols-<target>   its debug symbols (dSYM zip, PDB or .debug file)
#   vsix               the nine platform packages and the universal package
#
# Signing is optional and off until configured (docs/release.md, "Signing"):
# macOS binaries are signed and notarized when the MACOS_CERTIFICATE_P12
# secret exists, and Windows binaries are signed with Azure Artifact Signing
# when the WINDOWS_SIGNING_ENDPOINT variable exists. Callers pass
# `secrets: inherit` and grant `id-token: write`.
name: Build

on:
  workflow_call:
    outputs:
      version:
        description: The extension version, from package.json.
        value: ${{ jobs.package.outputs.version }}
      pre-release:
        description: "'true' when the version is a pre-release (odd minor)."
        value: ${{ jobs.package.outputs.pre-release }}

permissions:
  contents: read

jobs:
  server:
    name: server (${{ matrix.target }})
    strategy:
      fail-fast: false
      matrix:
        include:
          - { target: win32-x64, triple: x86_64-pc-windows-msvc, os: windows-latest }
          # The x64 runner cannot run an Arm64 binary; `probe-win32-arm64` does.
          - { target: win32-arm64, triple: aarch64-pc-windows-msvc, os: windows-latest, probe: --no-probe }
          - { target: darwin-x64, triple: x86_64-apple-darwin, os: macos-latest }
          - { target: darwin-arm64, triple: aarch64-apple-darwin, os: macos-latest }
          - { target: linux-x64, triple: x86_64-unknown-linux-gnu, os: ubuntu-latest }
          - { target: linux-arm64, triple: aarch64-unknown-linux-gnu, os: ubuntu-latest }
          - { target: linux-armhf, triple: armv7-unknown-linux-gnueabihf, os: ubuntu-latest }
          - { target: alpine-x64, triple: x86_64-unknown-linux-musl, os: ubuntu-latest }
          - { target: alpine-arm64, triple: aarch64-unknown-linux-musl, os: ubuntu-latest }
    runs-on: ${{ matrix.os }}
    defaults:
      run:
        shell: bash
    steps:
      - uses: actions/checkout@v5
      - uses: dtolnay/rust-toolchain@stable
        with:
          targets: ${{ matrix.triple }}
          components: ${{ runner.os == 'Linux' && 'llvm-tools' || '' }}
      - uses: Swatinem/rust-cache@v2
        with:
          key: ${{ matrix.target }}
      - name: Install cargo-zigbuild
        if: runner.os == 'Linux'
        run: |
          python3 -m venv "$RUNNER_TEMP/zig"
          "$RUNNER_TEMP/zig/bin/pip" install cargo-zigbuild==0.23.4 ziglang==0.15.2
          echo "$RUNNER_TEMP/zig/bin" >> "$GITHUB_PATH"
      - name: Set up qemu for the Arm probes
        if: runner.os == 'Linux'
        uses: docker/setup-qemu-action@v3
        with:
          platforms: arm64,arm
      - name: Build
        run: scripts/dist/build.sh ${{ matrix.target }} dist
      - name: Sign and notarize
        if: runner.os == 'macOS'
        env:
          MACOS_CERTIFICATE_P12: ${{ secrets.MACOS_CERTIFICATE_P12 }}
          MACOS_CERTIFICATE_PASSWORD: ${{ secrets.MACOS_CERTIFICATE_PASSWORD }}
          MACOS_SIGNING_IDENTITY: ${{ vars.MACOS_SIGNING_IDENTITY }}
          MACOS_NOTARY_APPLE_ID: ${{ secrets.MACOS_NOTARY_APPLE_ID }}
          MACOS_NOTARY_PASSWORD: ${{ secrets.MACOS_NOTARY_PASSWORD }}
          MACOS_NOTARY_TEAM_ID: ${{ vars.MACOS_NOTARY_TEAM_ID }}
        run: |
          if [[ -z $MACOS_CERTIFICATE_P12 ]]; then
            echo "macOS signing is not configured (no MACOS_CERTIFICATE_P12 secret); the binary stays ad-hoc signed."
            exit 0
          fi
          scripts/dist/sign-macos.sh dist/${{ matrix.target }}/clippings
      - name: Check glibc floor and probe
        run: scripts/dist/check.sh ${{ matrix.target }} dist/${{ matrix.target }}/clippings${{ startsWith(matrix.target, 'win32-') && '.exe' || '' }} ${{ matrix.probe }}
      - uses: actions/upload-artifact@v4
        with:
          name: server-${{ matrix.target }}
          path: dist/${{ matrix.target }}/
          if-no-files-found: error
      - uses: actions/upload-artifact@v4
        with:
          name: symbols-${{ matrix.target }}
          path: dist/symbols/
          if-no-files-found: error

  # Runs only when Windows signing is configured, in the `signing`
  # environment that the Azure identity's federated credential names.
  sign-windows:
    needs: server
    if: vars.WINDOWS_SIGNING_ENDPOINT != ''
    environment: signing
    permissions:
      contents: read
      id-token: write
    runs-on: windows-latest
    defaults:
      run:
        shell: bash
    steps:
      - uses: actions/checkout@v5
      - uses: actions/download-artifact@v5
        with:
          pattern: server-win32-*
          path: dist
      - uses: azure/login@v3
        with:
          client-id: ${{ vars.AZURE_CLIENT_ID }}
          tenant-id: ${{ vars.AZURE_TENANT_ID }}
          allow-no-subscriptions: true
      - uses: azure/artifact-signing-action@v2
        with:
          endpoint: ${{ vars.WINDOWS_SIGNING_ENDPOINT }}
          signing-account-name: ${{ vars.WINDOWS_SIGNING_ACCOUNT }}
          certificate-profile-name: ${{ vars.WINDOWS_SIGNING_PROFILE }}
          files-folder: ${{ github.workspace }}\dist
          files-folder-filter: exe
          files-folder-recurse: true
          file-digest: SHA256
          timestamp-rfc3161: http://timestamp.acs.microsoft.com
          timestamp-digest: SHA256
      - name: Check the signed binaries
        run: |
          scripts/dist/check.sh win32-x64 dist/server-win32-x64/clippings.exe
          scripts/dist/check.sh win32-arm64 dist/server-win32-arm64/clippings.exe --no-probe
      - uses: actions/upload-artifact@v4
        with:
          name: server-win32-x64
          path: dist/server-win32-x64/
          overwrite: true
      - uses: actions/upload-artifact@v4
        with:
          name: server-win32-arm64
          path: dist/server-win32-arm64/
          overwrite: true

  probe-win32-arm64:
    needs: [server, sign-windows]
    # After signing when it runs; `sign-windows` is skipped when unconfigured.
    if: ${{ !cancelled() && !failure() }}
    runs-on: windows-11-arm
    defaults:
      run:
        shell: bash
    steps:
      - uses: actions/checkout@v5
      - uses: actions/download-artifact@v5
        with:
          name: server-win32-arm64
          path: dist/win32-arm64
      - run: scripts/dist/check.sh win32-arm64 dist/win32-arm64/clippings.exe

  package:
    needs: [server, sign-windows, probe-win32-arm64]
    if: ${{ !cancelled() && !failure() }}
    runs-on: ubuntu-latest
    outputs:
      version: ${{ steps.version.outputs.version }}
      pre-release: ${{ steps.version.outputs.pre-release }}
    steps:
      - uses: actions/checkout@v5
      - uses: pnpm/action-setup@v4
        with:
          package_json_file: extension/package.json
      - uses: actions/setup-node@v5
        with:
          node-version: 22
          cache: pnpm
          cache-dependency-path: extension/pnpm-lock.yaml
      - run: pnpm -C extension install --frozen-lockfile
      - id: version
        run: node extension/scripts/version.mjs
      # One bundle serves all ten packages.
      - run: pnpm -C extension build --production
      - uses: actions/download-artifact@v5
        with:
          pattern: server-*
          path: dist/server
      - name: Package
        run: |
          for target in win32-x64 win32-arm64 linux-x64 linux-arm64 linux-armhf alpine-x64 alpine-arm64 darwin-x64 darwin-arm64; do
            exe=clippings
            [[ $target == win32-* ]] && exe=clippings.exe
            node extension/scripts/package.mjs --target "$target" --server "dist/server/server-$target/$exe" --out dist/vsix
          done
          node extension/scripts/package.mjs --universal --out dist/vsix
          ls -l dist/vsix
      - uses: actions/upload-artifact@v4
        with:
          name: vsix
          path: dist/vsix/*.vsix
          if-no-files-found: error
```

Replace `.github/workflows/ci.yml` with:

```yaml
name: CI

on:
  push:
    branches: [main]
  pull_request:

permissions:
  contents: read

jobs:
  rust:
    strategy:
      fail-fast: false
      matrix:
        os: [ubuntu-latest, macos-latest, windows-latest]
    runs-on: ${{ matrix.os }}
    steps:
      - uses: actions/checkout@v5
      - uses: dtolnay/rust-toolchain@stable
        with:
          components: rustfmt, clippy
      - uses: Swatinem/rust-cache@v2
      - run: git config --global init.defaultBranch main
      - run: cargo fmt --all --check
      - run: cargo clippy --locked --all-targets -- -D warnings
      - run: cargo test --all --locked
        env:
          INSTA_UPDATE: "no"

  # Extension unit, script and integration tests (spec 12.5) against the
  # debug server that `pnpm test` builds. VS Code is pinned to the
  # `engines.vscode` floor on every OS, so the suite is deterministic, its
  # download is cached, and the manifest's minimum version is proven; one
  # `stable` job catches upstream changes before users meet them.
  extension:
    name: extension (${{ matrix.os }}, VS Code ${{ matrix.vscode }})
    strategy:
      fail-fast: false
      matrix:
        os: [ubuntu-latest, macos-latest, windows-latest]
        vscode: ["1.91.0"]
        include:
          - os: ubuntu-latest
            vscode: stable
    runs-on: ${{ matrix.os }}
    env:
      CLIPPINGS_TEST_VSCODE: ${{ matrix.vscode }}
    steps:
      - uses: actions/checkout@v5
      - uses: dtolnay/rust-toolchain@stable
      - uses: Swatinem/rust-cache@v2
      - uses: pnpm/action-setup@v4
        with:
          package_json_file: extension/package.json
      - uses: actions/setup-node@v5
        with:
          node-version: 22
          cache: pnpm
          cache-dependency-path: extension/pnpm-lock.yaml
      - name: Cache the VS Code download
        if: matrix.vscode != 'stable'
        uses: actions/cache@v4
        with:
          path: extension/.vscode-test
          key: vscode-test-${{ runner.os }}-${{ runner.arch }}-${{ matrix.vscode }}
      - run: pnpm -C extension install --frozen-lockfile
      - name: pnpm -C extension test
        if: runner.os == 'Linux'
        run: xvfb-run -a pnpm -C extension test
      - name: pnpm -C extension test
        if: runner.os != 'Linux'
        run: pnpm -C extension test

  # The nine server builds and ten packages, on main only: they take longer
  # than the tests, and the release workflow builds them again from its tag.
  dist:
    if: github.event_name == 'push' && github.ref == 'refs/heads/main'
    uses: ./.github/workflows/build.yml
    secrets: inherit
    permissions:
      contents: read
      id-token: write
```

- [ ] **Step 3: Check the unconfigured path locally**

Run: `bash -n scripts/dist/sign-macos.sh && scripts/dist/sign-macos.sh`
Expected: FAIL with `usage: sign-macos.sh <binary>`: the syntax is valid and the script needs its argument.

Run the workflow's macOS signing step as CI runs it with no certificate secret:

Run:

```bash
step=$(ruby -ryaml -e 'puts YAML.load_file(".github/workflows/build.yml")["jobs"]["server"]["steps"].find { |s| s["name"] == "Sign and notarize" }["run"]')
MACOS_CERTIFICATE_P12= bash -eo pipefail -c "${step//\$\{\{ matrix.target \}\}/darwin-arm64}"
```
Expected: `macOS signing is not configured (no MACOS_CERTIFICATE_P12 secret); the binary stays ad-hoc signed.`

Run: `actionlint .github/workflows/*.yml`
Expected: no output and exit code 0.

Without actionlint (`brew install actionlint`, or `go install github.com/rhysd/actionlint/cmd/actionlint@v1.7.12`), at least check that the YAML parses with `ruby -ryaml -e 'ARGV.each { |f| YAML.load_file(f) }' .github/workflows/*.yml`, and rely on the CI run below.

- [ ] **Step 4: Commit**

```bash
git add -A
git commit -F - <<'EOF'
ci: sign and notarize macOS binaries and sign Windows binaries when configured

Co-Authored-By: <model attribution>
Claude-Session: https://claude.ai/code/session_01EuAdHjqQeFrieAZqvsQ325
EOF
```

- [ ] **Step 5: Push and run CI**

Push, then wait for the pull request's CI run (opened in Task 7):

```bash
git push
sleep 10
id=$(gh run list --workflow ci.yml --branch "$(git branch --show-current)" --limit 1 --json databaseId --jq '.[0].databaseId')
gh run watch "$id" --exit-status
```

Expected: `gh run watch` exits 0, and `gh run view "$id"` lists these jobs:

- `rust (ubuntu-latest)`, `rust (macos-latest)`, `rust (windows-latest)`: success
- `extension (ubuntu-latest, VS Code 1.91.0)`, `extension (macos-latest, VS Code 1.91.0)`, `extension (windows-latest, VS Code 1.91.0)`, `extension (ubuntu-latest, VS Code stable)`: success
- `dist`: skipped, because it runs only on pushes to `main`

The signing paths first run in Task 12's dry run: both macOS server logs show the not-configured line and `build / sign-windows` is skipped, while `probe-win32-arm64` and `package` still run.

If a job fails, read its log with `gh run view "$id" --log-failed`, fix the cause in this task's files, amend the commit (`git commit --amend --no-edit`), push with `git push --force-with-lease`, and watch the new run.

### Task 12: Release tagged versions, with a manual dry run and credential-gated publishing

Spec 8.2's release: a `v*` tag builds all ten packages, attaches them and the nine symbol files to a GitHub release, and publishes to the Marketplace and Open VSX only when their credentials are configured. A `plan` job checks the version against the tag (Review Focus 3) and decides what runs (ruling 18): a tag push releases, a manual run releases only with `dry-run` off from a tag, and anything else is a dry run. The Marketplace job signs in with Microsoft Entra ID when configured and falls back to a PAT (ruling 19). This task's dry run is the plan's proof.

**Files:**
- Create: `.github/workflows/release.yml`

**Interfaces:**
- Consumes: `.github/workflows/build.yml` (Tasks 9 to 11) and its `vsix` and `symbols-<target>` artifacts; `extension/scripts/version.mjs` (Task 4).
- Produces: `.github/workflows/release.yml`, triggered by `v*` tags and by `workflow_dispatch` with the boolean input `dry-run` (default true); the `plan` job's outputs `version`, `pre-release`, `release` (`true`|`false`), `marketplace` (`entra`|`pat`|`none`) and `open-vsx` (`true`|`false`); the jobs `build`, `release`, `marketplace` (environment `marketplace`) and `open-vsx`.

- [ ] **Step 1: Write the release workflow**

Create `.github/workflows/release.yml`:

```yaml
# Releases Clippings (spec 8.2). A `v*` tag builds all ten packages,
# attaches them and the debug symbols to a GitHub release, and publishes to
# the Marketplace and Open VSX when their tokens are configured. A manual run
# is a dry run by default: it builds, checks and uploads the packages as
# workflow artifacts, and skips the release and publish jobs.
name: Release

on:
  push:
    tags: ["v*"]
  workflow_dispatch:
    inputs:
      dry-run:
        description: Build and check only; create no release and publish nothing.
        type: boolean
        default: true

permissions:
  contents: read

concurrency:
  group: release-${{ github.ref }}
  cancel-in-progress: false

jobs:
  plan:
    runs-on: ubuntu-latest
    outputs:
      version: ${{ steps.version.outputs.version }}
      pre-release: ${{ steps.version.outputs.pre-release }}
      release: ${{ steps.plan.outputs.release }}
      marketplace: ${{ steps.plan.outputs.marketplace }}
      open-vsx: ${{ steps.plan.outputs.open-vsx }}
    steps:
      - uses: actions/checkout@v5
      - uses: actions/setup-node@v5
        with:
          node-version: 22
      - name: Check the version
        id: version
        env:
          REF_TYPE: ${{ github.ref_type }}
          REF_NAME: ${{ github.ref_name }}
        run: |
          if [[ $REF_TYPE == tag ]]; then
            node extension/scripts/version.mjs --tag "$REF_NAME"
          else
            node extension/scripts/version.mjs
          fi
      - name: Decide what runs
        id: plan
        env:
          EVENT: ${{ github.event_name }}
          REF_TYPE: ${{ github.ref_type }}
          DRY_RUN: ${{ inputs.dry-run }}
          HAS_ENTRA: ${{ vars.AZURE_CLIENT_ID != '' && vars.AZURE_TENANT_ID != '' }}
          HAS_VSCE_PAT: ${{ secrets.VSCE_PAT != '' }}
          HAS_OVSX_PAT: ${{ secrets.OVSX_PAT != '' }}
        run: |
          # A tag push releases. A manual run releases only when dry-run is
          # off, and only from a tag. Anything else is a dry run.
          release=false
          if [[ $EVENT == workflow_dispatch && $DRY_RUN == false ]]; then
            if [[ $REF_TYPE != tag ]]; then
              echo "::error::Only a v* tag can be released; run from a tag or keep dry-run on."
              exit 1
            fi
            release=true
          elif [[ $EVENT == push && $REF_TYPE == tag ]]; then
            release=true
          fi
          # The Marketplace prefers Microsoft Entra ID through OIDC; a PAT
          # is the fallback. Azure DevOps global PATs stop working on
          # 1 December 2026.
          marketplace=none open_vsx=false
          if [[ $release == true ]]; then
            if [[ $HAS_ENTRA == true ]]; then
              marketplace=entra
            elif [[ $HAS_VSCE_PAT == true ]]; then
              marketplace=pat
            fi
            [[ $HAS_OVSX_PAT == true ]] && open_vsx=true
          fi
          {
            echo "release=$release"
            echo "marketplace=$marketplace"
            echo "open-vsx=$open_vsx"
          } | tee -a "$GITHUB_OUTPUT"

  build:
    needs: plan
    uses: ./.github/workflows/build.yml
    # For the optional signing steps.
    secrets: inherit
    permissions:
      contents: read
      id-token: write

  release:
    needs: [plan, build]
    if: needs.plan.outputs.release == 'true'
    runs-on: ubuntu-latest
    permissions:
      contents: write
    steps:
      - uses: actions/download-artifact@v5
        with:
          name: vsix
          path: assets
      - uses: actions/download-artifact@v5
        with:
          pattern: symbols-*
          path: assets
          merge-multiple: true
      - name: Create the GitHub release
        env:
          GH_TOKEN: ${{ github.token }}
          GH_REPO: ${{ github.repository }}
          TAG: ${{ github.ref_name }}
          PRE_RELEASE: ${{ needs.plan.outputs.pre-release }}
        run: |
          ls -l assets
          flags=()
          [[ $PRE_RELEASE == true ]] && flags+=(--prerelease)
          gh release create "$TAG" assets/* --verify-tag --title "Clippings ${TAG#v}" --generate-notes "${flags[@]}"

  # Publishing jobs run only when their credentials are configured. Until
  # the publisher `clippings-dev` is replaced by a real one (spec 7.1),
  # none are and both jobs are skipped. See docs/release.md.
  marketplace:
    needs: [plan, release]
    if: needs.plan.outputs.marketplace != 'none'
    runs-on: ubuntu-latest
    # The Azure identity's federated credential names this environment.
    environment: marketplace
    permissions:
      contents: read
      id-token: write
    steps:
      - uses: actions/checkout@v5
      - uses: pnpm/action-setup@v4
        with:
          package_json_file: extension/package.json
      - uses: actions/setup-node@v5
        with:
          node-version: 22
          cache: pnpm
          cache-dependency-path: extension/pnpm-lock.yaml
      - run: pnpm -C extension install --frozen-lockfile
      - uses: actions/download-artifact@v5
        with:
          name: vsix
          path: vsix
      - name: Sign in to Microsoft Entra ID
        if: needs.plan.outputs.marketplace == 'entra'
        uses: azure/login@v3
        with:
          client-id: ${{ vars.AZURE_CLIENT_ID }}
          tenant-id: ${{ vars.AZURE_TENANT_ID }}
          allow-no-subscriptions: true
      - name: Publish to the Visual Studio Marketplace
        env:
          AUTH: ${{ needs.plan.outputs.marketplace }}
          VSCE_PAT: ${{ needs.plan.outputs.marketplace == 'pat' && secrets.VSCE_PAT || '' }}
          PRE_RELEASE: ${{ needs.plan.outputs.pre-release }}
        run: |
          flags=()
          [[ $AUTH == entra ]] && flags+=(--azure-credential)
          [[ $PRE_RELEASE == true ]] && flags+=(--pre-release)
          pnpm -C extension exec vsce publish --packagePath "$PWD"/vsix/*.vsix --no-dependencies --skip-duplicate "${flags[@]}"

  open-vsx:
    needs: [plan, release]
    if: needs.plan.outputs.open-vsx == 'true'
    runs-on: ubuntu-latest
    steps:
      - uses: actions/setup-node@v5
        with:
          node-version: 22
      - uses: actions/download-artifact@v5
        with:
          name: vsix
          path: vsix
      - name: Publish to Open VSX
        env:
          OVSX_PAT: ${{ secrets.OVSX_PAT }}
          PRE_RELEASE: ${{ needs.plan.outputs.pre-release }}
        run: |
          flags=()
          [[ $PRE_RELEASE == true ]] && flags+=(--pre-release)
          for vsix in vsix/*.vsix; do
            npx --yes ovsx@1.2.0 publish --packagePath "$vsix" --skip-duplicate "${flags[@]}"
          done
```

- [ ] **Step 2: Check the plan logic locally**

Extract the `Decide what runs` script and run it for eight combinations of event, ref, dry run and credentials:

Run:

```bash
plan=$(mktemp)
ruby -ryaml -e 'puts YAML.load_file(".github/workflows/release.yml")["jobs"]["plan"]["steps"].find { |s| s["id"] == "plan" }["run"]' >"$plan"
cases=0 bad=0
decide() { # event ref-type dry-run entra vsce-pat ovsx-pat expected
  local got
  got=$(EVENT=$1 REF_TYPE=$2 DRY_RUN=$3 HAS_ENTRA=$4 HAS_VSCE_PAT=$5 HAS_OVSX_PAT=$6 GITHUB_OUTPUT=/dev/null \
    bash -eo pipefail "$plan" 2>&1 | tr '\n' ' ' | sed 's/ $//') || true
  cases=$((cases + 1))
  if [[ $got != "$7" ]]; then
    bad=$((bad + 1))
    echo "case $cases ($1 $2 dry-run=$3): got '$got', expected '$7'"
  fi
}
decide push tag "" false false false 'release=true marketplace=none open-vsx=false'
decide push tag "" true true true 'release=true marketplace=entra open-vsx=true'
decide push tag "" false true false 'release=true marketplace=pat open-vsx=false'
decide push branch "" true true true 'release=false marketplace=none open-vsx=false'
decide workflow_dispatch branch true true true true 'release=false marketplace=none open-vsx=false'
decide workflow_dispatch tag true true true true 'release=false marketplace=none open-vsx=false'
decide workflow_dispatch tag false true false true 'release=true marketplace=entra open-vsx=true'
decide workflow_dispatch branch false true true true '::error::Only a v* tag can be released; run from a tag or keep dry-run on.'
rm -f "$plan"
echo "$cases cases, $bad wrong"
[[ $bad == 0 ]]
```
Expected: `8 cases, 0 wrong`.

Run: `actionlint .github/workflows/*.yml`
Expected: no output and exit code 0.

Without actionlint (`brew install actionlint`, or `go install github.com/rhysd/actionlint/cmd/actionlint@v1.7.12`), at least check that the YAML parses with `ruby -ryaml -e 'ARGV.each { |f| YAML.load_file(f) }' .github/workflows/*.yml`, and rely on the CI run below.

- [ ] **Step 3: Commit**

```bash
git add -A
git commit -F - <<'EOF'
ci: release tagged versions, with a manual dry run and token-gated publishing

Co-Authored-By: <model attribution>
Claude-Session: https://claude.ai/code/session_01EuAdHjqQeFrieAZqvsQ325
EOF
```

- [ ] **Step 4: Push, run CI, and run the release workflow as a dry run**

Push, then wait for the pull request's CI run (opened in Task 7):

```bash
git push
sleep 10
id=$(gh run list --workflow ci.yml --branch "$(git branch --show-current)" --limit 1 --json databaseId --jq '.[0].databaseId')
gh run watch "$id" --exit-status
```

Expected: `gh run watch` exits 0, and `gh run view "$id"` lists these jobs:

- `rust (ubuntu-latest)`, `rust (macos-latest)`, `rust (windows-latest)`: success
- `extension (ubuntu-latest, VS Code 1.91.0)`, `extension (macos-latest, VS Code 1.91.0)`, `extension (windows-latest, VS Code 1.91.0)`, `extension (ubuntu-latest, VS Code stable)`: success
- `dist`: skipped, because it runs only on pushes to `main`

If a job fails, read its log with `gh run view "$id" --log-failed`, fix the cause in this task's files, amend the commit (`git commit --amend --no-edit`), push with `git push --force-with-lease`, and watch the new run.

Then dispatch the dry run. Keep `dry-run=true`; never dispatch with it off (Global Constraints).

```bash
branch=$(git branch --show-current)
gh workflow run release.yml --ref "$branch" -f dry-run=true
sleep 10
rid=$(gh run list --workflow release.yml --branch "$branch" --limit 1 --json databaseId --jq '.[0].databaseId')
gh run watch "$rid" --exit-status
```

Expected: `gh run watch` exits 0 after about ten minutes, and `gh run view "$rid"` lists:

- `plan`: success
- `build / server (<target>)` for all nine targets: success
- `build / sign-windows`: skipped (signing is not configured)
- `build / probe-win32-arm64`, `build / package`: success
- `release`, `marketplace`, `open-vsx`: skipped (a dry run)

Check the logs and the packages:

```bash
gh run view "$rid" --log | grep -E 'macOS signing is not configured|highest glibc|symbol table kept|protocolVersion' | sort | uniq -c | sort -rn | head -30
rm -rf dist/ci-vsix && gh run download "$rid" -n vsix -D dist/ci-vsix
for f in dist/ci-vsix/*.vsix; do
  t=$(basename "$f" .vsix); t=${t#clippings-}; t=${t%-*}
  if [[ $t == universal ]]; then node extension/scripts/vsix.mjs "$f" --universal
  else node extension/scripts/vsix.mjs "$f" --target "$t"; fi
done
```

Expected: the log shows the not-configured line for `darwin-x64` and `darwin-arm64`, `highest glibc symbol version 2.28` for the three `linux-*` targets, `symbol table kept, no debug info` for the seven macOS, Linux and Alpine targets, and `"protocolVersion":1` for all nine probes plus the Arm64 re-probe. Ten packages download, and each prints `ok (<target>, pre-release)`, the nine platform ones with `bin/clippings` or `bin/clippings.exe`. The prototype's packages were 2.3 to 2.8 MB, and the universal one 262 KB.

If a job fails, `gh run view "$rid" --log-failed` shows why; fix it in the file of the task that owns the step (`build.sh` Task 2, `package.mjs` Task 5, the matrix Task 9, `check.sh` Task 10, signing Task 11, this workflow here), commit, push, and dispatch again.

### Task 13: Describe how to cut, check and publish a release

`docs/release.md` is the release procedure spec 8.2 now points to: what a release contains, versions, cutting a release, the dry run, turning on publishing (the publisher-ID switch, Entra ID, the PAT fallback and Open VSX, rulings 19 and 21), turning on signing (ruling 20), how the build works, and checking packages, including in an isolated VS Code.

**Files:**
- Create: `docs/release.md`

**Interfaces:**
- None; later readers rely on the section names `Dry run`, `Publishing`, `Signing` and `Checking packages`.

- [ ] **Step 1: Write the release procedure**

Create `docs/release.md`:

````markdown
# Releasing Clippings

How the ten packages are built, how to cut a release, how to turn on publishing, and how to check a release before and after. The rules come from spec section 8.

## What a release contains

| Asset | What it is |
|---|---|
| `clippings-<target>-<version>.vsix` | one package per VS Code target, with that target's server in `extension/bin/` beside `platform.ok` |
| `clippings-universal-<version>.vsix` | the package without a server; it works only when `clippings.server.path` points at a server the user built |
| `clippings-darwin-<arch>.dSYM.zip` | macOS debug symbols |
| `clippings-win32-<arch>.pdb` | Windows debug symbols |
| `clippings-<linux or alpine>-<arch>.debug` | Linux and Alpine debug symbols; the shipped binary's `.gnu_debuglink` names this file |

The targets are `win32-x64`, `win32-arm64`, `linux-x64`, `linux-arm64`, `linux-armhf`, `alpine-x64`, `alpine-arm64`, `darwin-x64` and `darwin-arm64`. The `linux-*` servers need glibc 2.28 or later.

## Versions

Versions are plain `major.minor.patch`: the Marketplace rejects semver pre-release suffixes. An **odd minor** version is a pre-release, packaged and published with `--pre-release`; an **even minor** version is stable. So `0.1.x` is the first pre-release line and `0.2.0` the first stable release.

The version lives in two places that must agree: `extension/package.json` and `[workspace.package]` in the root `Cargo.toml`. `node extension/scripts/version.mjs` checks both and prints the channel; the release workflow also checks that the tag is `v` plus that version.

## Cutting a release

1. Bump the version in `extension/package.json` and `Cargo.toml`, run `cargo check` so `Cargo.lock` follows, and commit on `main`.
2. Check it locally:

   ```sh
   node extension/scripts/version.mjs --tag v0.1.0
   ```

3. Run the release workflow as a dry run on `main` and wait for it to pass (see [Dry run](#dry-run)).
4. Tag and push:

   ```sh
   git tag v0.1.0
   git push origin v0.1.0
   ```

The tag starts `.github/workflows/release.yml`, which:

1. **plan**: checks the tag against the version and decides the channel and which publishing jobs run;
2. **build** (`.github/workflows/build.yml`): builds the server for the nine targets, checks each, and packages the ten VSIX files;
3. **release**: creates the GitHub release for the tag with generated notes, marked pre-release for odd minors, with the ten packages and nine symbol files attached;
4. **marketplace** and **open-vsx**: publish the ten packages, each only when its credentials are configured (see [Publishing](#publishing)).

If the release job fails after the build, re-run the failed jobs from the workflow run page. Publishing uses `--skip-duplicate`, so re-running a publish job skips the packages that were already accepted.

## Dry run

Actions > Release > Run workflow, with **dry-run** ticked (the default), or:

```sh
gh workflow run release.yml --ref main
gh run watch
```

A dry run does everything except the release and publishing jobs, which show as skipped. The packages are the run's `vsix` artifact, and each target's server and symbols are the `server-<target>` and `symbols-<target>` artifacts. A manual run with dry-run off is refused unless it runs from a `v*` tag.

CI builds the same packages on every push to `main` (the `dist` job in `.github/workflows/ci.yml`).

## Publishing

Publishing is off until the extension has a real publisher. Today `publisher` in `extension/package.json` is the placeholder `clippings-dev` (spec 7.1), no credentials are configured, and both publishing jobs are skipped. Each store is turned on separately, by configuring its credentials.

### Switching the publisher ID

1. Create a Marketplace publisher at <https://marketplace.visualstudio.com/manage>. Its ID is permanent and becomes the first half of the extension ID, `<publisher>.clippings`.
2. Replace `clippings-dev` with the new ID in:
   - `extension/package.json` (`publisher`);
   - `extension/src/test/unit/manifest.test.ts`;
   - the `getExtension('clippings-dev.clippings')` calls in `extension/src/test/integration/` (`helpers.ts`, `activation.test.ts`, `commands.test.ts`, `server.test.ts`);
   - the comment on the `marketplace` job in `.github/workflows/release.yml`, and the log path under [Checking packages](#checking-packages).

   `git grep clippings-dev -- extension .github docs/release.md` lists them. `extension/scripts/vsix.test.mjs` uses the ID only as sample data and can stay.
3. Update spec 7.1, which records the placeholder.

Use the same ID as the Open VSX namespace so the extension has one ID everywhere.

### Visual Studio Marketplace: Microsoft Entra ID (preferred)

Azure DevOps global personal access tokens, the kind `vsce` has used, are retired on 1 December 2026. The `marketplace` job therefore signs in with Microsoft Entra ID through GitHub's OIDC token (`azure/login`) and publishes with `vsce publish --azure-credential`. No secret is stored.

One-time setup:

1. In the Azure portal, create a **user-assigned managed identity** (Managed Identities > Create; any subscription and resource group), or an **app registration** in Microsoft Entra ID. Note its **client ID** and your **tenant ID**.
2. Add a **federated credential** to it (managed identity: Settings > Federated credentials; app registration: Certificates & secrets > Federated credentials), with the scenario **GitHub Actions deploying Azure resources**:
   - organization `GeorgeIpsum`, repository `clippings`;
   - entity type **Environment**, environment name `marketplace`. The release workflow's `marketplace` job runs in that environment, so the subject is `repo:GeorgeIpsum/clippings:environment:marketplace`;
   - audience `api://AzureADTokenExchange`, the default.
3. Add the identity to the Marketplace publisher: at <https://marketplace.visualstudio.com/manage/publishers/>, open the publisher's **Members**, add the identity by its name (for an app registration, the name of the service principal) and give it the **Contributor** role. Publisher membership needs the identity to be known to Azure DevOps: if the Members dialog cannot find it, first add it as a user of an Azure DevOps organization in the same tenant (Organization settings > Users, Stakeholder access is enough).
4. In the GitHub repository, set the **variables** (Settings > Secrets and variables > Actions > Variables, at repository level so the `plan` job can see them) `AZURE_CLIENT_ID` and `AZURE_TENANT_ID`.
5. Optionally, protect the `marketplace` environment (Settings > Environments) with required reviewers, so each publish waits for an approval.

A subscription is not needed: the login uses `allow-no-subscriptions`.

### Visual Studio Marketplace: personal access token (fallback)

When `AZURE_CLIENT_ID` and `AZURE_TENANT_ID` are not both set, the job falls back to a `VSCE_PAT` repository secret: an Azure DevOps personal access token with the **Marketplace > Manage** scope. Marketplace publishing has traditionally needed a global (*All accessible organizations*) token, and global tokens are retired on 1 December 2026, so treat this path as temporary. With neither configured, the `marketplace` job is skipped.

### Open VSX

1. Create an Eclipse Foundation account at <https://accounts.eclipse.org> and sign in to <https://open-vsx.org> with GitHub, linking the Eclipse account in your profile settings.
2. Sign the **Eclipse Foundation Open VSX Publisher Agreement** from your Open VSX profile.
3. Create an access token under Settings > Access Tokens.
4. Create the namespace, which must equal the `publisher` ID, once:

   ```sh
   npx ovsx create-namespace <publisher> -p <token>
   ```

5. Save the token as the repository secret `OVSX_PAT`.

The `open-vsx` job publishes each package with `ovsx publish --skip-duplicate`, plus `--pre-release` on odd minors, and is skipped without `OVSX_PAT`.

## Signing

Signing is optional and off by default. The build workflow's signing steps run only when their credentials are configured; unconfigured, macOS logs that it keeps the linker's ad-hoc signature and the Windows signing job is skipped, which every dry run shows.

Packages installed from the Marketplace, Open VSX or a VSIX file do not need signed binaries: files that VS Code extracts carry no macOS quarantine attribute and no Windows Mark-of-the-Web, so Gatekeeper and SmartScreen do not inspect the server. Signing matters for binaries users download from the GitHub release and run themselves, and for managed machines whose policies require signed code.

### macOS: Developer ID and notarization

`scripts/dist/sign-macos.sh` imports the certificate into a throwaway keychain, signs the binary with the hardened runtime and a secure timestamp, verifies the signature, and submits a zip of the binary to `notarytool`, failing unless Apple accepts it. A bare binary cannot have its ticket stapled; Gatekeeper finds the ticket online. The dSYM is unaffected, because signing does not change the binary's UUID.

Configure, in the repository settings:

| Name | Kind | Value |
|---|---|---|
| `MACOS_CERTIFICATE_P12` | secret | base64 of a `.p12` export of the **Developer ID Application** certificate and its private key (`base64 -i cert.p12`) |
| `MACOS_CERTIFICATE_PASSWORD` | secret | the `.p12` password |
| `MACOS_SIGNING_IDENTITY` | variable | the certificate's name, `Developer ID Application: <Name> (<TEAMID>)` |
| `MACOS_NOTARY_APPLE_ID` | secret | the Apple ID that notarizes |
| `MACOS_NOTARY_PASSWORD` | secret | an app-specific password for that Apple ID (appleid.apple.com > Sign-In and Security) |
| `MACOS_NOTARY_TEAM_ID` | variable | the Apple Developer team ID |

Signing runs when `MACOS_CERTIFICATE_P12` is set, and the check that follows probes the signed binary.

### Windows: Azure Artifact Signing

The `sign-windows` job signs both Windows binaries with Azure Artifact Signing (formerly Trusted Signing) through `azure/artifact-signing-action`, then probes the signed `win32-x64` binary and replaces the `server-win32-*` artifacts; the Arm64 probe and the packages use the signed binaries. The PDBs still match, because an Authenticode signature does not change the debug directory.

One-time setup:

1. Create an **Artifact Signing account** and complete **identity validation**, then create a **certificate profile** (Public Trust) in it.
2. Use the same kind of Azure identity as for the Marketplace (it can be the same one) and give it the **Artifact Signing Certificate Profile Signer** role on the signing account.
3. Add a federated credential for entity type **Environment**, environment name `signing` (subject `repo:GeorgeIpsum/clippings:environment:signing`). The `sign-windows` job runs in that environment. CI's `dist` job on `main` signs too, so leave the environment unprotected or allow `main` and `v*` tags in its deployment rules.
4. Set the repository variables `AZURE_CLIENT_ID` and `AZURE_TENANT_ID` (shared with the Marketplace), and:

| Variable | Value |
|---|---|
| `WINDOWS_SIGNING_ENDPOINT` | the account's region endpoint, such as `https://eus.codesigning.azure.net/` |
| `WINDOWS_SIGNING_ACCOUNT` | the Artifact Signing account name |
| `WINDOWS_SIGNING_PROFILE` | the certificate profile name |

The job runs when `WINDOWS_SIGNING_ENDPOINT` is set. Signatures are timestamped by `http://timestamp.acs.microsoft.com`, so they outlive the short-lived signing certificates.

## How the build works

`scripts/dist/build.sh <target>` builds one server with the `dist` profile: the `release` profile (fat LTO, one codegen unit, `panic = "unwind"`) plus full debug info, which it then moves into the symbols file:

- **macOS** builds natively with `split-debuginfo = "packed"`; rustc writes the dSYM, then strips the debug info from the binary (`strip = "debuginfo"`).
- **Windows** builds natively; MSVC writes the PDB beside the binary, which carries no debug info.
- **Linux and Alpine** build with `cargo zigbuild`, using `<triple>.2.28` for the glibc floor; `llvm-objcopy` from the rustup `llvm-tools` component splits the debug info into the `.debug` file and strips it from the binary (`--strip-debug`). `CLIPPINGS_BUILD_TOOL=cargo` builds with plain cargo instead, for the `debian:10` container fallback the spec allows; zigbuild builds all five Linux targets today, so CI does not use it.

macOS, Linux and Alpine binaries keep their symbol table, so a panic backtrace in the Clippings output channel (the client sets `RUST_BACKTRACE=1`) names its functions; file and line numbers need the symbols file. This makes those binaries about 15 to 30 % larger. Windows binaries cannot do this: MSVC executables carry no symbol table, and names come only from the PDB, which stays a release asset, so Windows backtraces show `<unknown>` frames unless the PDB is placed beside `clippings.exe`.

The server uses mimalloc on Windows and the system allocator elsewhere.

`scripts/dist/check.sh <target> <binary>` then checks it:

- a macOS, Linux or Alpine binary must have a `clippings::main` symbol, and a Linux or Alpine binary no `.debug_info` section;
- a glibc binary's highest `GLIBC_` symbol version, from `readelf -V`, must not exceed 2.28;
- `clippings probe` must print JSON whose `protocolVersion` equals `PROTOCOL_VERSION` in `extension/src/protocol.ts`. Linux binaries run in `debian:10` (glibc 2.28 exactly) and Alpine binaries in `alpine:3.22`, the Arm ones under qemu; macOS binaries run on the Apple silicon runner, `darwin-x64` under Rosetta; the `win32-arm64` binary runs on a Windows Arm runner.

The package job bundles the extension once with `pnpm -C extension build --production`, then runs `extension/scripts/package.mjs` for each target and for the universal package. It copies the server and `platform.ok` into `extension/bin`, runs `vsce package --no-dependencies --target <target>` (plus `--pre-release` for odd minors), and checks the result with `extension/scripts/vsix.mjs`: the expected files and nothing else, the server's format and architecture, its executable bit, the manifest's target, version and pre-release flag, and no server in the universal package.

## Checking packages

Check a downloaded package:

```sh
node extension/scripts/vsix.mjs clippings-linux-x64-0.1.0.vsix --target linux-x64
node extension/scripts/vsix.mjs clippings-universal-0.1.0.vsix --universal
```

Try one in an isolated VS Code, without touching your own profile (keep the directory short: VS Code's IPC socket path has a length limit on macOS):

```sh
dir=$(mktemp -d /tmp/cv.XXXX)
code --user-data-dir "$dir" --extensions-dir "$dir/ext" --install-extension clippings-darwin-arm64-0.1.0.vsix
code --user-data-dir "$dir" --extensions-dir "$dir/ext" --new-window some-folder
grep -r "Using bundled server" "$dir/logs"
```

The Clippings output channel, also written to `logs/*/window1/exthost/clippings-dev.clippings/Clippings.log`, starts with `Using bundled server (<extension>/bin/clippings): clippings <version> (<arch>-<os>)`.

## Packaging locally

On macOS or Windows, for the machine's own target:

```sh
scripts/dist/build.sh darwin-arm64
pnpm -C extension install
pnpm -C extension build --production
pnpm -C extension package --target darwin-arm64 --server ../dist/darwin-arm64/clippings
pnpm -C extension package --universal
```

Packages land in `dist/vsix/`. Linux targets need `cargo-zigbuild` and zig, or run the same commands in a Linux container.
````

- [ ] **Step 2: Check the files it names**

Run: `git grep --untracked -l clippings-dev -- extension .github docs/release.md`
Expected: the list the publisher-ID section gives: `.github/workflows/release.yml`, `docs/release.md`, `extension/package.json`, `extension/scripts/vsix.test.mjs`, `extension/src/test/integration/activation.test.ts`, `extension/src/test/integration/commands.test.ts`, `extension/src/test/integration/helpers.ts`, `extension/src/test/integration/server.test.ts` and `extension/src/test/unit/manifest.test.ts`.

Then read the document once through, running nothing that tags, releases or publishes.

- [ ] **Step 3: Commit**

```bash
git add -A
git commit -F - <<'EOF'
docs: describe how to cut, check and publish a release

Co-Authored-By: <model attribution>
Claude-Session: https://claude.ai/code/session_01EuAdHjqQeFrieAZqvsQ325
EOF
```

### Task 14: Record this plan's rulings in the spec

The spec is the binding authority, so the rulings at the top of this plan go into it wherever they change or clarify behaviour, and `clippings bench` moves out of the packaging milestone (ruling 24). Rulings 3, 11, 13, 14, 21, 22 and 23 need no spec edit: they are implementation choices recorded in the code, `docs/release.md` and this plan.

**Files:**
- Modify: `docs/superpowers/specs/2026-09-23-clippings-design.md` (diff)

**Interfaces:**
- None.

- [ ] **Step 1: Edit the spec**

The edits, section by section:

1. **Section 5.12**, rendering: the flat view's directory label uses `/` on every platform (ruling 12).
2. **Section 8.2**, build: the zigbuild armhf fallback exists but is unused (ruling 4); every binary is probed and must report `PROTOCOL_VERSION`, with where each runs (ruling 5); the `dist` profile, debug info stripped with the symbol table kept, the three symbols formats, and the Windows exception (rulings 1 and 2); `pnpm build --production`, the full `.vscodeignore` whitelist and the package checks (rulings 6 to 8); pre-release parity and the Cargo version (rulings 9 and 10); the tag check, the symbol assets, Entra-preferred publishing with the PAT fallback and its 1 December 2026 retirement, optional signing and why packages do not need it, the dry run, CI's `main` builds, and `docs/release.md` (rulings 17 to 20).
3. **Section 12.5**: CI runs the suite on three operating systems pinned to 1.91.0, plus `stable` on Linux (ruling 15), and the path-form tests (ruling 16).
4. **Section 13**: the view rebuild optimisation belongs to the benchmarks and performance milestone, not to "plan 4" (ruling 24).
5. **Section 14**: milestone 4 no longer lists `clippings bench` and the first tilliX results, and lists the extension tests on three operating systems; milestone 5 no longer lists the final benchmark run; a new milestone 6, Benchmarks and performance, takes all three (ruling 24).

Apply this diff to `docs/superpowers/specs/2026-09-23-clippings-design.md` with `git apply`. It is the exact text of the edits above:

```diff
diff --git a/docs/superpowers/specs/2026-09-23-clippings-design.md b/docs/superpowers/specs/2026-09-23-clippings-design.md
--- a/docs/superpowers/specs/2026-09-23-clippings-design.md
+++ b/docs/superpowers/specs/2026-09-23-clippings-design.md
@@ -276,7 +276,7 @@
 **Rendering** happens in Rust. Each node carries:
 
 - `id`, as above.
-- `label`: for todos, `tree.labelFormat` applied with placeholders `line`, `column`, `tag`, `subtag`, `before`, `after`, `afterorbefore`, `filename` and `filepath`. Placeholder names are case-insensitive. `tag` and `subtag` accept `:uppercase`, `:lowercase` and `:capitalize`. An empty format, and any todo with extra lines, uses the raw label. For folders, the name. For files, the name, plus ` (<relative directory>)` in the flat view. For non-`file` documents, the URI's path basename.
+- `label`: for todos, `tree.labelFormat` applied with placeholders `line`, `column`, `tag`, `subtag`, `before`, `after`, `afterorbefore`, `filename` and `filepath`. Placeholder names are case-insensitive. `tag` and `subtag` accept `:uppercase`, `:lowercase` and `:capitalize`. An empty format, and any todo with extra lines, uses the raw label. For folders, the name. For files, the name, plus ` (<relative directory>)` in the flat view, with `/` separators on every platform. For non-`file` documents, the URI's path basename.
 - `description`: the count when `tree.showCountsInTree` is true and the node is a container; the status text for status nodes; otherwise none.
 - `tooltip`: `tree.tooltipFormat` for todos, the path for folders and files, the full URI for non-`file` documents, `Click to open <url>` for sub-tag nodes with a click URL.
 - `icon`: an icon descriptor (section 5.13), or none for todos when `tree.hideIconsWhenGroupedByTag` is true and grouping is active.
@@ -561,12 +561,12 @@
 
 ### 8.2 Build
 
-- macOS and Windows build on native runners. Linux and Alpine targets build with `cargo-zigbuild`, using its glibc-suffixed targets for the 2.28 floor. If zigbuild cannot build `linux-armhf`, that target builds in a `debian:10` container with native gcc, as rust-analyzer does.
-- CI checks that each glibc binary's highest `GLIBC_` symbol version is 2.28 or lower, and runs `clippings probe` under qemu for the arm64 and armv7 Linux binaries.
-- Release profile: link-time optimisation, one codegen unit, `panic = "unwind"`, symbols stripped from the shipped binary and kept as separate release assets. The global allocator is mimalloc on Windows and the system allocator elsewhere, as rust-analyzer does.
-- The extension has no `vscode:prepublish` script. A separate `pnpm build` step produces the esbuild bundle once. Each target then copies its binary and `platform.ok` into `extension/bin` and runs `vsce package --no-dependencies --target <target>`. `.vscodeignore` whitelists `dist/**` and `bin/**`.
-- Versions are plain `major.minor.patch`, because the Marketplace rejects semver pre-release versions. Odd minor versions are pre-releases, published with `--pre-release`. Even minor versions are stable.
-- Tags matching `v*` build all ten packages and attach them to a GitHub release. Publishing to the Marketplace and Open VSX runs only when their tokens are configured, with `--no-dependencies` and skip-duplicate.
+- macOS and Windows build on native runners. Linux and Alpine targets build with `cargo-zigbuild`, using its glibc-suffixed targets for the 2.28 floor. If zigbuild cannot build `linux-armhf`, that target builds in a `debian:10` container with native gcc, as rust-analyzer does. Zigbuild builds all five Linux and Alpine targets, so the fallback exists in `scripts/dist/build.sh` but CI does not use it.
+- CI checks that each glibc binary's highest `GLIBC_` symbol version is 2.28 or lower, and runs `clippings probe` under qemu for the arm64 and armv7 Linux binaries. Every binary is probed, and the probe must report the extension's `PROTOCOL_VERSION`: Linux binaries in `debian:10`, whose glibc is exactly 2.28, and Alpine binaries in Alpine, the Arm ones under qemu; macOS binaries on the Apple silicon runner, `darwin-x64` under Rosetta; `win32-arm64` on a Windows Arm runner.
+- Release profile: link-time optimisation, one codegen unit, `panic = "unwind"`, debug info stripped from the shipped binary and kept as separate release assets. The global allocator is mimalloc on Windows and the system allocator elsewhere, as rust-analyzer does. Shipped binaries use a `dist` profile, `release` plus full debug info, which `scripts/dist/build.sh` moves into a dSYM on macOS, a PDB on Windows, and a `.debug` file named by the binary's `.gnu_debuglink` on Linux and Alpine, and strips the debug info from the binary. The binary keeps its symbol table, so a panic backtrace in the output channel names its functions; CI checks this. Windows is the exception: an MSVC executable has no symbol table, its function names live only in the PDB, and the PDB is too large to ship in the package, so Windows backtraces name functions only when the PDB from the release is placed beside the binary. `release` itself stays fully stripped, without debug info.
+- The extension has no `vscode:prepublish` script. A separate `pnpm build --production` step produces the minified esbuild bundle once. Each target then copies its binary and `platform.ok` into `extension/bin` and runs `vsce package --no-dependencies --target <target>`. `.vscodeignore` whitelists `dist/**` and `bin/**`, plus `resources/**`, `package.json`, `LICENSE` and `README.md`. Every package is checked after packaging: the expected files and nothing else, the server's executable format, architecture and executable bit, and the manifest's target, version and pre-release flag; the universal package must have no `bin/`.
+- Versions are plain `major.minor.patch`, because the Marketplace rejects semver pre-release versions. Odd minor versions are pre-releases, packaged and published with `--pre-release`. Even minor versions are stable. The Cargo workspace version equals the extension's, so `clippings probe` reports the same version.
+- Tags matching `v*` build all ten packages and attach them and the nine symbol files to a GitHub release, after checking that the tag is `v` plus the version. Publishing to the Marketplace and Open VSX runs only when their credentials are configured, with `--no-dependencies` and skip-duplicate. The Marketplace job signs in with Microsoft Entra ID through GitHub OIDC and `vsce publish --azure-credential`, falling back to a `VSCE_PAT` token, because Azure DevOps global personal access tokens are retired on 1 December 2026; Open VSX uses an `OVSX_PAT` token. Signing is optional and off until configured: macOS binaries are signed with a Developer ID and notarized, and Windows binaries are signed with Azure Artifact Signing. Packages do not need it, because files VS Code extracts carry no quarantine attribute or Mark-of-the-Web. A manual run of the release workflow is a dry run unless told otherwise: it builds and checks everything and keeps the packages as workflow artifacts. CI builds the ten packages on every push to `main`. `docs/release.md` is the release procedure.
 
 ### 8.3 Local development
 
@@ -695,7 +695,7 @@
 
 `@vscode/test-electron` over the fixture workspace: tree labels in each view mode, every command, expand and collapse, decorations through a test hook, status bar text, context keys, settings import. This is a smoke suite, not an exhaustive one.
 
-Each run copies `tests/fixtures/workspace/` to a temporary directory and starts VS Code with a fresh user data directory holding quiet settings, so tests never touch the repository or the user's profile. Workspace trust is disabled, so untrusted behaviour is covered by unit tests. The server comes from `CLIPPINGS_SERVER_PATH`, which defaults to the debug build. Tests read state through test hooks that `activate` returns, and answer prompts through the one prompts module every command uses. The VS Code version is `stable` unless `CLIPPINGS_TEST_VSCODE` names another.
+Each run copies `tests/fixtures/workspace/` to a temporary directory and starts VS Code with a fresh user data directory holding quiet settings, so tests never touch the repository or the user's profile. Workspace trust is disabled, so untrusted behaviour is covered by unit tests. The server comes from `CLIPPINGS_SERVER_PATH`, which defaults to the debug build. Tests read state through test hooks that `activate` returns, and answer prompts through the one prompts module every command uses. The VS Code version is `stable` unless `CLIPPINGS_TEST_VSCODE` names another. CI runs the suite on Linux, macOS and Windows with VS Code pinned to the `engines.vscode` floor, 1.91.0, which is deterministic, cacheable and proves the declared minimum, and once more on Linux with `stable`, to catch upstream changes. Path-form tests open the workspace through another spelling of its path — a directory symlink on every OS, and an NTFS junction and the 8.3 short path on Windows — and check that the tree fills, the active file is revealed, an open file is decorated without a second tree node, and a disk change reaches the tree.
 
 ## 13. Performance targets and benchmarks
 
@@ -722,12 +722,13 @@
 
 Extension host time counts the provider's own synchronous work, handling `clippings/treeChanged`, recording children and building tree items, plus parsing the children response. VS Code's own tree conversion is not visible to the extension and is excluded. The prototype measured about 1.4 ms for 1,000 nodes.
 
-The view rebuild target reflects a prototype measurement of 12.5 ms on the same machine model. Sharing todo data between the index and the view, which would lower it further, is plan 4's optimisation.
+The view rebuild target reflects a prototype measurement of 12.5 ms on the same machine model. Sharing todo data between the index and the view, which would lower it further, is an optimisation for the benchmarks and performance milestone.
 
 ## 14. Milestones
 
 1. **Core**: filesystem boundary, roots, walker, exclusion layers and admission predicate, regex construction, scanner, tag extraction, index, scan modes, `clippings scan --json`, unit and golden tests.
 2. **Server**: view model with IDs and deltas, decorations and styles, status, navigation, export, protocol types, scheduler, `clippings lsp`, `clippings watch`, protocol integration tests.
 3. **Extension**: manifest and settings, import, server resolution and lifecycle, tree provider with client-side expansion, decorations, status bar, all commands, menus and context keys, extension tests.
-4. **Packaging**: CI for all targets with glibc and qemu checks, ten VSIX packages, release workflow, `clippings bench` and the first tilliX results.
-5. **Parity polish**: icons, scopes, export, multi-line edge cases, notebooks, reveal and track file, the parity oracle, final benchmark run.
+4. **Packaging**: CI for all targets with glibc and qemu checks, extension tests on Linux, macOS and Windows, ten VSIX packages, release workflow.
+5. **Parity polish**: icons, scopes, export, multi-line edge cases, notebooks, reveal and track file, the parity oracle.
+6. **Benchmarks and performance**: `clippings bench` with the synthetic corpus, the first tilliX results for every scan set, the section 13 targets measured and recorded in `docs/benchmarks/`, the view rebuild optimisation, and the final benchmark run.
```

- [ ] **Step 2: Check it reads cleanly**

Re-read sections 8, 12.5, 13 and 14 in full. Check that no sentence contradicts an edit, for example a remaining claim that symbols are stripped from the shipped binary, that only the Arm Linux binaries are probed, or that milestone 4 includes benchmarks, and that every edit states a behaviour, not a history.

- [ ] **Step 3: Run the full local suite**

Run: `cargo fmt --all --check && cargo clippy --all-targets -- -D warnings && INSTA_UPDATE=no cargo test --all`
Expected: no diffs, no warnings, and every test passes (204 passed, 2 ignored).

Run: `pnpm -C extension test`
Expected: `88 passing`, `pass 14`, `83 passing` and `4 passing`. A VS Code test window opens, runs the suite and closes.

- [ ] **Step 4: Commit**

The prototype's spec commit lacked the section 13 and 14 edits; the subject stays the same.

```bash
git add -A
git commit -F - <<'EOF'
docs(spec): record the packaging plan's rulings

Co-Authored-By: <model attribution>
Claude-Session: https://claude.ai/code/session_01EuAdHjqQeFrieAZqvsQ325
EOF
```

- [ ] **Step 5: Push and hand over**

Push the final commit and check the pull request's CI once more:

```bash
git push
sleep 10
id=$(gh run list --workflow ci.yml --branch "$(git branch --show-current)" --limit 1 --json databaseId --jq '.[0].databaseId')
gh run watch "$id" --exit-status
```

Expected: `gh run watch` exits 0, and `gh run view "$id"` lists these jobs:

- `rust (ubuntu-latest)`, `rust (macos-latest)`, `rust (windows-latest)`: success
- `extension (ubuntu-latest, VS Code 1.91.0)`, `extension (macos-latest, VS Code 1.91.0)`, `extension (windows-latest, VS Code 1.91.0)`, `extension (ubuntu-latest, VS Code stable)`: success
- `dist`: skipped, because it runs only on pushes to `main`

If a job fails, read its log with `gh run view "$id" --log-failed`, fix the cause in this task's files, amend the commit (`git commit --amend --no-edit`), push with `git push --force-with-lease`, and watch the new run.

Leave the pull request as a draft. The branch is ready for review; do not tag, release or publish.
