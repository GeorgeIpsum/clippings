# Clippings server: follow-ups

Known minor issues left after plan 2's reviews. None of them affects correctness in normal use; each was judged not worth a fix round. Plans 3 and 4 should pick them up where they touch the same code.

## Scheduler and server

- Directory rewalks for `Created` directories run on the scheduler thread. Move them to a worker with the same generation and touched-path protocol as full walks.
- A pathological line under the fancy-regex engine costs two backtrack budgets: after a per-line fallback hit, the next `find_at` searches the remaining range again.
- A running walk is not cancelled on shutdown. `exit` without `shutdown` returns exit code 0 instead of 1.
- A failed `client/registerCapability` response is ignored instead of falling back to the `notify` watcher.
- Every watcher overflow logs a warning. Spec 10.1 says "warns once".
- An invalid-regex `configure` still runs a full rescan with the previous pattern.
- With `ignoreGitSubmodules` on, `inside_pruned_dir` checks `ancestor/.git` for every ancestor of every event, as the walker does.
- The touched-path set has no cap during a long walk. It is freed when the walk is applied.
- The notebook-cell check compares URI strings. A `file:` buffer URI in a form other than `file_uri`'s would get a URI-keyed todo ID, which changes when the file opens or closes.
- `clippings watch` with a symlinked root: the server rebases event paths onto the configured roots, but `watch` has not been checked. It canonicalizes its roots, so it is probably fine.

## Core modules

- `percent_decode` returns a `String`, so percent-encoded non-UTF-8 path bytes decode lossily. This only happens on Linux.
- A position on a line past the end of a document clamps to the last line at the same character. VS Code's `validatePosition` clamps to the document end instead. Only a desynced client can send one.
- `apply_changes` rebuilds the `LineIndex` for every change in a batch.
- Folder compaction re-walks single-child chains from every node (`view/shape.rs`), which is O(depth²).
- Export's sibling scan compares `None == None` paths for non-file documents. It is also O(k²) per parent, but export only runs when the user asks.
- `colours::hex_to_rgba` is public but slices bytes without an `is_hex` guard. Its only caller guards.
- `pattern.rs` and `decorations.rs` import each other (for `CaptureRegex`).

## Tests and comments

- These have no direct unit tests:
  - `to_file_events`' remove, rename, access and other branches;
  - `unregister_params`;
  - `hex_to_rgba`, `set_rgb_alpha` and `rgb_components`.
- The Latin-1 test under the fancy engine is a regression guard only; it passes on the pre-fix code too.
- The never-index server test bounds responsiveness at 2 s, which depends on timing.
- The smoke script's churn wait does not assert `scanning: false`.
- `Settings::changes_from`'s styles exclusion subset needs a comment saying it mirrors the fields `GlobLayers` uses for buffer admission.
- The status test module has redundant imports.
