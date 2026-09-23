> Research artifact generated 2026-09-22 by a multi-agent read of todo-tree commit a6f60e0 (v0.0.224). File:line refs point into `src/` of that commit. Absolute scratchpad paths below refer to a temporary clone and are not in this repo.

# todo-tree feature parity inventory

Audience: engineers re-implementing the VS Code extension **todo-tree** (Gruntfuggly.todo-tree) with a Rust core. This is a behavioural inventory of what the shipped JavaScript does, including its bugs, so that the spec author can decide feature by feature what to keep, fix, or drop.

## Provenance and conventions

- **Code baseline.** The scratchpad copy of the repository at `/private/tmp/claude-501/-Users-g1n-lib-clippings/a3ed3a1e-2a16-483e-8eb2-9ab2efa643fe/scratchpad/todo-tree`, git commit `a6f60e0ce830c4649ac34fc05e5a1799ec91d151` (2023-04-18, last commit on `main`). `package.json` says version **0.0.224**, `engines.vscode ^1.72.0`, `activationEvents: ["onStartupFinished"]`, `main: ./dist/extension` (webpack bundle of `src/`). The Marketplace's last release is **0.0.226**; the delta between the repo head and 0.0.226 is not in the repo history and is listed under open questions.
- **Line references** are `file.js:NNN` into `src/` of that commit (e.g. `extension.js:511`). `package.json:NNN` / `README.md:NNN` refer to the repo root files.
- **Sources merged.** (1) reader `extension` (extension.js), (2) reader `tree` (tree.js; its input was truncated after the "nodesToGet" feature, the gap was filled from source), (3) gap fills for ripgrep.js / searchResults.js / multiline / attributes.js / highlights.js (the highlights gap fill was truncated inside the "Editor highlight pass" feature; the highlight-type, trigger, icon and validation material below was re-derived from source), (4) the GitHub performance-issue sweep, (5) the Rust-bridge fact-check. Items marked **[verified in source]** were checked directly against the files above while writing this document; items marked **[inference]** are the author's reasoning, not a cited fact.
- **Vocabulary used throughout** (all in extension.js):
  - `rebuild()` (:573-609): full rescan. Clears the result cache, spawns ripgrep per root, then re-scans open documents.
  - `refresh()` (:863-872): soft rebuild. Re-creates the tree from the cached results (no ripgrep) plus fresh open-document scans.
  - `refreshFile(doc)` (:777-861): scan one open document's buffer with the JS regex and replace its tree nodes.
  - `refreshTree()` (:125-133): 200 ms debounced `provider.refresh()` (fires the TreeDataProvider change event).
  - `triggerRescan()` (:611-618): 1000 ms debounced `rebuild()` (shares the timer handle with `refreshTree`).
  - `addResultsToTree()` (:135-151): push not-yet-added cached results into the tree, update status bar/badge/title, re-apply the tree text filter, schedule `refreshTree()`.
  - "workspaceState override": `context.workspaceState.get(key, settingValue)`. Once the user has clicked a view button in a workspace the Memento value wins over the setting, even when the stored value is `false` (config.js:17-40).

---

## 0. Contradictions and corrections (resolved)

| # | Claim A | Claim B | Resolution and evidence |
|---|---|---|---|
| 1 | reader `extension`: `todo-tree.stopScan` calls `ripgrep.kill()` which "kills any running rg child process"; deactivate does the same | gap fill: `kill()` is a no-op | **B is right** [verified in source]. ripgrep.js:16 declares module-level `var currentProcess;` that is never assigned; ripgrep.js:167 declares `var currentProcess = child_process.exec(...)` *inside* the Promise executor, which shadows the module variable; `kill()` (ripgrep.js:198-204) tests the module variable, finds `undefined`, does nothing. stopScan therefore only changes status-bar text and sets `interrupted = true`; rg runs to completion and its results are still added. |
| 2 | reader `extension`: after a rescan "open documents' unsaved buffer contents win over on-disk ripgrep results ... because refreshOpenFiles removes-then-re-adds per URI" | gap fill: `searchResults.remove(uri)` is identity-based and never removes ripgrep entries | **B is right** [verified in source]. searchResults.js:15-21 filters `match.uri !== uri` (object identity); ripgrep matches get a fresh `vscode.Uri.file()` per match (extension.js:333). The buffer "wins" only at the tree level because `provider.reset(document.uri)` (tree.js:960-1008) drops the file's nodes and the stale rg entries stay `added === true`. Any soft `refresh()` (`markAsNotAdded`) re-adds the stale rg entries (ghost nodes at old line numbers). |
| 3 | reader `extension`: `search()` with zero matches "removes any prior results for that path" | gap fill: that removal is a no-op | **B is right**, same identity reason (extension.js:340 passes a brand-new `Uri.file`). |
| 4 | reader `extension`: "If ripgrepArgs is undefined (no default), additional becomes 'undefined --hidden '" | gap fill: package.json default is `"--max-columns=1000 --no-config "` | **Both**: the default exists (package.json:1165-1169) so the `undefined` prefix only appears if a user sets the key to `null`. README.md:350 documents the default as only `--max-columns=1000`. |
| 5 | reader `extension` open question: does package.json declare `todo-tree.highlights.schemes`? If so `general.schemes` is rewritten every activation | — | **Not declared** [verified in source: only `todo-tree.general.schemes` at package.json:544]. extension.js:1159-1169 runs only when the user has a leftover, undeclared `todo-tree.highlights.schemes` in settings; for those users the array-reference comparison (`!==`) is always true and `general.schemes` is rewritten on every activation. Default users: no-op. |
| 6 | reader `extension`: `buildCounter` is written to workspaceState "but never read again" | reader `tree`: tree.js:454 reads it | **Both**: never read in extension.js, read once by the TreeNodeProvider constructor. |
| 7 | README.md:25 "defaultHighlight is not applied to sub tags" | gap fill: code applies it | **Code wins**: highlights.js:357-362 calls `getDecoration(subTag)`, which uses the same `getAttribute` fallback chain (attributes.js:38-45), so colours, ruler, font, gutterIcon and borderRadius all inherit from `defaultHighlight`. |
| 8 | README.md:53 rulerColour "will default to use the foreground colour" | gap fill | **Code wins**: default is the already-processed dark *background* colour or `editor.foreground` (highlights.js:129, :151-154). |
| 9 | README.md:45 iconColour falls back to foreground then background | gap fill | **Code wins**: order is `iconColor` (US spelling, undeclared), `iconColour`, scheme background colour (when useColourScheme), foreground, background, literal `green` (attributes.js:54-77). |
| 10 | README.md:477 multiline: untagged results "belong to the previous result that did have a tag" | gap fill (verified on rg 15.1.0) | **Code differs**: ripgrep.js:24-70 buffers lines until the *accumulated* text matches the JS regex and makes the *first* buffered line the primary result; and rg `--vimgrep -U` prints only the first line of a multi-line match, so ripgrep-scan results never carry continuation lines. Only the editor scan (extension.js:829-840) produces `extraLines`. |
| 11 | reader `tree`: attributes.js:27 quirk "for hideFromTree" | highlights gap fill | **Generalised** [verified in source]: attributes.js:25-27 indexes `customHighlight[tag]` (by the tag, not the matching key) for *every* attribute, so regex-style or case-variant `customHighlight` keys never take effect anywhere (type, colours, icon, ruler, hideFrom*). |
| 12 | issue #887 reporter: git is launched "through a shell rather than execFile" with "no single-flight guard" | — | **Correct as far as the code goes** [verified in source]: extension.js:628 `child_process.exec("git rev-parse HEAD", { cwd })` (shell), inside `setInterval` (extension.js:652), no in-flight tracking, interval never cleared on deactivate. The "exponential" growth the reporter saw is not explained by the code alone; one process per folder per tick accumulates only if git processes do not exit **[inference]**. |
| 13 | issue #882 reporter: "no debouncing" on document change | reader `extension`: 500 ms trailing debounce | **Both partly**: there *is* a 500 ms trailing timer (extension.js:1202-1203) but it is one global handle for all documents, and the work it defers is a full-document JS regex scan plus a full tree pipeline pass (see §12). |
| 14 | reader `extension`: 37 commands | package.json: 34 commands | **Both** [verified in source]: 34 declared (`contributes.commands`) plus 3 registered only in code (`todo-tree.openUrl`, `todo-tree.stopScan`, `todo-tree.onStatusBarClicked`), which therefore have no palette entry. |
| 15 | package.json enum for `filtering.useBuiltInExcludes` is `["none","file excludes","search excludes","file and search excludes"]` | config.js tests the strings | **New finding [verified in source]**: config.js:155 tests `=== "file exclude"` (singular). The enum value `"file excludes"` never matches, so `files.exclude` is honoured only via `"file and search excludes"`. |
| 16 | README.md:60-71 says `type` defaults to `tag` | code | **Effectively the same, via a bug** [verified in source]: `getType` (highlights.js:196-199) falls back to `getConfiguration('todo-tree.highlights').get('highlight')`, an undeclared key, so the default is `undefined`, which reaches the final `else` branch (highlights.js:343-346) = tag-only range. |
| 17 | icons.validateIcons accepts `todo-tree` / `todo-tree-filled` | — | **Bug [verified in source]**: icons.js:137 `(icon !== 'todo-tree' \|\| icon !== 'todo-tree-filled')` is always true, so those two built-in names are reported as "Invalid icons" at startup although `getIcon` renders them (icons.js:33-59). |
| 18 | issue #55 "fixed in v0.0.72" (maxBuffer) | gap fill | The fix made the buffer configurable and stopped surfacing the error; the current code still uses `exec` with `maxBuffer` and **silently truncates** output beyond `ripgrepMaxBuffer` KB (ripgrep.js:166-193, verified with Node 22 in the gap fill). |
| 19 | reader `extension`: "regex.regex scope" | package.json | `todo-tree.regex.regex` is declared `scope: language-overridable` (package.json) but every reader uses `getConfiguration('todo-tree.regex')` with no resource/language scope (config.js:71), so per-language overrides are never honoured **[verified in source]**. |
| 20 | README.md:322 documents `foreroundColourScheme` | package.json | Typo in README; the declared key is `foregroundColourScheme`. |

---

## 1. Activation and lifecycle

### 1.1 Manifest facts [verified in source]

| Item | Value |
|---|---|
| `activationEvents` | `["onStartupFinished"]` (since v0.0.197, after issue #279) |
| `main` | `./dist/extension` (webpack; `.vscodeignore` drops `src/`, `node_modules/`, `test/`) |
| `engines.vscode` | `^1.72.0` |
| `extensionKind` | not declared → VS Code default for an extension with `main` = workspace extension (runs on the remote in Remote-SSH/WSL/containers) |
| `capabilities` (virtual/untrusted workspaces) | not declared |
| View container | `todo-tree-container` in the activity bar, icon `resources/todo-tree-container.svg` |
| View | `todo-tree-view`, `when: "!todo-tree-is-empty"` |
| Keybindings / viewsWelcome / colors | none contributed |
| Runtime deps | `@primer/octicons ^17`, `comment-patterns ^0.10.1`, `fast-strftime`, `find ^0.3`, `micromatch ^4`, `regexp-match-indices ^1.0.2`, `treeify ^1.1` |

### 1.2 `activate(context)` (extension.js:51-110, :1925)

Order of operations:

1. `buildCounter = workspaceState.get('buildCounter', 1)`; stores `++buildCounter` (first activation stores 2). Only consumer: tree.js:454.
2. `currentFilter` restored from workspaceState `currentFilter`.
3. `config.init(context)` (also builds the tagGroups lookup, config.js:10-15); `highlights.init(context, debug)`; `utils.init(config)`; `attributes.init(config)`.
4. `provider = new tree.TreeNodeProvider(context, debug, setButtonsAndContext)`.
5. Status bar item: `createStatusBarItem(Left, 0)`.
6. `todoTreeView = createTreeView('todo-tree-view', { treeDataProvider: provider })` — no `showCollapseAll`, no `canSelectMany`.
7. `var fileSystemWatcher;` declared, never assigned (file watcher removed in 0.0.224).
8. provider, status bar item and tree view pushed to `subscriptions`.
9. `TextDocumentContentProvider` registered for scheme `todotree-export` (export, §10).
10. `ignoreMarkdownUpdate = globalState.get('ignoreMarkdownUpdate', false)`.
11. `register()` called at :1925.

Module state (all live for the extension lifetime, extension.js:20-49): `searchList` (root paths), `currentFilter`, `interrupted`, `selectedDocument` (only ever assigned `undefined`, :1717 — dead), `refreshTimeout`, `fileRefreshTimeout`, `hideTimeout`, `autoGitRefreshTimer`, `periodicRefreshTimer`, `lastGitHead` (folder fsPath → stdout), `openDocuments` (uri.toString() → TextDocument), `provider`, `ignoreMarkdownUpdate`, `markdownUpdatePopupOpen`.

### 1.3 Ripgrep availability gate (extension.js:1260-1265; config.js:82-113)

Before any command or listener is registered: `if (!config.ripgrepPath())` → error `Todo-Tree: Failed to find vscode-ripgrep - please install ripgrep manually and set 'todo-tree.ripgrep' to point to the executable` and `return`. Consequences: none of the 37 commands exist, no listeners, no migration, no status bar content; the (empty) tree view and export provider were already created.

`ripgrepPath()` probes, in order, `fs.existsSync` on: `todo-tree.ripgrep.ripgrep`; `<appRoot>/node_modules/vscode-ripgrep/bin/rg[.exe]`; `<appRoot>/node_modules.asar.unpacked/vscode-ripgrep/bin/`; `<appRoot>/node_modules/@vscode/ripgrep/bin/`; `<appRoot>/node_modules.asar.unpacked/@vscode/ripgrep/bin/`. `rg.exe` when `process.platform` starts with `win`. VS Code 1.122 moved the bundled binary (issue #925, §12), which turns this gate into a hard failure for every user without a manual path.

### 1.4 Startup tail of `register()` (extension.js:1887-1922)

`context.subscriptions.push(outputChannel)` (pushes `undefined`, harmless) → `resetOutputChannel()` → `migrateSettings()` → `validateColours()` → `validateIcons()` → `validatePlaceholders()` → `setButtonsAndContext()` → `resetGitWatcher()` → `resetPeriodicRefresh()`. Then:

- `tree.scanAtStartup === true`: `rebuild()`; then for **each** visible editor with a valid scheme: add to `openDocuments` and call `refreshOpenFiles()` *inside the loop* (N visible editors → N calls, each re-scanning every tracked document: O(N²) `refreshFile` calls); then `documentChanged(activeEditor.document)` if there is an active editor (highlight + 500 ms `refreshFile`). `rebuild()`'s own `.finally(refreshOpenFiles)` runs them all again when ripgrep finishes.
- else: `todoTreeView.message = 'Click the refresh button to scan...'` (cleared to `''` at the start of every `rebuild()`).

### 1.5 `deactivate()` (extension.js:1928-1932)

`ripgrep.kill()` (no-op, §0 #1) then `provider.clear([])`. Intervals and pending timeouts are **not** cleared.

### 1.6 Timer and debounce inventory

| Handle | Delay | Work | Notes |
|---|---|---|---|
| `refreshTimeout` via `refreshTree()` (:125-133) | 200 ms trailing | `provider.refresh(); setButtonsAndContext()` | Shares the handle with `triggerRescan` |
| `refreshTimeout` via `triggerRescan()` (:611-618) | 1000 ms trailing | `rebuild()` | A `refreshTree()` within 1 s (e.g. keystroke → refreshFile → addResultsToTree) **cancels the pending rescan**; git/periodic rescans can be lost while typing |
| `fileRefreshTimeout` (:1198-1204) | 500 ms trailing | `refreshFile(document)` | One handle for all documents: two documents changing within 500 ms → only the last is scanned |
| `hideTimeout` (:725-726) | 1000 ms trailing | `hideTreeIfEmpty()` | Scheduled after every `setButtonsAndContext()` |
| `highlightTimer[editorId]` (highlights.js:367-379) | `highlights.highlightDelay` (500) trailing | `highlight(editor)` | Per editor id (uri JSON + viewColumn) |
| `autoGitRefreshTimer` (:620-658) | `general.automaticGitRefreshInterval` **seconds** | `checkGitHead()` | setInterval; not cleared on deactivate |
| `periodicRefreshTimer` (:660-678) | `general.periodicRefreshInterval` **minutes** | `triggerRescan()` | setInterval; not cleared on deactivate |
| one-shot, not cancellable | 500 ms | trackFile `showInTree(uri)` (:1715) | Rapid editor switches queue several reveals |
| one-shot | 15 000 ms | reset `markdownUpdatePopupOpen` (:987-991) | |
| one-shot | 150 ms | clear line flash decoration (:1665-1668) | Decoration type never disposed |
| not debounced | — | `todo-tree.refresh` → `rebuild()`, save/open → `refreshFile`, `onDidChangeWorkspaceFolders` → `rebuild()`, every filter/scope command → `rebuild()` | Concurrent `rebuild()` calls are not serialised: `searchResults.clear()` runs while earlier rg promises may still resolve |

### 1.7 Persisted state

| Store | Key | Written by | Read by |
|---|---|---|---|
| workspaceState | `buildCounter` | activation (++), resetCache (undefined) | tree.js:454 |
| workspaceState | `currentFilter`, `filtered` | filter / filterClear (:907-914, :1273-1287) | activation, `setButtonsAndContext` |
| workspaceState | `includeGlobs`, `excludeGlobs` | folder/file/scope filter commands | `getOptions`, `applyGlobs`, `isIncluded`, `setButtonsAndContext`, tree.js:477-478 (status node) |
| workspaceState | `submoduleExcludeGlobs` | `rebuild()` when `ignoreGitSubmodules` (:596-606) — stale value is not cleared when the setting is turned off | `buildGlobsForRipgrep` |
| workspaceState | `tagsOnly`, `flat`, `expanded`, `groupedByTag`, `groupedBySubTag` | view-style / expand / group commands (:882-905) | config.js:17-40 (override the `todo-tree.tree.*` defaults) |
| workspaceState | `expandedNodes` | tree.js:1094-1104, :1025-1026, :1047-1048 | tree.js:634-641 |
| workspaceState | `grouped` (legacy) | resetCache only | nothing |
| globalState | `migratedVersion` | "Never Show This Again" buttons | versioned notices (:1095-1157) |
| globalState | `ignoreMarkdownUpdate` | markdown prompt | activation (:110) |

`todo-tree.resetCache` (:1481-1510) clears: includeGlobs, excludeGlobs, expandedNodes, submoduleExcludeGlobs, buildCounter, currentFilter, filtered, tagsOnly, flat, expanded, grouped (workspaceState) and migratedVersion, ignoreMarkdownUpdate (globalState); then `purgeFolder(storageUri)` and `purgeFolder(globalStorageUri)` (`fs.readdir` + `unlinkSync` per file, non-recursive; throws on a missing folder or a sub-directory). It does **not** clear `groupedByTag`/`groupedBySubTag`, does not reset the in-memory `currentFilter`/`ignoreMarkdownUpdate`, and does not refresh.

### 1.8 Legacy flat-setting migration (extension.js:1029-1094)

Runs on **every** activation. For each `(setting, type, destination)` pair, inspects `todo-tree.<setting>` and, for each of globalValue / workspaceValue / workspaceFolderValue that passes `typeMatch` (`typeof item == type || (type == 'array' && item && item.length > 0)`; note `typeof x == 'array'` is never true, so any non-empty string also "matches" as array), writes `todo-tree.<destination>.<setting>` at the corresponding target. The old key is never deleted, so it re-fires each activation, and every write re-enters `onDidChangeConfiguration` (typically a full `rebuild()`).

Pairs (32): autoRefresh→tree, customHighlight→highlights, debug→general, defaultHighlight→highlights, excludedWorkspaces→filtering, excludeGlobs→filtering, expanded→tree, filterCaseSensitive→tree, flat→tree, grouped→tree (writes the undeclared `todo-tree.tree.grouped`), hideIconsWhenGroupedByTag→tree, hideTreeWhenEmpty→tree, highlightDelay→highlights, includedWorkspaces→filtering, includeGlobs→filtering, labelFormat→tree, passGlobsToRipgrep→filtering, regex→regex, regexCaseSensitive→regex, revealBehaviour→general, ripgrep→ripgrep, ripgrepArgs→ripgrep, ripgrepMaxBuffer→ripgrep, rootFolder→general, showBadges→tree, showCountsInTree→tree, sortTagsOnlyViewAlphabetically→tree, statusBar→general, statusBarClickBehaviour→general, tags→general, tagsOnly→tree, trackFile→tree.

### 1.9 Versioned one-time notices (extension.js:1095-1169, :55-67)

`migratedVersion` (globalState, default 0) advances **only** when the user clicks "Never Show This Again"; otherwise the notice repeats every activation. Checks are independent (189 does not suppress 210/223).

- `< 189` and `tree.showInExplorer === true`: executes `vscode.moveViews {viewIds:['todo-tree-view'], destinationId:'workbench.view.explorer'}`; info "Todo-Tree: 'showInExplorer' has been deprecated..." with Open Settings (`workbench.action.openSettingsJson`) / Never Show This Again.
- `< 210` and `general.revealBehaviour` not in `['start of line','start of todo','end of todo']`: info about removed values; Open Settings (`workbench.action.openSettings`) / Never Show This Again.
- `< 223` and `general.enableFileWatcher === true` (undeclared key): info "File watcher functionality will be removed..."; More Info (opens issue #723) / Open Settings / Never Show This Again.
- Always: the `highlights.schemes` → `general.schemes` copy (§0 #5). `settingLocation(setting)` = WorkspaceFolder if `inspect().workspaceFolderValue !== undefined`, else Workspace if `workspaceValue !== undefined`, else Global.

### 1.10 Validation warnings (extension.js:1222-1253; colours.js; icons.js; utils.js:371-415)

- `validateColours()`: `colours.validateColours` checks `defaultHighlight.{foreground,background,iconColour,rulerColour}` and the same under every `customHighlight.<tag>` with `utils.isValidColour` (named colour, theme-colour id from the 388-name snapshot, hex 3/4/6/8, `rgb()`/`rgba()`); `colours.validateIconColours` flags codicon + non-theme colour and octicon + theme colour. Messages: `Todo Tree: Invalid colour settings: ...` / `Todo Tree: Invalid icon colour settings: ... Codicons can only use theme colours. / Theme colours can only be used with Codicons.` Both use dotted `config.get('customHighlight.<tag>.x')` paths, so tags containing `.` are silently skipped.
- `validateIcons()`: codicons must be in codiconNames or productIconNames; other icons must be octicons (bug §0 #17 for `todo-tree`/`todo-tree-filled`). Message `Todo Tree: Invalid icons: ...`.
- `validatePlaceholders()`: `utils.formatLabel(labelFormat, {}, unexpected)` — any remaining `${...}` → error `Todo Tree: Unexpected placeholders (...)`.
- Colours + icons re-run when any of `highlights.enabled/useColourScheme/foregroundColourScheme/backgroundColourScheme/defaultHighlight/customHighlight` changes; placeholders re-run on `tree.labelFormat`.

### 1.11 Debug output channel (extension.js:69-76, :112-123; ripgrep.js:74-81)

`resetOutputChannel()` disposes and, if `general.debug === true`, creates channel "Todo Tree". `debug(text)` prefixes `HH:MM:SS.mmm`. Emitted lines: `Searching <root>...`, ` Match (File): <JSON of every match>`, `Applying globs to N items...`, `Remaining items: N`, `Found N items`, `Attempting to create local storage folder <path>`, `Rescan triggered by change to git repository`, `Setting automatic Git refresh interval to N seconds` / `Automatic Git refresh disabled`, `Setting periodic refresh interval to N minutes` / `Periodic refresh disabled`, `Folder filter include:<json>` / `exclude:<json>`, `Migrating global/workspace/workspaceFolder setting '<name>'`, `Opening <url>`; from ripgrep.js: `Writing pattern file:<path>`, `No pattern file found - passing regex in command`, `Pattern:<regex>`, `Command: <full exec string>`, `Search results:\n<every stdout chunk>`, `Search failed:\n<stderr>`. With debug on, the whole rg stdout and a JSON of each match are written to the channel (slow on large scans).

### 1.12 Configuration-change decision tree (extension.js:1801-1876) [verified in source]

Handler runs only if `affectsConfiguration('todo-tree')` or `files.exclude` or `explorer.compactFolders`.

0. `todo-tree.regex.regex` → **return immediately** (no rescan, no context update; the user must refresh manually).
1. First match only: any of `highlights.enabled / useColourScheme / foregroundColourScheme / backgroundColourScheme / defaultHighlight / customHighlight` → `validateColours(); validateIcons(); documentChanged()`. Else `tree.labelFormat` → `validatePlaceholders()`. Else `general.debug` → `resetOutputChannel()`. Else `general.automaticGitRefreshInterval` → `resetGitWatcher()`. Else `general.periodicRefreshInterval` → `resetPeriodicRefresh()`.
2. First match only: `general.tagGroups` → `config.refreshTagGroupLookup(); rebuild(); documentChanged()`. Else `tree.showCountsInTree` or `tree.showBadges` → `refresh()`. Else any of `todo-tree.filtering`, `todo-tree.regex` (except `.regex`, caught in step 0), `todo-tree.ripgrep`, `todo-tree.tree`, `general.rootFolder`, `general.tags`, `files.exclude` → `rebuild(); documentChanged()`. Else `general.showActivityBarBadge` → `updateInformation()`. Else → `refresh()` (covers all other `general.*`, `highlights.*` incl. `highlightDelay`, `explorer.compactFolders`).
3. Always `setButtonsAndContext()`.

`search.exclude` changes are not observed. Every settings write the extension itself performs (status-bar cycle, toggles, addTag, scan-mode commands, migration) re-enters this handler.

### 1.13 Global RegExp shim (highlights.js:2)

`require("regexp-match-indices").shim()` replaces `RegExp.prototype.exec` for the **whole extension host**. It is used only for `match.indices` in the `capture-groups` highlight type (highlights.js:293-305). It breaks other extensions that use newer flags (`d`, `v`) — issue #853 (§12). A re-implementation must not reproduce this; Node 16+ supports the `d` flag natively.

---

## 2. Scanning

### 2.1 Scan modes (`todo-tree.tree.scanMode`; constants extension.js:35-38)

| Mode | ripgrep over roots | Open-document scans | Close behaviour | Status-bar suffix |
|---|---|---|---|---|
| `workspace` (constant `SCAN_MODE_WORKSPACE_AND_OPEN_FILES`) | yes | yes (`refreshOpenFiles`, save/open/change) | keep if `fileName === root` or starts with `root + path.sep` for any root; else remove (:1767-1795) | none |
| `workspace only` | yes | **none** (`refreshOpenFiles` skipped :473; `shouldRefreshFile()` false :1255-1258) | nothing | none |
| `open files` | **none** unless `general.rootFolder` is set (`searchWorkspaces` pushes nothing, :451-469) | yes | remove (:1759-1761) | ` (in open files)` |
| `current file` | none unless `rootFolder` set | only the active editor's document is scanned (:802); on active-editor change `provider.clear()` then `refreshFile(new doc)` (:1703-1707) | remove | ` (in current file)` |

`shouldRefreshFile()` = `tree.autoRefresh === true && scanMode !== 'workspace only'`. `getRootFolders()` is **not** gated by scan mode: a configured `rootFolder` is ripgrep-scanned in every mode. The tree shows a "Scan mode: ..." status node when `tree.showCurrentScanMode` (tree.js:524-535; `workspace` is displayed as `workspace and open files`).

### 2.2 Root resolution

`getRootFolders()` (extension.js:527-571): reads `general.rootFolder`. If it contains `${workspaceFolder}`: one entry per workspace folder with all occurrences replaced by `folder.uri.fsPath`; with no workspace folders `valid=false`. Else if non-empty: `vscode.Uri.file(rootFolder).fsPath` (single root, bypasses workspace folders). Then `rootFolders.forEach(rootFolder => rootFolder = utils.replaceEnvironmentVariables(rootFolder))` assigns to the callback parameter only — **environment-variable expansion is a no-op** (`utils.replaceEnvironmentVariables`, utils.js:464-472, replaces `${NAME}` with `process.env[NAME]` or `''`). If valid, filters by `utils.isIncluded(folder, filtering.includedWorkspaces, filtering.excludedWorkspaces)`. Returns `undefined` (not `[]`) when invalid; both callers (`rebuild` :591, close handler :1780) then throw `TypeError` on `.length` after the status bar has been set to "Scanning...".

`searchWorkspaces(list)` (:451-469): only in `workspace` / `workspace only`; pushes `folder.uri.fsPath` for each workspace folder whose `uri.scheme === 'file'` and passes the includedWorkspaces/excludedWorkspaces test. Non-file-scheme folders are skipped entirely.

### 2.3 Full rescan pipeline: `rebuild()` (extension.js:573-609) → `iterateSearchList()` (:503-525) → `search()` (:323-351)

1. `todoTreeView.message = ''`; `searchResults.clear(); searchList = []`; `provider.clear(workspaceFolders)`; `interrupted = false`.
2. Status bar: text `Todo-Tree: Scanning...`, tooltip `Click to interrupt scan`, command `todo-tree.stopScan`, `show()` (shown even when `general.statusBar` is `none`).
3. `searchList = getRootFolders()`; if empty, `searchWorkspaces(searchList)`.
4. If `filtering.ignoreGitSubmodules`: `submoduleExcludeGlobs` (undeclared → implicit global) = concat of `utils.getSubmoduleExcludeGlobs(root)` per root, stored in workspaceState. `getSubmoduleExcludeGlobs` (utils.js:435-447) = `find.fileSync('.git', rootPath)` — a **synchronous full-tree walk** (including node_modules) per rebuild — returning the parent dirs of every `.git` entry except the root (issue #358).
5. `iterateSearchList().finally(refreshOpenFiles).then(addResultsToTree)`.

`iterateSearchList()`: `searchList.reduce((p, entry) => p.finally(() => search(getOptions(entry))), Promise.resolve())` — one rg per root, **strictly sequential**, continuing after failures. Final `.finally`: `debug('Found N items')`; `if (getConfiguration('todo-tree.ripgrep').get('passGlobsToRipgrep') !== true) applyGlobs()` — **wrong namespace** (the declared key is `todo-tree.filtering.passGlobsToRipgrep`, package.json:913), so the client-side `applyGlobs()` **always runs**; then `addResultsToTree(); setButtonsAndContext()`. Empty list: `addResultsToTree(); setButtonsAndContext()` immediately.

`search(options)`: `debug('Searching <filename>...')`; `ripgrep.search('/', options)` — the first argument is the process **cwd** (filesystem root); the folder searched is `options.filename`. Resolve: if `matches.length > 0`, for each match `match.uri = vscode.Uri.file(match.fsPath)`, debug JSON, `searchResults.add(match)`; else `searchResults.remove(Uri.file(options.filename))` (no-op, §0 #3). Reject: `showErrorMessage('Todo-Tree: ' + e.message + (e.stderr ? ' (' + e.stderr + ')' : ''))` — plain-object rejections from ripgrep.js have no `.message`, so the user sees `Todo-Tree: undefined`.

Net order per rebuild: sequential rg → `applyGlobs` (always) → `addResultsToTree` → `setButtonsAndContext` → `refreshOpenFiles` (each open doc: `refreshFile` → `addResultsToTree`) → `addResultsToTree` again. `addResultsToTree` runs at least twice plus once per open document.

### 2.4 Ripgrep option construction: `getOptions(filename)` (extension.js:392-449)

| Field | Value |
|---|---|
| `regex` | `'"' + utils.getRegexSource() + '"'` (double-quoted; used only with `-e`) |
| `unquotedRegex` | `utils.getRegexSource()` (written to the pattern file) |
| `rgPath` | `config.ripgrepPath()` |
| `globs` | only when `filtering.passGlobsToRipgrep === true` and non-empty: `buildGlobsForRipgrep(...)` (§2.10) |
| `filename` | the root path (target) |
| storage dir | `context.storageUri.fsPath` created with `mkdirSync({recursive:true})` if missing; **dereferenced unconditionally** (:421) → `TypeError` in a no-folder window when `rootFolder` is set |
| `outputChannel` | the debug channel |
| `additional` | `ripgrep.ripgrepArgs` + `' --hidden '` if `filtering.includeHiddenFiles` + `' -i '` if `regex.regexCaseSensitive === false` (strict; `undefined` = case-sensitive) |
| `maxBuffer` | `ripgrep.ripgrepMaxBuffer` (KB) |
| `multiline` | regex **source string** contains the two characters `\n`, or `regex.enableMultiLine === true` |
| `patternFilePath` | when the storage dir exists and `ripgrep.usePatternFile === true`: `<storageUri>/<crypto.randomBytes(6).readUIntLE(0,6).toString(36)>.txt` (new name per call) |

### 2.5 Ripgrep command line and process (ripgrep.js:72-204) [verified in source]

- Pre-checks reject with a **plain object** `{ error }` (no `.message`): falsy cwd (`No cwd provided`), single argument (unreachable), `!fs.existsSync(options.rgPath)` (`ripgrep executable not found (<path>)`), cwd missing (`root folder not found (<cwd>)`). `options.regex ||= ''`, `options.globs ||= []`.
- rgPath: Windows (`/^win/`) → wrapped in double quotes; POSIX → only spaces backslash-escaped (other shell metacharacters untouched).
- Command: `<rg> --no-messages --vimgrep -H --column --line-number --color never <additional>` + ` -U ` if multiline + (`-f "<patternFile>"` after `writeFileSync(unquotedRegex + '\n')` | `-e "<regex>"`) + ` -g "<glob>"` per glob + ` "<filename>"` (one trailing backslash stripped on Windows) or ` .` if no filename.
- Executed with **`child_process.exec(execString, { cwd, maxBuffer })`** — `/bin/sh -c` on POSIX, `cmd.exe /d /s /c` on Windows, never an argv array. Everything user-controlled is only double-quoted; inside POSIX double quotes `$`, backtick, `\` (before `$ \` " \`) and `"` are interpreted. Verified consequence (gap fill): with `usePatternFile=false`, a tag `TODO$` reaches rg as `TODO$` (end anchor) instead of literal `\$`. Globs, the storage path and folder names are exposed **always**. The default regex survives sh unchanged. Windows `cmd.exe` hazards (`%VAR%`, `^`) untested.
- `maxBuffer = (options.maxBuffer || 200) * 1024`. The exec callback is unused: stdout chunks are concatenated; the **first stderr chunk** unlinks the pattern file and rejects `new RipgrepError(<chunk>, "")`; `close` unlinks the pattern file and resolves `formatResults(results, multiline)` **regardless of exit code**. Verified: exceeding maxBuffer makes Node kill rg (SIGTERM), `close` resolves with the partial stdout — **silent truncation, possibly mid-line**; exit 1 (no matches) → `[]`; `--no-messages` hides IO errors (exit 2, no stderr) → partial results resolve normally; regex parse errors are on stderr → rejection → `Todo-Tree: rg: regex parse error ...`.
- `--no-config` means the user's `RIPGREP_CONFIG_PATH` is ignored; `--hidden` / `-i` are appended after `ripgrepArgs` so they override contrary user args.
- One rg per root, sequential; results are only added after the whole list finishes.

### 2.6 Output parsing: `Match` (ripgrep.js:206-249) and `formatResults` (:24-70)

- Each stdout line → `new Match(line)`. Primary regex `^(?<file>.*):(?<line>\d+):(?<column>\d+):(?<todo>.*)`. `.*` is greedy, so the **last** `:d+:d+:` wins: `/x/a.js:5:1:// TODO ratio 1:2:3:4` → file `/x/a.js:5:1:// TODO ratio 1`, line 2, column 3 (verified). Windows drive paths parse correctly via the greedy group; the fallback branch (:222-245: peel `X:` when `matchText[1] === ':'`, split on `:`, column only when exactly 4 parts) is hit only by lines without `:d+:d+:` (truncated last line).
- `--vimgrep` prints **one line per match**: two tags on one source line → two `Match` objects (same line, different columns); tree.js dedup (`findTodoNode`: label + fsPath + line, tree.js:67-70) may collapse them.
- `--max-columns=1000` (default `ripgrepArgs`): a matching line longer than 1000 **bytes** is replaced by one line `<file>:<line>:<origcol>:[Omitted long line with N matches]` (verified). `createTodoNode` then does `match.substr(column - 1)` (tree.js:203) → often `''` → label `line N`, tag `''`; such tag-less nodes are counted under `TODO` (tree.js:392) and their icon is looked up by label.
- rg `--column` is a 1-based **byte** offset; the tree slices the JS string by char index and reveals with `Position(line, column-1)`. Non-ASCII text before the tag → wrong slice (possibly losing the tag) and a cursor past the tag (verified: `ééé // TODO utf` → column 8, char index 5).
- CRLF files: whether `match` keeps a trailing `\r` (affecting `endColumn`) is untested.
- **Multiline `formatResults`** (:33-65): iterate lines in rg order; push each `Match` and test `utils.getRegexForEditorSearch(true)` against the accumulated `match` texts joined by `\n`; on the first success the **first** buffered Match becomes the result and all others become its `extraLines`; buffers reset. Consequences (verified on rg 15.1.0): (a) rg `--vimgrep -U` prints only the first line of a multi-line match, so continuation lines never reach this code; (b) with an optional multi-line tail (both README examples) every line matches alone → every rg result gets `extraLines: []`; (c) if a line does not satisfy the JS regex (mandatory `\n`, omitted-line placeholder, shell-mangled `-e` regex) it stays buffered and, when a later line makes the accumulated text match, that earlier line becomes the primary result and the genuinely tagged later line is demoted to an extra line — across file boundaries; (d) lines still buffered at EOF are dropped. The local `buffer` array is dead code.

### 2.7 Multiline detection and regex flag parity

| Aspect | ripgrep | JS editor regex (`utils.getRegexForEditorSearch`, utils.js:321-339) |
|---|---|---|
| Source | `utils.getRegexSource()` (`($TAGS)` → `utils.getTagRegex()`), utils.js:310-319 | same |
| Multiline | ` -U ` when source contains literal `\n` or `enableMultiLine` | flag `s` **only** when `enableMultiLine === true`; flag `m` always |
| Case | ` -i ` when `regexCaseSensitive === false` | flag `i` when `=== false` |
| Global | n/a | `g` when requested (`highlight`, `refreshFile`, `goToPrevious`; not `goToNext`) |
| Engine | Rust regex (no backrefs/lookaround; `.` excludes `\n` even under `-U`) | ECMAScript |

A `\n`-containing regex without `enableMultiLine` gets no `s` on the JS side (matches rg); `enableMultiLine` adds `s` on the JS side only (rg gets no `(?s)`), so `.` semantics diverge in that configuration. `utils.getRegexForRipGrep` (utils.js:341-350) is exported but unused. README.md:490-494 warns of a performance reduction with `-U` (rg reads whole files).

### 2.8 Editor (buffer) scan: `refreshFile(document)` (extension.js:777-861)

1. `searchResults.remove(document.uri)` **unconditionally** (identity-based; removes only this TextDocument's previous editor entries).
2. Eligible if `config.isValidScheme(uri)` (scheme in `general.schemes`, config.js:146-150) and `isIncluded(uri)` (§6.3) and (`scanMode !== 'current file'` or `document.fileName === activeTextEditor.document.fileName`).
3. `text = document.getText()`; `regex = getRegexForEditorSearch(true)`. Per match: while `text[match.index]` is `\n` or `\r`, `match.index++` and strip the first char of `match[0]`; `sections = match[0].split('\n')`; primary result `{ uri: document.uri, line: pos.line + 1, column: pos.character + 1, match: <full text of that line> }` (`addResult(offset, false)`, :779-795). If `sections.length > 1`: `result.extraLines = []`, `offset += sections[0].length + 1`, then for each remaining section `addResult(offset, true)` (line text with `utils.removeLineComments` applied: trim, then strip a leading single-line comment marker for the language via `comment-patterns`; `.jsonc` treated as `.js`) and `offset += section.length + 1`. Added unless `searchResults.contains(result)` (identity + `==` line/column; effectively only dedups two regex matches resolving to the same line/column in this pass).
4. `provider.reset(document.uri)` if anything was added, else `provider.remove(null, document.uri)`; then `addResultsToTree()`.

Notes: a document failing scheme/inclusion still has its entries removed and `provider.remove` called (editing a newly excluded file removes it from the tree); `isMatch`/`found` (:809-815) is dead code; continuation results have `column` 1 and comment-stripped `match`, so `endColumn` is short; a `\r\n` document keeps `\r` on sections (split on `\n` only).

Triggers: `onDidSaveTextDocument` (immediate, if valid scheme, basename ≠ `settings.json`, `shouldRefreshFile()`; :1730-1739); `onDidOpenTextDocument` (immediate; also adds to `openDocuments`; :1741-1751); `onDidChangeTextDocument` → `documentChanged` (500 ms shared debounce; :1183-1220); `onDidChangeActiveTextEditor` (current-file mode immediate `refreshFile`; always `documentChanged` → 500 ms `refreshFile` when autoRefresh; :1697-1728); `refreshOpenFiles()` (:471-480; every entry of `openDocuments`, insertion order, unless `workspace only`).

`openDocuments` (:30): added on active-editor change (any scheme), on open (when `shouldRefreshFile()` and valid scheme), at startup for visible editors; removed on close; never cleared by `rebuild`/`resetCache`.

### 2.9 Result cache: searchResults.js [verified in source]

Module-level array. `add` push; `clear`; `remove(uri)` keeps `match.uri !== uri` (identity); `contains(result)` identity + `==` line/column; `addToTree(tree)` calls `tree.add` for entries with `added !== true` and sets `added = true`; `markAsNotAdded()`; `filter(fn)` (used by `applyGlobs`); `count()` = raw length (includes stale rg entries and editor duplicates); `containsMarkdown()` = any `uri.fsPath` ending `.md` (drives the markdown upgrade prompt).

Ghost semantics: editing an open file keeps the file's rg entries (`added=true`, old lines) in the cache; the tree is correct only because `provider.reset` removed the nodes. Any soft `refresh()` (view-style/expand/group commands, `showCountsInTree`/`showBadges` change, status-bar cycle, any `general.*` change, `explorer.compactFolders`) calls `markAsNotAdded()` and re-adds **both** the stale rg entries and the current editor entries, so moved/deleted TODOs reappear at their old lines unless `findTodoNode` (identical label + fsPath + line) collapses them. Only `rebuild()` clears them. In `current file` mode other documents' entries stay `added=true` after `provider.clear`, so they are absent until a `refresh()` transiently re-adds them. Closed-then-reopened documents get a new `Uri` instance, so old editor entries can never be removed by identity in `workspace` mode.

### 2.10 Ignore and glob semantics

**Glob list for rg** (`buildGlobsForRipgrep` / `addGlobs`, extension.js:353-390; only when `passGlobsToRipgrep === true`): order = `[...filtering.includeGlobs, ...workspaceState includeGlobs, ...filtering.excludeGlobs prefixed '!', ...workspaceState excludeGlobs prefixed '!']`; then, if `shouldUseBuiltInFileExcludes()`, `'!' + key` for every `files.exclude` key whose value `=== true` (object values such as `{when:...}` ignored); if `shouldUseBuiltInSearchExcludes()`, same for `search.exclude`; if `ignoreGitSubmodules`, `'!' + g` for each submodule folder. rg `-g` semantics apply (gitignore-style, later globs win). rg also honours `.gitignore`/`.ignore` and skips hidden files and binaries by default (`--hidden` only when `includeHiddenFiles`).

**Client-side post-filter** (`applyGlobs`, :481-501; always runs, §0 #15's sibling bug at :511): if config + temp include/exclude globs are non-empty, `searchResults.filter(m => utils.isIncluded(m.uri.fsPath, includes, excludes))`. Does **not** apply `files.exclude`/`search.exclude`/submodule/hidden rules.

**Per-document test** (`isIncluded(uri)`, :747-775): false if `!uri.fsPath`; excludes = `filtering.excludeGlobs` + temp excludes + (`files.exclude` true-keys if file excludes enabled) + (`search.exclude` true-keys if search excludes enabled); `included = utils.isIncluded(fsPath, includes + temp, excludes)`; result `included && (!utils.isHidden(fsPath) || includeHiddenFiles)`. Does **not** consult submodule globs (an open file inside an ignored submodule still appears via the editor scan). Runs on every keystroke path per visible editor.

**`utils.isIncluded(name, includes, excludes)`** (utils.js:352-369): backslashes in globs → `/`; `included = includes.length === 0 || micromatch.isMatch(name, includes)`; then excluded if `micromatch.isMatch(name, excludes)`. `name` is the **absolute** fsPath (Windows: backslashes, drive letter). micromatch vs rg glob differences are unverified (open question). **`utils.isHidden`** (utils.js:449-452): basename contains `.` **and** `path.extname === ''` — i.e. only dot-files without an extension (`.env`, `.gitignore`); `.eslintrc.json` is not "hidden".

**Built-in excludes** (config.js:152-162): `shouldUseBuiltInFileExcludes` tests `"file exclude"` (never matches the enum value `"file excludes"`, §0 #15) or `"file and search excludes"`; `shouldUseBuiltInSearchExcludes` tests `"search excludes"` or `"file and search excludes"`.

**Workspace-folder filters**: `includedWorkspaces`/`excludedWorkspaces` are matched with `utils.isIncluded` against the folder **fsPath** (README says "workspace names"; wildcards work via micromatch).

**Temporary globs** (workspaceState `includeGlobs`/`excludeGlobs`): set by the folder/file context commands and scopes (§6.2); `createFolderGlob(folderPath, rootPath, filter)` (utils.js:417-433): Windows → `**/<folderPath relative to dirname(root), slashes normalised><filter>`; POSIX → `<absolute folderPath><filter>` with `//` collapsed. `excludeThisFile` stores the raw absolute path.

### 2.11 Git HEAD polling and periodic refresh (extension.js:620-678)

- `resetGitWatcher()`: clear existing interval; if `general.automaticGitRefreshInterval > 0`, `setInterval(checkGitHead, s*1000)`. `checkGitHead`: for each **workspace folder** (not `rootFolder`) `child_process.exec('git rev-parse HEAD', { cwd })`; `gitHead = stdout` (errors ignored); if `lastGitHead[folder] !== undefined && gitHead != lastGitHead[folder]` → `triggerRescan()`; store. First poll only records. Non-git folders yield empty stdout consistently. No in-flight guard, shell spawn, never cleared on deactivate (issue #887).
- `resetPeriodicRefresh()`: `setInterval(triggerRescan, min*60*1000)` when `general.periodicRefreshInterval > 0`.
- Both are subject to the shared-timer cancellation (§1.6).
- `onDidChangeWorkspaceFolders` (:1875-1880): `provider.clear(folders); provider.rebuild(); rebuild()` (no debounce). `lastGitHead` entries for removed folders are never pruned.

---

## 3. Tag extraction and regex semantics

### 3.1 Tag list and `($TAGS)` expansion

- `config.tags()` (config.js:115-119): `general.tags`, or `["TODO"]` when empty. Default `["BUG","HACK","FIXME","TODO","XXX","[ ]","[x]"]`.
- `utils.getTagRegex()` (utils.js:183-194): copy, `.sort().reverse()` (reverse lexicographic, so longer tags with a common prefix sort first only when the extra characters sort higher — the README's "specify the longer tag first" advice is therefore not what the code does; order is by sort, not by config order), each tag: `\` → three backslashes (`replace(/\\/g, '\\\\\\')`), then `|{}()[]^$+*?.-` escaped with `\`. Joined with `|` and wrapped in `( ... )`. Tags containing a backslash produce an invalid escape.
- `utils.getRegexSource()` (utils.js:310-319): `regex.regex.split('($TAGS)').join(getTagRegex())` — the literal token `($TAGS)` including parentheses; a bare `$TAGS` is not expanded here (but `extractTag` tests for `$TAGS` without parentheses, utils.js:207).
- Default regex (package.json): `(//|#|<!--|;|/\*|^|^[ \t]*(-|\d+.))\s*($TAGS)` — comment leaders, line start, or a Markdown list bullet / numbered item, optional whitespace, tag.

### 3.2 `utils.extractTag(text, matchOffset)` (utils.js:196-272) [verified in source]

Given the matched text (tree: text sliced from the match column; highlights: `match[0]`), returns `{ tag, withoutTag, before, after, tagOffset, subTag }`.

- If `regex.regex` contains `$TAGS`: `tagRegex = new RegExp(getTagRegex(), flags)` (`i` when case-insensitive), `subTagRegex = new RegExp(subTagRegex, flags)`; `tagMatch = tagRegex.exec(text)` (**first tag occurrence anywhere in `text`**, unanchored). `rightOfTagText = text after the tag, trimmed`; `subTag = subTagRegex.exec(rightOfTagText)[1]` if a capture group matched; `rightOfTag = rightOfTagText.replace(subTagRegex, '')` (whole sub-tag match removed; assigned to an undeclared global `rightOfTag`). If `rightOfTag` is empty: `withoutTag = before = text.substr(0, matchOffset ? matchOffset - 1 : tagMatch.index).trim()`, `after = ''`; else `before = same`, `withoutTag = after = rightOfTag`. `originalTag` = the entry of `config.tags()` equal to the match (exact, or case-insensitive when `regexCaseSensitive` is false — so the *configured* casing is reported).
- Else (no `$TAGS`, non-blank regex): `match = new RegExp(regex, flags).exec(text)`; tag = whole match; `before` = text before it, `after` = text after.
- `tag` is `''` when nothing matched. Note the `matchOffset - 1` slice: tree.js passes `result.column` although the text was already sliced to start at that column, so `before` is computed against the wrong origin.

`utils.updateBeforeAndAfter(result, text, matchOffset)` (utils.js:274-308): same computation over a different `text` (highlights pass it the text from match start to end of the match's first line) updating `tagOffset`, `subTag`, `before`, `after`, `text`.

### 3.3 Sub-tags (`todo-tree.regex.subTagRegex`, default `""`)

Applied to the text right of the tag. Capture group 1 → `subTag`; the whole match is removed from `after`/`withoutTag`. Compiled with `i` in `extractTag`/`updateBeforeAndAfter` when case-insensitive, but **without flags** in `highlight()` (highlights.js:249), so tree grouping and editor sub-tag highlighting can disagree on case. Sub-tags drive: tree grouping (§4), `${subtag}` placeholders, `tree.subTagClickUrl`, and `tag-and-subTag` highlighting (only when `customHighlight[subTag]` exists, highlights.js:316).

### 3.4 Tag groups (`todo-tree.general.tagGroups`, config.js:169-183)

`{ GROUP: [tag, ...] }` inverted into `tag → GROUP` at init and on setting change. `config.tagGroup(tag)` returns the group name. Where the group name replaces the tag: tree node `tag` (icons, counts, hideFromTree, sorting, tag-node lookup; `actualTag` keeps the real tag for `${tag}` placeholders), highlight decoration key and every attribute lookup (highlights.js:265-266), colour-scheme index (`config.tags().indexOf(group)` = -1 → undefined colours). README says all grouped tags must also be in `general.tags`.

### 3.5 Comment stripping (utils.js:76-181)

- `removeBlockComments(text, fileName)`: `.jsonc`→`.js`, `.vue`→`.html`, `.hs`→`.cpp` (with `/*`,`/**`,`*/` rewritten to `{-`,`-}` in the pattern source), Markdown → HTML patterns; if the language has multi-line comments, `comment-patterns.regex(fileName)` is exec'd once and the first non-empty content capture group replaces `text`. Used by `createTodoNode` on the joined match text (tree.js:212).
- `removeLineComments(text, fileName)`: trim, then strip one leading `singleLineComment.start` marker for the language. Used only for editor-scan continuation lines.

### 3.6 Positions

- Editor scan: `line = position.line + 1`, `column = position.character + 1` (1-based chars).
- rg: 1-based line, 1-based **byte** column (§2.6).
- Tree stores `line` 0-based, `column` 1-based, `endColumn = column + match.length` (full line length, overshoots when column > 1) (tree.js:230-246).
- Reveal: `general.revealBehaviour` → `Position(line, endColumn-1)` / `(line, 0)` / `(line, column-1)`; other values → no selection (tree.js:707-724).

### 3.7 Go to next / previous (extension.js:1547-1640)

- `todo-tree.goToNext`: non-global regex (`'m'` + optional `i`/`s`). Per selection: search `text.substring(cursorOffset)`; if a match sits at index 0 (cursor already on a match) advance by its length and search again; offset = cursor + index (+1 if the match starts with `\n`). All-or-nothing across selections; no wrap-around; `revealRange` on the first selection. Throws with no active editor.
- `todo-tree.goToPrevious`: global regex re-created per selection; `while (result = regex.exec(text.substring(0, cursorOffset)))` (undeclared global `result`) keeps the last match; excludes a match starting exactly at the cursor; same all-or-nothing rule.

### 3.8 Markdown regex upgrade prompt (extension.js:979-1025)

From `addResultsToTree` when `searchResults.containsMarkdown()`, guarded by `markdownUpdatePopupOpen`/`ignoreMarkdownUpdate`. Fires only if `regex.regex` still contains the old `|^\s*- \[ \])` alternation. Message "Todo Tree: There is now an improved method of locating markdown TODOs." with More Info (opens README#markdown-support) / Never Show This Again (globalState); if the regex equals the declared default, adds "Would you like to update your settings automatically?" + Yes → `addTag('[ ]')`, `addTag('[x]')`, regex set (Global) to `(//|#|<!--|;|/\*|^|^[ \t]*(-|\d+.))\s*($TAGS)`. Dismissal sets the in-memory ignore flag for the session. The regex write triggers nothing (§1.12 step 0); the tags write triggers a rebuild.

### 3.9 Hazards to design around

- A user regex that can match the empty string makes `while (regex.exec(text))` loop forever in `highlight()` and `refreshFile()` (global exec does not advance on empty matches) — extension host hang.
- Shell mangling of `-e` regex / globs / paths (§2.5).
- rg vs JS regex dialect differences; the two scanners can disagree, and the editor scan overrides the tree for open files.

---

## 4. Data model and tree view

### 4.1 Node shapes (tree.js:11-27, :118-270)

Two node kinds, `type` = `"path"` (PATH) or `"todo"` (TODO); roots live in the module-global `nodes` array. Every node has `id = buildCounter*1000000 + nodeCounter++` (`nodeCounter` never resets; `buildCounter` from workspaceState, bumped `% 100` by `provider.rebuild()` without persisting), `visible: true`, `fsPath`, `label`.

| Variant | Extra fields | Created by |
|---|---|---|
| Workspace root | `isWorkspaceNode`, `isFolder:true`, `nodes`; label = folder name (file scheme) or `uri.authority`; fsPath = `uri.fsPath` or `authority + fsPath` (no separator) | `addWorkspaceFolders()` (:430-439) when folders exist and not tags-only; called by `clear(folders)` and lazily by `add()` when `nodes` is empty |
| Path node | `pathElement`, `nodes`, `isFolder`, `subTag`; fsPath = `path.join(root, ...elements)` | folders, files, and tag/sub-tag group nodes in flat and tree views (:134-150). Tag-group nodes add `tag` + `isRootTagNode:true`; sub-tag group nodes add `subTag`. **Bug:** callers pass `subTag` as the 3rd positional (`isFolder`) argument, so group nodes get `isFolder = <subTag string or undefined>` |
| Flat file node | `pathLabel` (`(relative dir)` or `''`), `nodes`; **no** `isFolder`, no `subTag` | flat view (:152-166) |
| Tags-only tag node | `isRootTagNode`, `tag` (= `TAG` or `TAG (sub)`), `nodes`; fsPath = fsPath of the **first todo that created it** | tags-only + group-by-tag (:168-182) |
| Tags-only sub-tag node | `isRootTagNode`, `subTag`, `isFolder:true`, `nodes`; fsPath = the sub-tag string | tags-only + group-by-sub-tag (:184-199) |
| TODO | `uri`, `tag` (group name if grouped), `actualTag`, `subTag`, `line` (0-based), `column` (1-based), `endColumn`, `before`, `after`, `extraLines: []`, optionally `isExtraLine`, `hidden`, `parent` | `createTodoNode(result)` (:201-270) |
| Status pseudo-node | `{label, notExported:true, isStatusNode:true, icon, tooltip?, empty?}`; no `type`, no `fsPath` | `getChildren(root)` only (:476-535) |

`createTodoNode`: `joined = match.substr(column-1)` + `'\n' + extraLine.match` for each extra line; `text = removeBlockComments(joined)`; `extracted = extractTag(text, column)`; `label = extracted.withoutTag || 'line ' + result.line`; when **not** grouped by tag: `label = extracted.tag` if `result.extraLines` is truthy (an empty `[]` from multiline-mode rg results counts!), else `tag + ' ' + label`; when grouped the tag is omitted. `tag = tagGroup || extracted.tag`. Extra lines: each `extraLine.match` is **overwritten** with the corresponding comment-stripped line (mutating the cached result), `extraLine.uri = result.uri`; skipped if empty or equal to the tag; otherwise a recursive `createTodoNode` with `isExtraLine = true`.

### 4.2 Placement (`add(result)`, tree.js:857-958) [verified in source]

`fullPath = uri.scheme === 'file' ? uri.fsPath : path.join(uri.authority, uri.fsPath)`; `rootNode = locateWorkspaceNode(fullPath)` (:272-284: prefix match `fsPath === root || fsPath.startsWith(root + sep)`, iterates all roots, **last match wins**, exact-case). `hidden = true` when `config.shouldHideFromTree(tag || label)`. `tagPath = subTag ? tag + ' (' + subTag + ')' : tag`.

| Mode | Placement |
|---|---|
| tags-only + groupedByTag | root tag node found by `findTagNode(tagPath)` or created (pushed to the end); untagged todos pushed to root if not duplicate |
| tags-only + groupedBySubTag (groupedByTag wins if both) | root sub-tag node found by `findSubTagNode` or created (**unshifted to the front**); todos without sub-tag pushed to root |
| tags-only, ungrouped | todos pushed straight to root if no `findTodoNode` duplicate; no `parent` |
| flat (`shouldFlatten()`) **or file outside every workspace folder** | `locateFlatChildNode` (:286-327): optional group level (tag node via `findTagNode`, else sub-tag node; fsPath = `join(root or JSON.stringify(result), tag)`); `nodePath = subTag ? join(fullPath, subTag) : fullPath`; node found by exact fsPath or created (`label = basename(nodePath)`, `pathLabel = '(' + dirname(relative or absolute) + ')'`) |
| tree | `relativePath.split(path.sep)` (+ `subTag` appended as a trailing pseudo-folder when not grouped by sub-tag, :931-937); `locateTreeChildNode` (:329-384): optional group level above the folder chain (tag node fsPath = `join(root, [subTag], tag)`; sub-tag node), then one path node per element found by `findPathNode(pathElement)` or created with `isFolder = level < last`, **every created node receiving the current todo's `subTag`** |

Common tail: `childNode.expanded = result.expanded` (always undefined); if no `findTodoNode` duplicate (label + fsPath + line; column ignored; case-sensitive): `todo.parent = childNode`, push, `childNode.showCount = true` (never read). If `pathElements` is empty (file path equals the root) the todo is silently dropped.

Lookup case sensitivity: `findTagNode`/`findSubTagNode` follow `regex.regexCaseSensitive` (config read per comparison); `findPathNode`/`findExactPath`/`findTodoNode` are always case-sensitive.

Known consequences: a sub-tagged todo in flat view gets a node labelled with the sub-tag and `pathLabel` = the file; `reset(uri)`/`remove(uri)` never match `file/sub` nodes (stale until full refresh); in tree view the file node becomes `isFolder:true` (folder icon, `folder` context menu) with a sub-tag child; a folder first created by a sub-tagged todo keeps that `subTag` forever (no badge, no `file` context value, `subTagClickUrl` command attached, `treeHasSubTags` set); tags-only tag nodes for sub-tagged todos are keyed `TAG (sub)` (icon lookup and tag-order sort see `-1`).

### 4.3 `getChildren` / `getParent` (tree.js:458-583)

Root: `availableNodes` = nodes with `nodes === undefined` or `nodes.length > 0` (raw count; hidden children count); `rootNodes = availableNodes.filter(isVisible)`; `nodesToGet = rootNodes.length`. Then the filter status node: `totalFilters = tempIncludes + tempExcludes (+1 if currentFilter)`; tooltip lines `Tree Filter: "<text>"`, `Include: <glob>`, `Exclude: <glob>`; if > 0: label `N filter(s) active`, tooltip += `\nRight click for filter options`, icon `filter`. If no visible roots: label += (`, ` if non-empty) + `Nothing found`, icon `issues`, `empty = availableNodes.length === 0`. Unshifted to the front if the label is non-empty. Then, if `tree.showCurrentScanMode`, `{label: 'Scan mode: ' + mode, icon: 'search'}` unshifted to the very front. Finally **root compaction**: every root with `isRootTagNode && nodes.length === 1` is replaced by its single child (raw length; a hidden single child gets promoted and rendered; the promoted node's `parent` still points at the removed tag node).

PATH node: compact-folders skip (§4.5) then `nodes.filter(isVisible)` or `undefined`. TODO node: `extraLines.filter(isVisible)` if non-empty, else `node.text` (never set → `undefined`). `getParent` = `node.parent` (only TODOs under a container have one; `reveal()` of a deep PATH node is treated as a root).

### 4.4 `getTreeItem` (tree.js:585-775) [verified in source]

- Label = `node.label + (' ' + pathLabel)`; TODO labels reformatted with `tree.labelFormat` **only when** `extraLines` is empty (:701-705).
- `resourceUri = Uri.file(fsPath)` when `tree.showBadges` and no `tag` and no `subTag` (:605-608) — enables SCM/problem decorations on workspace/folder/file/flat nodes; uses `Uri.file` even for remote paths.
- Tooltip: TODO → `formatLabel(tree.tooltipFormat, node)`; PATH → `fsPath` (fabricated for tag nodes); sub-tag nodes with `subTagClickUrl` → `Click to open <url>`; status → `node.tooltip`.
- Collapsible state (PATH): `expandedNodes[fsPath]` if defined, else `tree.expanded` (workspaceState override) (:634-641). TODO with extra lines: always Expanded (:684-687).
- Icons (:648-663, :689-699): PATH with `tag` → `icons.getIcon(tag)`; workspace → `ThemeIcon('window')`; `isFolder` truthy → `ThemeIcon.Folder`; else `ThemeIcon.File`. TODO: only when `hideIconsWhenGroupedByTag !== true` or no grouping active: `icons.getIcon(tag || label)`; extra-line nodes → `iconPath = 'no-icon'` (bogus path used as an indent placeholder). Status nodes → `ThemeIcon(node.icon)` (`filter`/`issues`/`search`).
- Sub-tag click (:665-680): any PATH node with `subTag !== undefined` and non-blank `tree.subTagClickUrl` gets `command: todo-tree.openUrl` with `formatLabel(url, node)` (`${tag}` → `undefined` for PATH nodes).
- TODO command (:707-732): `todo-tree.revealInFile` with `[node.uri || Uri.file(fsPath), { selection }]` per `general.revealBehaviour` (read from config per rendered item).
- Counts (:743-749): when `tree.showCountsInTree` and PATH: `description = sum(countTags(node, {}, false))` — visible, non-hidden TODOs, keyed by `tag || 'TODO'`, excluding `hideFromActivityBar` tags; extra lines not counted; recomputed per rendered node.
- `contextValue`: `folder` when `isFolder === true` (workspace roots, folders, tags-only sub-tag nodes, files with a sub-tag child); `file` when not root-tag/workspace/status/TODO and `subTag === undefined`; otherwise none (tag-group nodes, inherited-subTag path nodes).
- `treeHasSubTags = true` for any rendered node with `subTag !== undefined` (:760-763; reset in `refresh()`); read by `hasSubTags()` → context `todo-tree-has-sub-tags` (only rendered nodes count).
- `nodesToGet` countdown (:643-646, :765-773): Expanded PATH nodes add their visible children; each non-status item decrements; at 0 `onTreeRefreshed()` (= `setButtonsAndContext`) fires. Expanded TODO children are not counted (can fire early / go negative).
- Status nodes render with `label = ''` and `description = node.label` (dimmed) (:735-741).
- Compact folders (:620-632): when `explorer.compactFolders && !tree.disableCompactFolders` and `node.tag === undefined`: chain `label += '/' + child.label` while the current node has exactly one PATH child that itself has PATH children (takes `nodes[0]`, which may be a TODO if one sorts first); `getChildren` skips while `nodes.length === 1 && nodes[0].nodes.length > 0 && nodes[0].isFolder` (counts all children, so a folder with one sub-folder plus one todo is chained in the label but not skipped).

### 4.5 Sorting (tree.js:72-116, :1170-1202) [verified in source]

`sort()` is a no-op when `tree.sort === false` (then rg's own order and `add()` insertion order stand). Otherwise recursive, then per level:

- Tags-only view: `tree.sortTagsOnlyViewAlphabetically` → folders first, then `label` ascending, ties by line/column; else folders first, then by index of `node.tag` in `config.tags()` (`-1` for `TAG (sub)` keys, groups, and untagged → sorts first), ties by filename and line.
- Otherwise `sortByFilenameAndLine`: folders first (`isFolder` compared with `===`; `undefined` vs `false` vs string all differ, so flat file nodes, sub-tag pseudo-folders and TODOs interleave inconsistently); two root tag nodes → by `config.tags()` order, then line/column; otherwise `fsPath` ascending (code-point order, case-sensitive), then line, then column.

`config.tags()` is called per comparison. Children are not re-sorted in `getChildren`; `sort()` runs in `refresh()` (tree.js:792-799) before the change event.

### 4.6 Filter, reset, remove, getElement, export (tree.js:801-1010, :1073-1156) [verified in source]

- `filter(text)`: `matcher = new RegExp(text, filterCaseSensitive ? '' : 'i')` (**the filter text is a regex**, invalid input throws); TODO nodes: `visible = !text || matcher.test(label)` (label = the stored node label, which includes the tag when not grouped and is `tag`-only for multiline nodes); recurses into `nodes` and `extraLines`; any node with children/extra lines gets `visible = visibleChildren + visibleExtraLines > 0` (a multi-line parent is visible iff some extra line matches). `clearTreeFilter()` sets everything visible.
- `reset(uri)` (used after a successful `refreshFile`): removes root-level TODOs for that path, tag-less TODOs for that path, and for nodes with `fsPath === fullPath` **or `isRootTagNode`**: in tags-only mode keeps only TODO children from other files, otherwise `nodes = []`. Does not touch `expandedNodes`.
- `remove(callback, uri)` (used on close / no matches): drops every node with `fsPath === fullPath` at any depth (including a tags-only tag node whose fsPath happens to be that file, taking other files' todos with it), then prunes empty non-workspace containers; deletes and persists `expandedNodes` per removed node; calls `callback(fsPath)` per removal.
- `getElement(filename, found)`: depth-first; calls `found(child)` for **every** node with that fsPath (a file under several tag groups → several reveals). Callback never invoked if absent.
- `exportTree()`: `getChildren()` (so status nodes are excluded via `notExported`, root compaction applies) → `exportChildren`: PATH → `{ [label]: {...children} }`; TODO → key `line N` (1-based; prefixed with `fsPath + ' '` in tags-only view), value `formatLabel(labelFormat, node) + pathLabel` when `labelFormat !== ''` else `node.label`. Extra lines are never exported; hidden nodes **are** (no `isVisible` check).
- `getFirstNode()`: first visible root (unused by extension.js).

### 4.7 View title, counts, buttons, context keys (extension.js:176-199, :680-745)

- Title (inside `updateInformation`): `Flat` / `Tags` / `Tree` (workspaceState-overridden `flat`/`tagsOnly`), + ` (N)` when `N > 0 && tree.showCountsInTree`; `N` = current-file total when `general.statusBar === 'current file'`, else workspace total.
- `setButtonsAndContext()` issues 22 `setContext` calls on every run: `todo-tree-show-{reveal,scan-mode,view-style,group-by-tag,group-by-sub-tag,filter,refresh,expand,export}-button` (from `tree.buttons.*`; reveal additionally requires `!tree.trackFile`), `todo-tree-expanded`, `todo-tree-flat`, `todo-tree-tags-only`, `todo-tree-grouped-by-tag`, `todo-tree-grouped-by-sub-tag`, `todo-tree-filtered`, `todo-tree-collapsible` (`!tagsOnly || groupedByTag || groupedBySubTag`), `todo-tree-folder-filter-active`, `todo-tree-global-filter-active` (the filter string itself), `todo-tree-can-toggle-compact-folders` (`explorer.compactFolders === true`), `todo-tree-has-sub-tags`, `todo-tree-scan-mode`; then schedules `hideTreeIfEmpty` (1 s): `todo-tree-is-empty = tree.hideTreeWhenEmpty && no non-status root children` (the view's `when` clause).
- Expansion events: `onDidExpandElement`/`onDidCollapseElement` → `provider.setExpanded(element.fsPath, bool)` (keyed by fsPath: nodes sharing an fsPath share state).

### 4.8 Menus and when-clauses (package.json `contributes.menus`) [verified in source]

View title (`view =~ /todo-tree/`, group `navigation@N`): export (@1, `todo-tree-show-export-button`), reveal (@2, `tags-only == false && show-reveal-button`), scan-mode cycle buttons (@3, one per current mode: `workspace only`→scanOpenFilesOnly, `open files`→scanCurrentFileOnly, `current file`→scanWorkspaceAndOpenFiles, `workspace`→scanWorkspaceOnly; all need `show-scan-mode-button`), view style (@4: `flat==false && tags-only==false`→showFlatView; `flat==true && tags-only==false`→showTagsOnlyView; `flat==false && tags-only==true`→showTreeView), group by tag/ungroup (@5), group by sub-tag/ungroup (@6, also `has-sub-tags`), filter/filterClear (@7, by `todo-tree-filtered`), refresh (@8), expand/collapse (@9, by `todo-tree-expanded`).

Item context menu groups: `1-filters` (filter, filterClear [`global-filter-active`], excludeThisFolder [`viewItem == folder`], excludeThisFile [`viewItem == file`], showOnlyThisFolder / showOnlyThisFolderAndSubfolders [`folder`], removeFilter / resetAllFilters [`folder-filter-active`]); `2-toggles` (toggleItemCounts, toggleBadges, toggleCompactFolders [`can-toggle-compact-folders`]); `3-view` (the four scan-mode commands except the current one); `4-tree` (expand/collapse, showFlatView [`flat==false`], showTagsOnlyView [`tags-only==false`], showTreeView [`flat==true || tags-only==true`], group/ungroup by tag, group/ungroup by sub-tag [`has-sub-tags`]); `5-misc1` exportTree; `6-misc2` reveal. Command palette: the four folder/file context commands are hidden (`when: false`); everything else declared is visible.

---

## 5. Highlighting and decorations

### 5.1 Trigger paths (extension.js:1183-1220, :1697-1728, :1916; highlights.js:367-379)

`highlights.triggerHighlight(editor)` is called for: each visible editor showing a changed document (every change event), the newly active editor, all visible editors after a `documentChanged()` with no argument (highlights/tagGroups/filtering/regex/ripgrep/tree/rootFolder/tags/files.exclude changes), and the active editor at startup. Gating in extension.js: scheme in `general.schemes` **and** `isIncluded(uri)` (excluded files are never highlighted). `triggerHighlight` debounces per `editorId` (`JSON.stringify(document.uri) + viewColumn`) with `highlights.highlightDelay` ms. `init()` pushes the `decorations` map into `context.subscriptions` (not disposable; harmless).

### 5.2 The highlight pass (`highlight(editor)`, highlights.js:215-365) [verified in source]

1. Dispose every decoration type previously created for this editor id; `decorations[id] = []` (happens even when highlighting is disabled, so toggling `highlights.enabled` off clears highlights on the next trigger).
2. If `highlights.enabled` (default true): `text = getText()`, `regex = getRegexForEditorSearch(true)`, `subTagRegex = new RegExp(config.subTagRegex())` (no flags).
3. Per match: `extracted = extractTag(match[0])`; if a tag was found, `updateBeforeAndAfter(extracted, text from match start to end of the match's first line)`; `tag = tagGroup(extracted.tag) || extracted.tag` (the **group name** is the decoration key and attribute name); `offsetStart = match.index + tagOffset`, `offsetEnd = +tag length`; if no tag, `offsetStart` moves to the first non-space char of the match and the key is the whole match text. `type = getType(tag)`; `none` skips.
4. Range per type:

| `type` value | Range |
|---|---|
| (default: `undefined`, via undeclared `highlights.highlight`, §0 #16) or any unknown string, or `tag` | tag only (`startPos`..`endPos`) |
| `text` | tag start .. end of the line where the whole match ends |
| `tag-and-comment` | match start (comment leader) .. tag end |
| `text-and-comment` | match start .. end of the line where the match ends |
| `tag-and-subTag` / `tag-and-subtag` | tag only, **plus** a separate decoration for the sub-tag (text after the tag to end of line, `subTagRegex` capture 1, only if `customHighlight[subTag]` exists) keyed by the sub-tag string |
| `line` / `whole-line` | from end of the match's last line to column 0 of the tag's line (reversed range; decoration type has `isWholeLine: true` only for `whole-line`) |
| `capture-groups:n,m,...` | each listed regex group's `match.indices[group]` range (needs the `regexp-match-indices` shim) |
| `none` | nothing |

5. One `getDecoration(key)` per distinct key (tags and sub-tags), pushed to `decorations[id]`, `editor.setDecorations`.

Hazards: empty-match infinite loop (§3.9); for multi-line matches `updateBeforeAndAfter` only sees the first line; every pass creates fresh `TextEditorDecorationType`s (dispose + create per tag per editor per pass); per match `extractTag` compiles 2 RegExps, `updateBeforeAndAfter` 2 more, `getType` one per `customHighlight` key.

### 5.3 Attribute resolution (`attributes.getAttribute(tag, attr, default, ignoreDefaultHighlight)`, attributes.js:8-47) [verified in source]

Iterates `Object.keys(customHighlight)`; each key is regex-escaped and compiled **unanchored** (flag `i` when `regexCaseSensitive === false`); on `tag.match(regex)` sets `result = customHighlight[tag]` (indexed by the **tag**, not the key). Net effect: only an exact property name resolves; substring or case-variant keys never do (§0 #11). Then `tagSettings[attr]` if defined; else, unless `ignoreDefaultHighlight === true`, `defaultHighlight[attr]`; else the default. `ignoreDefaultHighlight` is passed as `useColourScheme` for `foreground`, `background`, `iconColour` only. No caching; one RegExp per key per call; called ~10× per `getDecoration`, per node on add (hideFromTree), per counted node (hideFromStatusBar/ActivityBar), 3-5× per `icons.getIcon`.

### 5.4 Per-tag attributes (customHighlight / defaultHighlight)

| Attribute | Default | Consumer |
|---|---|---|
| `type` | `undefined` → tag-only (§5.2) | highlights.js:196-199 |
| `foreground`, `background` | scheme colour when `useColourScheme` (by `config.tags()` index, `-1` for groups → undefined), else undefined | attributes.js:88-108 |
| `opacity` | 100 | `applyOpacity` (hex → always `rgba(...)`; `<1` treated as a fraction ×100 for hex; rgb changed only when `!== 100`; 4/8-digit hex alpha **overrides** opacity) |
| `rulerColour` | processed dark background, else `editor.foreground` | highlights.js:129-138 |
| `rulerOpacity` | 100 | only effective for hex/rgb ruler colours |
| `rulerLane` | 4 (right); strings `none`→no marker, `left`=1, `center`=2, `right`=4, `full`=7; unknown string → no marker | highlights.js:11-18, :114-117 |
| `borderRadius` | `0.2em` | decoration option |
| `fontStyle`, `fontWeight` | `normal` | |
| `textDecoration` | `''` | |
| `gutterIcon` | false → `gutterIconPath = icons.getIcon(...).dark` (codicons return a `ThemeIcon` without `.dark` → no gutter icon) | highlights.js:124 |
| `icon` | undefined (defaults in `customHighlight` for BUG/HACK/FIXME/XXX/[ ]/[x]) | icons.js |
| `iconColour`, `iconColor` (US spelling, undeclared, checked first) | see §5.6 | attributes.js:54-77 |
| `hideFromTree`, `hideFromStatusBar`, `hideFromActivityBar` | false | tree.js:869-872, :397-405 |

Colour handling in `getDecoration` (highlights.js:49-149): a string matching `/(foreground|background)/i` → `new ThemeColor(string)`; otherwise if not `isValidColour` → `ThemeColor('editor.foreground')`/`('editor.background')`; otherwise passed through (theme ids without those words reach the decoration as invalid CSS and do nothing). Background then `applyOpacity`. Complementary foreground (`utils.complementaryColour`, luminance 0.179) is **unreachable** (hex was already converted to rgba). Both undefined → `backgroundColor = ThemeColor('editor.foreground')`, `color = ThemeColor('editor.background')` (inverted). Light and dark get identical values.

### 5.5 Colour scheme (attributes.js:79-108; config.js:221-234)

`useColourScheme` (default false): `foregroundColourScheme` default `[white,black,black,white,white,white,black]`, `backgroundColourScheme` default `[red,orange,yellow,green,blue,indigo,violet]`, indexed by `config.tags().indexOf(tag) % length`; index `-1` (tag groups, tags only in customHighlight) → `undefined`. README.md:317 "overrides defaultHighlight but not customHighlight" holds for foreground/background/iconColour only.

### 5.6 Icons (`icons.getIcon(context, tag, debug)`, icons.js:12-120) [verified in source]

`colour = attributes.getIconColour(tag)` (order: `iconColor`, `iconColour`, scheme background when scheme on, foreground, background, literal `green`). `iconName = attributes.getIcon(tag)`. Creates `globalStorageUri` if missing. Cases: `todo-tree` / `todo-tree-filled` → an inline SVG written to `<globalStorage>/<name>-<colour>.svg` (theme colour → `green`); codicon `$(name)` → `new ThemeIcon(name, ThemeColor(colour) if colour is a theme id)`; other name → octicon (unknown → `check`), SVG via `octicons[name].toSVG({fill: colour})` written to `<globalStorage>/todo-<name>-<colour>.svg`; no icon name and a valid CSS colour → built-in check-circle SVG `todo-<colour>.svg`; otherwise bundled `resources/icons/{dark,light}/todo-green.svg`. Returns `{dark, light}` paths (or a `ThemeIcon`). Synchronous `existsSync`/`writeFileSync` per call, per rendered item.

### 5.7 Line flash on reveal (extension.js:1642-1673)

`todo-tree.revealInFile(uri, selection)` → `vscode.open` then `flashLine`: whole-line `TextEditorDecorationType` with `editor.rangeHighlightBackground` on the active editor's current line, cleared after 150 ms; the type is never disposed (one leak per click); flashes whatever editor is active after open.

---

## 6. Filtering (user-facing layers)

### 6.1 Layers in evaluation order

1. Workspace roots: `includedWorkspaces` / `excludedWorkspaces` (micromatch on folder fsPath), `rootFolder`.
2. ripgrep-side: `.gitignore`/`.ignore`/hidden/binary defaults, `-g` globs (config + temp + built-in excludes + submodules) when `passGlobsToRipgrep`, `--hidden` when `includeHiddenFiles`.
3. Client-side `applyGlobs` (config + temp globs only; always).
4. Per-document `isIncluded` (globs + built-in excludes + hidden rule) and `general.schemes` for editor scans and highlights.
5. Scan mode (§2.1).
6. `hideFromTree` (node stays in the structure but invisible; containers with only hidden children still appear at root; `filter()` then hides them).
7. Tree text filter (`todo-tree.filter`): regex over TODO labels, persisted as `currentFilter`/`filtered`, re-applied on every `addResultsToTree`; cancelling the input box leaves `filtered` true until `filterClear`.
8. Counts: `hideFromStatusBar` (status bar) / `hideFromActivityBar` (badge and in-tree counts).

### 6.2 Filter commands (extension.js:1311-1479, :1512-1519)

| Command | Effect |
|---|---|
| `showOnlyThisFolder(node)` | workspaceState `includeGlobs = [createFolderGlob(node.fsPath, workspaceRoot, '/*')]` (replaces previous temp includes); `rebuild()` |
| `showOnlyThisFolderAndSubfolders(node)` | same with `/**/*` |
| `excludeThisFolder(node)` | append `createFolderGlob(..., '/**/*')` to temp excludes if absent; `rebuild()` |
| `excludeThisFile(node)` | append the raw absolute `node.fsPath`; `rebuild()` |
| `switchScope` | `filtering.scopes` (`[{name, includeGlobs, excludeGlobs}]`; string or array via `toGlobArray`, comma-split); none → warning with Open Settings (writes `[]` at Global then opens settings JSON) / OK; quick pick marks the scope whose JSON-serialised glob arrays equal the current temp lists with `$(check)`; selection **replaces** both temp lists; first scope with that name wins; `rebuild()` |
| `removeFilter` | multi-select quick pick: `Clear Tree Filter` (if a text filter is set), `Exclude Folder: <glob minus /**/*>`, `Exclude File: <path>` (no `*`), `Exclude: <glob>`, `Include Folder and Subfolders: ...`, `Include Folder: ...` (minus `/*`), `Include: ...`; label collisions overwrite; removes exact strings; `rebuild()` even for an empty selection |
| `resetAllFilters` | temp includes/excludes `= []`; `rebuild()`; `clearTreeFilter()` (submodule globs untouched) |
| `filter` / `filterClear` | input box "Filter tree"; sets/clears `currentFilter`, `filtered`, `provider.filter`, `refreshTree()` |

---

## 7. Commands

Category (all): `Todo Tree`. "Palette" = visible in the command palette. Registration: extension.js:1267-1695 (all inside `register()`, after the rg gate).

| Command id | Title (package.nls) | Behaviour | Exposed |
|---|---|---|---|
| `todo-tree.showFlatView` | Show Flat View | workspaceState `tagsOnly=false`, `flat=true` → `refresh()` | palette, view title (`$(list-unordered)`), item context |
| `todo-tree.showTagsOnlyView` | Show Tags Only View | `flat=false`, `tagsOnly=true` → `refresh()` | palette, view title (`$(list-flat)`), item context |
| `todo-tree.showTreeView` | Show Tree View | `tagsOnly=false`, `flat=false` → `refresh()` | palette, view title (`$(list-tree)`), item context |
| `todo-tree.refresh` | Refresh | `rebuild()` (no debounce) | palette, view title (`$(refresh)`), status bar after stopScan |
| `todo-tree.expand` | Expand Tree | workspaceState `expanded=true` → `clearExpansionState()` + `refresh()` | palette, view title (`$(expand-all)`), item context |
| `todo-tree.collapse` | Collapse Tree | `expanded=false` → same | palette, view title (`$(collapse-all)`), item context |
| `todo-tree.filter` | Filter Tree | input box → regex filter (§6.2) | palette, view title (`$(filter)`), item context |
| `todo-tree.filterClear` | Clear Tree Filter | clear text filter | palette, view title (`$(clear-all)`), item context |
| `todo-tree.groupByTag` / `ungroupByTag` | Group by Tag / Ungroup by Tag | workspaceState `groupedByTag` → `refresh()` | palette, view title (`$(tag)` / `$(unfold)`), item context |
| `todo-tree.groupBySubTag` / `ungroupBySubTag` | Group by Sub Tag / Ungroup by Sub Tag | workspaceState `groupedBySubTag` → `refresh()` | palette, view title (`$(chrome-restore)` / `$(chrome-maximize)`, needs `has-sub-tags`), item context |
| `todo-tree.scanOpenFilesOnly` | Scan Open Files Only | write `tree.scanMode='open files'` at **Workspace** target (fails in a no-folder window) → config handler → `rebuild()` | palette, view title (`$(files)`), item context |
| `todo-tree.scanCurrentFileOnly` | Scan Current File Only | `'current file'` | palette, view title (`$(symbol-file)`), item context |
| `todo-tree.scanWorkspaceAndOpenFiles` | Scan Workspace And Open Files | `'workspace'` | palette, view title (`$(folder-active)`), item context |
| `todo-tree.scanWorkspaceOnly` | Scan Workspace Only | `'workspace only'` | palette, view title (`$(folder)`), item context |
| `todo-tree.addTag` | Add Tag | input box "New tag" (placeholder `e.g. FIXME`) → append to `general.tags` at Global if not present (exact match) | palette |
| `todo-tree.removeTag` | Remove Tag | multi-select quick pick "Select tags to remove" → write Global | palette |
| `todo-tree.exportTree` | Export Tree | open virtual document `todotree-export:<formatted exportPath>` (§10) | palette, view title (`$(save)`), item context |
| `todo-tree.showOnlyThisFolder` | Only Show This Folder | §6.2 | item context (`viewItem == folder`); hidden from palette |
| `todo-tree.showOnlyThisFolderAndSubfolders` | Only Show This Folder And Subfolders | §6.2 | item context (folder); hidden from palette |
| `todo-tree.switchScope` | Switch Scope | §6.2 | palette |
| `todo-tree.excludeThisFolder` | Hide This Folder | §6.2 | item context (folder); hidden from palette |
| `todo-tree.excludeThisFile` | Hide This File | §6.2 | item context (`viewItem == file`); hidden from palette |
| `todo-tree.removeFilter` | Remove Filter | §6.2 | palette, item context (`folder-filter-active`) |
| `todo-tree.resetAllFilters` | Reset All Filters | §6.2 | palette, item context (`folder-filter-active`) |
| `todo-tree.reveal` | Reveal Current File In Tree | `provider.getElement(activeEditor fsPath, el => treeView.reveal(el, {focus:false, select:true}))` only if the view is visible | palette, view title (`$(location)`, hidden when `trackFile`), item context |
| `todo-tree.resetCache` | Reset Cache | §1.7; no refresh | palette |
| `todo-tree.toggleItemCounts` | Toggle Item Counts | flip `tree.showCountsInTree` at Workspace | palette, item context |
| `todo-tree.toggleBadges` | Toggle Badges | flip `tree.showBadges` at Workspace | palette, item context |
| `todo-tree.toggleCompactFolders` | Toggle Compact Folders | flip `tree.disableCompactFolders` at Workspace | palette, item context (`can-toggle-compact-folders`) |
| `todo-tree.goToNext` | Go To Next | §3.7 | palette |
| `todo-tree.goToPrevious` | Go To Previous | §3.7 | palette |
| `todo-tree.revealInFile` | Reveal In File | `vscode.open(uri, {selection})` + 150 ms line flash | tree item click; palette (useless without args) |
| `todo-tree.openUrl` (registration only) | — | `env.openExternal(Uri.parse(url))` | sub-tag node click |
| `todo-tree.stopScan` (registration only) | — | `ripgrep.kill()` (no-op), status bar "Todo-Tree: Scanning interrupted." / "Click to restart" / command `todo-tree.refresh`, `interrupted = true` (skips `updateInformation` until next rebuild) | status bar item during a scan |
| `todo-tree.onStatusBarClicked` (registration only) | — | §9 | status bar item |

Built-in commands executed: `setContext` (22 keys), `todo-tree-view.focus`, `vscode.moveViews`, `workbench.action.openSettingsJson`, `workbench.action.openSettings`, `vscode.open`.

**Totals: 37 extension commands (34 declared + 3 registration-only) and 6 built-in commands.**

---

## 8. Settings

### 8.1 Declared settings (package.json `contributes.configuration`, 70 keys) [verified in source]

Columns: key (prefix `todo-tree.`), type, default, semantics, read by, effect of a change (per §1.12; "rebuild" = `rebuild()+documentChanged()`, "refresh" = `refresh()`; `setButtonsAndContext()` always runs).

| Key | Type | Default | Semantics | Read by | On change |
|---|---|---|---|---|---|
| `general.debug` | boolean | `false` | create the "Todo Tree" output channel | extension.js:112-123, ripgrep.js:74-81 | resetOutputChannel + refresh |
| `general.automaticGitRefreshInterval` | integer (seconds) | `0` | poll `git rev-parse HEAD` per workspace folder; rescan on change; 0 = off | extension.js:620-658 | resetGitWatcher + refresh |
| `general.periodicRefreshInterval` | integer (minutes) | `0` | periodic `triggerRescan()`; 0 = off | extension.js:660-678 | resetPeriodicRefresh + refresh |
| `general.revealBehaviour` | enum `start of line` / `start of todo` / `end of todo` | `"start of todo"` | cursor placement on item click; other values → no selection | tree.js:707-724; migration notice :1119-1136 | refresh |
| `general.exportPath` | string | `"~/todo-tree-%Y%m%d-%H%M.txt"` | `~` expanded, `${ENV}` replaced, strftime; `.json` → JSON else ASCII tree | extension.js:1298-1309, utils.js:454-482 | refresh |
| `general.rootFolder` | string | `""` | search root override; `${workspaceFolder}` expanded per folder; env vars **not** expanded (bug); scanned in every scan mode | extension.js:527-571 | rebuild |
| `general.schemes` | array | `["file","ssh","untitled","vscode-notebook-cell"]` | `uri.scheme` allow-list for editor scans, highlights, tracking | config.js:146-150 | refresh |
| `general.statusBar` | enum `none` / `total` / `tags` / `top three` / `current file` | `"none"` | status bar content (§9) | extension.js:153-279 | refresh |
| `general.showIconsInsteadOfTagsInStatusBar` | boolean | `false` | codicon/icon names instead of tag names | extension.js:214-247 | refresh |
| `general.statusBarClickBehaviour` | enum `cycle` / `reveal` / `toggle highlights` | `"reveal"` | §9 | config.js:136-144 | refresh |
| `general.tagGroups` | object `{group:[tags]}` | `{}` | tag → group mapping (§3.4) | config.js:169-183 | refreshTagGroupLookup + rebuild |
| `general.tags` | array | `["BUG","HACK","FIXME","TODO","XXX","[ ]","[x]"]` | tag list; empty → `["TODO"]`; order = colour-scheme index, status-bar order, tag-node sort order | config.js:115-119, utils.js:183-194 | rebuild |
| `general.showActivityBarBadge` | boolean | `false` | badge = workspace total (else 0) | extension.js:160-166 | updateInformation |
| `highlights.customHighlight` | object | `{"BUG":{"icon":"bug"},"HACK":{"icon":"tools"},"FIXME":{"icon":"flame"},"XXX":{"icon":"x"},"[ ]":{"icon":"issue-draft"},"[x]":{"icon":"issue-closed"}}` | per-tag attributes (§5.4); exact key match only | attributes.js:8-47 | validate colours/icons + documentChanged + refresh |
| `highlights.defaultHighlight` | object | `{}` | fallback attributes (also for sub-tags) | attributes.js:38-45 | same |
| `highlights.enabled` | boolean | `true` | master switch for editor decorations | highlights.js:245 | same |
| `highlights.highlightDelay` | integer (ms) | `500` | per-editor highlight debounce | highlights.js:377 | refresh |
| `highlights.useColourScheme` | boolean | `false` | colour tags by `general.tags` index; makes `defaultHighlight` fg/bg/iconColour ignored | attributes.js:56-108 | validate + documentChanged + refresh |
| `highlights.foregroundColourScheme` | array | `["white","black","black","white","white","white","black"]` | cycled by tag index | attributes.js:88-97 | same |
| `highlights.backgroundColourScheme` | array | `["red","orange","yellow","green","blue","indigo","violet"]` | cycled by tag index; also icon colour | attributes.js:63-66, :99-108 | same |
| `filtering.excludedWorkspaces` | array (globs) | `[]` | exclude workspace folders (matched on fsPath) | extension.js:459-466, :558-566 | rebuild |
| `filtering.excludeGlobs` | array | `["**/node_modules/*/**"]` | exclude globs (rg `-g !`, `applyGlobs`, `isIncluded`) | extension.js:366-390, :481-501, :747-775 | rebuild |
| `filtering.ignoreGitSubmodules` | boolean | `false` | exclude folders containing `.git` (synchronous full walk per rebuild) | extension.js:596-606, utils.js:435-447 | rebuild |
| `filtering.includedWorkspaces` | array (globs) | `[]` | include-list of workspace folders; empty = all | as excludedWorkspaces | rebuild |
| `filtering.includeGlobs` | array | `[]` | include globs | as excludeGlobs | rebuild |
| `filtering.includeHiddenFiles` | boolean | `false` | rg `--hidden`; allow dot-files without extension in editor scans | extension.js:439-442, :763-773 | rebuild |
| `filtering.passGlobsToRipgrep` | boolean | `true` | pass globs as rg `-g` (client-side filter runs regardless) | extension.js:406 | rebuild |
| `filtering.scopes` | array of `{name, includeGlobs, excludeGlobs}` | `[]` | named temp-glob presets | extension.js:1329-1379 | rebuild |
| `filtering.useBuiltInExcludes` | enum `none` / `file excludes` / `search excludes` / `file and search excludes` | `"none"` | add `files.exclude` / `search.exclude` true-keys; **`file excludes` is dead** (§0 #15) | config.js:152-162 | rebuild |
| `tree.autoRefresh` | boolean | `true` | enable editor scans on open/save/change and file tracking | extension.js:1255-1258, :1709 | rebuild |
| `tree.disableCompactFolders` | boolean | `false` | disable compaction even when `explorer.compactFolders` | config.js:185-189 | rebuild |
| `tree.expanded` | boolean | `false` | default node state (workspaceState `expanded` overrides) | config.js:27-30 | rebuild |
| `tree.filterCaseSensitive` | boolean | `false` | `i` flag for the tree filter regex | tree.js:803 | rebuild |
| `tree.flat` | boolean | `false` | flat view default (workspaceState `flat` overrides) | config.js:32-35 | rebuild |
| `tree.groupedByTag` | boolean | `false` | (workspaceState overrides) | config.js:17-20 | rebuild |
| `tree.groupedBySubTag` | boolean | `false` | (workspaceState overrides) | config.js:22-25 | rebuild |
| `tree.hideIconsWhenGroupedByTag` | boolean | `false` | no TODO icons when any grouping is active | tree.js:689 | rebuild |
| `tree.hideTreeWhenEmpty` | boolean | `false` | sets `todo-tree-is-empty` → view hidden | extension.js:729-745 | rebuild |
| `tree.labelFormat` | string | `"${tag} ${after}"` | placeholders `line, column, tag[:uppercase/:lowercase/:capitalize], subtag[...], before, after, afterorbefore, filename, filepath` (case-insensitive); `''` → raw label | tree.js:701-705, utils.js:371-415 | validatePlaceholders + rebuild |
| `tree.scanAtStartup` | boolean | `true` | run the initial scan at activation | extension.js:1899-1922 | rebuild (value only used at startup) |
| `tree.scanMode` | enum `workspace` / `open files` / `current file` / `workspace only` | `"workspace"` | §2.1 | config.js:211-214 and extension.js throughout | rebuild |
| `tree.showBadges` | boolean | `true` | `resourceUri` on path nodes (SCM/problem decorations) | tree.js:605-608 | refresh |
| `tree.showCountsInTree` | boolean | `false` | per-node counts and ` (N)` in the title | tree.js:743-749, extension.js:190-198 | refresh |
| `tree.showInExplorer` | boolean | `false` | **deprecated**; `<189` migration moves the view | extension.js:1097-1118 | rebuild |
| `tree.showCurrentScanMode` | boolean | `true` | "Scan mode: ..." status node | tree.js:524-535 | rebuild |
| `tree.showScanOpenFilesOrWorkspaceButton` | boolean | `false` | **deprecated**; not read | — | rebuild |
| `tree.subTagClickUrl` | string | `""` | URL (with placeholders) opened when a sub-tag node is clicked | tree.js:665-680 | rebuild |
| `tree.showTagsFromOpenFilesOnly` | boolean | `false` | **deprecated**; not read | — | rebuild |
| `tree.sortTagsOnlyViewAlphabetically` | boolean | `false` | tags-only sort by label instead of tag order | tree.js:1188-1195 | rebuild |
| `tree.sort` | boolean | `true` | disable all tree sorting | tree.js:1170-1202 | rebuild |
| `tree.tagsOnly` | boolean | `false` | tags-only view default (workspaceState overrides) | config.js:37-40 | rebuild |
| `tree.tooltipFormat` | string | `"${filepath}, line ${line}"` | TODO tooltip (same placeholders) | tree.js:612-613 | rebuild |
| `tree.trackFile` | boolean | `true` | reveal the active file in the tree 500 ms after activation; hides the reveal button | extension.js:1709-1719, :701 | rebuild |
| `tree.buttons.reveal` | boolean | `true` | title button (also needs `!trackFile`) | extension.js:680-727 | rebuild |
| `tree.buttons.scanMode` | boolean | `false` | title button | same | rebuild |
| `tree.buttons.viewStyle` | boolean | `true` | title button | same | rebuild |
| `tree.buttons.groupByTag` | boolean | `true` | title button | same | rebuild |
| `tree.buttons.groupBySubTag` | boolean | `false` | title button (also needs sub-tags) | same | rebuild |
| `tree.buttons.filter` | boolean | `true` | title button | same | rebuild |
| `tree.buttons.refresh` | boolean | `true` | title button | same | rebuild |
| `tree.buttons.expand` | boolean | `true` | title button | same | rebuild |
| `tree.buttons.export` | boolean | `false` | title button | same | rebuild |
| `regex.regex` | string (`scope: language-overridable`, never honoured, §0 #19) | `"(//|#|<!--|;|/\\*|^|^[ \\t]*(-|\\d+.))\\s*($TAGS)"` | the match regex; `($TAGS)` expanded | utils.js:310-339 | **nothing** (§1.12 step 0) |
| `regex.regexCaseSensitive` | boolean | `true` | rg `-i` / JS `i`; tag-node lookup; attribute key regex | config.js:57-60 | rebuild |
| `regex.subTagRegex` | string | `""` | sub-tag extraction (§3.3) | utils.js:210-221, highlights.js:249 | rebuild |
| `regex.enableMultiLine` | boolean | `false` | rg `-U`; JS `s` flag; multiline result grouping | extension.js:431, utils.js:332 | rebuild |
| `ripgrep.ripgrep` | string | `""` | explicit rg path | config.js:97 | rebuild |
| `ripgrep.ripgrepArgs` | string | `"--max-columns=1000 --no-config "` | extra rg args (`null` → literal `undefined` in the command) | ripgrep.js:117 | rebuild |
| `ripgrep.ripgrepMaxBuffer` | integer (KB) | `200` | `exec` maxBuffer; overflow = silent truncation | ripgrep.js:166 | rebuild |
| `ripgrep.usePatternFile` | boolean | `true` | `-f <tempfile>` instead of `-e "<regex>"` | extension.js:433-437, ripgrep.js:123-137 | rebuild |

### 8.2 Keys read by the code but not declared

| Key | Where | Effect |
|---|---|---|
| `todo-tree.ripgrep.passGlobsToRipgrep` | extension.js:511 | always `undefined` → `applyGlobs()` always runs |
| `todo-tree.highlights.highlight` | highlights.js:198 | default for `type` → `undefined` → tag-only |
| `todo-tree.highlights.schemes` | extension.js:1159-1160 | legacy; when present, copied to `general.schemes` every activation |
| `todo-tree.general.enableFileWatcher` | extension.js:1138 | only the `<223` deprecation notice |
| `todo-tree.tree.grouped` | written by migration (:1029-1094) | never read |
| `customHighlight.<tag>.iconColor` / `defaultHighlight.iconColor` | attributes.js:58 | US-spelling icon colour, checked before `iconColour` |

### 8.3 VS Code settings read

`files.exclude` (true-keys as excludes when enabled; change → rebuild), `search.exclude` (same; changes not observed), `explorer.compactFolders` (compaction + `can-toggle-compact-folders`; change → refresh).

### 8.4 Highlight attribute sub-keys (18)

`type, foreground, background, opacity, rulerColour, rulerOpacity, rulerLane, borderRadius, fontStyle, fontWeight, textDecoration, gutterIcon, icon, iconColour, iconColor, hideFromTree, hideFromStatusBar, hideFromActivityBar` — semantics in §5.4.

### 8.5 Legacy flat keys (32) — see §1.8.

**Totals: 70 declared settings (3 deprecated), 6 undeclared keys read/written, 3 external VS Code settings, 18 highlight attribute sub-keys, 32 legacy flat keys.**

---

## 9. Status bar

Item: left, priority 0, always `command = todo-tree.onStatusBarClicked` after `updateInformation()`; during a scan text `Todo-Tree: Scanning...` / command `todo-tree.stopScan`; after stopScan `Todo-Tree: Scanning interrupted.` / `todo-tree.refresh` (persists until the next rebuild because `updateInformation` is skipped while `interrupted`).

`updateInformation()` (extension.js:153-279): `counts = provider.getTagCountsForActivityBar()` (visible TODOs, minus `hideFromActivityBar`; extra lines not counted); badge `= showActivityBarBadge ? total : 0` (workspace-wide even in `current file` mode). If `general.statusBar === 'current file'`: `counts = provider.getTagCountsForStatusBar(activeEditor.fileName)` (minus `hideFromStatusBar`) and `total` is recomputed — this total also feeds the view title. Then:

| `general.statusBar` | Text | Tooltip |
|---|---|---|
| `total` | `$(check) <total>` | `Todo-Tree total` |
| `tags` / `current file` | tags in **`config.tags()` order** (tags not in the list are omitted), each with count > 0: `TAG: n ` or, with `showIconsInsteadOfTagsInStatusBar` and icon ≠ `defaultHighlight.icon` (loose `!=`), `$(icon) n  `; final `= showIcons ? text.trim() : '$(check) ' + text.trim()`; `$(check) 0` when `counts` has no keys | `Todo-Tree tags counts` / `Todo-Tree tags counts in current file` |
| `top three` | keys sorted by count desc, ties by name asc, first 3, same formatting | `Todo-Tree top three tag counts` |
| anything else (`none`) | `hide()` | |

Suffix ` (in open files)` / ` (in current file)` per scan mode. Runs on every `addResultsToTree` (unless interrupted), active-editor change (when included), close-document removal, and `showActivityBarBadge` change; overwrites "Scanning..." if a `refreshFile` happens mid-scan.

Click (`todo-tree.onStatusBarClicked`, :281-321): `reveal` → `todo-tree-view.focus` if the view is not visible; `toggle highlights` → flip `highlights.enabled` at `settingLocation('highlights.enabled')`; `cycle` → `total`→`tags`→`top three`→`current file`→`total` written at **Global** (a workspace-level value keeps winning, making the click appear inert) with info messages `Todo Tree: Now showing tag counts` / `...top three tag counts` / `...total tags in current file` / `...total tags`.

---

## 10. Export

`todo-tree.exportTree` (extension.js:1298-1309): `exportPath = formatExportPath(replaceEnvironmentVariables(general.exportPath), new Date())` (`${ENV}` → env value or `''`; `~` → home; `strftime`); `uri = Uri.parse('todotree-export:' + exportPath)`; `openTextDocument(uri)` then `showTextDocument(doc, {preview:true})`. The `todotree-export` content provider (:99-108) returns `JSON.stringify(provider.exportTree(), null, 2)` when `path.extname(uri.path) === '.json'`, else `treeify.asTree(provider.exportTree(), true)`. Nothing is written to disk; the user saves the read-only virtual document (README notes File: Save does not work, only Save As). Content is generated at open time from the current tree (§4.6 for the shape). Windows paths through `Uri.parse` may parse oddly (drive letter as authority) — untested.

---

## 11. Multi-root and remote behaviour

### 11.1 Multi-root

- One ripgrep process per workspace folder (or per `${workspaceFolder}` expansion of `rootFolder`), strictly sequential, results added after all finish.
- One workspace root node per folder in `workspaceFolders` order (suppressed in tags-only view); files are attached to the **last** matching root by prefix (nested folders: the later-listed one wins regardless of depth) — §4.2.
- `includedWorkspaces` / `excludedWorkspaces` filter roots by fsPath glob; non-`file` scheme folders are never scanned.
- Git polling runs per workspace folder; `onDidChangeWorkspaceFolders` → immediate full rebuild.
- Scan-mode commands and the three toggles write at `ConfigurationTarget.Workspace` (rejected in a no-folder window); `addTag`/`removeTag`/status-bar cycle write Global.
- Close-document retention in `workspace` mode uses root prefix tests against `getRootFolders()`/`searchWorkspaces()`.
- Expansion state is keyed by fsPath, so identical paths under different tag groups share state.
- Files outside every folder (e.g. via `rootFolder`) go to root-level flat nodes with an absolute `pathLabel`.

### 11.2 Remote (Remote-SSH / WSL / Dev Containers / Codespaces)

- No `extensionKind` → runs as a **workspace extension on the remote**, so `vscode.env.appRoot`, `process.platform`, `child_process`, `fs` and `context.storageUri` all refer to the remote server; rg is resolved from the remote server's `appRoot` (the server bundles `@vscode/ripgrep` too; issue #939 reports the 1.122 breakage on Remote-SSH servers as well).
- Workspace folders on the remote have scheme `file` there, so `searchWorkspaces` works; the `authority` code paths in tree.js (:124-126, :864, :962, :1012) only matter for non-file schemes such as virtual workspaces, where `locateWorkspaceNode` (`authority + fsPath`, no separator) and `add()` (`path.join(authority, fsPath)`) build different strings and may not match.
- Default `general.schemes` includes `ssh` (legacy), `untitled`, `vscode-notebook-cell`; notebook cells are matched per cell document (issues #556/#883: only the active cell after save).
- `showBadges` uses `Uri.file(fsPath)` for `resourceUri` regardless of scheme.
- Issue #377/#636 show remote-specific pathologies: rg spawned on the remote with default thread count; many processes when the (now removed) file watcher fired.
- **VS Code for the Web**: impossible for the current design (child processes, `fs`, native rg); only a wasm-based core could run there (§13).

---

## 12. Known performance pathologies

### 12.1 Code-level hot paths (from the inventories) [verified in source where marked]

| Path | Cost | Where |
|---|---|---|
| Per keystroke (any visible editor) | `documentChanged`: `isIncluded` (micromatch over globs) per visible editor showing the doc; `triggerHighlight` per such editor | extension.js:1183-1220 |
| Highlight pass (after `highlightDelay`) | `getText()` + global regex over the whole document; per match 5 RegExp compilations + `getType` (RegExp per customHighlight key) + `positionAt`/`lineAt`; dispose + create a `TextEditorDecorationType` per tag per pass | highlights.js:215-365 |
| `refreshFile` (after 500 ms) | `getText()` + global regex over the whole document; `provider.reset` walks the whole tree; `addResultsToTree` → `searchResults.addToTree` walks the **entire cache** (O(all results)) + `provider.filter(currentFilter)` (O(nodes)) + `refreshTree` | extension.js:777-861, :135-151 |
| `provider.refresh()` (200 ms later) | recursive `sort()` of every level with `config.tags()`/`getConfiguration` calls inside comparators; change event → VS Code re-requests `getChildren`/`getTreeItem` for visible nodes: per PATH node `countTags` over its subtree when counts are on, `icons.getIcon` with `fs.existsSync` per item, `getAttribute` loops; then `setButtonsAndContext` (22 `setContext` commands) + `hideTreeIfEmpty` | tree.js:792-799, :585-775; extension.js:680-745 |
| Full scan | one rg per root sequentially; stdout string concatenation; `Match` regex per line; `applyGlobs` micromatch over every result (always); `addToTree` with linear `find` per sibling level (O(n²) in tags-only ungrouped and for large files) | ripgrep.js, extension.js:481-525, tree.js:857-958 |
| Startup | migration writes (each re-entering the config handler → rebuild/refresh), validation, `rebuild()`, `refreshOpenFiles` **N times for N visible editors**, then again after rg | extension.js:1887-1922 |
| `ignoreGitSubmodules` | synchronous `find.fileSync('.git', root)` full-tree walk per root per rebuild | utils.js:435-447 |
| Git polling | shell `exec` per folder per tick, no in-flight guard | extension.js:620-658 |
| Debug channel on | full rg stdout + JSON per match into the output channel | ripgrep.js:172, extension.js:334 |
| Global RegExp shim | `RegExp.prototype.exec` replaced host-wide by a polyfill (correctness issue #853; performance impact on other extensions not measured) | highlights.js:2 |

### 12.2 Issue catalogue

| Issue | Symptom | Root cause (as established) | Status |
|---|---|---|---|
| [#882](https://github.com/Gruntfuggly/todo-tree/issues/882) (open, +8) | 50 000-line file: each change takes ~2 s; typing 15 chars → 30 s; Vim/Copilot/IntelliSense unusable | Not confirmed by maintainer. Code: every change → highlight pass + `refreshFile` (§12.1) over the full document on a single shared 500 ms timer; the fork's fix commit (4e35370) lists: one global open-file timer, re-parsing workspace-covered open files after scans, whole-tree rebuilds per change, stale highlight passes not suppressed on version change | unfixed upstream |
| [#733](https://github.com/Gruntfuggly/todo-tree/issues/733) (open, +8) | continuous re-scanning, typing lag, IntelliSense lag, another extension 2-3× slower | User isolated to `scanMode: workspace` (open-file rescans on edit); `workspace only`/`current file` avoid it | unfixed |
| [#865](https://github.com/Gruntfuggly/todo-tree/issues/865) (open, +9) | auto-reported unresponsive host (0.0.226, macOS arm64) | not identified; cross-linked to #887 | unfixed |
| [#887](https://github.com/Gruntfuggly/todo-tree/issues/887) (open) | pages of `git rev-parse HEAD` processes, 100 % CPU, after wake-from-sleep; only with ≥1 TODO; survives Reload Window | `checkGitHead` in `setInterval` with `exec` (shell), no single-flight guard (extension.js:620-658); workaround `automaticGitRefreshInterval: 0` (default). Feature added in 0.0.224 as the file-watcher replacement | unfixed; fork uses single-flight `execFile` |
| [#621](https://github.com/Gruntfuggly/todo-tree/issues/621) (open, +2) | "took 77-95 % of 0.8-4.2 s" every few seconds; 70 MB cpuprofiles; 40 s freezes; also with 0 TODOs | Maintainer: "builds a list of everything first then removes things" = `applyGlobs` post-filter (always on, :511) + flat cache scans + `addToTree` per pass; fork replaced flat storage with URI-indexed results and incremental per-document replacement | unfixed |
| [#495](https://github.com/Gruntfuggly/todo-tree/issues/495) (closed) | high CPU; excludeGlobs ignored by the watcher; pattern file per file | `enableFileWatcher` with default `fileWatcherGlob **/*` rescanned per change | fixed by removing the watcher (0.0.224) |
| [#636](https://github.com/Gruntfuggly/todo-tree/issues/636) (open) | rg.exe 10+ MB/s disk reads; many rg processes during rebase; zombie rg on Remote SSH | file watcher spawning one rg per changed file, unbounded | watcher removed |
| [#723](https://github.com/Gruntfuggly/todo-tree/issues/723) (closed, +28) | maintainer: watcher "constant source of problems" | default watch-all | removed in 0.0.224; replacement = git polling (#887) + periodic refresh |
| [#886](https://github.com/Gruntfuggly/todo-tree/issues/886) (open, +4) | after VS Code 1.99 everything slow; Codespaces org-wide slowdown | mainly VS Code 1.99.0/1 regression (vscode#245719) amplified by todo-tree | n/a |
| [#776](https://github.com/Gruntfuggly/todo-tree/issues/776) (open, +3) | activation 8.8 s; switching tabs between >1 MB PHP files → 5-7 s CPU before highlights | not identified; consistent with the active-editor path (`documentChanged` → highlight pass + 500 ms `refreshFile` of the new document even without edits) **[inference]** | unfixed |
| [#689](https://github.com/Gruntfuggly/todo-tree/issues/689) (open, +1) | lag with C# + todo-tree; correlates with SCM refresh; 30 MB `.7z` in root | not confirmed; requests: skip binaries, auto-exclude slow files, honour `.gitignore` for open files | unfixed |
| [#522](https://github.com/Gruntfuggly/todo-tree/issues/522) (open) | IntelliSense 2-3 s; activation 2 000+ ms | not identified; startup scan; mitigations `useBuiltInExcludes`, `scanAtStartup: false` | unfixed |
| [#478](https://github.com/Gruntfuggly/todo-tree/issues/478) (closed, +1) | huge Angular workspace, 30 s unresponsive at open | expected: full scan at startup | tunables only |
| [#433](https://github.com/Gruntfuggly/todo-tree/issues/433) (closed) | "took 100 % of 5161 ms"; one Next.js project (600 files × 700 lines, 3-4 TODOs) | not identified | closed |
| [#377](https://github.com/Gruntfuggly/todo-tree/issues/377) (closed, +1) | SSH remote: terminal freezes 10-15 s after saving; reporter asked for non-blocking, incremental refresh | maintainer suspected remote scan + rg threads; suggested `-j 1`; added `workspace only` mode and debug timestamps | closed unresolved |
| [#358](https://github.com/Gruntfuggly/todo-tree/issues/358) (closed) | host hangs seconds per scan with `ignoreGitSubmodules` | `find.fileSync('.git')` full walk (utils.js:435-447) | reporter later could not reproduce; code unchanged |
| [#279](https://github.com/Gruntfuggly/todo-tree/issues/279) (closed) | 14 s activation | synchronous startup scan | fixed in 0.0.197 (`onStartupFinished`); `scanAtStartup: false` → 14 ms |
| [#752](https://github.com/Gruntfuggly/todo-tree/issues/752) (closed) | +72 ms startup; git-repo scanning lag | migrate/validate/setButtons at startup; declined | closed |
| [#576](https://github.com/Gruntfuggly/todo-tree/issues/576) (open) | highlight flash on every tab switch even with `highlightDelay: 1` | active-editor change → `documentChanged` → `triggerHighlight` (new TextEditor has no decorations until the pass runs) and a 500 ms `refreshFile`; the throttle is applied to activation as well as edits | unfixed |
| [#643](https://github.com/Gruntfuggly/todo-tree/issues/643) (open, +3) | Prettier format-on-save much slower | not identified; save → immediate `refreshFile` + change events → highlight passes | unfixed |
| [#828](https://github.com/Gruntfuggly/todo-tree/issues/828) (open) | Search, fuzzy file search, TS IntelliSense freeze | not identified | unfixed |
| [#662](https://github.com/Gruntfuggly/todo-tree/issues/662) (closed) | typing pauses in C#/ASP.NET | not identified | closed |
| [#55](https://github.com/Gruntfuggly/todo-tree/issues/55) (closed) | `stdout maxBuffer exceeded`, empty tree | `exec` maxBuffer 200 KB | made configurable; now silent truncation (§2.5) |
| [#30](https://github.com/Gruntfuggly/todo-tree/issues/30) (closed) | CPU pinned scanning node_modules; hundreds of third-party TODOs | no default exclusions; open editors bypass `.gitignore` | `**/node_modules/*/**` default exclude added (0.0.221) |
| [#853](https://github.com/Gruntfuggly/todo-tree/issues/853) (open, +8) | other extensions throw `Invalid flags: dg/dsu/dis` from todo-tree's bundle | global `RegExp.prototype.exec` shim (highlights.js:2) | unfixed |
| [#925](https://github.com/Gruntfuggly/todo-tree/issues/925) (open, +65; also #928 #923 #929 #930 #926 #938 #939) | "Failed to find vscode-ripgrep" after VS Code 1.122 | hard-coded rg paths under `appRoot` (config.js:82-113); VS Code moved the binary (vscode#318691); PR #924 unmerged | unfixed; manual `ripgrep.ripgrep` path |

Top requested features absent from todo-tree (for the spec author's backlog): git-diff-scoped views (#800 +8, #288, #219); non-blocking/incremental/single-flight scanning (#882, #733, #621, #377, #576; implemented only in the better-todo-tree fork, #935 +19); `.gitignore`/binary/slow-file handling for open files (#689, #711, #642); LaTeX/Vim comment leaders in the default regex (#757 +6, #129 +21); per-tag regex/labels (#811, #871, #236); block-comment/multi-line TODOs (#763, #644, #902, #654, #255, #140); notebook support (#556, #883); @-mention grouping (#792, #762, #60, #313); file→tag nesting (#43); Live Share (#772); hide completed `[x]` (#889, #793); focus/toggle keybinding (#824); web support (#542); per-group totals (#831). Repository status: unmaintained since 2023-04-18 (#864).

---

## 13. Rust ↔ VS Code bridge options

Runtime facts driving the choice (fact-checked 2026-09-22): desktop extension host = Electron 43.7.3 (Node 24.21.0, NODE_MODULE_VERSION 148); remote server = plain Node 24.21.0 (NODE_MODULE_VERSION 137; glibc ≥ 2.28, x64/arm64/armhf/Alpine-musl); web host = browser worker (single file, no `require`, no child processes, JS + WebAssembly only). `extensionKind` `workspace` runs where the files are. `@vscode/vsce` 4.0.0 hard-codes exactly ten targets: `win32-x64, win32-arm64, linux-x64, linux-arm64, linux-armhf, alpine-x64, alpine-arm64, darwin-x64, darwin-arm64, web`; a VSIX packaged without `--target` is the fallback for platforms without a specific package; VSIX installation runs no scripts (so "postinstall download" means download-on-activate into `context.globalStorageUri`, with no VS Code proxy support for extensions).

| Option | How | Pros | Cons | Distribution | Key evidence |
|---|---|---|---|---|---|
| **1. napi-rs Node-API addon (`.node`) in-process** | `napi` 3.13.0 + `@napi-rs/cli` 3.10.5; `require()` the `.node`; sync FFI or async tasks / threadsafe functions | lowest overhead, shared Buffers, no protocol; **Node-API ABI stability** means one binary per OS/arch/libc loads in both the Electron desktop host and the Node remote server without electron-rebuild (V8/NAN addons would not); mature cross-compile (`--use-napi-cross`, zigbuild/xwin); Node 24 supports Node-API 10 (napi-rs default level 4) | no web host at all; must ship 9 native builds (musl for alpine, armv7 for armhf, glibc ≤ 2.28); a native crash kills the shared extension host; bundlers must treat `.node` as external; native debugging in the host unsupported (vscode#73008); npm `optionalDependencies` per platform do not help at VSIX install | (A) platform-specific VSIXs (`napi build --target <triple>` → `vsce package --target <t>`), as in microsoft/vscode-platform-specific-sample; (B) one universal VSIX with all `.node` files under `native/<platform>/` and runtime selection (neophack/vscode-git-graph-rs). Triple→target: x86_64-pc-windows-msvc→win32-x64, aarch64-pc-windows-msvc→win32-arm64, x86_64/aarch64-unknown-linux-gnu→linux-x64/arm64, armv7-unknown-linux-gnueabihf→linux-armhf, x86_64/aarch64-unknown-linux-musl→alpine-x64/arm64, x86_64/aarch64-apple-darwin→darwin-x64/arm64 | nodejs.org N-API docs; napi.rs support-compatibility, integrations (Electron), cli/build, cross-build, webassembly; electron/node-abi registry; microsoft/vscode `.npmrc` + `remote/.npmrc`; VS Code remote-extensions and linux docs; vscode-platform-specific-sample; vscode-git-graph-rs; coder/xum#4309 (community report of a Node-22-built Node-API addon loading in Electron 43) |
| **2. Separate Rust binary over stdio (LSP via `vscode-languageclient` 10.1.1 or custom JSON-RPC via `vscode-jsonrpc` 9.0.2)** | `Executable {command,args,options}` + `TransportKind.stdio`; client `cp.spawn`s and wires `StreamMessageReader/Writer`; Rust side `lsp-server`/`tower-lsp` or plain JSON-RPC | strongest isolation (crash/hang/OOM restartable); zero Node/Electron ABI coupling; identical under Remote (spawned on the remote); reusable CLI/other editors; mature, maintained tooling | JSON per call, no shared memory, streaming must be designed; **no web host**; process lifecycle/versions/PATH/Windows `shell` quirks; same 9-platform matrix plus VSIX executable-bit loss when packaging on Windows (vscode-vsce#512) | platform-specific VSIXs exactly as rust-analyzer's release workflow (binary in `server/`, `vsce package --target`, plus a `no-server` fallback VSIX with a `*.server.path` setting); or universal VSIX + download-on-activate into `globalStorageUri` (offline/proxy/allow-list caveats) | vscode-languageserver-node `client/src/node/main.ts`; LSP extension guide; vscode-jsonrpc README; rust-analyzer `release.yaml` and VS Code manual; publishing-extension docs; web-extensions guide; network docs; vscode-vsce#512 |
| **3a. `wasm32-wasip1` via the VS Code WASI toolkit** (`ms-vscode.wasm-wasi-core` 1.0.2 as `extensionDependencies`, `@vscode/wasm-wasi` 1.0.1, optional `@vscode/wasm-wasi-lsp`) | `Wasm.load()` → `createProcess(module, {stdio})` → WASI syscalls mapped to `workspace.fs` (works on remote/virtual FS); LSP over WASI stdio | one artifact for desktop, remote **and web**; no native matrix, no signing, no exec bits; sandboxed; std-using Rust compiles; repo still active (2026-09-11) | WASI preview1 + wasi-threads only (no sockets, no process spawn, 32-bit); slower, synchronous execution with `Atomics.wait` host-call overhead; `@vscode/wasm-wasi-lsp` only ever pre-release (0.1.0-pre.9, April 2024, peer on a `-next` languageclient); needs VS Code ≥ 1.88 and an extra dependency extension | one universal VSIX with the `.wasm`, loaded via `workspace.fs.readFile(Uri.joinPath(context.extensionUri, ...))`; `main` + `browser` entries cover all hosts | vscode-wasm `wasm-wasi-core` README/package.json, `wasm-wasi` README; npm registry versions; VS Code blogs 2023-06-05 and 2024-06-07; web-extensions guide |
| **3b. `wasm32-unknown-unknown` in-process** (component model: `wit-bindgen` 0.62 + `@vscode/wasm-component-model` 1.0.2 / `wit2ts`; or `wasm-bindgen` 0.2.128 glue) | compile a library, `WebAssembly.compile(bytes from workspace.fs)`, call typed exports; optionally inside a worker | direct typed calls, no process, one universal VSIX for all hosts; no dependency extension; sandboxed; ideal for pure computation (regex/tag extraction/tree building) | no FS/clock/threads unless imported from JS (VS Code APIs are async and hard to proxy); no pointers/object graphs across the boundary; long calls block the host unless moved to a worker; wasm-bindgen's `web` glue needs adapting to the single-file, no-`importScripts` worker | universal VSIX with `.wasm` + glue bundled; `browser` entry for web; consider `wasm-opt` for size | VS Code blogs 2024-05-08 and 2024-06-07; wasm-component-model README; crates.io/npm versions; wasm-bindgen deployment docs; vsce `package.ts` targets |
| **4. Hybrid: native (1 or 2) for desktop/remote + WASI build as the web/unknown-platform fallback** | same `#[napi]` crate built with `napi build --target wasm32-wasip1-threads` (generated Node + browser loaders; `NAPI_RS_FORCE_WASI` for tests); or binary + wasm-wasi-core | one codebase, native speed where available, universal reach; graceful degradation instead of "unsupported platform" | two runtime paths to test (napi-rs: "not a direct substitute for a native addon"); browser WASI + threads needs cross-origin isolation and custom bundling; larger VSIX or more CI | nine platform VSIXs plus a fallback VSIX (no `--target`, or `--target web`) carrying only the wasm; or one universal VSIX with runtime selection | napi.rs webassembly, cross-build, release docs; publishing-extension fallback semantics; rust-analyzer no-server pattern |

**Implications specific to todo-tree [synthesis, not a cited fact]:** the current design's worst platform bugs live in the ripgrep boundary (rg path discovery #925, `sh -c` quoting, `maxBuffer` truncation, non-cancellable scans, byte columns, `--vimgrep -U` first-line-only output). A Rust core can link ripgrep's own crates (`grep-regex`, `grep-searcher`, `ignore`, `globset`) in-process and stream results with a cancellation token, which removes the rg binary and the shell entirely under options 1, 2 and 4 and is mandatory under 3a/3b (no process spawning). The VS Code-facing pieces (TreeDataProvider, decorations, status bar, `workspaceState`, settings migration, menus/context keys) must stay in TypeScript regardless of option; the protocol between core and host should therefore be designed around: (a) incremental result batches keyed by file URI (fixing the identity-based cache), (b) per-document scans of unsaved buffers (the host must ship buffer text to the core, or the core must expose a regex-equivalent that runs on the host side — note the JS-vs-Rust regex dialect split is itself a parity question), (c) a single-flight scan scheduler, (d) the tree model, sorting, filtering and counts (pure computation, good wasm candidates).

---

## 14. Open questions for the spec author

1. **Baseline version.** The repo head is 0.0.224; the Marketplace ships 0.0.226. Decide whether to diff the published VSIX (`dist/extension.js`) against this inventory before freezing parity targets.
2. **Bug-for-bug parity list.** For each of the following decide keep / fix / drop: `applyGlobs` always running (wrong key, extension.js:511); `useBuiltInExcludes: "file excludes"` dead (config.js:155); `rootFolder` env-var expansion no-op; `regex.regex` changes not triggering a rescan; `regex.regex` language overrides ignored; `getRootFolders()` returning `undefined` and crashing `rebuild()`; shared `refreshTimeout` letting keystrokes cancel git/periodic rescans; one global 500 ms `fileRefreshTimeout` for all documents; `stopScan` being a no-op; silent `maxBuffer` truncation; identity-based `searchResults.remove` and the resulting ghost nodes after soft refreshes; `endColumn` overshoot; `isFolder = subTag` on group nodes; tags-only tag node fsPath = first file; `validateIcons` rejecting `todo-tree`/`todo-tree-filled`; `customHighlight` keys effectively exact-match only; `iconColor` US-spelling precedence; complementary foreground unreachable; `rulerColour` default = background; per-activation legacy migration re-firing; `migratedVersion` notices repeating until "Never Show This Again".
3. **Regex engine and dialect.** The tree and highlights use ECMAScript regex; the workspace scan uses Rust regex; open buffers override disk results. Pick one engine for the Rust core and specify the compatibility contract for user regexes (backreferences, lookaround, `\n` multiline detection, `s`/`m`/`i` flags, empty-match protection).
4. **Column semantics.** rg byte columns vs JS UTF-16 char columns; define the wire format (UTF-8 byte offset? char? `Position`?) and the reveal behaviour for non-ASCII lines.
5. **Multiline semantics.** The README's described behaviour, the `formatResults` buffering, and the editor-scan `extraLines` all differ; rg cannot return continuation lines under `--vimgrep`. Specify the intended model (probably: the core returns the full multi-line match with per-line offsets).
6. **Glob engine.** micromatch on absolute paths (client-side), rg `-g` globs, `files.exclude`/`search.exclude` true-keys, `isHidden`'s narrow definition, Windows `createFolderGlob` form, `excludeThisFile` raw absolute paths. Decide whether `globset` semantics may replace micromatch and document the differences (the gap fill could not verify micromatch behaviour; node_modules is present in the scratchpad copy now, so this can be tested).
7. **Result cache and incremental model.** Replace the flat array + identity semantics with a per-URI index; define how unsaved-buffer results shadow disk results and how they are reconciled on save/close/rescan across the four scan modes.
8. **State precedence.** Keep the workspaceState-overrides-setting rule for flat/tagsOnly/expanded/groupedByTag/groupedBySubTag (and its "setting has no effect after the first click" consequence)? Keep `resetCache` not clearing `groupedBy*`?
9. **Tree model quirks to keep or not:** root compaction of single-child tag nodes, sub-tag pseudo-folders under files, sub-tag inheritance by folder nodes, `TAG (sub)` keys in tags-only view, last-match-wins workspace root, case-sensitive path lookup, `findTodoNode` ignoring column, compact-folder label/children mismatch, `nodesToGet` callback semantics, tooltips = fabricated fsPath for tag nodes, export shape (hidden nodes exported, extra lines not).
10. **Sorting contract.** Folders-first with tri-state `isFolder`, `config.tags()` order for root tag nodes, `tree.sort=false` meaning "rg order" (rg is multithreaded, so unstable). Specify a deterministic order.
11. **Highlight contract.** Which `type` values to support (`tag-and-subTag` needing `customHighlight[subTag]`; `capture-groups` needing indices; `line` reversed range), decoration re-creation per pass vs cached decoration types, per-editor vs per-document keys, whether `defaultHighlight` applies to sub-tags, and colour handling (theme-id detection by substring, opacity boundary at exactly 1, alpha-in-hex override).
12. **Icons.** SVGs written into `globalStorageUri` per colour/name (synchronous fs); keep, or move to `ThemeIcon`/`IconPath` objects generated in memory?
13. **Commands and menus.** The three registration-only commands (`openUrl`, `stopScan`, `onStatusBarClicked`) and the four palette-hidden context commands; Workspace-target writes failing in no-folder windows; Global-target writes for tags/status bar that are shadowed by workspace values.
14. **Status bar.** `tags`/`current file` showing only tags present in `general.tags` (tag groups and custom tags omitted), `top three` ignoring that list, badge always workspace-wide, title count switching to current-file total.
15. **Git/periodic refresh.** Keep polling at all? If so: single-flight `execFile`, clear intervals on deactivate, include `rootFolder`, and decide behaviour after sleep/resume.
16. **Startup.** `scanAtStartup` O(N²) `refreshOpenFiles`, migration writes re-entering the config handler, whether to keep the legacy flat-key migration (and stop rewriting `general.schemes`).
17. **Platform matrix and bridge.** Choose among §13 options; decide whether web support (vscode.dev) is a requirement (forces wasm), whether the core embeds the grep crates (removes rg discovery, quoting, truncation, cancellation problems), and how buffers of open documents reach the core.
18. **Unverified items from the gap fills:** bundled rg version behaviour (verified on rg 15.1.0 only); `TextDocument.uri` identity assumption; CRLF trailing `\r` in rg output; Windows `cmd.exe` quoting; `[Omitted long line]` threshold on older rg; `Uri.parse('todotree-export:C:\\...')` on Windows.
19. **Truncated inputs.** The `tree` reader input ended inside the "nodesToGet" feature and the highlights gap fill inside the "Editor highlight pass" feature; the missing material was re-derived from source for this document (§4.5-4.6, §5.2-5.6), but any feature those readers documented beyond the truncation point and that is not visible in source (e.g. test expectations in `test/tests.js`) was not merged.
