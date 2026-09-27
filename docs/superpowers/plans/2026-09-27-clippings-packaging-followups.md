# Clippings packaging: follow-ups

These are the known issues left after plan 4's per-task reviews and its whole-branch review. None of them blocks a release. Plans 5 and 6 should pick them up wherever they touch the same files.

## Not yet proven with real credentials

- Only the dry run has exercised the release workflow (runs 36337659545 and 36353088613). The macOS and Windows signing jobs and the Entra, PAT and Open VSX publishing jobs have been checked statically but never run. Add the first real release's run ID to `docs/release.md`.
- The tag guard in `release.yml`'s `plan` job was tested by running its shell body locally against real commits, not by pushing a tag. Watch it on the first real tag.
- The Marketplace publisher is `shmr`. The Open VSX namespace `shmr` still has to be created (`ovsx create-namespace shmr`) before Open VSX publishing is turned on.

## Tests

- **Notebook test cleanup.** The `ENOENT … copyfile …notes.ipynb` line in a failing macOS run is VS Code's local history logging a copy of the deleted notebook; the notebook test passed in that run. The run failed because a `clippings/children` request to a retired server never settled and VS Code's tree view waited on it for good, which is now fixed. The `after` hook could still wait for `onDidCloseNotebookDocument` before `rmSync` and end with `whenIdle(api)`, but no failure has been traced to it.
- `.vscode-test.pathforms.mjs` builds every path form's scratch profile when the config loads, even when a `--label` selects one form.

## Workflows and scripts

- The runner forces `actions/upload-artifact@v4`, `actions/cache@v4`, `pnpm/action-setup@v4` and `docker/setup-qemu-action@v3` onto Node 24, and warns about it on every run. When bumping them, bump `upload-artifact` and `download-artifact` together.
- The lint job fetches actionlint's download script by tag (`v1.7.12`) without checking a checksum.
- `build.sh` honours `CARGO_TARGET_DIR` but not `CARGO_BUILD_TARGET_DIR` or `build.target-dir` in a cargo config. `cargo metadata … .target_directory` would cover all three.
- Rust 1.98.1 is pinned in four places and Node 22.23.3 in five, with nothing checking that they agree. `docs/release.md` lists the places.
