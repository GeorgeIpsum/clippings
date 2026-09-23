# Clippings design

Status: sections approved in brainstorming on 2026-09-22 and 2026-09-23. Revised after an adversarial review (56 findings, all confirmed and applied). Awaiting written-spec review.
Author: Ibrahim Saberi, with Claude.

## 1. Summary

Clippings is a reimplementation of the Gruntfuggly/todo-tree VS Code extension with a Rust core. It keeps full feature parity with todo-tree and makes scanning, indexing and view updates fast enough that users never have to trade scan frequency for editor responsiveness on large monorepos.

The Rust core runs as a separate process spawned by the extension over stdio. It owns walking, scanning, the per-file index, the tree view model, decoration ranges and styles, status text and export. The TypeScript extension is a thin relay that maps server output onto VS Code APIs.

### Goals

1. Every documented todo-tree feature and setting works, with the deviations listed in section 11.
2. A cold scan runs within 1.25 times ripgrep's time on the same files. Typing in an editor costs the extension host nothing beyond language client document sync. Applying a tree update costs the extension host time proportional to the visible nodes it touches, within the targets in section 13.
3. Idle means idle: no timers, polling or rescans unless the user enables them.
4. Correct under Remote-SSH, WSL and dev containers.

### Non-goals

- VS Code for the Web and virtual workspaces. Section 5.1 keeps a web build possible later.
- An on-disk persisted index.
- New features from todo-tree's issue tracker, such as git-diff-scoped views. Clippings adds exactly two features over todo-tree: the built-in never-index list (section 5.3) and file watching (section 5.10).
- Language-overridable `regex.regex`. todo-tree declares it but never honours it.

## 2. Background and evidence

Research artifacts, kept in the repo:

- `docs/research/todo-tree-parity-inventory.md`: behavioural inventory of todo-tree commit a6f60e0 (v0.0.224), with 70 settings, 37 commands, 25 performance issues mapped to code-level causes, and file:line references. "Inventory §N" below points into it.
- `docs/research/rust-vscode-bridge-survey.md`: deep read of ten Rust-backed VS Code extensions, including rust-analyzer, Ruff, taplo, tinymist, CodeLLDB, Zowe, Slint and Harper. "Survey §N" below points into it.

Benchmark repository: the tilliX monorepo at `~/tilli/tilliX`, measured on 2026-09-23 on a 14-core, 48 GB Apple Silicon machine. The repository changes over time, because git worktrees under `.claude/worktrees` come and go, so every benchmark run records its file counts.

| Scan set | Files | ripgrep 15.1, user's seven tags |
|---|---|---|
| todo-tree defaults: `.gitignore` honoured, hidden files skipped | 1,813 | 10 to 20 ms |
| hidden files included, ignore files off, `node_modules`, `.turbo`, `.next` and `.git` pruned | 10,559 | 150 to 190 ms |
| everything except `.git`, including `node_modules` | 465,426 | 7.3 to 8.3 s |

With todo-tree's defaults the `.claude` worktrees were never scanned: `.claude` is a hidden directory and `.claude/worktrees/` is listed in `.git/info/exclude`. A measurement on 2026-09-22 suggested otherwise because it counted files with `find`, which ignores both rules.

ripgrep itself is not the bottleneck. todo-tree's slowness comes from JavaScript work on the extension host thread: a full-document regex pass and a whole-cache tree pass on one shared 500 ms timer, per-render icon file checks, quadratic tree placement, and rescans that cannot be cancelled (inventory §12). Its optional submodule detection also walks the entire tree synchronously on every rescan, which takes 5.4 s on tilliX including `node_modules`.

## 3. Architecture

```
VS Code extension host (TypeScript, thin)
  tree provider over node ID strings, client-side expansion state
  decoration types, status bar, commands, settings, persisted view state
        |  stdio JSON-RPC via vscode-languageclient
        |  standard LSP: initialize, incremental document sync, didChangeWatchedFiles, workspace folders
        |  custom clippings/* messages (section 6)
        v
clippings lsp  (Rust, one process per window)
  walker      ignore + globset, built-in never-index list
  scanner     grep-searcher + grep-regex, all cores, cancellable
  index       file -> matches, open buffers shadow disk
  view model  tree, flat, tags-only; grouping, sort, filter, counts
  decorations ranges and styles per open document version
  status      status bar text, badge, view title, context values
```

The same binary exposes `clippings scan`, `clippings watch`, `clippings bench` and `clippings probe`, so the CLI, the benchmarks and the extension exercise identical code.

This follows established practice in shipped extensions. rust-analyzer backs two tree views through custom requests. Deno maintains its test explorer tree from `deno/testModule` notifications keyed by document. tinymist fills five sidebar views from its server. The C/C++ extension version-stamps every decoration push and drops stale ones (survey §2, §5).

## 4. Repository layout

```
Cargo.toml                    workspace
crates/clippings-core/        library: config, walker, scanner, index, view, decorations, status, protocol types
crates/clippings/             binary: subcommands lsp, scan, watch, bench, probe
extension/                    VS Code extension: TypeScript, esbuild, vscode-languageclient
  src/protocol.ts             mirror of crates/clippings-core/src/protocol.rs
  bin/                        bundled server binary and platform.ok marker, filled at package time
tests/fixtures/workspace/     fixture workspace for golden, protocol and extension tests
tests/oracle/                 parity oracle: vendored todo-tree src at a6f60e0 with its MIT license, Node runner
docs/research/                research artifacts
docs/benchmarks/              recorded benchmark results
.github/workflows/            CI and release
```

Key Rust dependencies: `ignore`, `globset`, `grep-searcher`, `grep-regex`, `grep-matcher`, `regex`, `regex-syntax`, `fancy-regex`, `memchr`, `lsp-server`, `lsp-types`, `serde`, `serde_json`, `crossbeam-channel`, `notify`, `chrono`, `tracing`, and `mimalloc` on Windows only.

Key Node dependencies: `vscode-languageclient` 10.x, `@primer/octicons`, `esbuild`, `@vscode/vsce`, `@vscode/test-electron`. pnpm is the package manager.

## 5. Rust core

### 5.1 Filesystem boundary

File content reads go through one `Fs` trait: the scanner reads each file's bytes through it and calls `search_slice` or `search_reader`, never `search_path`. Open-buffer text and line indexes come from the same layer. The walker module uses the `ignore` crate, which calls `std::fs` itself, so it is native-only; a later web build replaces the walker as a whole and serves reads from the client, as taplo, tinymist and Slint do (survey §5 item 8).

### 5.2 Roots

Two kinds of root are distinct:

- **Tree roots** are the workspace folders with scheme `file`, filtered by `filtering.includedWorkspaces` and `filtering.excludedWorkspaces` matched against the folder path. The tree shows one root node per tree root, in folder order, except in the tags-only view.
- **Scan roots** are what the walker walks. If `general.rootFolder` is non-empty they are that folder: `${workspaceFolder}` expands to one scan root per workspace folder and `${NAME}` expands to the environment variable NAME, or empty. Otherwise scan roots are the tree roots.

A file is placed under the deepest tree root that contains it. A file under no tree root, including files found under a `rootFolder` outside the workspace and non-`file` documents such as `untitled:` buffers, is shown at the top level as a flat file node.

A **walked root** is a scan root that the current scan mode walks (section 5.9).

### 5.3 Walker and exclusion layers

The walk uses the `ignore` crate's parallel walker with one worker per available core. It calls `add_custom_ignore_filename(".rgignore")`, sets `hidden(!filtering.includeHiddenFiles)`, applies the exclusion layers in `filter_entry`, and leaves every other `WalkBuilder` option at its default. That honours `.gitignore`, `.ignore`, `.rgignore`, `.git/info/exclude` and the global git excludes file as ripgrep does, requires a git repository for `.gitignore` to apply, and does not follow symlinks.

Nested git repositories and worktrees are ordinary directories and are walked, as in todo-tree. When `filtering.ignoreGitSubmodules` is true, any directory below a scan root that contains a `.git` entry is skipped during the walk.

Exclusion layers:

1. **Built-in never-index list**, new setting `clippings.filtering.builtInExcludes`. Entries are matched only against path components below the scan root, never against the root itself or its ancestors, so a workspace opened inside `.claude/worktrees/x` or under a `target/` directory still indexes. An entry without a `/` matches a directory name at any depth below the root. An entry with a `/` matches a trailing run of path components. Default:
   `.git`, `.hg`, `.svn`, `node_modules`, `.pnpm-store`, `.yarn/cache`, `.claude`, `.next`, `.nuxt`, `.output`, `.turbo`, `.cache`, `.parcel-cache`, `.svelte-kit`, `.angular`, `.vercel`, `.sst`, `.terraform`, `target`, `__pycache__`, `.venv`, `venv`, `.gradle`, `.idea`, `.vs`.
2. **User globs**: `filtering.includeGlobs` and `filtering.excludeGlobs`.
3. **Temporary globs** from the folder and file context menu commands and from scopes.
4. **VS Code excludes**: the keys whose value is exactly `true` in `files.exclude` when `filtering.useBuiltInExcludes` is `file excludes` or `file and search excludes`, and in `search.exclude` when it is `search excludes` or `file and search excludes`.

Glob semantics for layers 2 to 4:

- Globs are compiled with `globset` using its platform defaults for backslashes: an escape character on Unix, a separator on Windows. Literal separators are on, so `*` never crosses `/`. Matching is case-sensitive.
- Candidate paths use `/` separators. A path matches a glob if the glob matches either its absolute path or its path relative to its scan root.
- Includes: if any include globs exist, a file must match at least one.
- Excludes prune directories as well as files. An exclude glob ending in `/**` or `/**/*` is also tested against directory paths with that suffix removed, and a matching directory is not descended into.
- `*` and `**` match path segments that begin with a dot, unlike todo-tree's micromatch.

Temporary globs are built from the node's absolute path with glob metacharacters escaped by `globset::escape`: `<path>/*` for Only Show This Folder, `<path>/**/*` for Only Show This Folder And Subfolders and for Hide This Folder, and the escaped path alone for Hide This File. The suffixes let Remove Filter derive its labels as todo-tree does.

**Admission predicate.** Every path that enters the index from the disk, whether by the walk, a file event or a document close, must pass the same predicate: inside a walked root, layers 1 to 4, the ignore-file rules for its ancestors (built per scan root with `ignore::gitignore`), the hidden rule, submodule skipping, and binary detection in the scanner.

**Open buffers** are admitted differently, as in todo-tree: layers 2 to 4 and todo-tree's buffer hidden-file rule, where a file is hidden when its base name starts with `.` and has no extension. Neither `.gitignore` nor the built-in list applies to open buffers.

### 5.4 Regex construction

- Tags are `general.tags`, or `["TODO"]` when empty.
- Each tag is escaped for Rust regex syntax. Tags are ordered longest first, ties in configured order, and joined with `|`.
- The literal token `($TAGS)` in `regex.regex` is replaced by `(` + alternation + `)`.
- Flags: multi-line anchors `(?m)` always, `(?i)` when `regex.regexCaseSensitive` is false, and `(?s)` when `regex.enableMultiLine` is true.
- Multi-line search mode is used when the regex source contains the two characters `\n`, when `regex.enableMultiLine` is true, or when `grep-regex` rejects the pattern in line mode because it can match a line terminator.
- The pattern is compiled with `grep-regex`. Only when `regex-syntax` reports an unsupported feature, namely look-around or backreferences, is it compiled with `fancy-regex` behind a `grep_matcher::Matcher` adapter. The adapter reports `\n` as its line terminator so line-mode searches use the fast path. fancy-regex runs in its Unicode-aware bytes mode and searches the bytes in place, so invalid UTF-8 needs no conversion, and `.` and character classes match Unicode scalar values as they do under `grep-regex`. A runtime error such as a backtrack limit is logged and skips that match or line, so one pathological line does not hide a file's other todos: the search resumes line by line from the failed search's start, each line with its own backtrack budget and with look-behind still seeing the text before it, and a line that fails again is logged and skipped.
- If neither engine accepts the pattern, the server enters the error state in section 10.1.
- Matches of zero length are skipped, so no pattern can loop.
- The same compiled matcher is used for the workspace walk, open buffers, decorations and navigation, so the four can never disagree.

### 5.5 Scanner

Each file's bytes are read through the `Fs` trait and searched with `grep-searcher`. Memory mapping is never used: `MmapChoice::never()`. Line mode uses the searcher's roll buffer. Multi-line mode reads the whole file with a heap limit of 64 MiB; larger files are logged and skipped in multi-line mode. BOMs are sniffed and UTF-16 files are transcoded.

**Binary files** are detected with `BinaryDetection::quit(b'\0')`. Results are buffered per file and discarded if the sink's `binary_data` callback fires. For whole-buffer searches, where the searcher only inspects the first 64 KB, the scanner also runs `memchr` for a NUL over the whole buffer before accepting results.

**Per-match results.** grep-searcher reports one `SinkMatch` per matching line, or per merged group of lines in multi-line mode, without match offsets. For each `SinkMatch` the scanner runs the matcher's `find_iter` or `captures_iter` over the `SinkMatch` bytes, anchored against the whole slice so `(?m)` anchors and look-behind see real line context. Each hit becomes one match with its byte range, line number and, for multi-line hits, its continuation lines. Two tags on one line therefore give two matches.

**Text window.** The text used for tag extraction runs from the match start to the end of the line on which the match ends, capped at 1,000 bytes after the match start, cut at a UTF-8 boundary. This replaces ripgrep's default `--max-columns=1000` from todo-tree's `ripgrepArgs`. `before` is capped the same way. Decoration ranges are computed from the full match and are not capped.

Workers check a cancellation flag between files.

### 5.6 Tag extraction

Tag extraction ports todo-tree's `extractTag` and `updateBeforeAndAfter` (inventory §3.2) with the fixes in section 11.

- When `regex.regex` contains `$TAGS`, the tag is the first occurrence of the tag alternation in the text window. The reported tag is the configured spelling, compared case-insensitively when `regex.regexCaseSensitive` is false.
- Otherwise the whole match is the tag.
- `after` is the text right of the tag, trimmed, with the sub-tag match removed.
- `before` is the text on the tag's line before the match start, trimmed.
- The sub-tag is capture group 1 of `regex.subTagRegex` applied to the text right of the tag, compiled with the same case flag as the tag regex.
- The group is `general.tagGroups` looked up by tag. Grouping is applied when the view is built, so changing tag groups never rescans.
- For multi-line matches, each continuation line becomes an extra line with its single-line comment leader stripped using the `comment-patterns` language table ported to Rust. Extra lines that are empty or equal to the tag are skipped. Block comment markers are stripped from the primary text as todo-tree does.

Label parity is checked against upstream by the oracle in section 12.3.

### 5.7 Positions and URIs

The core stores byte offsets and converts through a per-file line index. On the wire every position is 0-based line and 0-based UTF-16 code unit, which is LSP's default encoding.

The server echoes the exact URI string it received in `didOpen` in every per-document message. It never re-serialises URIs of open documents. For files it discovers itself it produces `file:` URIs in the same form VS Code uses, including the percent-encoded Windows drive colon.

### 5.8 Index

Paths are interned into file IDs, following rust-analyzer's `vfs` crate. The index holds:

- **Disk results**, one per admitted file: mtime, size and matches from the last walk or file event.
- **Buffer results**, one per open document URI: document version and matches. They are keyed by the full URI, so notebook cells, which share their notebook's path, are separate entries.

Every open buffer is scanned in every scan mode, because decorations need its matches.

The **effective result** of a file, which is what the tree shows, is decided per scan mode (section 5.9):

- If the mode lets open buffers feed the tree and the file has open buffers, the effective result comes from its buffers. For a notebook this is the union of its open cells' results, and each todo carries its cell URI for reveal.
- Otherwise, if the file is inside a walked root and has a disk result, the effective result is the disk result.
- Otherwise the file is absent from the tree.

Disk events for a file whose buffers currently supply its effective result update its disk result but not its effective result. When the last buffer of a file closes, its buffer results are dropped. If the file is inside a walked root, it is rescanned from disk under the admission predicate and the disk result becomes effective. Otherwise it leaves the tree.

### 5.9 Scan modes

| `tree.scanMode` | Walked roots | Open buffers feed the tree |
|---|---|---|
| `workspace` | all scan roots | all open documents |
| `workspace only` | all scan roots | none |
| `open files` | only a `rootFolder` scan root, if set | all open documents |
| `current file` | only a `rootFolder` scan root, if set | only the active editor's document |

- In `current file` mode, when no text editor is active, no buffer feeds the tree, as in todo-tree where the tree is cleared.
- When `tree.autoRefresh` is false, neither opening or editing a document, nor file events, nor a watcher overflow update the tree. Only an explicit refresh, periodic refresh and git refresh do, and each of them rescans every open document as well as the disk, as todo-tree's `rebuild` refreshes open files. Decorations still update.
- When `tree.scanAtStartup` is false, the first walk waits for an explicit refresh, and `clippings/status` reports `needsScan` so the client can show a hint.

### 5.10 Scheduler

One scheduler thread owns the index and view and applies all changes to them.

- **File events** come from `workspace/didChangeWatchedFiles`. The server registers one `**/*` watcher per walked root with dynamic registration, so VS Code's own recursive watcher does the watching, and re-registers when the walked roots change. If the client does not support dynamic registration, the server falls back to a `notify` watcher, which is also what `clippings watch` uses. This is rust-analyzer's loader design (survey §5 item 3). A watcher overflow is treated as a watcher failure, which triggers a full rescan of the disk and every open document when `tree.autoRefresh` is true; `notify` itself reports an overflow as an event flagged for rescan.
- File events are coalesced for 50 ms and filtered by the admission predicate. A Created or Changed event is statted: a file is rescanned. A directory is rewalked only for a Created event; a Changed event on a directory does nothing, because changes below it arrive as their own events. A Deleted event removes the entry for that path and every entry whose path begins with that path followed by `/`, because VS Code reports a deleted folder as one event.
- An event for a path inside a directory the walker would prune is dropped before any stat: this covers the built-in never-index list, user and VS Code exclude globs, hidden and ignore-file rules, and submodules. An event for such a directory itself costs one directory check. A rewalk of a created directory prunes with the same rules, though it does not reload ignore-file rules on its own. Ignore-file events (`.gitignore`, `.ignore`, `.rgignore`) clear the ignore cache and start a full rescan, but only when ignore files are respected and the file is not inside a pruned directory. Events within one batch are deduplicated per path.
- Buffer edits arrive through incremental document sync. A document's buffer is rescanned for the tree after 150 ms without edits. Its decorations are recomputed after `highlights.highlightDelay` milliseconds without edits.
- `clippings/activeEditor` records the active document. In `current file` scan mode it schedules a view rebuild. In `current file` status bar mode it schedules a status recompute.
- After any change that affects the tree, the view is rebuilt after a 50 ms coalescing window.
- A full rescan runs when regex, tags, sub-tag regex, multi-line, case sensitivity, any exclusion layer or the VS Code excludes it pulls in, hidden files, submodule handling, scan roots or scan mode change, when `tree.autoRefresh` changes from false to true, and on explicit refresh.
- Full rescans are single-flight. Starting one cancels any running one. A rescan replaces entries file by file and, when complete, removes entries for files it did not see. A cancelled rescan skips the removal step, so the index holds fresh results for scanned files and previous results for the rest. A full scan's results do not overwrite paths that file events touched while it ran: those entries keep the state the events produced, and a completed scan still removes entries it did not see, other than those paths.
- **`clippings watch`** prints one JSON line per path whose todos change, driven by the same `notify` watcher. It reports `removed` when the path no longer exists or is no longer admitted, decided from the live filesystem rather than the event's kind, because backends such as FSEvents can report one deletion as several events with different kinds for the same path. An event inside a never-index directory, and an event for such a directory itself, print nothing. A broken stdout pipe, such as a killed reader, ends the process with exit code 0 instead of an error.

**Panics.** Full walks and git polls run on worker threads. Buffer scans, view builds and decoration computation run on the scheduler thread under `catch_unwind`. A panic while scanning or decorating one document is logged, and that document keeps its previous todos and decorations; the rest of the state is untouched. Request handlers that only read state are also run under `catch_unwind` and answer with an error. Only a panic during index mutation drops the index and rebuilds it with a full rescan. Recovery guards each document the same way, so a document that always panics cannot take the server down.

Changes in paths excluded by the user's `files.watcherExclude` are not reported by VS Code and are picked up only by a rescan.

### 5.11 Git and periodic refresh

- `general.automaticGitRefreshInterval` in seconds: the server runs `git rev-parse HEAD` per workspace folder with `std::process::Command`, never a shell, with at most one in flight per folder. A changed HEAD triggers a full rescan. Nested repositories are never polled. Zero disables it, which is the default.
- `general.periodicRefreshInterval` in minutes: a full rescan on that interval. Zero disables it, which is the default.
- With both at zero the server has no timers.

### 5.12 View model

The view is rebuilt from the effective results after every relevant change and diffed against the previous view.

**View state** comes from the client in the configuration object: flat, tags-only, grouped by tag, grouped by sub-tag, filter text, and temporary globs. Values the user set by clicking view buttons override the settings, as in todo-tree. Expansion state is not part of the view model (section 7.5).

**Placement** ports todo-tree's `add` (inventory §4.2) with hash-map lookups, so it is linear in the number of matches.

- **Tree view**: tree root, folder chain, file, todo. With grouping by tag, a tag level sits above the folder chain. With grouping by sub-tag, a sub-tag level sits above it. When not grouped by sub-tag, a sub-tagged todo sits under a sub-tag pseudo-folder below its file. With grouping by both tag and sub-tag, tag grouping wins: no sub-tag level and no pseudo-folder are created, as in todo-tree.
- **Flat view**: file nodes under an optional tag or sub-tag level. When not grouped by sub-tag, a sub-tagged todo sits under a sub-tag pseudo-folder below its file node, as in the tree view.
- **Tags-only view**: todos under tag roots, keyed `TAG (sub-tag)` when a sub-tag is present, or under sub-tag roots, or ungrouped at the top level. Tree root nodes are not shown.
- Todos are de-duplicated by URI, line and column.
- Todos whose tag has `hideFromTree` are kept in the model and hidden.
- Multi-line todos have their extra lines as children.

**Root compaction**: a root tag node with exactly one visible child is replaced by that child.

**Compact folders**: when `explorer.compactFolders` is true and `tree.disableCompactFolders` is false, a folder whose only child is a folder is merged with it and the labels are joined with `/`.

**Status pseudo-nodes** at the top, in order: `Scan mode: <mode>` when `tree.showCurrentScanMode` is true, where mode `workspace` displays as `workspace and open files`; then the filter node, `N filter(s) active`, with the active filters in its tooltip, or `Nothing found` when no todos are visible. They render with an empty label and their text in the description, as in todo-tree.

**Raw label** of a todo, as in todo-tree: `after`, or `line N` when `after` is empty, prefixed with `<tag> ` unless grouped by tag. A multi-line todo's raw label is its tag alone. An extra line's raw label is its comment-stripped text.

**Filter**: the filter text is a regular expression, case-sensitive per `tree.filterCaseSensitive`, tested against raw labels. If it is not a valid regex it is matched as literal text. A todo is visible if its raw label matches or, for a multi-line todo, any extra line matches. Containers stay visible when any descendant is visible.

**Sorting** when `tree.sort` is true: folders before files, root tag nodes in configured tag order, then path in code-point order, then line, then column. In the tags-only view, `tree.sortTagsOnlyViewAlphabetically` sorts by label instead of tag order. When `tree.sort` is false, children are ordered by path, line and column only.

**Counts** are computed once per build, bottom-up. A container's count is the number of visible todos below it whose tag lacks `hideFromActivityBar`. Extra lines are not counted.

**Node IDs** are strings, because 64-bit integers lose precision in JavaScript. A node's ID is its parent's ID, a `/`, and its own key:

| Node | Own key |
|---|---|
| tree root | `w:<folder uri>` |
| tag level | `g:<tag or group>` |
| sub-tag level or pseudo-folder | `s:<sub-tag>` |
| folder | `d:<absolute path>`; a compacted chain uses the deepest folder's path |
| file | `f:<absolute path>`, or `f:<uri>` for non-`file` documents |
| todo | `t:<line>:<column>` |
| extra line | `x:<index>` |
| status node | `status:scan-mode`, `status:filter` |

Top-level nodes have no parent prefix. A node promoted by root compaction keeps its own ID. Todo IDs include the line, so inserting lines above a todo gives it a new ID; this only affects selection on that todo. A todo whose parent is not a file node — in the tags-only view — uses the own key `t:<uri>:<line>:<column>` instead, since line and column alone would collide across files.

**Rendering** happens in Rust. Each node carries:

- `id`, as above.
- `label`: for todos, `tree.labelFormat` applied with placeholders `line`, `column`, `tag`, `subtag`, `before`, `after`, `afterorbefore`, `filename` and `filepath`. Placeholder names are case-insensitive. `tag` and `subtag` accept `:uppercase`, `:lowercase` and `:capitalize`. An empty format, and any todo with extra lines, uses the raw label. For folders, the name. For files, the name, plus ` (<relative directory>)` in the flat view. For non-`file` documents, the URI's path basename.
- `description`: the count when `tree.showCountsInTree` is true and the node is a container; the status text for status nodes; otherwise none.
- `tooltip`: `tree.tooltipFormat` for todos, the path for folders and files, the full URI for non-`file` documents, `Click to open <url>` for sub-tag nodes with a click URL.
- `icon`: an icon descriptor (section 5.13), or none for todos when `tree.hideIconsWhenGroupedByTag` is true and grouping is active.
- `hasChildren`, and `defaultExpanded`: true for multi-line todos, otherwise the effective `tree.expanded` state.
- `contextValue`: `folder` for tree roots and folders; `file` for file nodes; none for tag, sub-tag, status and todo nodes.
- `resourceUri`: the node's file URI when `tree.showBadges` is true, for folders and files only.
- `command`: for todos, reveal the URI at the position chosen by `general.revealBehaviour`; `end of todo` reveals at the end of the line on which the match starts, carried as `textEnd` on each todo. For sub-tag nodes with `tree.subTagClickUrl`, open that URL with placeholders applied.

**Deltas.** After a rebuild the server compares every node's rendered fields and child ID list with the previous build and sends the IDs of **parents to refresh**: the parent of every node whose own fields changed, and every node whose child ID list changed. A top-level change is reported as `null`, meaning the root. A view mode or grouping change is always reported as `[null]`.

### 5.13 Decorations

Decorations are computed for each open document whose scheme is in `general.schemes` and that passes the open-buffer admission rules, when `highlights.enabled` is true.

**Key** per match: the group name if the tag is grouped, else the tag. A sub-tag gets its own key. A match without a tag uses its trimmed match text as the key, and its range (the "tag" row below) starts after the match's leading whitespace and ends at the raw match end; trailing whitespace is not trimmed, as in todo-tree.

**Ranges** by `type` attribute:

| `type` | Range |
|---|---|
| unset, unknown, or `tag` | the tag |
| `text` | tag start to end of the line where the match ends |
| `tag-and-comment` | match start to tag end |
| `text-and-comment` | match start to end of the line where the match ends |
| `tag-and-subTag`, `tag-and-subtag` | the tag, plus the sub-tag under its own key when `customHighlight` has an entry for the sub-tag; the sub-tag is searched from the tag's end to the end of the line containing the match's end, as in todo-tree |
| `line` | start of the tag's line to end of the match's last line |
| `whole-line` | the same range, with the style's whole-line flag set |
| `capture-groups:n,m` | the listed capture groups of the match |
| `none` | nothing |

**Attributes** are resolved per key: an exact `highlights.customHighlight` key match, then `highlights.defaultHighlight`, then the built-in default. When `highlights.useColourScheme` is true, foreground, background and icon colour come from the colour scheme arrays indexed by the key's position in `general.tags`, and `defaultHighlight` does not supply those three. Keys not in `general.tags`, such as groups, sub-tags and untagged matches, get no scheme colours, as in todo-tree. The 18 attributes and their defaults are listed in inventory §5.4.

**Styles** are computed in Rust from the attributes: theme colour detection for strings containing `foreground` or `background`, opacity applied to hex and rgb colours with hex alpha taking precedence, ruler colour defaulting to the processed background, ruler lane mapping, font style and weight, text decoration, border radius, and the gutter icon descriptor. When a background is set and the foreground is not, the foreground is black or white by luminance with threshold 0.179. This auto-contrast applies to hex and `rgb()` backgrounds only; a named colour background gets no auto-contrast foreground, as in todo-tree.

**Icon descriptors**: a codicon with optional theme colour, an octicon name with colour, todo-tree's own outline or filled icon with colour, the check-circle icon with colour, or the bundled default. An unknown octicon name falls back to `check`.

### 5.14 Status

The server computes:

- The status bar text and tooltip for each `general.statusBar` mode: `none`, `total`, `tags`, `top three` and `current file`, with icons instead of tag names when `general.showIconsInsteadOfTagsInStatusBar` is true, and the scan-mode suffix ` (in open files)` or ` (in current file)`. Formats follow inventory §9, with todo-tree's exact spacing: each item ends in a space, two after an icon, and items are joined with another space. The tooltip is `Clippings total`, `Clippings tags counts`, `Clippings tags counts in current file` or `Clippings top three tag counts`. Status bar counts exclude tags with `hideFromStatusBar` in every mode, including `total`; in `current file` mode with no active editor, the counts cover the whole workspace, as in todo-tree.
- The activity bar badge: the workspace total excluding `hideFromActivityBar` tags when `general.showActivityBarBadge` is true, else zero, with tooltip `N todos`.
- The view title: `Tree`, `Flat` or `Tags`, with ` (N)` appended when `tree.showCountsInTree` is true and N is positive. N is the current-file total in `current file` status bar mode, else the workspace total.
- `hasSubTags`: whether any node in the current view has a sub-tag. `isEmpty`: whether the view has no todo nodes.
- Scanning, interrupted and `needsScan` state, the error state, configuration warnings for invalid colours and label placeholders, and the server instance ID. Icon-name validation happens in the extension, which owns the octicon set.

### 5.15 Navigation and export

- **Go to next and previous** search the open buffer with the shared matcher from each selection's cursor. Next targets the first match starting after the cursor. Previous targets the last match that ends at or before the cursor, so the match the cursor is inside is skipped, as in todo-tree. Neither wraps. If any selection has no match, no selection moves.
- **Export** produces the visible tree without status nodes. Folder and file nodes are keys holding objects, with flat-view file keys including their path label. A todo is a key `line N` whose value is its formatted label, or `line N:C` when another todo shares its line. In the tags-only view the key is prefixed with the file path. A multi-line todo's value is an object whose single key is its formatted label and whose value holds one empty object per extra line, keyed by the extra line's text. The output is JSON when `general.exportPath` ends in `.json` and an ASCII tree in treeify's format otherwise. The export path expands `~`, `${NAME}` environment variables and strftime placeholders.

## 6. Protocol

Transport: JSON-RPC over stdio. The server uses the `lsp-server` crate. The client uses `vscode-languageclient`. Custom messages are declared once in `crates/clippings-core/src/protocol.rs` and mirrored in `extension/src/protocol.ts`, the pattern rust-analyzer uses with `lsp/ext.rs` and `lsp_ext.ts`.

### 6.1 Standard LSP usage

- `initialize`: workspace folders, the full resolved configuration in `initializationOptions`, and a protocol version number. The server rejects a mismatched protocol version.
- Text document sync: open, close and incremental change, for the schemes in `general.schemes`.
- `workspace/didChangeWorkspaceFolders`: roots change, full rescan.
- `client/registerCapability` and `client/unregisterCapability` for `workspace/didChangeWatchedFiles`, and the resulting notifications.
- `$/cancelRequest` for the custom requests.

Full scans are not LSP requests and do not use LSP progress. They report through `clippings/status` and are cancelled with `clippings/stopScan`.

### 6.2 Custom messages

| Method | Direction | Kind | Params and result |
|---|---|---|---|
| `clippings/configure` | client to server | notification | the full resolved configuration (section 6.3) |
| `clippings/activeEditor` | client to server | notification | `{ uri: string or null }` |
| `clippings/rescan` | client to server | notification | `{}` |
| `clippings/stopScan` | client to server | notification | `{}` |
| `clippings/children` | client to server | request | `{ parent: string or null }` returns `{ nodes: ViewNode[] }` |
| `clippings/find` | client to server | request | `{ uri, line: number or null }` returns `{ paths: ViewNode[][] }`, each path the nodes from the top level down to a matching node |
| `clippings/navigate` | client to server | request | `{ uri, positions: Position[], direction: "next" or "previous" }` returns `{ ranges: Range[] or null }` |
| `clippings/export` | client to server | request | `{}` returns `{ path: string, content: string }` |
| `clippings/treeChanged` | server to client | notification | `{ refresh: (string or null)[] }`, parent IDs to refresh, `null` for the root |
| `clippings/styles` | server to client | notification | `{ generation: number, reset: boolean, styles: { [key]: DecorationStyle } }` |
| `clippings/decorations` | server to client | notification | `{ uri, version: number, generation: number, ranges: { [key]: Range[] } }` |
| `clippings/status` | server to client | notification | `{ instance, scanning, interrupted, needsScan, error, warnings, statusBar: { text, tooltip, visible }, badge: { value, tooltip }, viewTitle, hasSubTags, isEmpty }` |

**Styles and decorations semantics:**

- `generation` increases only when `reset` is true. A non-reset `styles` message adds keys to the current generation.
- The server sends `styles` for new keys before the `decorations` message that uses them.
- A `decorations` message fully replaces the previous one for that document. Keys absent from `ranges` are cleared.
- The client drops a `decorations` message whose generation is not its current generation, or whose version is older than the document's current version.
- After a reset the server resends decorations for every open document.

**Instances.** `instance` is a random ID chosen at server start. When the client sees a new instance it discards its node cache, decoration cache and style generation.

### 6.3 Configuration object

All `clippings.*` settings are declared with window scope, except `server.path`, which is machine-overridable and restricted in untrusted workspaces. The client resolves them at window level and adds:

- the true keys of `files.exclude` and `search.exclude`, and `explorer.compactFolders`;
- the view state and temporary globs from workspace storage.

The object has `general`, `highlights`, `filtering`, `tree` and `regex` as nested objects mirroring the settings groups, plus `viewState { flat, tagsOnly, expanded, groupedByTag, groupedBySubTag, filter, includeGlobs, excludeGlobs }`, `filesExclude`, `searchExclude` and `explorerCompactFolders`. Unknown fields are ignored.

On each `clippings/configure` the server diffs against the previous object. Every row whose fields changed applies:

| Changed fields | Server action |
|---|---|
| any full-rescan trigger in section 5.10 | full rescan, then view rebuild |
| `general.tags`, `general.tagGroups`, regex settings, exclusion settings | new style generation and decoration recompute for all open documents |
| any highlight attribute, colour scheme, `highlights.enabled`, `highlights.highlightDelay` | new style generation and decoration recompute |
| `general.tagGroups`, view state, `tree.*` settings, `general.revealBehaviour`, filter, `hideFromTree`, `hideFromActivityBar`, icon attributes | view rebuild |
| `general.statusBar`, `general.showIconsInsteadOfTagsInStatusBar`, `general.showActivityBarBadge`, `hideFromStatusBar` | status recompute |
| `general.automaticGitRefreshInterval`, `general.periodicRefreshInterval` | reset the corresponding timer |

## 7. VS Code extension

### 7.1 Manifest

- `name` `clippings`, display name `Clippings`, category `Clippings`.
- `publisher` is the maintainer's Marketplace publisher ID. Until one exists the value is `clippings-dev` and CI publishing jobs are skipped.
- `engines.vscode`: the minimum version required by the pinned `vscode-languageclient` 10.x release.
- `extensionKind: ["workspace"]`, so the platform VSIX installs where the files are and the server runs there.
- `activationEvents: ["onStartupFinished"]`.
- `capabilities.untrustedWorkspaces`: supported `limited`, with `clippings.server.path` as a restricted setting.
- `capabilities.virtualWorkspaces`: false.
- An activity bar view container `clippings` with one view `clippings-view`, whose `when` clause is `!clippings-is-empty`.

### 7.2 Settings

All settings live under `clippings.*` with todo-tree's structure below the prefix, so `todo-tree.general.tags` becomes `clippings.general.tags`. Defaults are identical to todo-tree v0.0.224 (inventory §8.1).

Carried over unchanged in name, type and default:

- `general.*`: `automaticGitRefreshInterval`, `periodicRefreshInterval`, `revealBehaviour`, `exportPath`, `rootFolder`, `schemes`, `statusBar`, `showIconsInsteadOfTagsInStatusBar`, `statusBarClickBehaviour`, `tagGroups`, `tags`, `showActivityBarBadge`.
- `highlights.*`: `customHighlight`, `defaultHighlight`, `enabled`, `highlightDelay`, `useColourScheme`, `foregroundColourScheme`, `backgroundColourScheme`.
- `filtering.*`: `excludedWorkspaces`, `excludeGlobs`, `ignoreGitSubmodules`, `includedWorkspaces`, `includeGlobs`, `includeHiddenFiles`, `scopes`, `useBuiltInExcludes`.
- `tree.*`: `autoRefresh`, `disableCompactFolders`, `expanded`, `filterCaseSensitive`, `flat`, `groupedByTag`, `groupedBySubTag`, `hideIconsWhenGroupedByTag`, `hideTreeWhenEmpty`, `labelFormat`, `scanAtStartup`, `scanMode`, `showBadges`, `showCountsInTree`, `showCurrentScanMode`, `subTagClickUrl`, `sortTagsOnlyViewAlphabetically`, `sort`, `tagsOnly`, `tooltipFormat`, `trackFile`.
- `tree.buttons.*`: `reveal`, `scanMode`, `viewStyle`, `groupByTag`, `groupBySubTag`, `filter`, `refresh`, `expand`, `export`.
- `regex.*`: `regex`, `regexCaseSensitive`, `subTagRegex`, `enableMultiLine`.

That is 61 carried settings.

New:

| Key | Type | Default | Meaning |
|---|---|---|---|
| `filtering.builtInExcludes` | string array | the list in section 5.3 | directories never indexed |
| `server.path` | string | `""` | path to a server binary; `~` and `${workspaceFolder}` expand |
| `server.logLevel` | `error`, `warn`, `info`, `debug`, `trace` | `info` | server log level in the output channel |
| `trace.server` | `off`, `messages`, `verbose` | `off` | JSON-RPC message tracing, the language client's standard setting |

Dropped:

| todo-tree key | Reason |
|---|---|
| `ripgrep.ripgrep`, `ripgrep.ripgrepArgs`, `ripgrep.ripgrepMaxBuffer`, `ripgrep.usePatternFile` | no ripgrep binary |
| `filtering.passGlobsToRipgrep` | globs always apply in the walker |
| `tree.showInExplorer`, `tree.showScanOpenFilesOrWorkspaceButton`, `tree.showTagsFromOpenFilesOnly` | deprecated and unread in todo-tree |
| `general.debug` | replaced by `server.logLevel`; the importer maps `true` to `debug` |

### 7.3 Importing todo-tree settings

On activation, if any `todo-tree.*` key has an explicit global or workspace value, no `clippings.*` key has one, and the user has not chosen Never, a notification offers to import. Choices are Import, Not Now and Never. The command `clippings.importTodoTreeSettings` runs the import on demand.

The import copies each carried key's global and workspace values to the same scopes under `clippings.*`, maps `general.debug` as described above, and skips the other dropped keys. Workspace-folder values are skipped and logged: todo-tree read its settings at window level, so VS Code already ignored them. Keybindings are not imported.

### 7.4 Server lifecycle

**Resolution order**, stopping at the first candidate that passes the probe:

1. the `CLIPPINGS_SERVER_PATH` environment variable;
2. `clippings.server.path`, prompting Allow or Deny once per resolved path when it comes from workspace settings;
3. the bundled `bin/clippings` next to `bin/platform.ok`;
4. `clippings` on PATH, in development builds only.

**Probe**: run `<candidate> probe`, which prints `{ version, target, protocolVersion }` as JSON and exits 0, with a 15 second timeout. The bundled binary is made executable first if needed. A candidate fails if it does not start, times out, exits non-zero, or reports a different protocol version. If every candidate fails, one error lists each candidate and its failure, with buttons to open the output channel and the setting. This is tinymist's aggregation pattern with CodeLLDB's marker file (survey §5 item 2).

**Spawn**: `clippings lsp` through the language client, with `RUST_BACKTRACE=1` and the log level in the environment. stdout carries JSON-RPC only and stderr goes to the Clippings output channel. The server exits when stdin closes.

**Crashes**: a custom error handler restarts the server after each crash until five crashes occur within three minutes. It then stops and shows a notification with Restart and Show Log. The client discards its caches when it sees a new server instance (section 6.2).

**Restarts**: `clippings.restartServer` restarts on demand. A change to `general.schemes`, `server.path`, `server.logLevel` or `trace.server` restarts the client, because the document selector and spawn environment are fixed at start.

One server serves all workspace folders in a window.

### 7.5 Tree view

- The tree element type is the node ID string. The provider keeps a map from ID to the last node received and maps each node to a TreeItem.
- `TreeItem.id` is the node ID prefixed with an **epoch** number held by the client.
- `getChildren(id)` calls `clippings/children` and records each child's parent. `getTreeItem(id)` uses the cached node, or returns a TreeItem with only the ID set when the node is not cached.
- On `clippings/treeChanged` the client fires the change event for each listed parent, with `undefined` for `null`. VS Code then refetches those parents' children. IDs the client has never loaded are ignored.
- **Expansion is client-side.** `collapsibleState` comes from the client's expansion map, keyed by node ID, and falls back to the node's `defaultExpanded`. Expand and collapse events update the map in workspace storage and are not sent to the server.
- **Expand Tree and Collapse Tree** set the persisted `expanded` view state, clear the expansion map, and bump the epoch, then refresh the root. New TreeItem IDs make VS Code read `collapsibleState` afresh; VS Code otherwise keeps its own expansion state for known IDs. The epoch also bumps on `clippings.resetCache` and when `tree.expanded` changes, and never on ordinary deltas, so selection and focus survive edits.
- **Reveal and track file** call `clippings/find`, cache the returned nodes and their parents, and call `TreeView.reveal` on the first path's last node.
- **Track file** runs 500 ms after the active editor changes, when `tree.autoRefresh` and `tree.trackFile` are true, the document's scheme is in `general.schemes`, and the view is visible. It reveals without taking focus.
- Todo clicks open the document at the node's position and flash the line for 150 ms using one reused decoration type.
- When `needsScan` is set, the view's message is `Click the refresh button to scan...`. It clears when a scan starts.

### 7.6 Commands, menus and context keys

Every todo-tree command exists under the `clippings.` prefix with the same suffix, title, icon, menu placement and when-clause, with `todo-tree-*` context keys renamed `clippings-*`. That covers the 34 declared commands and the three registration-only commands `openUrl`, `stopScan` and `onStatusBarClicked` (inventory §7, §4.8).

New commands: `clippings.importTodoTreeSettings`, `clippings.restartServer`, `clippings.showLog`.

**Context keys** are set by the client after every configuration change and every `clippings/status`:

| Key | Value |
|---|---|
| `clippings-show-reveal-button` | `tree.buttons.reveal && !tree.trackFile` |
| `clippings-show-scan-mode-button`, `-view-style-button`, `-group-by-tag-button`, `-group-by-sub-tag-button`, `-filter-button`, `-refresh-button`, `-expand-button`, `-export-button` | the matching `tree.buttons.*` setting |
| `clippings-expanded`, `clippings-flat`, `clippings-tags-only`, `clippings-grouped-by-tag`, `clippings-grouped-by-sub-tag` | the effective view state |
| `clippings-filtered` | a filter text is set |
| `clippings-collapsible` | `!tagsOnly || groupedByTag || groupedBySubTag` |
| `clippings-folder-filter-active` | any temporary include or exclude glob is set |
| `clippings-global-filter-active` | the filter text itself |
| `clippings-can-toggle-compact-folders` | `explorer.compactFolders === true` |
| `clippings-has-sub-tags` | `hasSubTags` from status |
| `clippings-scan-mode` | the scan mode |
| `clippings-is-empty` | `tree.hideTreeWhenEmpty && isEmpty` |

**Setting writes:**

- Scan mode commands and the item count, badge and compact folder toggles write to the workspace scope when a folder is open, else to the global scope.
- `addTag`, `removeTag`, the status bar cycle and the status bar toggle-highlights click write to the workspace scope if the key has a workspace value, else to the global scope.
- The status bar cycle shows todo-tree's four information messages, with `Clippings` in place of `Todo Tree`.

View-state commands update workspace storage and send `clippings/configure`. `clippings.resetCache` clears all persisted view state, filters and the expansion map, bumps the epoch, then sends `clippings/configure`.

Export opens a read-only virtual document under the `clippings-export` scheme with the server's content, named by the formatted export path.

### 7.7 Decorations, status bar and icons

- The `styles` handler creates each `TextEditorDecorationType` synchronously before it returns, so a following `decorations` message always finds its types. Gutter icons use deterministic file paths, and a missing icon file is written synchronously the first time its name and colour appear.
- Decorations are applied to every visible editor of the document, following the rules in section 6.2.
- The client keeps the last applied decorations per document URI and version. When an editor becomes visible, the cached entry is applied immediately if its version equals the document's current version. The entry is dropped when the document closes.
- The status bar item sits on the left at priority 0 and shows the server's text. While a full scan runs it shows scanning and clicking stops the scan. After a stop it shows interrupted and clicking refreshes. Otherwise a click follows `general.statusBarClickBehaviour`: `reveal` focuses the view, `toggle highlights` flips `highlights.enabled`, and `cycle` steps total, tags, top three, current file.
- The view's badge and title follow `clippings/status`. Configuration warnings are shown once per distinct set.
- Icons: codicons become theme icons. Octicons, todo-tree's own icons and the check-circle icon are rendered to SVG once per name and colour under global storage and cached in memory.

### 7.8 Persisted state

Workspace storage: `flat`, `tagsOnly`, `expanded`, `groupedByTag`, `groupedBySubTag`, `currentFilter`, `filtered`, `includeGlobs`, `excludeGlobs`, `expandedNodes`, `epoch`.

Global storage:

- `importOffer`: absent until the user chooses Never in the import prompt, then `never`. Not Now leaves it absent, so the offer returns on the next activation.
- `serverPathDecisions`: a map from resolved server path to `allow` or `deny`. `resetCache` does not clear it.

## 8. Packaging and release

### 8.1 Targets

| VS Code target | Rust triple |
|---|---|
| `win32-x64` | `x86_64-pc-windows-msvc` |
| `win32-arm64` | `aarch64-pc-windows-msvc` |
| `linux-x64` | `x86_64-unknown-linux-gnu` with a glibc 2.28 floor |
| `linux-arm64` | `aarch64-unknown-linux-gnu` with a glibc 2.28 floor |
| `linux-armhf` | `armv7-unknown-linux-gnueabihf` with a glibc 2.28 floor |
| `alpine-x64` | `x86_64-unknown-linux-musl` |
| `alpine-arm64` | `aarch64-unknown-linux-musl` |
| `darwin-x64` | `x86_64-apple-darwin` |
| `darwin-arm64` | `aarch64-apple-darwin` |

Plus one universal VSIX without a binary or `platform.ok`, which works only when `server.path` points at a user-built binary. This is rust-analyzer's no-server package.

### 8.2 Build

- macOS and Windows build on native runners. Linux and Alpine targets build with `cargo-zigbuild`, using its glibc-suffixed targets for the 2.28 floor. If zigbuild cannot build `linux-armhf`, that target builds in a `debian:10` container with native gcc, as rust-analyzer does.
- CI checks that each glibc binary's highest `GLIBC_` symbol version is 2.28 or lower, and runs `clippings probe` under qemu for the arm64 and armv7 Linux binaries.
- Release profile: link-time optimisation, one codegen unit, `panic = "unwind"`, symbols stripped from the shipped binary and kept as separate release assets. The global allocator is mimalloc on Windows and the system allocator elsewhere, as rust-analyzer does.
- The extension has no `vscode:prepublish` script. A separate `pnpm build` step produces the esbuild bundle once. Each target then copies its binary and `platform.ok` into `extension/bin` and runs `vsce package --no-dependencies --target <target>`. `.vscodeignore` whitelists `dist/**` and `bin/**`.
- Versions are plain `major.minor.patch`, because the Marketplace rejects semver pre-release versions. Odd minor versions are pre-releases, published with `--pre-release`. Even minor versions are stable.
- Tags matching `v*` build all ten packages and attach them to a GitHub release. Publishing to the Marketplace and Open VSX runs only when their tokens are configured, with `--no-dependencies` and skip-duplicate.

### 8.3 Local development

`pnpm dev` builds the debug server. The extension launch configuration sets `clippings.server.path` to that binary and `RUST_BACKTRACE=1`, so F5 runs the extension against a fresh build.

## 9. Multi-root and remote

- One root node per tree root, in workspace folder order, suppressed in the tags-only view.
- Each file belongs to the deepest containing tree root.
- Workspace folder changes arrive through standard LSP and trigger a full rescan.
- Under Remote-SSH, WSL and containers the workspace-kind extension and its server both run on the remote host, and the platform VSIX matching the remote OS, architecture and libc is installed there.

## 10. Error handling and logging

### 10.1 Server

- Panics follow the rules in section 5.10.
- An unreadable file is logged at debug level and skipped. The scan continues.
- A regex that fails under both engines puts the server in an error state. The last good index and view stay in place, `clippings/status` carries the error, and the client shows one warning with an Open Settings action.
- If the watcher reports an overflow or error, the server runs a full rescan and warns once, unless `tree.autoRefresh` is false (section 5.9).

### 10.2 Client

- A missing or failing binary produces the aggregated probe error from section 7.4.
- Server crashes follow the restart policy in section 7.4.
- Stale decorations are dropped by generation and version.
- Setting writes that fail are caught and shown as a warning.

### 10.3 Logging

Server logs go to stderr at `server.logLevel` and appear in the Clippings output channel. `trace.server` traces JSON-RPC messages in the same channel.

## 11. Parity policy

Documented behaviour is reproduced. Undocumented bugs are fixed. Every deviation from todo-tree v0.0.224 is listed here.

### 11.1 Kept as in todo-tree

- All carried setting semantics and defaults, read at window level.
- Four scan modes, three view modes, grouping by tag and by sub-tag, tag groups, root compaction, compact folders, sub-tag pseudo-folders, status pseudo-nodes.
- Raw labels, label formatting, and labels of multi-line todos.
- Every highlight type and attribute, colour scheme precedence and indexing, exact-match `customHighlight` keys, opacity and ruler rules, gutter icons only for SVG icons.
- Temporary folder and file filters, scopes, filter removal and reset.
- Status bar modes, formats, click behaviours and cycle messages; activity bar badge; view title.
- Reveal behaviour and track file, including its gating.
- Clicked view state in workspace storage overriding the settings.
- The buffer hidden-file rule, and neither `.gitignore` nor the built-in list applying to open buffers.
- No wrap-around for go to next and previous.
- In `current file` mode with no active text editor, the tree is empty.

### 11.2 Changed from todo-tree

New behaviour:

- **File watching.** Changes on disk update the tree through VS Code's file watcher, in every mode that walks roots, including saves in `workspace only` mode. todo-tree 0.0.224 had no watcher and relied on git or periodic polling.
- **Built-in never-index list** (section 5.3).

Fixes:

- Columns are UTF-16 positions instead of ripgrep byte offsets, so non-ASCII lines reveal and label correctly.
- Multi-line matches return the full match with continuation lines, as the README describes.
- The tag alternation is ordered longest first, as the README promises.
- One regex engine serves the walk, buffers, decorations and navigation.
- Long lines are truncated to 1,000 bytes for labels instead of being replaced by an omitted-line placeholder.
- Buffer rescans are debounced per document instead of on one shared timer.
- Closing a document in `workspace` mode replaces its buffer results with a fresh disk scan, instead of keeping the last buffer results.
- Notebook cells are indexed per cell and grouped under their notebook.
- Stop scan cancels the scan.
- Changing `regex.regex` triggers a rescan.
- `rootFolder` expands environment variables.
- `useBuiltInExcludes: "file excludes"` works.
- Sub-tag regex case sensitivity matches the tag regex in both the tree and highlights.
- `before` is the line text before the match, not a slice from the wrong origin.
- An invalid filter regex falls back to literal matching instead of throwing. A multi-line todo is also visible when its own label matches the filter.
- `tree.sort: false` gives a deterministic order.
- Tag and sub-tag group nodes have no fabricated file paths and no context value, so folder filter commands do not appear on them. Folders never inherit a sub-tag.
- Expansion state is kept per node instead of shared by every node with the same path.
- Files belong to the deepest tree root instead of the last listed one.
- Todos are de-duplicated by line and column instead of label and line.
- Temporary filter globs escape glob metacharacters in paths, so folders such as `app/[slug]` can be hidden.
- Status bar counts exclude `hideFromStatusBar` tags in every mode, instead of `hideFromActivityBar` in all modes but current file.
- The `todo-tree` and `todo-tree-filled` icon names validate.
- Auto-contrast foreground works when only a background is set.
- The `line` highlight type produces a forward range.
- Decorations are reapplied from cache when an editor becomes visible, instead of flashing while the pass reruns.
- Decoration types are cached per key instead of recreated per pass, and the reveal flash reuses one type.
- Git polling is single-flight without a shell and never polls nested repositories. Submodule detection happens during the walk.
- Commands write settings to a scope that takes effect, and work with no folder open.
- `resetCache` clears all persisted view state.
- Export reflects the visible tree, includes extra lines, and never merges two todos on one line or two same-named files into one key.
- No global `RegExp.prototype.exec` replacement.
- Glob patterns match dot-prefixed path segments.

### 11.3 Dropped

- The ripgrep settings and `passGlobsToRipgrep`, because there is no ripgrep binary. Behaviour that users obtained through `ripgrep.ripgrepArgs`, such as `--no-ignore`, `--follow` or `--max-filesize`, has no setting equivalent. The `clippings scan` and `clippings bench` subcommands accept `--hidden` and `--no-ignore` for benchmarking only.
- The three deprecated tree settings.
- `general.debug`, replaced by `server.logLevel`.
- Workspace-folder-scope setting values, which todo-tree also ignored.
- todo-tree's 32-key legacy flat-setting migration, its versioned upgrade notices, the markdown regex upgrade prompt, and the `highlights.schemes` rewrite on activation. The importer in section 7.3 serves users coming from todo-tree.

## 12. Testing

### 12.1 Rust unit tests

Written test-first, per module: tag regex expansion and ordering, engine selection and fallback, tag extraction, sub-tags and groups, text window capping, placement in each view mode, compaction, sorting, filtering, counts, node IDs, deltas, label and tooltip formatting, decoration ranges per type, style resolution, UTF-16 conversion, multi-line matches, glob semantics, escaping and directory pruning, built-in excludes below the root only, the admission predicate, effective-result selection per scan mode, notebook cells, status text per mode, export formats.

### 12.2 Golden tests

`tests/fixtures/workspace/` contains: files in several languages, non-ASCII text before tags, CRLF line endings, multi-line todos, two tags on one line, adjacent multi-line matches, sub-tags, grouped tags, a line longer than 1,000 bytes, a binary file with its NUL near the start and one with its NUL after 64 KB and todos on both sides, gitignored files, a folder named `[slug]`, a nested git repository, a `node_modules` directory, a `.claude` directory, and hidden files. `clippings scan --json` output and the rendered view for each view mode and grouping are compared with checked-in golden files.

### 12.3 Parity oracle

`tests/oracle/` vendors todo-tree's `src/` at commit a6f60e0 with its MIT license and a stub for the `vscode` module. A Node runner applies upstream `getTagRegex`, `extractTag` and `formatLabel` to a corpus of comment lines and compares tag, sub-tag, after text and formatted label with `clippings scan --json`. Differences covered by section 11.2 are listed in an allow file. It runs as an optional CI job.

### 12.4 Protocol integration tests

Rust tests spawn `clippings lsp` against the fixture workspace and assert on messages: initialize and receive the first status and tree; request children; open a buffer and receive styles then decorations; edit it and receive a tree delta; change a file on disk and receive a delta; delete a directory and see its todos removed; switch scan modes and see watcher registrations change; change configuration and observe the right actions; cancel a scan; open notebook cells.

### 12.5 Extension tests

`@vscode/test-electron` over the fixture workspace: tree labels in each view mode, every command, expand and collapse, decorations through a test hook, status bar text, context keys, settings import. This is a smoke suite, not an exhaustive one.

## 13. Performance targets and benchmarks

`clippings bench` measures the core, and an extension test hook measures extension host time. Every run records the machine, the file count of each scan set, and ripgrep's time on the same set with equivalent flags and `--max-columns=1000` as the reference. Results are recorded in `docs/benchmarks/` for each milestone.

Scan sets:

1. **tilliX defaults**: the benchmark repository with default settings. This is what users feel.
2. **tilliX wide**: hidden files on, ignore files off, built-in excludes reduced to `.git`, `node_modules`, `.turbo` and `.next`.
3. **tilliX full**: everything except `.git`, including `node_modules`. This is the throughput stress test.
4. **Synthetic**: a deterministic corpus of 200,000 source files produced by `clippings bench --generate <dir> --seed <n>`, with a fixed todo density. It is the repeatable regression baseline that does not depend on the state of any real repository.

| Measure | Target |
|---|---|
| Cold scan, tilliX defaults | under 50 ms |
| Cold scan, every scan set | within 1.25 times ripgrep's time on the same set |
| Server work for one changed file, excluding coalescing windows, p99 | under 10 ms |
| View rebuild and diff with 10,000 todos | under 15 ms |
| Decoration computation for a 50,000-line open buffer | under 20 ms |
| Extension host work per keystroke from Clippings | none beyond language client incremental sync |
| Extension host time to apply a delta that refreshes 1,000 visible nodes | under 30 ms |
| Timer wakeups when idle with git and periodic refresh off | zero |
| Server resident memory, tilliX wide | under 150 MB |

The view rebuild target reflects a prototype measurement of 12.5 ms on the same machine model. Sharing todo data between the index and the view, which would lower it further, is plan 4's optimisation.

## 14. Milestones

1. **Core**: filesystem boundary, roots, walker, exclusion layers and admission predicate, regex construction, scanner, tag extraction, index, scan modes, `clippings scan --json`, unit and golden tests.
2. **Server**: view model with IDs and deltas, decorations and styles, status, navigation, export, protocol types, scheduler, `clippings lsp`, `clippings watch`, protocol integration tests.
3. **Extension**: manifest and settings, import, server resolution and lifecycle, tree provider with client-side expansion, decorations, status bar, all commands, menus and context keys, extension tests.
4. **Packaging**: CI for all targets with glibc and qemu checks, ten VSIX packages, release workflow, `clippings bench` and the first tilliX results.
5. **Parity polish**: icons, scopes, export, multi-line edge cases, notebooks, reveal and track file, the parity oracle, final benchmark run.
