# Clippings Extension Implementation Plan (Plan 3 of 6)

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build the VS Code extension: the manifest and settings, server resolution and lifecycle, the tree provider with client-side expansion, decorations, the status bar, every command, menus and context keys, the todo-tree settings import, and the extension test suites.

**Architecture:** The extension in `extension/` is a thin relay over `clippings lsp`. `ServerConnection` resolves and probes the binary, runs it through `vscode-languageclient` and turns the custom `clippings/*` notifications into events. Everything VS Code-facing hangs off those events in `extension.ts`: a tree provider over node ID strings with the expansion map and epoch kept on the client, a decoration manager keyed by style generation and document version, a status controller, context keys, and the commands, which write view state or settings and push the configuration object back to the server. Logic that needs no VS Code API lives in pure modules (`config/configuration.ts`, `server/candidates.ts`, `filters/globs.ts`, `context/keys.ts`, `decorations/rules.ts`, `status/presentation.ts`, `icons/svg.ts`, `importer/plan.ts` and others) so mocha tests it in plain Node. Integration tests run the real extension and the debug server in a VS Code test instance.

**Tech Stack:** TypeScript 7.0.2 (strict, `module: preserve`, `moduleResolution: bundler`), esbuild 0.28.2, pnpm 10.27.0 (pinned through `packageManager`), Node 22, vscode-languageclient 10.1.1, @primer/octicons 19.38.0, @types/vscode 1.91.0 with `engines.vscode` `^1.91.0`, @types/node 22.20.4, mocha 11.8.0 with @types/mocha 10.0.10, @vscode/test-cli 0.0.15, @vscode/test-electron 3.1.0, @vscode/vsce 4.0.0. Every version is pinned exactly in `extension/package.json`. Integration tests run on VS Code `stable` (1.139.0 when the prototype ran). Rust: `tracing-subscriber` 0.3 with only the `fmt` and `std` features (0.3.23 resolved) is the one new crate.

**Spec:** `docs/superpowers/specs/2026-09-23-clippings-design.md`. This plan implements section 7, section 8.3, the extension parts of 10.2, 10.3 and 12.5, and the extension host line of the section 13 table. The last task records this plan's rulings in the spec.

**Builds on:** plan 2, merged on `main` at `de1447a`. The server's wire protocol (`crates/clippings-core/src/protocol.rs`), `clippings lsp` and `clippings probe` are used as they are, with three small server changes in Tasks 4, 5 and 20.

**Research:** three documents sit in `docs/research/`, committed on `main` with this plan: `clippings-manifest-mapping.md` (how each todo-tree manifest entry maps to Clippings), `clippings-contributes.json` (the generated `contributes` block) and `extension-toolchain.md` (the toolchain survey behind the pinned versions). Task 1's `contributes` block is `clippings-contributes.json` with one when-clause fixed (ruling 20).

## Global Constraints

- TypeScript is strict: `strict`, `noUnusedLocals`, `noUnusedParameters`, `noFallthroughCasesInSwitch`, `noImplicitOverride`, `isolatedModules` and `verbatimModuleSyntax`, as Task 1's `tsconfig.json` sets them. `pnpm -C extension typecheck` must pass before every commit. There is no formatter: follow the existing style, with two-space indents, single quotes, trailing commas and lines under 120 columns.
- The extension stays a thin relay. It never scans, matches tags, shapes the tree, computes decoration ranges or counts todos; the server does. The client maps server output onto VS Code objects, keeps client-only state (expansion map, epoch, view state, server path approvals, the import decision) and sends the configuration object.
- Unit tests run under mocha in plain Node, where the `vscode` module does not exist. A module that a unit test imports may reference `vscode` only through `import type`, which the bundler erases.
- For the Rust tasks (4, 5 and 20): `cargo fmt --all` must leave no diff, `cargo clippy --all-targets -- -D warnings` must pass, and `INSTA_UPDATE=no cargo test --all` must pass before the commit.
- Commit messages: the subject line from the task, a blank line, then your model attribution trailer (`Co-Authored-By: ...`). Use the heredoc form shown in each task so the blank line is kept.
- `PROTOCOL_VERSION` is `1` on both sides. `extension/src/protocol.ts` mirrors `crates/clippings-core/src/protocol.rs` field for field, and the probe rejects a server that reports any other version.
- Dependencies are pinned to exact versions. Task 1 runs `pnpm -C extension install` and commits the generated `extension/pnpm-lock.yaml`; no later task adds a dependency. In a fresh checkout, restore them with `pnpm -C extension install --frozen-lockfile`.
- Test isolation: integration tests run through `@vscode/test-cli` and `@vscode/test-electron`. `extension/.vscode-test.mjs` copies `tests/fixtures/workspace/` to a new temporary directory and starts VS Code with `--disable-extensions`, `--disable-workspace-trust` and a fresh `--user-data-dir` holding quiet settings. Tests may edit fixture files and settings freely, and must never touch the repository copy of the fixture or the user's own VS Code profile. Each test that changes a setting or a file restores it.
- A VS Code test window appears on screen during every integration run and closes by itself. Do not click in it or type into it while tests run.
- The server under test comes from the `CLIPPINGS_SERVER_PATH` environment variable. `.vscode-test.mjs` sets it to `target/debug/clippings` (`clippings.exe` on Windows) unless it is already set, and the unit tests that spawn a server (`src/test/unit/lspProcess.ts`) read the same variable with the same default. `pnpm -C extension dev` builds that binary; run it after any Rust change and before the first test run. `pnpm -C extension test` runs `dev`, `typecheck`, `build`, `test:unit` and `test:integration` in that order.
- Unit tests run from the esbuild output: `pnpm -C extension build` bundles `src/extension.ts` into `dist/extension.js` and every `*.test.ts` into `out/test/`, and `pnpm -C extension test:unit` runs `out/test/unit/**/*.test.js`. Always build before running unit tests.
- Waits in integration tests are driven by events from the extension's test hooks (`waitFor`, `treeBecomes`, `nextEvent` in `src/test/integration/helpers.ts`), never by fixed sleeps.
- Performance target, checked in Task 18: the extension host spends under 30 ms applying a `clippings/treeChanged` delta that refreshes 1,000 visible nodes.

## Review Focus

These inputs are implied by the spec but easy to miss, and each would bite a real user. Each has a test in the task that owns the code.

1. **A setting of the wrong type in `settings.json`**, such as `"clippings.general.schemes": "file"`, which VS Code passes on unvalidated. The server must still start, on the defaults, and warn `Invalid configuration, using the defaults: ...`; the client must build its document selector from the default schemes. Before this plan the server exited during `initialize` and the extension never started. Tests `a_setting_of_the_wrong_type_starts_on_the_defaults_with_a_warning` (Rust), `falls back to the defaults for a value of the wrong type` (unit) and `starts on the defaults with a warning when a setting has the wrong type` (integration) in Task 20.
2. **Todos in Jupyter notebook cells** must appear under their notebook in the tree and be decorated in each cell's editor, through the `vscode-notebook-cell` scheme. Test `lists and decorates the todos in notebook cells` in Task 14.
3. **Folders whose names contain spaces, brackets or parentheses**, such as `app [slug] (old)` in a Next.js project, must filter exactly through Only Show This Folder and Hide This Folder, because the temporary glob escapes them the way the server's glob matcher reads them. Test `filters a folder whose name has spaces and brackets` in Task 10.
4. **An untrusted workspace that sets `clippings.server.path`**, as a cloned repository can, must not run that binary or even prompt; the user's own value or the bundled server applies. Test `ignores a workspace server.path in an untrusted workspace` in Task 3.
5. **A server that keeps crashing, exits while it starts, or hangs while it starts**, as a broken or mismatched binary can, must stop after five crashes within three minutes, leave no server process behind, show a notice with Restart and Show Log, and come back on Restart Server without hanging. Tests `stops restarting after five crashes in three minutes and restarts on demand`, `counts a server that exits while starting as a crash and restarts on demand` and `counts a server that does not finish starting in time as a crash and kills it` in Task 6; Task 10 adds the checks that the notice goes through the prompts module and that the tree loads after the restart.

## Rulings made while prototyping this plan

Each was taken where the spec was silent, unworkable or at odds with todo-tree. Task 21 writes them into the spec.

1. **The Run Extension launch configuration sets the `CLIPPINGS_SERVER_PATH` environment variable**, with `RUST_BACKTRACE=1` and a `windows` block pointing at `clippings.exe`, instead of setting `clippings.server.path`. A launch configuration cannot set a setting, and the variable is resolution candidate 1, so no Allow or Deny prompt appears (Task 19, spec 8.3).
2. **Reset All Filters also clears the text filter.** todo-tree made every node visible but kept `currentFilter`, so the filter came back on the next refresh (Task 10, spec 7.6 and 11.2).
3. **The status bar cycle writes `general.statusBar` where the value lives**, the workspace scope if it has a workspace value and the global scope otherwise, as spec 7.6 requires. todo-tree wrote to the global scope, where a workspace value hid the change (Task 13).
4. **The icon artwork is Clippings' own.** The activity bar container icon, the `todo-tree` and `todo-tree-filled` icons and the default gutter icon are new drawings, and the check-circle is the octicon `check-circle-fill` (MIT). todo-tree's `resources/icons` files are licensed CC BY-ND 3.0, not MIT, so none is copied (Tasks 1, 7 and 15, spec 7.7).
5. **Codicon names are not validated**, because the extension has no codicon list. Octicon names and the two todo-tree names are; any other name that is not written `$(name)` is reported once as a warning (Task 15, spec 7.7).
6. **`clippings.reveal` acts only while the view is visible**, from the command palette too, as in todo-tree (Task 9, spec 7.5).
7. **Track file cancels a pending reveal** when the active editor changes again within 500 ms. todo-tree queued them, revealing files the user had already left (Task 9, spec 7.5 and 11.2).
8. **When the epoch bumps.** A change that makes the server re-render the whole tree is a change of view mode, grouping, `expanded` or scan mode, and the server reports it as a root refresh (`[null]`). For such a change the client bumps the epoch at that root refresh; for any other Expand, Collapse, Reset Cache or `tree.expanded` change it bumps at once and refreshes the root. Bumping before the server re-renders would make VS Code cache the old defaults under the new IDs (Task 8, spec 7.5).
9. **The epoch survives Reset Cache and only grows**, so a tree item ID is never reused (Tasks 2 and 8, spec 7.5).
10. **Folder and file filter commands read the path from the node's own key** (`w:<folder uri>`, `d:<path>` or `f:<path>`), found through the parent recorded for the node, because keys can contain `/`. A file node of a document without a file path, such as an untitled buffer, has nothing to filter (Task 10, spec 7.6).
11. **Temporary globs escape metacharacters as `globset::escape` does**, wrapping each of `?*[]{}` in brackets, and use forward slashes on Windows, so the server's matcher reads the glob back as the literal path (Task 10, spec 7.6).
12. **Every prompt goes through one module.** Input boxes, quick picks and messages that commands show go through `ui/prompts.ts`, which integration tests script and read. Commands take only the arguments VS Code passes them (Task 10, spec 12.5).
13. **Test hooks live in what `activate` returns**, under its `test` field (`src/testApi.ts`): server state, tree items, the epoch, the last flash, perf counters, context keys, status bar text, decoration entries and options, view state and the import offer. VS Code offers no API to read most of these (Tasks 6 to 18, spec 12.5).
14. **Integration runs are isolated.** `.vscode-test.mjs` copies the fixture to a temporary directory and passes a fresh `--user-data-dir` with quiet settings (AI features, git, telemetry and updates off) and `--disable-workspace-trust`. Runs never touch the repository or a profile, and the untrusted-workspace path is covered by unit tests only (Tasks 1 and 7, spec 12.5).
15. **New settings placement.** `filtering.builtInExcludes` is in the Filtering group, and `server.path`, `server.logLevel` and `trace.server` form a Server group at order 7, the slot of todo-tree's dropped Ripgrep group (Task 1, spec 7.2).
16. **Import wording.** The offer reads `Clippings found Todo Tree settings. Import them into Clippings?` with Import, Not Now and Never. A finished import says `Clippings: imported N settings from Todo Tree.`, and each skipped value is logged (Task 17, spec 7.3).
17. **The configuration object carries whole groups**, including client-only keys such as `tree.buttons`, `filtering.scopes` and `general.statusBarClickBehaviour`, relying on the server ignoring unknown fields as spec 6.3 allows. One read of the settings serves both sides (Task 2).
18. **The server logs to stderr** (Rust change, Task 4). The binary had no tracing subscriber, so every server log of spec 10.3 was dropped. It now writes to stderr through `tracing-subscriber`, without ANSI colours, at the level in `CLIPPINGS_LOG` (default `info`), which the client sets from `server.logLevel`, and logs one line at startup (spec 7.4 and 10.3).
19. **The first status precedes the first styles reset** (Rust change, Task 5). `Server::start` sent the generation-0 `styles` reset before the first `status`. A client that discards its style generation on seeing a new instance, as spec 6.2 requires, then dropped that reset, and with it every decoration after a restart (spec 6.2).
20. **The Show Tree View context menu clause is parenthesised**: `view == clippings-view && (clippings-flat == true || clippings-tags-only == true)`. todo-tree's clause had no parentheses, so its second condition escaped the view guard and the item could appear in other views' menus; an undocumented bug, fixed as spec 11 directs. Every menu tests `view == clippings-view` where todo-tree matched `view =~ /todo-tree/` (Task 1, spec 7.6).
21. **`general.revealBehaviour` has three values**, `start of line`, `start of todo` and `end of todo`, as in v0.0.224, which the spec pins. The `leave focus in tree` value that v0.0.225 added is not carried (Task 1, spec 7.2).
22. **Command category and Marketplace category.** Every command's category is `Clippings`. The manifest's Marketplace `categories` is `["Other"]`, because Marketplace categories come from a fixed list (Task 1, spec 7.1).
23. **A workspace `server.path` in an untrusted workspace is ignored without a prompt**, and the user's global value applies. Granting trust restarts the server when a workspace value exists. (Tasks 3 and 6, spec 7.4).
24. **A setting of the wrong type starts the server on the defaults** (Rust change, Task 20). The server exited during `initialize` when it could not read the settings, so a single hand-edited value stopped the whole extension. It now starts on the defaults and warns `Invalid configuration, using the defaults: ...`, matching how it already keeps the previous object on an unreadable `clippings/configure`. The client's own reads of `general.schemes`, for the document selector and track file, fall back to the default schemes the same way (spec 6.3, 7.4 and 10.1).
25. **Scanning texts.** While a full scan runs the status bar shows `Clippings: Scanning...` with the tooltip `Click to interrupt scan`, and after a stop `Clippings: Scanning interrupted.` with `Click to restart`: todo-tree's texts with `Clippings` in place of `Todo Tree` (Task 13, spec 7.7).
26. **What counts as extension host time.** The provider's own synchronous work, which is handling `clippings/treeChanged`, recording children and building tree items, plus parsing the children response. VS Code's own tree conversion is not visible to the extension. The prototype measured about 1.4 ms for 1,000 nodes against the 30 ms target, and about 40 ms end to end including the server round trip and VS Code's tree IPC, which is not host time (Task 18, spec 13).
27. **The test VS Code version is `stable`, not pinned**, so the suite follows current VS Code. `CLIPPINGS_TEST_VSCODE` names another version when a run must be reproduced (Task 1, spec 12.5).
28. **A server that fails to start counts as a crash.** When the server exits before the client is running, as during `initialize`, or hangs there, the language client's `start()` can stay pending forever, and a restart that waited on it hung, leaving the extension dead. The connection now treats an exit before running, a rejected start, or a start that does not finish within 10 seconds (`START_TIMEOUT_MS`), as a start failure: it settles the start at once, kills the server process (SIGTERM, then SIGKILL, each with a 2-second wait), disposes that language client, records a crash in one five-in-three-minutes count shared by every client of the connection, and starts a fresh client until the limit, then shows the crash notice with Restart and Show Log. `clippings.restartServer` forgets past crashes and always builds a fresh client. Clippings counts crashes itself instead of using the language client's default handler, whose count lived and died with each client, spawns the server process itself, because the language client forgets its process handle once the connection closes, even while that process still runs, and always continues on connection errors: writes to a server that just died fail before the connection closes, and the default handler's shutdown after three of them stopped the client without any restart or notice (Tasks 6 and 10, spec 7.4).
29. **The client sends no `$/cancelRequest`.** It passes no cancellation tokens: children, find and navigate requests are short, and VS Code's `TreeDataProvider.getChildren` receives no token to pass on. Spec 6.1 no longer lists it (Task 21).

---

### Task 1: Scaffold the manifest, build and test harness

Creates the `extension/` package that every later task builds on (spec 7.1, 7.2, 8.2 and 12.5). `package.json` carries the whole `contributes` block, with 37 commands, 65 settings (all window scoped except the machine-overridable `server.path`), the menus and the `clippings` view container, plus the capabilities of spec 7.1 and exactly pinned dependencies. esbuild bundles the extension and the tests, and `.vscode-test.mjs` runs integration tests against a temporary copy of a new fixture workspace with a fresh profile. The manifest tests pin the platform facts of spec 7.1 and check that no menu clause escapes its view guard (ruling 20).

**Files:**
- Create: `extension/.gitignore`, `extension/.vscodeignore`, `extension/package.json`, `extension/tsconfig.json`, `extension/build.mjs`, `extension/.vscode-test.mjs`, `extension/src/types/octicons.d.ts`, `extension/resources/clippings-container.svg`, `extension/src/extension.ts`
- Create (generated by `pnpm install`): `extension/pnpm-lock.yaml`
- Modify: `.gitignore`
- Test: `extension/src/test/integration/activation.test.ts`, `extension/src/test/unit/manifest.test.ts`, and the fixture workspace `tests/fixtures/workspace/README.md`, `tests/fixtures/workspace/docs/plan.md`, `tests/fixtures/workspace/lib/notes.rs`, `tests/fixtures/workspace/node_modules/leftpad/index.js`, `tests/fixtures/workspace/src/app.ts`, `tests/fixtures/workspace/src/util/strings.py`

**Interfaces:**
- Produces: `extension/package.json` with the scripts `build`, `watch`, `typecheck`, `dev`, `test:unit`, `test:integration`, `test` and `package`; the 37 `clippings.*` commands later tasks register; the 65 `clippings.*` settings later tasks read; the view container `clippings` and the view `clippings-view`.
- Produces, in `extension.ts`: `interface ClippingsApi { readonly version: string }`, `function activate(context: vscode.ExtensionContext): ClippingsApi`, `function deactivate(): void`. Task 6 moves `ClippingsApi` to `src/testApi.ts` and adds its `test` hooks.
- Produces: the fixture workspace `tests/fixtures/workspace/`, whose tree Task 7 asserts, and types for `@primer/octicons` in `src/types/octicons.d.ts`.

- [ ] **Step 1: Create the package and the build and test harness**

The root `.gitignore` gains one line so the fixture's `node_modules` folder is committed; the fixture needs it to show that `node_modules` is never indexed. `package.json` holds the full manifest. Its `contributes` block is `docs/research/clippings-contributes.json` with the Show Tree View context-menu clause parenthesised (ruling 20).

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
```

Create `extension/.gitignore`:

```gitignore
out/
.vscode-test/
```

Create `extension/.vscodeignore`:

```gitignore
**
!dist/**
!bin/**
!resources/**
!package.json
!LICENSE
!README.md
```

Create `extension/package.json`:

```json
{
  "name": "clippings",
  "displayName": "Clippings",
  "description": "Fast TODO tree backed by a Rust language server, with todo-tree's features and settings.",
  "version": "0.1.0",
  "publisher": "clippings-dev",
  "license": "MIT",
  "private": true,
  "categories": [
    "Other"
  ],
  "keywords": [
    "todo",
    "fixme",
    "tasks",
    "tree",
    "highlight"
  ],
  "engines": {
    "vscode": "^1.91.0"
  },
  "extensionKind": [
    "workspace"
  ],
  "activationEvents": [
    "onStartupFinished"
  ],
  "main": "./dist/extension.js",
  "capabilities": {
    "untrustedWorkspaces": {
      "supported": "limited",
      "description": "The server binary path setting is ignored in untrusted workspaces.",
      "restrictedConfigurations": [
        "clippings.server.path"
      ]
    },
    "virtualWorkspaces": false
  },
  "contributes": {
    "viewsContainers": {
      "activitybar": [
        {
          "id": "clippings",
          "title": "TODOs",
          "icon": "resources/clippings-container.svg"
        }
      ]
    },
    "views": {
      "clippings": [
        {
          "id": "clippings-view",
          "name": "TODOs",
          "when": "!clippings-is-empty"
        }
      ]
    },
    "menus": {
      "view/title": [
        {
          "command": "clippings.exportTree",
          "when": "view == clippings-view && clippings-show-export-button == true",
          "group": "navigation@1"
        },
        {
          "command": "clippings.reveal",
          "when": "view == clippings-view && clippings-tags-only == false && clippings-show-reveal-button == true",
          "group": "navigation@2"
        },
        {
          "command": "clippings.scanOpenFilesOnly",
          "when": "view == clippings-view && clippings-scan-mode == 'workspace only' && clippings-show-scan-mode-button == true",
          "group": "navigation@3"
        },
        {
          "command": "clippings.scanCurrentFileOnly",
          "when": "view == clippings-view && clippings-scan-mode == 'open files' && clippings-show-scan-mode-button == true",
          "group": "navigation@3"
        },
        {
          "command": "clippings.scanWorkspaceAndOpenFiles",
          "when": "view == clippings-view && clippings-scan-mode == 'current file' && clippings-show-scan-mode-button == true",
          "group": "navigation@3"
        },
        {
          "command": "clippings.scanWorkspaceOnly",
          "when": "view == clippings-view && clippings-scan-mode == 'workspace' && clippings-show-scan-mode-button == true",
          "group": "navigation@3"
        },
        {
          "command": "clippings.showFlatView",
          "when": "view == clippings-view && clippings-flat == false && clippings-tags-only == false && clippings-show-view-style-button == true",
          "group": "navigation@4"
        },
        {
          "command": "clippings.showTagsOnlyView",
          "when": "view == clippings-view && clippings-flat == true && clippings-tags-only == false && clippings-show-view-style-button == true",
          "group": "navigation@4"
        },
        {
          "command": "clippings.showTreeView",
          "when": "view == clippings-view && clippings-flat == false && clippings-tags-only == true && clippings-show-view-style-button == true",
          "group": "navigation@4"
        },
        {
          "command": "clippings.groupByTag",
          "when": "view == clippings-view && clippings-grouped-by-tag == false && clippings-show-group-by-tag-button == true",
          "group": "navigation@5"
        },
        {
          "command": "clippings.ungroupByTag",
          "when": "view == clippings-view && clippings-grouped-by-tag == true && clippings-show-group-by-tag-button == true",
          "group": "navigation@5"
        },
        {
          "command": "clippings.groupBySubTag",
          "when": "view == clippings-view && clippings-grouped-by-sub-tag == false && clippings-show-group-by-sub-tag-button == true && clippings-has-sub-tags == true",
          "group": "navigation@6"
        },
        {
          "command": "clippings.ungroupBySubTag",
          "when": "view == clippings-view && clippings-grouped-by-sub-tag == true && clippings-show-group-by-sub-tag-button == true && clippings-has-sub-tags == true",
          "group": "navigation@6"
        },
        {
          "command": "clippings.filter",
          "when": "view == clippings-view && clippings-filtered == false && clippings-show-filter-button == true",
          "group": "navigation@7"
        },
        {
          "command": "clippings.filterClear",
          "when": "view == clippings-view && clippings-filtered == true && clippings-show-filter-button == true",
          "group": "navigation@7"
        },
        {
          "command": "clippings.refresh",
          "when": "view == clippings-view && clippings-show-refresh-button == true",
          "group": "navigation@8"
        },
        {
          "command": "clippings.expand",
          "when": "view == clippings-view && clippings-expanded == false && clippings-show-expand-button == true",
          "group": "navigation@9"
        },
        {
          "command": "clippings.collapse",
          "when": "view == clippings-view && clippings-expanded == true && clippings-show-expand-button == true",
          "group": "navigation@9"
        }
      ],
      "view/item/context": [
        {
          "command": "clippings.filter",
          "when": "view == clippings-view && clippings-filtered == false",
          "group": "1-filters@1"
        },
        {
          "command": "clippings.filterClear",
          "when": "view == clippings-view && clippings-global-filter-active",
          "group": "1-filters@2"
        },
        {
          "command": "clippings.excludeThisFolder",
          "when": "view == clippings-view && viewItem == folder",
          "group": "1-filters@3"
        },
        {
          "command": "clippings.excludeThisFile",
          "when": "view == clippings-view && viewItem == file",
          "group": "1-filters@4"
        },
        {
          "command": "clippings.showOnlyThisFolder",
          "when": "view == clippings-view && viewItem == folder",
          "group": "1-filters@5"
        },
        {
          "command": "clippings.showOnlyThisFolderAndSubfolders",
          "when": "view == clippings-view && viewItem == folder",
          "group": "1-filters@6"
        },
        {
          "command": "clippings.removeFilter",
          "when": "view == clippings-view && clippings-folder-filter-active",
          "group": "1-filters@7"
        },
        {
          "command": "clippings.resetAllFilters",
          "when": "view == clippings-view && clippings-folder-filter-active",
          "group": "1-filters@8"
        },
        {
          "command": "clippings.toggleItemCounts",
          "when": "view == clippings-view",
          "group": "2-toggles"
        },
        {
          "command": "clippings.toggleBadges",
          "when": "view == clippings-view",
          "group": "2-toggles"
        },
        {
          "command": "clippings.toggleCompactFolders",
          "when": "view == clippings-view && clippings-can-toggle-compact-folders == true",
          "group": "2-toggles"
        },
        {
          "command": "clippings.scanOpenFilesOnly",
          "when": "view == clippings-view && clippings-scan-mode != 'open files'",
          "group": "3-view"
        },
        {
          "command": "clippings.scanCurrentFileOnly",
          "when": "view == clippings-view && clippings-scan-mode != 'current file'",
          "group": "3-view"
        },
        {
          "command": "clippings.scanWorkspaceAndOpenFiles",
          "when": "view == clippings-view && clippings-scan-mode != 'workspace'",
          "group": "3-view"
        },
        {
          "command": "clippings.scanWorkspaceOnly",
          "when": "view == clippings-view && clippings-scan-mode != 'workspace only'",
          "group": "3-view"
        },
        {
          "command": "clippings.expand",
          "when": "view == clippings-view && clippings-expanded == false",
          "group": "4-tree@1"
        },
        {
          "command": "clippings.collapse",
          "when": "view == clippings-view && clippings-expanded == true",
          "group": "4-tree@1"
        },
        {
          "command": "clippings.showFlatView",
          "when": "view == clippings-view && clippings-flat == false",
          "group": "4-tree@2"
        },
        {
          "command": "clippings.showTagsOnlyView",
          "when": "view == clippings-view && clippings-tags-only == false",
          "group": "4-tree@3"
        },
        {
          "command": "clippings.showTreeView",
          "when": "view == clippings-view && (clippings-flat == true || clippings-tags-only == true)",
          "group": "4-tree@4"
        },
        {
          "command": "clippings.groupByTag",
          "when": "view == clippings-view && clippings-grouped-by-tag == false",
          "group": "4-tree@5"
        },
        {
          "command": "clippings.ungroupByTag",
          "when": "view == clippings-view && clippings-grouped-by-tag == true",
          "group": "4-tree@5"
        },
        {
          "command": "clippings.groupBySubTag",
          "when": "view == clippings-view && clippings-grouped-by-sub-tag == false && clippings-has-sub-tags == true",
          "group": "4-tree@6"
        },
        {
          "command": "clippings.ungroupBySubTag",
          "when": "view == clippings-view && clippings-grouped-by-sub-tag == true && clippings-has-sub-tags == true",
          "group": "4-tree@6"
        },
        {
          "command": "clippings.exportTree",
          "when": "view == clippings-view",
          "group": "5-misc1"
        },
        {
          "command": "clippings.reveal",
          "when": "view == clippings-view",
          "group": "6-misc2"
        }
      ],
      "commandPalette": [
        {
          "command": "clippings.showOnlyThisFolder",
          "when": "false"
        },
        {
          "command": "clippings.showOnlyThisFolderAndSubfolders",
          "when": "false"
        },
        {
          "command": "clippings.excludeThisFolder",
          "when": "false"
        },
        {
          "command": "clippings.excludeThisFile",
          "when": "false"
        }
      ]
    },
    "commands": [
      {
        "command": "clippings.showFlatView",
        "title": "Show Flat View",
        "category": "Clippings",
        "icon": "$(list-unordered)"
      },
      {
        "command": "clippings.showTagsOnlyView",
        "title": "Show Tags Only View",
        "category": "Clippings",
        "icon": "$(list-flat)"
      },
      {
        "command": "clippings.showTreeView",
        "title": "Show Tree View",
        "category": "Clippings",
        "icon": "$(list-tree)"
      },
      {
        "command": "clippings.refresh",
        "title": "Refresh",
        "category": "Clippings",
        "icon": "$(refresh)"
      },
      {
        "command": "clippings.expand",
        "title": "Expand Tree",
        "category": "Clippings",
        "icon": "$(expand-all)",
        "enablement": "clippings-collapsible"
      },
      {
        "command": "clippings.collapse",
        "title": "Collapse Tree",
        "category": "Clippings",
        "icon": "$(collapse-all)",
        "enablement": "clippings-collapsible"
      },
      {
        "command": "clippings.filter",
        "title": "Filter Tree",
        "category": "Clippings",
        "icon": "$(filter)"
      },
      {
        "command": "clippings.filterClear",
        "title": "Clear Tree Filter",
        "category": "Clippings",
        "icon": "$(clear-all)"
      },
      {
        "command": "clippings.groupByTag",
        "title": "Group by Tag",
        "category": "Clippings",
        "icon": "$(tag)"
      },
      {
        "command": "clippings.ungroupByTag",
        "title": "Ungroup by Tag",
        "category": "Clippings",
        "icon": "$(unfold)"
      },
      {
        "command": "clippings.groupBySubTag",
        "title": "Group by Sub Tag",
        "category": "Clippings",
        "icon": "$(chrome-restore)"
      },
      {
        "command": "clippings.ungroupBySubTag",
        "title": "Ungroup by Sub Tag",
        "category": "Clippings",
        "icon": "$(chrome-maximize)"
      },
      {
        "command": "clippings.scanOpenFilesOnly",
        "title": "Scan Open Files Only",
        "category": "Clippings",
        "icon": "$(files)"
      },
      {
        "command": "clippings.scanCurrentFileOnly",
        "title": "Scan Current File Only",
        "category": "Clippings",
        "icon": "$(symbol-file)"
      },
      {
        "command": "clippings.scanWorkspaceAndOpenFiles",
        "title": "Scan Workspace And Open Files",
        "category": "Clippings",
        "icon": "$(folder-active)"
      },
      {
        "command": "clippings.scanWorkspaceOnly",
        "title": "Scan Workspace Only",
        "category": "Clippings",
        "icon": "$(folder)"
      },
      {
        "command": "clippings.addTag",
        "title": "Add Tag",
        "category": "Clippings"
      },
      {
        "command": "clippings.removeTag",
        "title": "Remove Tag",
        "category": "Clippings"
      },
      {
        "command": "clippings.exportTree",
        "title": "Export Tree",
        "category": "Clippings",
        "icon": "$(save)"
      },
      {
        "command": "clippings.showOnlyThisFolder",
        "title": "Only Show This Folder",
        "category": "Clippings",
        "icon": "$(filter)"
      },
      {
        "command": "clippings.showOnlyThisFolderAndSubfolders",
        "title": "Only Show This Folder And Subfolders",
        "category": "Clippings",
        "icon": "$(filter)"
      },
      {
        "command": "clippings.switchScope",
        "title": "Switch Scope",
        "category": "Clippings",
        "icon": "$(filter)"
      },
      {
        "command": "clippings.excludeThisFolder",
        "title": "Hide This Folder",
        "category": "Clippings",
        "icon": "$(filter)"
      },
      {
        "command": "clippings.excludeThisFile",
        "title": "Hide This File",
        "category": "Clippings",
        "icon": "$(filter)"
      },
      {
        "command": "clippings.removeFilter",
        "title": "Remove Filter",
        "category": "Clippings",
        "icon": "$(filter)"
      },
      {
        "command": "clippings.resetAllFilters",
        "title": "Reset All Filters",
        "category": "Clippings",
        "icon": "$(clear-all)"
      },
      {
        "command": "clippings.reveal",
        "title": "Reveal Current File In Tree",
        "category": "Clippings",
        "icon": "$(location)"
      },
      {
        "command": "clippings.resetCache",
        "title": "Reset Cache",
        "category": "Clippings"
      },
      {
        "command": "clippings.toggleItemCounts",
        "title": "Toggle Item Counts",
        "category": "Clippings"
      },
      {
        "command": "clippings.toggleBadges",
        "title": "Toggle Badges",
        "category": "Clippings"
      },
      {
        "command": "clippings.toggleCompactFolders",
        "title": "Toggle Compact Folders",
        "category": "Clippings"
      },
      {
        "command": "clippings.goToNext",
        "title": "Go To Next",
        "category": "Clippings"
      },
      {
        "command": "clippings.goToPrevious",
        "title": "Go To Previous",
        "category": "Clippings"
      },
      {
        "command": "clippings.revealInFile",
        "title": "Reveal In File",
        "category": "Clippings"
      },
      {
        "command": "clippings.importTodoTreeSettings",
        "title": "Import Settings from Todo Tree",
        "category": "Clippings"
      },
      {
        "command": "clippings.restartServer",
        "title": "Restart Server",
        "category": "Clippings"
      },
      {
        "command": "clippings.showLog",
        "title": "Show Log",
        "category": "Clippings"
      }
    ],
    "configuration": [
      {
        "title": "General",
        "order": 1,
        "type": "object",
        "properties": {
          "clippings.general.automaticGitRefreshInterval": {
            "default": 0,
            "markdownDescription": "Polling interval (in seconds) for automatically refreshing the tree when your repository is updated. Set to '0' to disable.",
            "type": "integer",
            "scope": "window"
          },
          "clippings.general.periodicRefreshInterval": {
            "default": 0,
            "markdownDescription": "Periodic refresh interval (in minutes) for automatically refreshing the tree. Set to '0' to disable.",
            "type": "integer",
            "scope": "window"
          },
          "clippings.general.revealBehaviour": {
            "default": "start of todo",
            "enum": [
              "start of line",
              "start of todo",
              "end of todo"
            ],
            "markdownDescription": "Sets where the cursor is positioned when revealing a todo.",
            "markdownEnumDescriptions": [
              "Moves the cursor to the start of the line",
              "Moves the cursor to the beginning of the todo",
              "Moves the cursor to the end of the todo"
            ],
            "type": "string",
            "scope": "window"
          },
          "clippings.general.exportPath": {
            "default": "~/todo-tree-%Y%m%d-%H%M.txt",
            "markdownDescription": "Path to use when exporting the tree. Environment variables will be expanded, e.g `${HOME}` and the path is passed through strftime (see <https://github.com/samsonjs/strftime>). Set the extension to `.json` to export as a JSON record.",
            "type": "string",
            "scope": "window"
          },
          "clippings.general.rootFolder": {
            "default": "",
            "markdownDescription": "Folder in which to run the search (defaults to the workspace folder).",
            "type": "string",
            "scope": "window"
          },
          "clippings.general.schemes": {
            "default": [
              "file",
              "ssh",
              "untitled",
              "vscode-notebook-cell"
            ],
            "items": {
              "type": "string"
            },
            "markdownDescription": "Editor schemes to find TODOs in. To find TODOs in settings files, for instance, add `vscode-userdata` or for output windows, add `output`.",
            "type": "array",
            "scope": "window"
          },
          "clippings.general.statusBar": {
            "default": "none",
            "enum": [
              "none",
              "total",
              "tags",
              "top three",
              "current file"
            ],
            "markdownDescription": "What to show in the status bar - nothing, total count, counts per tag, top three counts per tag or count of tags in the current file.",
            "markdownEnumDescriptions": [
              "Only show the scanning status in the status bar",
              "Show the total count of tags in the status bar",
              "Show a breakdown of the count of each tag in the status bar",
              "Show the count of the top three tags in the status bar",
              "Show the count of tags in the current file in the status bar"
            ],
            "type": "string",
            "scope": "window"
          },
          "clippings.general.showIconsInsteadOfTagsInStatusBar": {
            "default": false,
            "markdownDescription": "Show icons instead of tags in the status bar",
            "type": "boolean",
            "scope": "window"
          },
          "clippings.general.statusBarClickBehaviour": {
            "default": "reveal",
            "enum": [
              "cycle",
              "reveal",
              "toggle highlights"
            ],
            "markdownDescription": "What to do when the status bar is clicked.",
            "markdownEnumDescriptions": [
              "Toggle between showing total count and the top three tag counts",
              "Reveal the tree view",
              "Toggle highlighting"
            ],
            "type": "string",
            "scope": "window"
          },
          "clippings.general.tagGroups": {
            "default": {},
            "markdownDescription": "Allows similar tags to be grouped under the same type, e.g. `{ \"FIX\": [\"FIXME\",\"FIXIT\"] }`. *Note: All tags must also be in the `clippings.general.tags` tag list. If a tag group is defined, custom highlights apply to the group, not the tags within the group.*",
            "type": "object",
            "additionalProperties": {
              "type": "array",
              "items": {
                "type": "string"
              }
            },
            "scope": "window"
          },
          "clippings.general.tags": {
            "default": [
              "BUG",
              "HACK",
              "FIXME",
              "TODO",
              "XXX",
              "[ ]",
              "[x]"
            ],
            "items": {
              "type": "string"
            },
            "markdownDescription": "List of tags. *Note, if one tag starts with another tag, the longer tag should be specified first to prevent the shorter tag being matched.*",
            "type": "array",
            "scope": "window"
          },
          "clippings.general.showActivityBarBadge": {
            "default": false,
            "markdownDescription": "Show a badge in the activity bar indicating the total number of TODOs",
            "type": "boolean",
            "scope": "window"
          }
        }
      },
      {
        "title": "Highlights",
        "order": 2,
        "type": "object",
        "properties": {
          "clippings.highlights.customHighlight": {
            "default": {
              "BUG": {
                "icon": "bug"
              },
              "HACK": {
                "icon": "tools"
              },
              "FIXME": {
                "icon": "flame"
              },
              "XXX": {
                "icon": "x"
              },
              "[ ]": {
                "icon": "issue-draft"
              },
              "[x]": {
                "icon": "issue-closed"
              }
            },
            "markdownDescription": "Custom configuration for highlighting, [Read more...](https://github.com/Gruntfuggly/todo-tree#highlighting).",
            "type": "object",
            "additionalProperties": {
              "type": "object",
              "properties": {
                "foreground": {
                  "type": "string",
                  "format": "color-hex"
                },
                "background": {
                  "type": "string",
                  "format": "color-hex"
                },
                "opacity": {
                  "type": "number"
                },
                "fontWeight": {
                  "type": "string"
                },
                "fontStyle": {
                  "type": "string"
                },
                "textDecoration": {
                  "type": "string"
                },
                "borderRadius": {
                  "type": "string"
                },
                "icon": {
                  "type": "string"
                },
                "iconColour": {
                  "type": "string",
                  "format": "color-hex"
                },
                "gutterIcon": {
                  "type": "boolean"
                },
                "rulerColour": {
                  "type": "string",
                  "format": "color-hex"
                },
                "rulerOpacity": {
                  "type": "number"
                },
                "rulerLane": {
                  "type": "string",
                  "enum": [
                    "none",
                    "left",
                    "center",
                    "right",
                    "full"
                  ]
                },
                "type": {
                  "type": "string",
                  "enum": [
                    "tag",
                    "text",
                    "tag-and-comment",
                    "tag-and-subTag",
                    "text-and-comment",
                    "line",
                    "whole-line",
                    "none"
                  ]
                },
                "hideFromTree": {
                  "type": "boolean"
                },
                "hideFromStatusBar": {
                  "type": "boolean"
                },
                "hideFromActivityBar": {
                  "type": "boolean"
                }
              }
            },
            "scope": "window"
          },
          "clippings.highlights.defaultHighlight": {
            "default": {},
            "markdownDescription": "Default configuration for highlighting. [Read more...](https://github.com/Gruntfuggly/todo-tree#highlighting).",
            "type": "object",
            "properties": {
              "foreground": {
                "type": "string",
                "format": "color-hex"
              },
              "background": {
                "type": "string",
                "format": "color-hex"
              },
              "opacity": {
                "type": "number"
              },
              "fontWeight": {
                "type": "string"
              },
              "fontStyle": {
                "type": "string"
              },
              "textDecoration": {
                "type": "string"
              },
              "borderRadius": {
                "type": "string"
              },
              "icon": {
                "type": "string"
              },
              "iconColour": {
                "type": "string",
                "format": "color-hex"
              },
              "gutterIcon": {
                "type": "boolean"
              },
              "rulerColour": {
                "type": "string",
                "format": "color-hex"
              },
              "rulerOpacity": {
                "type": "number"
              },
              "rulerLane": {
                "type": "string",
                "enum": [
                  "none",
                  "left",
                  "center",
                  "right",
                  "full"
                ]
              },
              "type": {
                "type": "string",
                "enum": [
                  "tag",
                  "text",
                  "tag-and-comment",
                  "tag-and-subTag",
                  "text-and-comment",
                  "line",
                  "whole-line",
                  "none"
                ]
              },
              "hideFromTree": {
                "type": "boolean"
              },
              "hideFromStatusBar": {
                "type": "boolean"
              },
              "hideFromActivityBar": {
                "type": "boolean"
              }
            },
            "scope": "window"
          },
          "clippings.highlights.enabled": {
            "default": true,
            "markdownDescription": "Set to false to disable highlighting.",
            "type": "boolean",
            "scope": "window"
          },
          "clippings.highlights.highlightDelay": {
            "default": 500,
            "markdownDescription": "Delay before highlighting tags within files (milliseconds).",
            "type": "integer",
            "scope": "window"
          },
          "clippings.highlights.useColourScheme": {
            "default": false,
            "markdownDescription": "Use a colour scheme to colour the tags. This scheme is applied to the tags in the order of tags. The colours can be modified using `clippings.highlights.foregroundColourScheme` and `clippings.highlights.backgroundColourScheme`. The colour scheme overrides colours in the default highlight, but not the custom highlight.",
            "type": "boolean",
            "scope": "window"
          },
          "clippings.highlights.foregroundColourScheme": {
            "default": [
              "white",
              "black",
              "black",
              "white",
              "white",
              "white",
              "black"
            ],
            "items": {
              "type": "string"
            },
            "markdownDescription": "A list of colours which is applied to tag highlights in the same order as the tags. Repeats if necessary and is overridden by `clippings.highlights.customHighlight`.",
            "type": "array",
            "scope": "window"
          },
          "clippings.highlights.backgroundColourScheme": {
            "default": [
              "red",
              "orange",
              "yellow",
              "green",
              "blue",
              "indigo",
              "violet"
            ],
            "items": {
              "type": "string"
            },
            "markdownDescription": "A list of colours which is applied to tag highlights in the same order as the tags. Repeats if necessary and is overridden by `clippings.highlights.customHighlight`.",
            "type": "array",
            "scope": "window"
          }
        }
      },
      {
        "title": "Filtering",
        "order": 3,
        "type": "object",
        "properties": {
          "clippings.filtering.excludedWorkspaces": {
            "default": [],
            "items": {
              "type": "string"
            },
            "markdownDescription": "An array of workspace names to exclude as roots in the tree (wildcards can be used).",
            "type": "array",
            "scope": "window"
          },
          "clippings.filtering.excludeGlobs": {
            "default": [
              "**/node_modules/*/**"
            ],
            "items": {
              "type": "string"
            },
            "markdownDescription": "Globs for use in limiting search results by exclusion (applied after **includeGlobs**), e.g. `[\"**/*.txt\"]` to ignore all .txt files.",
            "type": "array",
            "scope": "window"
          },
          "clippings.filtering.ignoreGitSubmodules": {
            "default": false,
            "markdownDescription": "If true, any subfolders containing a .git file will be ignored when searching.",
            "type": "boolean",
            "scope": "window"
          },
          "clippings.filtering.includedWorkspaces": {
            "default": [],
            "items": {
              "type": "string"
            },
            "markdownDescription": "An array of workspace names to include as roots in the tree (wildcards can be used). An empty array includes all workspace folders.",
            "type": "array",
            "scope": "window"
          },
          "clippings.filtering.includeGlobs": {
            "default": [],
            "items": {
              "type": "string"
            },
            "markdownDescription": "Globs for use in limiting search results by inclusion, e.g. `[\"**/unit-tests/*.js\"]` to only show .js files in unit-tests subfolders.",
            "type": "array",
            "scope": "window"
          },
          "clippings.filtering.includeHiddenFiles": {
            "default": false,
            "markdownDescription": "Include hidden files (starting with a .).",
            "type": "boolean",
            "scope": "window"
          },
          "clippings.filtering.scopes": {
            "default": [],
            "markdownDescription": "Scopes (sets of globs) that can be switched between",
            "type": "array",
            "scope": "window"
          },
          "clippings.filtering.useBuiltInExcludes": {
            "default": "none",
            "enum": [
              "none",
              "file excludes",
              "search excludes",
              "file and search excludes"
            ],
            "markdownDescription": "Add VSCode's `files.exclude` and/or `search.exclude` list to the ignored paths.",
            "markdownEnumDescriptions": [
              "Don't used any built in excludes",
              "Use the Files:Exclude setting",
              "Use the Search:Exclude setting",
              "Use the Files:Exclude and the Search:Exclude setting"
            ],
            "type": "string",
            "scope": "window"
          },
          "clippings.filtering.builtInExcludes": {
            "default": [
              ".git",
              ".hg",
              ".svn",
              "node_modules",
              ".pnpm-store",
              ".yarn/cache",
              ".claude",
              ".next",
              ".nuxt",
              ".output",
              ".turbo",
              ".cache",
              ".parcel-cache",
              ".svelte-kit",
              ".angular",
              ".vercel",
              ".sst",
              ".terraform",
              "target",
              "__pycache__",
              ".venv",
              "venv",
              ".gradle",
              ".idea",
              ".vs"
            ],
            "items": {
              "type": "string"
            },
            "markdownDescription": "Directory names Clippings never indexes below each scan root, in addition to `clippings.filtering.excludeGlobs` and `.gitignore`. An entry without a `/` matches a directory name at any depth below the root; an entry with a `/` matches a trailing run of path components. Never matched against the scan root itself or its ancestors.",
            "type": "array",
            "scope": "window"
          }
        }
      },
      {
        "title": "Tree",
        "order": 4,
        "type": "object",
        "properties": {
          "clippings.tree.autoRefresh": {
            "default": true,
            "markdownDescription": "Refresh the tree when files are opened or saved.",
            "type": "boolean",
            "scope": "window"
          },
          "clippings.tree.disableCompactFolders": {
            "default": false,
            "markdownDescription": "Prevent the tree from showing compact folders.",
            "type": "boolean",
            "scope": "window"
          },
          "clippings.tree.expanded": {
            "default": false,
            "markdownDescription": "When opening new workspaces, show the tree initially fully expanded.",
            "type": "boolean",
            "scope": "window"
          },
          "clippings.tree.filterCaseSensitive": {
            "default": false,
            "markdownDescription": "Set to true if you want the view filtering to be case sensitive.",
            "type": "boolean",
            "scope": "window"
          },
          "clippings.tree.flat": {
            "default": false,
            "markdownDescription": "When opening new workspaces, show the tree initially as flat list of files.",
            "type": "boolean",
            "scope": "window"
          },
          "clippings.tree.groupedByTag": {
            "default": false,
            "markdownDescription": "When opening new workspaces, show the tree initially grouped by tag.",
            "type": "boolean",
            "scope": "window"
          },
          "clippings.tree.groupedBySubTag": {
            "default": false,
            "markdownDescription": "When opening new workspaces, show the tree initially grouped by sub tag.",
            "type": "boolean",
            "scope": "window"
          },
          "clippings.tree.hideIconsWhenGroupedByTag": {
            "default": false,
            "markdownDescription": "Save some space by hiding the item icons when grouped by tag.",
            "type": "boolean",
            "scope": "window"
          },
          "clippings.tree.hideTreeWhenEmpty": {
            "default": false,
            "markdownDescription": "Hide the view if it is empty.",
            "type": "boolean",
            "scope": "window"
          },
          "clippings.tree.labelFormat": {
            "default": "${tag} ${after}",
            "markdownDescription": "Format for tree items.",
            "type": "string",
            "scope": "window"
          },
          "clippings.tree.scanAtStartup": {
            "default": true,
            "markdownDescription": "Normally the tree is built as soon as the window is opened. If you have a large code base and want to manually start the scan, set this to false.",
            "type": "boolean",
            "scope": "window"
          },
          "clippings.tree.scanMode": {
            "default": "workspace",
            "enum": [
              "workspace",
              "open files",
              "current file",
              "workspace only"
            ],
            "markdownDescription": "Set this to change which files are scanned.",
            "markdownEnumDescriptions": [
              "Scan the whole workspace (or workspaces) and open file",
              "Scan open files only",
              "Scan the current file only",
              "Scan the workspace but don't refresh files open in the editor"
            ],
            "type": "string",
            "scope": "window"
          },
          "clippings.tree.showBadges": {
            "default": true,
            "markdownDescription": "Show badges and SCM state in the tree view.",
            "type": "boolean",
            "scope": "window"
          },
          "clippings.tree.showCountsInTree": {
            "default": false,
            "markdownDescription": "Show counts of TODOs in the tree.",
            "type": "boolean",
            "scope": "window"
          },
          "clippings.tree.showCurrentScanMode": {
            "default": true,
            "markdownDescription": "Show the current scan mode at the top of the tree view",
            "type": "boolean",
            "scope": "window"
          },
          "clippings.tree.subTagClickUrl": {
            "default": "",
            "markdownDescription": "The URL to open when clicking on a sub tag in the tree. Can include placeholders as defined in `clippings.tree.labelFormat`.",
            "type": "string",
            "scope": "window"
          },
          "clippings.tree.sortTagsOnlyViewAlphabetically": {
            "default": false,
            "markdownDescription": "Sort items in the tags only view alphabetically instead of by file and line number.",
            "type": "boolean",
            "scope": "window"
          },
          "clippings.tree.sort": {
            "default": true,
            "markdownDescription": "The walker searches using multiple threads to improve performance. The tree is sorted when it is populated so that it stays stable. If you want to use the walker's own (unspecified, non-deterministic) order, set this to false.",
            "type": "boolean",
            "scope": "window"
          },
          "clippings.tree.tagsOnly": {
            "default": false,
            "markdownDescription": "When opening new workspaces, show only tag elements in tree.",
            "type": "boolean",
            "scope": "window"
          },
          "clippings.tree.tooltipFormat": {
            "default": "${filepath}, line ${line}",
            "markdownDescription": "Tree item tooltip format.",
            "type": "string",
            "scope": "window"
          },
          "clippings.tree.trackFile": {
            "default": true,
            "markdownDescription": "Track the current file in the tree view.",
            "type": "boolean",
            "scope": "window"
          }
        }
      },
      {
        "title": "Buttons",
        "order": 5,
        "type": "object",
        "properties": {
          "clippings.tree.buttons.reveal": {
            "default": true,
            "markdownDescription": "Show a button in the tree view title bar to reveal the current item (only when track file is not enabled).",
            "type": "boolean",
            "scope": "window"
          },
          "clippings.tree.buttons.scanMode": {
            "default": false,
            "markdownDescription": "Show a button in the tree view title bar to change the Scan Mode setting.",
            "type": "boolean",
            "scope": "window"
          },
          "clippings.tree.buttons.viewStyle": {
            "default": true,
            "markdownDescription": "Show a button in the tree view title bar to change the view style (tree, flat or tags only).",
            "type": "boolean",
            "scope": "window"
          },
          "clippings.tree.buttons.groupByTag": {
            "default": true,
            "markdownDescription": "Show a button in the tree view title bar to enable grouping items by tag.",
            "type": "boolean",
            "scope": "window"
          },
          "clippings.tree.buttons.groupBySubTag": {
            "default": false,
            "markdownDescription": "Show a button in the tree view title bar to enable grouping items by sub tag.",
            "type": "boolean",
            "scope": "window"
          },
          "clippings.tree.buttons.filter": {
            "default": true,
            "markdownDescription": "Show a button in the tree view title bar allowing the tree to be filtered by entering some text.",
            "type": "boolean",
            "scope": "window"
          },
          "clippings.tree.buttons.refresh": {
            "default": true,
            "markdownDescription": "Show a refresh button in the tree view title bar.",
            "type": "boolean",
            "scope": "window"
          },
          "clippings.tree.buttons.expand": {
            "default": true,
            "markdownDescription": "Show a button in the tree view title bar to expand or collapse the whole tree.",
            "type": "boolean",
            "scope": "window"
          },
          "clippings.tree.buttons.export": {
            "default": false,
            "markdownDescription": "Show a button in the tree view title bar to create a file showing the tree content.",
            "type": "boolean",
            "scope": "window"
          }
        }
      },
      {
        "title": "Regex",
        "order": 6,
        "type": "object",
        "properties": {
          "clippings.regex.regex": {
            "default": "(//|#|<!--|;|/\\*|^|^[ \\t]*(-|\\d+.))\\s*($TAGS)",
            "markdownDescription": "Regular expression for matching TODOs. Note: **($TAGS)** will be replaced by the expanded tag list. For some of the extension features to work, **($TAGS)** should be present in the regex, however, the basic functionality should still work if you need to explicitly expand the tag list.",
            "type": "string",
            "minLength": 1,
            "scope": "window"
          },
          "clippings.regex.regexCaseSensitive": {
            "default": true,
            "markdownDescription": "Use a case sensitive regular expression.",
            "type": "boolean",
            "scope": "window"
          },
          "clippings.regex.subTagRegex": {
            "default": "",
            "markdownDescription": "Regular expression for processing the text to the right of the tag, e.g. for extracting a sub tag, or removing unwanted characters.",
            "type": "string",
            "scope": "window"
          },
          "clippings.regex.enableMultiLine": {
            "default": false,
            "markdownDescription": "Force the regex to match over multiple lines. Allows use of `[\\s\\S]` to match anything including newlines.",
            "type": "boolean",
            "scope": "window"
          }
        }
      },
      {
        "title": "Server",
        "order": 7,
        "type": "object",
        "properties": {
          "clippings.server.path": {
            "default": "",
            "markdownDescription": "Path to a Clippings server binary to use instead of the bundled one. `~` and `${workspaceFolder}` are expanded. When set at workspace scope, the client prompts Allow or Deny once per resolved path before using it.",
            "type": "string",
            "scope": "machine-overridable"
          },
          "clippings.server.logLevel": {
            "default": "info",
            "enum": [
              "error",
              "warn",
              "info",
              "debug",
              "trace"
            ],
            "markdownDescription": "Log level for the Clippings server, shown in the Clippings output channel.",
            "type": "string",
            "scope": "window"
          },
          "clippings.trace.server": {
            "default": "off",
            "enum": [
              "off",
              "messages",
              "verbose"
            ],
            "markdownDescription": "Traces the communication between VS Code and the Clippings language server.",
            "type": "string",
            "scope": "window"
          }
        }
      }
    ]
  },
  "scripts": {
    "build": "node build.mjs",
    "watch": "node build.mjs --watch",
    "typecheck": "tsc --noEmit -p .",
    "dev": "cargo build --manifest-path ../Cargo.toml -p clippings",
    "test:unit": "mocha --ui bdd \"out/test/unit/**/*.test.js\"",
    "test:integration": "vscode-test",
    "test": "pnpm dev && pnpm typecheck && pnpm build && pnpm test:unit && pnpm test:integration",
    "package": "vsce package --no-dependencies"
  },
  "packageManager": "pnpm@10.27.0",
  "dependencies": {
    "@primer/octicons": "19.38.0",
    "vscode-languageclient": "10.1.1"
  },
  "devDependencies": {
    "@types/mocha": "10.0.10",
    "@types/node": "22.20.4",
    "@types/vscode": "1.91.0",
    "@vscode/test-cli": "0.0.15",
    "@vscode/test-electron": "3.1.0",
    "@vscode/vsce": "4.0.0",
    "esbuild": "0.28.2",
    "mocha": "11.8.0",
    "typescript": "7.0.2"
  },
  "pnpm": {
    "onlyBuiltDependencies": [
      "esbuild"
    ]
  }
}
```

Create `extension/tsconfig.json`:

```json
{
  "compilerOptions": {
    "target": "es2022",
    "lib": ["es2023"],
    "module": "preserve",
    "moduleResolution": "bundler",
    "types": ["node", "mocha", "vscode"],
    "strict": true,
    "noEmit": true,
    "noUnusedLocals": true,
    "noUnusedParameters": true,
    "noFallthroughCasesInSwitch": true,
    "noImplicitOverride": true,
    "exactOptionalPropertyTypes": false,
    "forceConsistentCasingInFileNames": true,
    "skipLibCheck": true,
    "isolatedModules": true,
    "verbatimModuleSyntax": true
  },
  "include": ["src/**/*.ts"]
}
```

Create `extension/build.mjs`:

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

Create `extension/.vscode-test.mjs`:

```js
// Extension integration tests (spec 12.5). Each run copies the fixture
// workspace and uses a fresh user data directory, so tests can edit files
// and settings without touching the repository or the user's profile.
import { defineConfig } from '@vscode/test-cli';
import { cpSync, mkdtempSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join, resolve } from 'node:path';

const here = import.meta.dirname;
const scratch = mkdtempSync(join(tmpdir(), 'clippings-test-'));
const workspace = join(scratch, 'workspace');
cpSync(resolve(here, '../tests/fixtures/workspace'), workspace, { recursive: true });
const server =
  process.env.CLIPPINGS_SERVER_PATH ?? resolve(here, '../target/debug/clippings' + (process.platform === 'win32' ? '.exe' : ''));

export default defineConfig({
  files: 'out/test/integration/**/*.test.js',
  version: process.env.CLIPPINGS_TEST_VSCODE ?? 'stable',
  extensionDevelopmentPath: here,
  workspaceFolder: workspace,
  launchArgs: ['--disable-extensions', `--user-data-dir=${join(scratch, 'user-data')}`, '--disable-workspace-trust'],
  env: { CLIPPINGS_SERVER_PATH: server, CLIPPINGS_TEST_WORKSPACE: workspace },
  mocha: { ui: 'bdd', timeout: 20000, slow: 2000 },
});
```

Create `extension/src/types/octicons.d.ts`:

```ts
// Types for the parts of @primer/octicons the extension uses; the package
// ships no declarations.
declare module '@primer/octicons' {
  interface Octicon {
    readonly symbol: string;
    toSVG(options?: Record<string, string | number>): string;
  }
  const octicons: Readonly<Record<string, Octicon>>;
  export = octicons;
}
```

Run: `pnpm -C extension install`
Expected: pnpm 10.27.0, the version the `packageManager` field names (Corepack selects it when enabled), installs the pinned dependencies, runs esbuild's install script, which `pnpm.onlyBuiltDependencies` allows, and writes `extension/pnpm-lock.yaml`. Commit that file with this task.

- [ ] **Step 2: Write the failing tests and the fixture workspace**

The fixture is a small workspace with todos in four languages, a multi-line todo, markdown checkboxes, and a `node_modules` folder whose todo must never show.

Create `tests/fixtures/workspace/README.md`:

```markdown
# Clippings extension fixture

A small workspace for the extension tests (spec 12.5). Tests copy it to a
temporary directory before VS Code opens it, so they may edit files freely.
```

Create `tests/fixtures/workspace/docs/plan.md`:

```markdown
# Plan

- [ ] write the guide
- [x] pick a name
```

Create `tests/fixtures/workspace/lib/notes.rs`:

```rust
// TODO first line of a long note
//   continued on a second line
//   and a third
pub fn noop() {}
```

Create `tests/fixtures/workspace/node_modules/leftpad/index.js`:

```js
// TODO never shown: node_modules is never indexed
module.exports = (s) => s;
```

Create `tests/fixtures/workspace/src/app.ts`:

```ts
export function main(): void {
  // TODO(alice) wire up the router
  route();
  // FIXME handle the error path
}

function route(): void {
  // HACK temporary shim until the router lands
}
```

Create `tests/fixtures/workspace/src/util/strings.py`:

```python
def normalise(text):
    # TODO normalise unicode before comparing
    return text.strip()  # BUG(bob) drops leading tabs too
```

Create `extension/src/test/integration/activation.test.ts`:

```ts
import * as assert from 'node:assert/strict';
import * as vscode from 'vscode';

describe('activation', () => {
  it('activates and returns its API', async () => {
    const ext = vscode.extensions.getExtension('clippings-dev.clippings');
    assert.ok(ext, 'extension is installed');
    const api = (await ext.activate()) as { version: string };
    assert.ok(ext.isActive);
    assert.equal(api.version, '0.1.0');
  });
});
```

Create `extension/src/test/unit/manifest.test.ts`:

```ts
import * as assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
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

- [ ] **Step 3: Run the build to verify it fails**

Run: `pnpm -C extension build`
Expected: FAIL with `✘ [ERROR] Could not resolve "src/extension.ts"`: the entry point does not exist yet.

- [ ] **Step 4: Write the extension entry point and the container icon**

The container icon is Clippings' own drawing (ruling 4).

Create `extension/resources/clippings-container.svg`:

```xml
<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="black" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round">
  <path d="M7 4.5H5.5a1 1 0 0 0-1 1v15a1 1 0 0 0 1 1h13a1 1 0 0 0 1-1v-15a1 1 0 0 0-1-1H17"/>
  <path d="M9.5 2.5v5a2.5 2.5 0 0 0 5 0v-4a1.5 1.5 0 0 0-3 0v4"/>
  <path d="M8 13.5l1.5 1.5 3-3"/>
  <path d="M8 18.5h8"/>
  <path d="M14.5 13.5H16"/>
</svg>
```

Create `extension/src/extension.ts`:

```ts
import type * as vscode from 'vscode';

/** What `activate` returns: the extension's API, used by the tests. */
export interface ClippingsApi {
  readonly version: string;
}

export function activate(context: vscode.ExtensionContext): ClippingsApi {
  const manifest = context.extension.packageJSON as { version: string };
  return { version: manifest.version };
}

export function deactivate(): void {}
```

- [ ] **Step 5: Run the tests to verify they pass**

Run: `pnpm -C extension dev && pnpm -C extension typecheck && pnpm -C extension build`
Expected: no errors.

Run: `pnpm -C extension test:unit`
Expected: `5 passing`.

Run: `pnpm -C extension test:integration`
Expected: `1 passing`. A VS Code test window opens, runs the suite and closes.

- [ ] **Step 6: Commit**

```bash
git add -A
git commit -F - <<'EOF'
feat(extension): scaffold the manifest, build and test harness

Co-Authored-By: <your model attribution>
EOF
```

### Task 2: Protocol mirror, configuration object and view state

`protocol.ts` mirrors every wire type of `crates/clippings-core/src/protocol.rs` and the Rust types it uses (spec 6). `config/configuration.ts` builds the configuration object of spec 6.3 as a pure function of the resolved `clippings.*` groups, the true keys of `files.exclude` and `search.exclude`, `explorer.compactFolders` and the view state (ruling 17), and `state/viewState.ts` persists the workspace storage keys of spec 7.8. A conformance test spawns the real `clippings lsp` and checks that each message carries exactly the fields of its TypeScript type, so the mirror cannot drift unnoticed.

**Files:**
- Create: `extension/src/config/configuration.ts`, `extension/src/config/read.ts`, `extension/src/protocol.ts`, `extension/src/state/viewState.ts`
- Test: `extension/src/test/unit/lspProcess.ts`, `extension/src/test/unit/memento.ts`, `extension/src/test/unit/configuration.test.ts`, `extension/src/test/unit/protocol.test.ts`, `extension/src/test/unit/viewState.test.ts`

**Interfaces:**
- Produces, in `config/configuration.ts`: `GROUPS` (const); `Group` (type); `ConfigurationSources` (interface); `function trueKeys(value: unknown): string[]`; `function viewStateOf(persisted: PersistedViewState): ViewState`; `function buildConfiguration(src: ConfigurationSources): Settings`; `EffectiveView` (interface); `function effectiveView(tree: Pick<Tree, keyof EffectiveView>, state: ViewState): EffectiveView`.
- Produces, in `config/read.ts`: `function readSources(viewState: PersistedViewState): ConfigurationSources`; `function readConfiguration(viewState: PersistedViewState): Settings`; `function affectsServer(e: vscode.ConfigurationChangeEvent): boolean`.
- Produces, in `protocol.ts`: `PROTOCOL_VERSION` (`1`), `Method` (the twelve `clippings/*` method names), and the wire types `Position`, `Range`, `RevealBehaviour`, `StatusBarMode`, `ScanMode`, `UseBuiltInExcludes`, `Attributes`, `General`, `Highlights`, `Filtering`, `Tree`, `RegexSettings`, `ViewState`, `Settings`, `InitializationOptions`, `Colour`, `IconDescriptor`, `DecorationStyle`, `StatusBar`, `Badge`, `NodeCommand`, `ViewNode`, `Direction`, `ActiveEditorParams`, `ChildrenParams`, `ChildrenResult`, `FindParams`, `FindResult`, `NavigateParams`, `NavigateResult`, `ExportResult`, `TreeChangedParams`, `StylesParams`, `DecorationsParams`, `StatusParams`. Field names follow the serde attributes of the Rust types.
- Produces, in `state/viewState.ts`: `Memento` (interface); `ViewFlags` (interface); `PersistedViewState` (interface); `WORKSPACE_KEYS` (const); `class ViewStateStore` with `constructor(private readonly memento: Memento)`, `snapshot(): PersistedViewState`, `async setFlags(flags: ViewFlags): Promise<void>`, `async setFilter(text: string | undefined): Promise<void>`, `async setGlobs(includeGlobs: string[], excludeGlobs: string[]): Promise<void>`, `expandedNodes(): Record<string, boolean>`, `async setExpandedNodes(nodes: Record<string, boolean>): Promise<void>`, `epoch(): number`, `async bumpEpoch(): Promise<number>`, `async reset(): Promise<void>`.

- [ ] **Step 1: Write the failing tests**

Create `extension/src/test/unit/lspProcess.ts`:

```ts
// A minimal JSON-RPC client over a spawned `clippings lsp`, for tests that
// check the wire shapes without VS Code.

import { spawn, type ChildProcessWithoutNullStreams } from 'node:child_process';
import { existsSync } from 'node:fs';
import { resolve } from 'node:path';

export function serverPath(): string {
  const path =
    process.env['CLIPPINGS_SERVER_PATH'] ??
    resolve(__dirname, '../../../../target/debug/clippings' + (process.platform === 'win32' ? '.exe' : ''));
  if (!existsSync(path)) throw new Error(`no server binary at ${path}; run \`pnpm dev\``);
  return path;
}

export interface Incoming {
  id?: number | string;
  method?: string;
  params?: unknown;
  result?: unknown;
  error?: { message: string };
}

export class LspProcess {
  private readonly child: ChildProcessWithoutNullStreams;
  private buffer = Buffer.alloc(0);
  private nextId = 1;
  readonly received: Incoming[] = [];
  private waiters: (() => void)[] = [];

  constructor(path = serverPath()) {
    this.child = spawn(path, ['lsp'], { stdio: 'pipe' });
    this.child.stdout.on('data', (chunk: Buffer) => this.onData(chunk));
  }

  private onData(chunk: Buffer): void {
    this.buffer = Buffer.concat([this.buffer, chunk]);
    for (;;) {
      const end = this.buffer.indexOf('\r\n\r\n');
      if (end < 0) return;
      const header = this.buffer.subarray(0, end).toString('ascii');
      const length = Number(/Content-Length: (\d+)/i.exec(header)?.[1]);
      if (this.buffer.length < end + 4 + length) return;
      const body = this.buffer.subarray(end + 4, end + 4 + length).toString('utf8');
      this.buffer = this.buffer.subarray(end + 4 + length);
      this.received.push(JSON.parse(body) as Incoming);
      for (const w of this.waiters.splice(0)) w();
    }
  }

  private write(message: object): void {
    const body = Buffer.from(JSON.stringify({ jsonrpc: '2.0', ...message }), 'utf8');
    this.child.stdin.write(`Content-Length: ${body.length}\r\n\r\n`);
    this.child.stdin.write(body);
  }

  notify(method: string, params: unknown): void {
    this.write({ method, params });
  }

  async request(method: string, params: unknown): Promise<unknown> {
    const id = this.nextId++;
    this.write({ id, method, params });
    const reply = await this.waitFor((m) => m.id === id && m.method === undefined);
    if (reply.error) throw new Error(reply.error.message);
    return reply.result;
  }

  /** Resolves with the first received message, past or future, that matches. */
  async waitFor(match: (m: Incoming) => boolean, timeoutMs = 10_000): Promise<Incoming> {
    const deadline = Date.now() + timeoutMs;
    for (;;) {
      const found = this.received.find(match);
      if (found) return found;
      if (Date.now() > deadline) throw new Error('timed out waiting for a server message');
      await new Promise<void>((resolveWait) => {
        this.waiters.push(resolveWait);
        setTimeout(resolveWait, 100);
      });
    }
  }

  async close(): Promise<void> {
    const exited = new Promise((r) => this.child.once('exit', r));
    try {
      await this.request('shutdown', null);
      this.notify('exit', null);
    } finally {
      this.child.stdin.end();
    }
    await exited;
  }
}
```

Create `extension/src/test/unit/memento.ts`:

```ts
import type { Memento } from '../../state/viewState';

/** An in-memory `vscode.Memento` for unit tests. */
export class MemoryMemento implements Memento {
  readonly values = new Map<string, unknown>();

  get<T>(key: string): T | undefined {
    return this.values.get(key) as T | undefined;
  }

  update(key: string, value: unknown): Promise<void> {
    if (value === undefined) this.values.delete(key);
    else this.values.set(key, structuredClone(value));
    return Promise.resolve();
  }
}
```

Create `extension/src/test/unit/configuration.test.ts`:

```ts
import * as assert from 'node:assert/strict';
import { buildConfiguration, effectiveView, trueKeys, viewStateOf } from '../../config/configuration';
import type { PersistedViewState } from '../../state/viewState';

const empty: PersistedViewState = { currentFilter: '', filtered: false, includeGlobs: [], excludeGlobs: [] };

describe('configuration builder', () => {
  it('keeps only exclude keys whose value is exactly true', () => {
    assert.deepEqual(trueKeys({ '**/.git': true, '**/x': false, '**/y': { when: '$(basename).ts' }, '**/z': 'true' }), [
      '**/.git',
    ]);
    assert.deepEqual(trueKeys(undefined), []);
    assert.deepEqual(trueKeys('nope'), []);
  });

  it('nests the five groups and adds the VS Code settings and view state', () => {
    const settings = buildConfiguration({
      groups: {
        general: { tags: ['TODO'], statusBarClickBehaviour: 'cycle' },
        highlights: { enabled: false },
        filtering: { excludeGlobs: [] },
        tree: { flat: true, buttons: { reveal: true } },
        regex: { regex: '($TAGS)' },
      },
      filesExclude: { '**/.git': true, '**/.DS_Store': false },
      searchExclude: { '**/dist': true },
      explorerCompactFolders: true,
      viewState: { ...empty, flat: false, currentFilter: 'fix', filtered: true, includeGlobs: ['/a/*'] },
    });
    assert.deepEqual(settings.general.tags, ['TODO']);
    assert.equal(settings.highlights.enabled, false);
    assert.equal(settings.tree.flat, true);
    assert.equal(settings.regex.regex, '($TAGS)');
    assert.deepEqual(settings.filesExclude, ['**/.git']);
    assert.deepEqual(settings.searchExclude, ['**/dist']);
    assert.equal(settings.explorerCompactFolders, true);
    assert.deepEqual(settings.viewState, { flat: false, filter: 'fix', includeGlobs: ['/a/*'], excludeGlobs: [] });
    // The object must survive JSON, which is how it reaches the server.
    assert.deepEqual(JSON.parse(JSON.stringify(settings)), settings);
  });

  it('treats a missing group as empty so the server fills defaults', () => {
    const settings = buildConfiguration({
      groups: { general: undefined, highlights: null, filtering: 3, tree: {}, regex: {} },
      filesExclude: undefined,
      searchExclude: undefined,
      explorerCompactFolders: 'yes',
      viewState: empty,
    });
    assert.deepEqual(settings.general, {});
    assert.deepEqual(settings.highlights, {});
    assert.deepEqual(settings.filtering, {});
    assert.equal(settings.explorerCompactFolders, false);
  });

  it('omits view flags the user never clicked', () => {
    const state = viewStateOf({ ...empty, tagsOnly: true });
    assert.deepEqual(state, { tagsOnly: true, filter: '', includeGlobs: [], excludeGlobs: [] });
    assert.ok(!('flat' in state));
  });

  it('sends the filter only while it is active', () => {
    assert.equal(viewStateOf({ ...empty, currentFilter: 'bug', filtered: false }).filter, '');
    assert.equal(viewStateOf({ ...empty, currentFilter: 'bug', filtered: true }).filter, 'bug');
  });

  it('applies clicked view state over the tree settings', () => {
    const tree = { flat: true, tagsOnly: false, expanded: false, groupedByTag: true, groupedBySubTag: false };
    const view = effectiveView(tree, { flat: false, expanded: true, filter: '', includeGlobs: [], excludeGlobs: [] });
    assert.deepEqual(view, { flat: false, tagsOnly: false, expanded: true, groupedByTag: true, groupedBySubTag: false });
  });
});
```

Create `extension/src/test/unit/protocol.test.ts`:

```ts
import * as assert from 'node:assert/strict';
import { mkdtempSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { pathToFileURL } from 'node:url';
import { buildConfiguration } from '../../config/configuration';
import {
  Method,
  PROTOCOL_VERSION,
  type ChildrenResult,
  type DecorationsParams,
  type DecorationStyle,
  type StatusParams,
  type StylesParams,
  type ViewNode,
} from '../../protocol';
import { LspProcess } from './lspProcess';

// Each list must name every field of its TypeScript type (the compiler checks
// that) and exactly the fields the server sends (the tests check that).
const statusFields = {
  instance: 1, scanning: 1, interrupted: 1, needsScan: 1, error: 1, warnings: 1,
  statusBar: 1, badge: 1, viewTitle: 1, hasSubTags: 1, isEmpty: 1,
} satisfies Record<keyof StatusParams, 1>;
const nodeFields = {
  id: 1, label: 1, description: 1, tooltip: 1, icon: 1, hasChildren: 1, defaultExpanded: 1,
  contextValue: 1, resourceUri: 1, command: 1,
} satisfies Record<keyof ViewNode, 1>;
const styleFields = {
  color: 1, backgroundColor: 1, overviewRulerColor: 1, overviewRulerLane: 1, borderRadius: 1, fontStyle: 1,
  fontWeight: 1, textDecoration: 1, isWholeLine: 1, gutterIcon: 1,
} satisfies Record<keyof DecorationStyle, 1>;
const stylesFields = { generation: 1, reset: 1, styles: 1 } satisfies Record<keyof StylesParams, 1>;
const decorationsFields = { uri: 1, version: 1, generation: 1, ranges: 1 } satisfies Record<keyof DecorationsParams, 1>;

function keys(value: unknown): string[] {
  return Object.keys(value as object).sort();
}

describe('protocol mirror against the real server', function () {
  this.timeout(20_000);
  let dir: string;
  let server: LspProcess;

  before(async () => {
    dir = mkdtempSync(join(tmpdir(), 'clippings-protocol-'));
    writeFileSync(join(dir, 'a.ts'), '// TODO first\n// FIXME second\n');
    server = new LspProcess();
    const settings = buildConfiguration({
      groups: { general: {}, highlights: { customHighlight: { TODO: { gutterIcon: true } } }, filtering: {}, tree: {}, regex: {} },
      filesExclude: {},
      searchExclude: {},
      explorerCompactFolders: false,
      viewState: { currentFilter: '', filtered: false, includeGlobs: [], excludeGlobs: [] },
    });
    await server.request('initialize', {
      processId: process.pid,
      workspaceFolders: [{ uri: pathToFileURL(dir).href, name: 'w' }],
      capabilities: {},
      initializationOptions: { protocolVersion: PROTOCOL_VERSION, settings },
    });
    server.notify('initialized', {});
  });

  after(async () => {
    await server.close();
    rmSync(dir, { recursive: true, force: true });
  });

  it('status carries exactly the StatusParams fields', async () => {
    const status = await server.waitFor((m) => m.method === Method.status);
    assert.deepEqual(keys(status.params), keys(statusFields));
  });

  it('children nodes carry exactly the ViewNode fields', async () => {
    await server.waitFor((m) => m.method === Method.treeChanged);
    const result = (await server.request(Method.children, { parent: null })) as ChildrenResult;
    assert.ok(result.nodes.length > 0);
    for (const node of result.nodes) assert.deepEqual(keys(node), keys(nodeFields));
    const root = result.nodes.find((n) => n.contextValue === 'folder');
    assert.ok(root, 'a workspace root node');
    assert.deepEqual(root.icon, { kind: 'codicon', name: 'window', colour: null });
  });

  it('styles and decorations carry their fields and tagged colours', async () => {
    const uri = pathToFileURL(join(dir, 'a.ts')).href;
    server.notify('textDocument/didOpen', {
      textDocument: { uri, languageId: 'typescript', version: 3, text: '// TODO first\n// FIXME second\n' },
    });
    const decorations = await server.waitFor((m) => m.method === Method.decorations);
    const params = decorations.params as DecorationsParams;
    assert.deepEqual(keys(params), keys(decorationsFields));
    assert.equal(params.uri, uri);
    assert.equal(params.version, 3);
    assert.deepEqual(keys(params.ranges), ['FIXME', 'TODO']);
    const styles = server.received
      .filter((m) => m.method === Method.styles)
      .map((m) => m.params as StylesParams)
      .find((s) => !s.reset);
    assert.ok(styles, 'a non-reset styles message');
    assert.deepEqual(keys(styles), keys(stylesFields));
    const todo = styles.styles['TODO'];
    assert.ok(todo);
    assert.deepEqual(keys(todo), keys(styleFields));
    assert.deepEqual(todo.color, { theme: 'editor.background' });
    assert.deepEqual(todo.gutterIcon, { kind: 'check', colour: 'green' });
  });
});
```

Create `extension/src/test/unit/viewState.test.ts`:

```ts
import * as assert from 'node:assert/strict';
import { ViewStateStore } from '../../state/viewState';
import { MemoryMemento } from './memento';

describe('view state store', () => {
  it('starts empty', () => {
    const store = new ViewStateStore(new MemoryMemento());
    assert.deepEqual(store.snapshot(), { currentFilter: '', filtered: false, includeGlobs: [], excludeGlobs: [] });
    assert.deepEqual(store.expandedNodes(), {});
    assert.equal(store.epoch(), 0);
  });

  it('persists flags, filter and globs under todo-tree’s keys', async () => {
    const memento = new MemoryMemento();
    const store = new ViewStateStore(memento);
    await store.setFlags({ flat: true, tagsOnly: false });
    await store.setFilter('fix');
    await store.setGlobs(['/w/src/*'], ['/w/lib/**/*']);
    assert.deepEqual(store.snapshot(), {
      flat: true,
      tagsOnly: false,
      currentFilter: 'fix',
      filtered: true,
      includeGlobs: ['/w/src/*'],
      excludeGlobs: ['/w/lib/**/*'],
    });
    assert.equal(memento.get('currentFilter'), 'fix');
    assert.equal(memento.get('filtered'), true);
    await store.setFilter(undefined);
    assert.equal(store.snapshot().filtered, false);
  });

  it('ignores values of the wrong type', () => {
    const memento = new MemoryMemento();
    memento.values.set('flat', 'yes');
    memento.values.set('includeGlobs', ['a', 3]);
    memento.values.set('epoch', 'x');
    const store = new ViewStateStore(memento);
    assert.equal(store.snapshot().flat, undefined);
    assert.deepEqual(store.snapshot().includeGlobs, ['a']);
    assert.equal(store.epoch(), 0);
  });

  it('reset clears everything but keeps the epoch growing', async () => {
    const memento = new MemoryMemento();
    const store = new ViewStateStore(memento);
    await store.setFlags({ expanded: true });
    await store.setExpandedNodes({ a: true });
    assert.equal(await store.bumpEpoch(), 1);
    await store.reset();
    assert.deepEqual([...memento.values.keys()], ['epoch']);
    assert.equal(await store.bumpEpoch(), 2);
  });
});
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `pnpm -C extension typecheck`
Expected: FAIL. `error TS2307: Cannot find module` for `../../config/configuration`, `../../state/viewState` and `../../protocol`.

- [ ] **Step 3: Write the implementation**

Create `extension/src/config/configuration.ts`:

```ts
// The configuration object the client sends in `initialize` and
// `clippings/configure` (spec 6.3). Pure: the caller reads the settings.

import type { Filtering, General, Highlights, RegexSettings, Settings, Tree, ViewState } from '../protocol';
import type { PersistedViewState } from '../state/viewState';

/** The `clippings.*` groups the server reads, as VS Code resolves them at window level. */
export const GROUPS = ['general', 'highlights', 'filtering', 'tree', 'regex'] as const;
export type Group = (typeof GROUPS)[number];

export interface ConfigurationSources {
  /** `getConfiguration('clippings').get(group)` for each group. */
  groups: Record<Group, unknown>;
  /** `files.exclude`, `search.exclude` and `explorer.compactFolders`. */
  filesExclude: unknown;
  searchExclude: unknown;
  explorerCompactFolders: unknown;
  viewState: PersistedViewState;
}

/** Keys of a VS Code exclude object whose value is exactly `true`. */
export function trueKeys(value: unknown): string[] {
  if (!value || typeof value !== 'object') return [];
  return Object.entries(value as Record<string, unknown>)
    .filter(([, v]) => v === true)
    .map(([k]) => k);
}

function group<T>(value: unknown): T {
  // JSON boundary: VS Code validated the values against the manifest schema
  // and the server reports anything it cannot read as a warning.
  return (value && typeof value === 'object' ? { ...value } : {}) as T;
}

export function viewStateOf(persisted: PersistedViewState): ViewState {
  const state: ViewState = {
    filter: persisted.filtered ? persisted.currentFilter : '',
    includeGlobs: [...persisted.includeGlobs],
    excludeGlobs: [...persisted.excludeGlobs],
  };
  for (const key of ['flat', 'tagsOnly', 'expanded', 'groupedByTag', 'groupedBySubTag'] as const) {
    const value = persisted[key];
    if (value !== undefined) state[key] = value;
  }
  return state;
}

export function buildConfiguration(src: ConfigurationSources): Settings {
  return {
    general: group<General>(src.groups.general),
    highlights: group<Highlights>(src.groups.highlights),
    filtering: group<Filtering>(src.groups.filtering),
    tree: group<Tree>(src.groups.tree),
    regex: group<RegexSettings>(src.groups.regex),
    viewState: viewStateOf(src.viewState),
    filesExclude: trueKeys(src.filesExclude),
    searchExclude: trueKeys(src.searchExclude),
    explorerCompactFolders: src.explorerCompactFolders === true,
  };
}

/** The effective view options: clicked view state over the `tree.*` settings. */
export interface EffectiveView {
  flat: boolean;
  tagsOnly: boolean;
  expanded: boolean;
  groupedByTag: boolean;
  groupedBySubTag: boolean;
}

export function effectiveView(tree: Pick<Tree, keyof EffectiveView>, state: ViewState): EffectiveView {
  return {
    flat: state.flat ?? tree.flat,
    tagsOnly: state.tagsOnly ?? tree.tagsOnly,
    expanded: state.expanded ?? tree.expanded,
    groupedByTag: state.groupedByTag ?? tree.groupedByTag,
    groupedBySubTag: state.groupedBySubTag ?? tree.groupedBySubTag,
  };
}
```

Create `extension/src/config/read.ts`:

```ts
// Reads the configuration sources from VS Code at window level (spec 6.3).

import * as vscode from 'vscode';
import type { Settings } from '../protocol';
import type { PersistedViewState } from '../state/viewState';
import { buildConfiguration, GROUPS, type ConfigurationSources, type Group } from './configuration';

export function readSources(viewState: PersistedViewState): ConfigurationSources {
  const clippings = vscode.workspace.getConfiguration('clippings');
  const groups = Object.fromEntries(GROUPS.map((g) => [g, clippings.get(g)])) as Record<Group, unknown>;
  return {
    groups,
    filesExclude: vscode.workspace.getConfiguration('files').get('exclude'),
    searchExclude: vscode.workspace.getConfiguration('search').get('exclude'),
    explorerCompactFolders: vscode.workspace.getConfiguration('explorer').get('compactFolders'),
    viewState,
  };
}

export function readConfiguration(viewState: PersistedViewState): Settings {
  return buildConfiguration(readSources(viewState));
}

/** Whether a configuration change can change the object the server receives. */
export function affectsServer(e: vscode.ConfigurationChangeEvent): boolean {
  return ['clippings', 'files.exclude', 'search.exclude', 'explorer.compactFolders'].some((s) =>
    e.affectsConfiguration(s),
  );
}
```

Create `extension/src/protocol.ts`:

```ts
// Wire types (spec section 6): a mirror of crates/clippings-core/src/protocol.rs
// and the Rust types it uses (settings.rs, position.rs, styles.rs,
// status.rs, view/render.rs, navigate.rs). Field names follow the serde
// attributes on the Rust side. Keep the two files in step.

export const PROTOCOL_VERSION = 1;

export const Method = {
  configure: 'clippings/configure',
  activeEditor: 'clippings/activeEditor',
  rescan: 'clippings/rescan',
  stopScan: 'clippings/stopScan',
  children: 'clippings/children',
  find: 'clippings/find',
  navigate: 'clippings/navigate',
  export: 'clippings/export',
  treeChanged: 'clippings/treeChanged',
  styles: 'clippings/styles',
  decorations: 'clippings/decorations',
  status: 'clippings/status',
} as const;

// ---- position.rs ----

/** 0-based line and 0-based UTF-16 column. */
export interface Position {
  line: number;
  character: number;
}

/** A half-open range of positions. */
export interface Range {
  start: Position;
  end: Position;
}

// ---- settings.rs and config.rs ----

export type RevealBehaviour = 'start of line' | 'start of todo' | 'end of todo';
export type StatusBarMode = 'none' | 'total' | 'tags' | 'top three' | 'current file';
export type ScanMode = 'workspace' | 'workspace only' | 'open files' | 'current file';
export type UseBuiltInExcludes = 'none' | 'file excludes' | 'search excludes' | 'file and search excludes';

/** Per-tag highlight attributes (`customHighlight.<key>` and `defaultHighlight`). */
export interface Attributes {
  type?: string;
  foreground?: string;
  background?: string;
  opacity?: number;
  rulerColour?: string;
  rulerOpacity?: number;
  /** A lane number or one of `none`, `left`, `center`, `right`, `full`. */
  rulerLane?: number | string;
  borderRadius?: string;
  fontStyle?: string;
  fontWeight?: string;
  textDecoration?: string;
  gutterIcon?: boolean;
  icon?: string;
  iconColour?: string;
  /** US spelling, checked before `iconColour` as in todo-tree. */
  iconColor?: string;
  hideFromTree?: boolean;
  hideFromStatusBar?: boolean;
  hideFromActivityBar?: boolean;
}

export interface General {
  automaticGitRefreshInterval: number;
  periodicRefreshInterval: number;
  revealBehaviour: RevealBehaviour;
  exportPath: string;
  rootFolder: string;
  schemes: string[];
  statusBar: StatusBarMode;
  showIconsInsteadOfTagsInStatusBar: boolean;
  tagGroups: Record<string, string[]>;
  tags: string[];
  showActivityBarBadge: boolean;
}

export interface Highlights {
  customHighlight: Record<string, Attributes>;
  defaultHighlight: Attributes;
  enabled: boolean;
  highlightDelay: number;
  useColourScheme: boolean;
  foregroundColourScheme: string[];
  backgroundColourScheme: string[];
}

export interface Filtering {
  excludedWorkspaces: string[];
  excludeGlobs: string[];
  ignoreGitSubmodules: boolean;
  includedWorkspaces: string[];
  includeGlobs: string[];
  includeHiddenFiles: boolean;
  useBuiltInExcludes: UseBuiltInExcludes;
  builtInExcludes: string[];
}

export interface Tree {
  autoRefresh: boolean;
  disableCompactFolders: boolean;
  expanded: boolean;
  filterCaseSensitive: boolean;
  flat: boolean;
  groupedByTag: boolean;
  groupedBySubTag: boolean;
  hideIconsWhenGroupedByTag: boolean;
  hideTreeWhenEmpty: boolean;
  labelFormat: string;
  scanAtStartup: boolean;
  scanMode: ScanMode;
  showBadges: boolean;
  showCountsInTree: boolean;
  showCurrentScanMode: boolean;
  subTagClickUrl: string;
  sortTagsOnlyViewAlphabetically: boolean;
  sort: boolean;
  tagsOnly: boolean;
  tooltipFormat: string;
  trackFile: boolean;
}

export interface RegexSettings {
  regex: string;
  regexCaseSensitive: boolean;
  subTagRegex: string;
  enableMultiLine: boolean;
}

/**
 * View state the user set by clicking view buttons, kept in the client's
 * workspace storage. A set value overrides the matching `tree.*` setting;
 * an absent one leaves the setting in force.
 */
export interface ViewState {
  flat?: boolean;
  tagsOnly?: boolean;
  expanded?: boolean;
  groupedByTag?: boolean;
  groupedBySubTag?: boolean;
  /** The tree filter text; empty means no filter. */
  filter: string;
  /** Temporary include globs from the folder context menu and scopes. */
  includeGlobs: string[];
  /** Temporary exclude globs from the folder and file context menus and scopes. */
  excludeGlobs: string[];
}

/**
 * The full resolved configuration (spec 6.3). The five groups mirror the
 * `clippings.*` settings; the client also sends the client-only keys in them
 * (`tree.buttons`, `filtering.scopes`, `general.statusBarClickBehaviour`),
 * which the server ignores.
 */
export interface Settings {
  general: General;
  highlights: Highlights;
  filtering: Filtering;
  tree: Tree;
  regex: RegexSettings;
  viewState: ViewState;
  /** Keys of `files.exclude` whose value is exactly `true`. */
  filesExclude: string[];
  /** Keys of `search.exclude` whose value is exactly `true`. */
  searchExclude: string[];
  explorerCompactFolders: boolean;
}

export interface InitializationOptions {
  protocolVersion: number;
  settings: Settings;
}

// ---- styles.rs ----

/** A theme colour id or a CSS colour string. */
export type Colour = { theme: string } | { css: string };

/** What the client renders as an icon. */
export type IconDescriptor =
  | { kind: 'codicon'; name: string; colour: string | null }
  | { kind: 'octicon'; name: string; colour: string }
  | { kind: 'todoTree'; filled: boolean; colour: string }
  | { kind: 'check'; colour: string }
  | { kind: 'default' }
  | { kind: 'folder' }
  | { kind: 'file' };

export interface DecorationStyle {
  color: Colour | null;
  backgroundColor: Colour | null;
  overviewRulerColor: Colour | null;
  overviewRulerLane: number | null;
  borderRadius: string;
  fontStyle: string;
  fontWeight: string;
  textDecoration: string;
  isWholeLine: boolean;
  gutterIcon: IconDescriptor | null;
}

// ---- status.rs ----

export interface StatusBar {
  text: string;
  tooltip: string;
  visible: boolean;
}

export interface Badge {
  value: number;
  tooltip: string;
}

// ---- view/render.rs ----

export type NodeCommand =
  | { kind: 'reveal'; uri: string; position: Position }
  | { kind: 'openUrl'; url: string };

export interface ViewNode {
  id: string;
  label: string;
  description: string | null;
  tooltip: string | null;
  icon: IconDescriptor | null;
  hasChildren: boolean;
  defaultExpanded: boolean;
  contextValue: string | null;
  resourceUri: string | null;
  command: NodeCommand | null;
}

// ---- navigate.rs ----

export type Direction = 'next' | 'previous';

// ---- custom messages ----

export interface ActiveEditorParams {
  uri: string | null;
}

export interface ChildrenParams {
  parent: string | null;
}

export interface ChildrenResult {
  nodes: ViewNode[];
}

export interface FindParams {
  uri: string;
  line: number | null;
}

export interface FindResult {
  /** Each path runs from a top-level node down to a matching node. */
  paths: ViewNode[][];
}

export interface NavigateParams {
  uri: string;
  positions: Position[];
  direction: Direction;
}

export interface NavigateResult {
  ranges: Range[] | null;
}

export interface ExportResult {
  path: string;
  content: string;
}

export interface TreeChangedParams {
  /** Parent IDs to refresh; `null` is the root. */
  refresh: (string | null)[];
}

export interface StylesParams {
  generation: number;
  reset: boolean;
  styles: Record<string, DecorationStyle>;
}

export interface DecorationsParams {
  uri: string;
  version: number;
  generation: number;
  ranges: Record<string, Range[]>;
}

export interface StatusParams {
  instance: string;
  scanning: boolean;
  interrupted: boolean;
  needsScan: boolean;
  error: string | null;
  warnings: string[];
  statusBar: StatusBar;
  badge: Badge;
  viewTitle: string;
  hasSubTags: boolean;
  isEmpty: boolean;
}
```

Create `extension/src/state/viewState.ts`:

```ts
// Persisted view state in workspace storage (spec 7.8): the view buttons
// the user clicked, the tree filter, temporary globs, the expansion map and
// the tree item epoch.

/** The subset of `vscode.Memento` the store needs. */
export interface Memento {
  get<T>(key: string): T | undefined;
  update(key: string, value: unknown): PromiseLike<void>;
}

/** View buttons whose clicked value overrides the matching `tree.*` setting. */
export interface ViewFlags {
  flat?: boolean;
  tagsOnly?: boolean;
  expanded?: boolean;
  groupedByTag?: boolean;
  groupedBySubTag?: boolean;
}

export interface PersistedViewState extends ViewFlags {
  currentFilter: string;
  filtered: boolean;
  includeGlobs: string[];
  excludeGlobs: string[];
}

const FLAG_KEYS = ['flat', 'tagsOnly', 'expanded', 'groupedByTag', 'groupedBySubTag'] as const;

/** Every workspace storage key the extension owns. */
export const WORKSPACE_KEYS = [
  ...FLAG_KEYS,
  'currentFilter',
  'filtered',
  'includeGlobs',
  'excludeGlobs',
  'expandedNodes',
  'epoch',
] as const;

function strings(value: unknown): string[] {
  return Array.isArray(value) ? value.filter((v): v is string => typeof v === 'string') : [];
}

function flag(value: unknown): boolean | undefined {
  return typeof value === 'boolean' ? value : undefined;
}

export class ViewStateStore {
  constructor(private readonly memento: Memento) {}

  snapshot(): PersistedViewState {
    const m = this.memento;
    const state: PersistedViewState = {
      currentFilter: typeof m.get('currentFilter') === 'string' ? (m.get<string>('currentFilter') ?? '') : '',
      filtered: m.get('filtered') === true,
      includeGlobs: strings(m.get('includeGlobs')),
      excludeGlobs: strings(m.get('excludeGlobs')),
    };
    for (const key of FLAG_KEYS) {
      const value = flag(m.get(key));
      if (value !== undefined) state[key] = value;
    }
    return state;
  }

  async setFlags(flags: ViewFlags): Promise<void> {
    for (const key of FLAG_KEYS) {
      if (key in flags) await this.memento.update(key, flags[key]);
    }
  }

  /** Sets the tree filter; empty or undefined clears it. */
  async setFilter(text: string | undefined): Promise<void> {
    await this.memento.update('currentFilter', text ?? '');
    await this.memento.update('filtered', !!text);
  }

  async setGlobs(includeGlobs: string[], excludeGlobs: string[]): Promise<void> {
    await this.memento.update('includeGlobs', includeGlobs);
    await this.memento.update('excludeGlobs', excludeGlobs);
  }

  /** Expansion state by node ID. */
  expandedNodes(): Record<string, boolean> {
    const value = this.memento.get<unknown>('expandedNodes');
    return value && typeof value === 'object' ? { ...(value as Record<string, boolean>) } : {};
  }

  async setExpandedNodes(nodes: Record<string, boolean>): Promise<void> {
    await this.memento.update('expandedNodes', nodes);
  }

  epoch(): number {
    const value = this.memento.get<unknown>('epoch');
    return typeof value === 'number' && Number.isSafeInteger(value) ? value : 0;
  }

  async bumpEpoch(): Promise<number> {
    const next = this.epoch() + 1;
    await this.memento.update('epoch', next);
    return next;
  }

  /** Clears everything but the epoch, which only ever grows. */
  async reset(): Promise<void> {
    for (const key of WORKSPACE_KEYS) {
      if (key !== 'epoch') await this.memento.update(key, undefined);
    }
  }
}
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `pnpm -C extension dev && pnpm -C extension typecheck && pnpm -C extension build`
Expected: no errors.

Run: `pnpm -C extension test:unit`
Expected: `18 passing`.

Run: `pnpm -C extension test:integration`
Expected: `1 passing`. A VS Code test window opens, runs the suite and closes.

- [ ] **Step 5: Commit**

```bash
git add -A
git commit -F - <<'EOF'
feat(extension): mirror the protocol and build the configuration object

Co-Authored-By: <your model attribution>
EOF
```

### Task 3: Server resolution and probe

Spec 7.4's resolution order: `CLIPPINGS_SERVER_PATH`; `clippings.server.path` with `~` and `${workspaceFolder}` expanded, asking Allow or Deny once per path when the value comes from workspace settings and remembering the answer in `serverPathDecisions` (spec 7.8); the bundled `bin/clippings` next to `bin/platform.ok`; and `clippings` on PATH in development builds. Each candidate runs `clippings probe` with a 15 second timeout, and when every candidate fails one error lists each with its reason. In an untrusted workspace a workspace `server.path` is ignored without a prompt (ruling 23, Review Focus 4). The logic is pure and tested with fake probe scripts; `server/locate.ts` wires it to VS Code.

**Files:**
- Create: `extension/src/server/approvals.ts`, `extension/src/server/candidates.ts`, `extension/src/server/locate.ts`, `extension/src/server/probe.ts`, `extension/src/server/resolve.ts`
- Test: `extension/src/test/unit/serverResolution.test.ts`

**Interfaces:**
- Consumes: `protocol.ts` (Task 2): `PROTOCOL_VERSION`; `state/viewState.ts` (Task 2): `Memento`.
- Produces, in `server/approvals.ts`: `Decision` (type); `DECISIONS_KEY` (const); `class ServerPathApprovals` with `constructor(private readonly globalState: Memento, private readonly ask: (path: string) => PromiseLike<Decision | undefined>)`, `async approve(path: string): Promise<boolean>`.
- Produces, in `server/candidates.ts`: `CandidateSource` (type); `Candidate` (interface); `ServerPathSetting` (interface); `ServerPathValues` (interface); `function serverPathSetting(values: ServerPathValues | undefined, trusted: boolean): ServerPathSetting | undefined`; `CandidateInputs` (interface); `function binaryName(platform: NodeJS.Platform): string`; `function expandServerPath(value: string, home: string, workspaceFolder: string | undefined): string`; `function findOnPath(name: string, env: Readonly<Record<string, string | undefined>>, exists: (path: string) => boolean): string | undefined`; `function candidates(inputs: CandidateInputs): Candidate[]`; `function describe(candidate: Candidate): string`.
- Produces, in `server/locate.ts`: `function locateServer(context: vscode.ExtensionContext, log: (message: string) => void): Promise<Resolved>`.
- Produces, in `server/probe.ts`: `PROBE_TIMEOUT_MS` (const); `ProbeInfo` (interface); `class ProbeError extends Error`; `function probe(path: string, timeoutMs = PROBE_TIMEOUT_MS): Promise<ProbeInfo>`; `function ensureExecutable(path: string): void`.
- Produces, in `server/resolve.ts`: `Resolved` (interface); `CandidateFailure` (interface); `class ResolutionError extends Error` with `constructor(readonly failures: CandidateFailure[])`; `ResolveDeps` (interface); `async function resolveServer(list: Candidate[], deps: ResolveDeps): Promise<Resolved>`.

- [ ] **Step 1: Write the failing tests**

Create `extension/src/test/unit/serverResolution.test.ts`:

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
      exists: (p) => ['/ext/bin/clippings', '/ext/bin/platform.ok', join('/b', 'clippings')].includes(p),
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

- [ ] **Step 2: Run the tests to verify they fail**

Run: `pnpm -C extension typecheck`
Expected: FAIL. `error TS2307: Cannot find module` for `../../server/approvals`, `../../server/candidates`, `../../server/probe` and `../../server/resolve`.

- [ ] **Step 3: Write the implementation**

Create `extension/src/server/approvals.ts`:

```ts
// Allow or Deny for a `server.path` set in workspace settings (spec 7.4),
// remembered per resolved path in global storage (spec 7.8).

import type { Memento } from '../state/viewState';

export type Decision = 'allow' | 'deny';
export const DECISIONS_KEY = 'serverPathDecisions';

export class ServerPathApprovals {
  constructor(
    private readonly globalState: Memento,
    /** Asks the user; `undefined` when they dismiss the prompt. */
    private readonly ask: (path: string) => PromiseLike<Decision | undefined>,
  ) {}

  private decisions(): Record<string, Decision> {
    const value = this.globalState.get<unknown>(DECISIONS_KEY);
    return value && typeof value === 'object' ? { ...(value as Record<string, Decision>) } : {};
  }

  async approve(path: string): Promise<boolean> {
    const known = this.decisions()[path];
    if (known) return known === 'allow';
    const decision = await this.ask(path);
    // A dismissed prompt denies this time and asks again next time.
    if (decision) await this.globalState.update(DECISIONS_KEY, { ...this.decisions(), [path]: decision });
    return decision === 'allow';
  }
}
```

Create `extension/src/server/candidates.ts`:

```ts
// Server resolution order (spec 7.4). Pure: the caller supplies the
// environment, the setting and the filesystem checks.

import { delimiter, join } from 'node:path';

export type CandidateSource = 'environment' | 'setting' | 'bundled' | 'path';

export interface Candidate {
  source: CandidateSource;
  path: string;
  /** For `setting`: the value came from workspace settings, so it needs the user's approval. */
  fromWorkspace?: boolean;
  /** Set when the candidate cannot even be tried, such as a missing bundled binary. */
  unavailable?: string;
}

export interface ServerPathSetting {
  value: string;
  fromWorkspace: boolean;
}

/** The values of `clippings.server.path` at each scope, as `inspect` reports them. */
export interface ServerPathValues {
  globalValue?: string | undefined;
  workspaceValue?: string | undefined;
  workspaceFolderValue?: string | undefined;
}

/**
 * The `server.path` value to try. A workspace value counts only in a trusted
 * workspace (spec 7.1): in an untrusted one it is ignored without a prompt,
 * and the user's own value applies.
 */
export function serverPathSetting(
  values: ServerPathValues | undefined,
  trusted: boolean,
): ServerPathSetting | undefined {
  const workspaceValue = values?.workspaceFolderValue ?? values?.workspaceValue;
  if (workspaceValue && trusted) return { value: workspaceValue, fromWorkspace: true };
  if (values?.globalValue) return { value: values.globalValue, fromWorkspace: false };
  return undefined;
}

export interface CandidateInputs {
  env: Readonly<Record<string, string | undefined>>;
  setting: ServerPathSetting | undefined;
  home: string;
  workspaceFolder: string | undefined;
  extensionPath: string;
  platform: NodeJS.Platform;
  /** Development and test builds also try `clippings` on PATH. */
  development: boolean;
  exists(path: string): boolean;
}

export function binaryName(platform: NodeJS.Platform): string {
  return platform === 'win32' ? 'clippings.exe' : 'clippings';
}

/** Expands a leading `~` and `${workspaceFolder}` in `server.path`. */
export function expandServerPath(value: string, home: string, workspaceFolder: string | undefined): string {
  let path = value.trim();
  if (path === '~' || path.startsWith('~/') || path.startsWith('~\\')) path = home + path.slice(1);
  return path.replaceAll('${workspaceFolder}', workspaceFolder ?? '');
}

/** The first `name` on PATH, like `which`. */
export function findOnPath(
  name: string,
  env: Readonly<Record<string, string | undefined>>,
  exists: (path: string) => boolean,
): string | undefined {
  const dirs = (env['PATH'] ?? env['Path'] ?? '').split(delimiter).filter((d) => d.length > 0);
  for (const dir of dirs) {
    const path = join(dir, name);
    if (exists(path)) return path;
  }
  return undefined;
}

export function candidates(inputs: CandidateInputs): Candidate[] {
  const list: Candidate[] = [];
  const fromEnv = inputs.env['CLIPPINGS_SERVER_PATH'];
  if (fromEnv) list.push({ source: 'environment', path: fromEnv });
  if (inputs.setting && inputs.setting.value.trim()) {
    list.push({
      source: 'setting',
      path: expandServerPath(inputs.setting.value, inputs.home, inputs.workspaceFolder),
      fromWorkspace: inputs.setting.fromWorkspace,
    });
  }
  const name = binaryName(inputs.platform);
  const bundled = join(inputs.extensionPath, 'bin', name);
  const marker = join(inputs.extensionPath, 'bin', 'platform.ok');
  list.push(
    inputs.exists(bundled) && inputs.exists(marker)
      ? { source: 'bundled', path: bundled }
      : { source: 'bundled', path: bundled, unavailable: 'not bundled in this package' },
  );
  if (inputs.development) {
    const onPath = findOnPath(name, inputs.env, inputs.exists);
    list.push(onPath ? { source: 'path', path: onPath } : { source: 'path', path: name, unavailable: 'not on PATH' });
  }
  return list;
}

export function describe(candidate: Candidate): string {
  switch (candidate.source) {
    case 'environment':
      return `CLIPPINGS_SERVER_PATH (${candidate.path})`;
    case 'setting':
      return `clippings.server.path (${candidate.path})`;
    case 'bundled':
      return `bundled server (${candidate.path})`;
    case 'path':
      return `${candidate.path} on PATH`;
  }
}
```

Create `extension/src/server/locate.ts`:

```ts
// Finds the server binary for this window (spec 7.4).

import { existsSync } from 'node:fs';
import { homedir } from 'node:os';
import * as vscode from 'vscode';
import { ServerPathApprovals, type Decision } from './approvals';
import { candidates, serverPathSetting } from './candidates';
import { ensureExecutable, probe } from './probe';
import { resolveServer, type Resolved } from './resolve';

async function ask(path: string): Promise<Decision | undefined> {
  const choice = await vscode.window.showWarningMessage(
    `This workspace sets clippings.server.path to ${path}. Allow Clippings to run it?`,
    { modal: true, detail: 'Clippings remembers your choice for this path.' },
    'Allow',
    'Deny',
  );
  return choice === 'Allow' ? 'allow' : choice === 'Deny' ? 'deny' : undefined;
}

export function locateServer(context: vscode.ExtensionContext, log: (message: string) => void): Promise<Resolved> {
  const list = candidates({
    env: process.env,
    setting: serverPathSetting(
      vscode.workspace.getConfiguration('clippings').inspect<string>('server.path'),
      vscode.workspace.isTrusted,
    ),
    home: homedir(),
    workspaceFolder: vscode.workspace.workspaceFolders?.[0]?.uri.fsPath,
    extensionPath: context.extensionPath,
    platform: process.platform,
    development: context.extensionMode !== vscode.ExtensionMode.Production,
    exists: existsSync,
  });
  const approvals = new ServerPathApprovals(context.globalState, ask);
  return resolveServer(list, {
    probe: (path) => {
      if (list.some((c) => c.source === 'bundled' && c.path === path)) ensureExecutable(path);
      return probe(path);
    },
    approve: (path) => approvals.approve(path),
    log,
  });
}
```

Create `extension/src/server/probe.ts`:

```ts
// `clippings probe` (spec 7.4): checks that a candidate binary runs and
// speaks this client's protocol version.

import { execFile } from 'node:child_process';
import { chmodSync, statSync } from 'node:fs';
import { PROTOCOL_VERSION } from '../protocol';

export const PROBE_TIMEOUT_MS = 15_000;

export interface ProbeInfo {
  version: string;
  target: string;
  protocolVersion: number;
}

/** A candidate that failed the probe, with a reason for the aggregated error. */
export class ProbeError extends Error {}

function parse(stdout: string): ProbeInfo {
  let value: unknown;
  try {
    value = JSON.parse(stdout);
  } catch {
    throw new ProbeError(`printed something other than JSON: ${JSON.stringify(stdout.slice(0, 80))}`);
  }
  const info = value as Partial<ProbeInfo> | null;
  if (
    !info ||
    typeof info.version !== 'string' ||
    typeof info.target !== 'string' ||
    typeof info.protocolVersion !== 'number'
  ) {
    throw new ProbeError(`printed unexpected JSON: ${stdout.trim().slice(0, 80)}`);
  }
  return { version: info.version, target: info.target, protocolVersion: info.protocolVersion };
}

export function probe(path: string, timeoutMs = PROBE_TIMEOUT_MS): Promise<ProbeInfo> {
  return new Promise((resolve, reject) => {
    execFile(path, ['probe'], { timeout: timeoutMs, windowsHide: true }, (error, stdout, stderr) => {
      if (error) {
        const e = error as NodeJS.ErrnoException & { killed?: boolean; code?: number | string };
        if (e.killed) return reject(new ProbeError(`timed out after ${timeoutMs / 1000} s`));
        if (typeof e.code === 'string') return reject(new ProbeError(`did not start (${e.code})`));
        const detail = stderr.trim().split('\n').pop() ?? '';
        return reject(new ProbeError(`exited with code ${e.code ?? 'unknown'}${detail ? `: ${detail}` : ''}`));
      }
      try {
        const info = parse(stdout);
        if (info.protocolVersion !== PROTOCOL_VERSION) {
          throw new ProbeError(
            `speaks protocol version ${info.protocolVersion}, but this extension needs ${PROTOCOL_VERSION}`,
          );
        }
        resolve(info);
      } catch (err) {
        reject(err);
      }
    });
  });
}

/** Makes a bundled binary executable if packaging lost the mode bits. */
export function ensureExecutable(path: string): void {
  if (process.platform === 'win32') return;
  const mode = statSync(path).mode;
  if ((mode & 0o111) === 0) chmodSync(path, mode | 0o755);
}
```

Create `extension/src/server/resolve.ts`:

```ts
// Tries each candidate in order and returns the first that passes the probe
// (spec 7.4), or fails with every candidate's reason.

import { describe, type Candidate } from './candidates';
import type { ProbeInfo } from './probe';

export interface Resolved {
  candidate: Candidate;
  info: ProbeInfo;
}

export interface CandidateFailure {
  candidate: Candidate;
  reason: string;
}

export class ResolutionError extends Error {
  constructor(readonly failures: CandidateFailure[]) {
    super(
      `Clippings could not start its server. ${failures
        .map((f) => `${describe(f.candidate)}: ${f.reason}`)
        .join('; ')}.`,
    );
  }
}

export interface ResolveDeps {
  probe(path: string): Promise<ProbeInfo>;
  /** Asks, or recalls, whether a workspace-set `server.path` may run. */
  approve(path: string): Promise<boolean>;
  log(message: string): void;
}

export async function resolveServer(list: Candidate[], deps: ResolveDeps): Promise<Resolved> {
  const failures: CandidateFailure[] = [];
  for (const candidate of list) {
    if (candidate.unavailable) {
      failures.push({ candidate, reason: candidate.unavailable });
      continue;
    }
    if (candidate.fromWorkspace && !(await deps.approve(candidate.path))) {
      failures.push({ candidate, reason: 'not allowed to run from workspace settings' });
      continue;
    }
    try {
      const info = await deps.probe(candidate.path);
      deps.log(`Using ${describe(candidate)}: clippings ${info.version} (${info.target})`);
      return { candidate, info };
    } catch (err) {
      const reason = err instanceof Error ? err.message : String(err);
      deps.log(`Skipping ${describe(candidate)}: ${reason}`);
      failures.push({ candidate, reason });
    }
  }
  throw new ResolutionError(failures);
}
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `pnpm -C extension dev && pnpm -C extension typecheck && pnpm -C extension build`
Expected: no errors.

Run: `pnpm -C extension test:unit`
Expected: `31 passing`.

Run: `pnpm -C extension test:integration`
Expected: `1 passing`. A VS Code test window opens, runs the suite and closes.

- [ ] **Step 5: Commit**

```bash
git add -A
git commit -F - <<'EOF'
feat(extension): resolve and probe the server binary

Co-Authored-By: <your model attribution>
EOF
```

### Task 4: Server logs on stderr

The server's `tracing` calls went nowhere, because the binary installed no subscriber, so the output channel of spec 10.3 stayed empty (ruling 18). `main` now installs a `tracing-subscriber` fmt subscriber writing to stderr, without ANSI colours, at the level in `CLIPPINGS_LOG` (`info` by default), and `main_loop::run` logs one line after the handshake. The test runs a short real `clippings lsp` session at two levels and reads its stderr.

**Files:**
- Modify: `Cargo.toml`, `crates/clippings/Cargo.toml`, `Cargo.lock` (diff), `crates/clippings-core/src/server/main_loop.rs`, `crates/clippings/src/main.rs`
- Test: `crates/clippings/tests/lsp.rs` (modified)

**Interfaces:**
- Produces: `clippings lsp` (and every other subcommand) logs to stderr at the level named by the `CLIPPINGS_LOG` environment variable: `error`, `warn`, `info`, `debug`, `trace` or `off`, `info` when unset or unreadable. Task 6 sets it from `clippings.server.logLevel`.
- Produces: the startup line `clippings <version> serving <n> workspace folder(s), protocol version 1` at `info`.

- [ ] **Step 1: Write the failing test**

Replace `crates/clippings/tests/lsp.rs` with:

```rust
//! Spawns `clippings lsp` and `clippings watch` and talks to them.

use lsp_server::{Message, Notification, Request, RequestId};
use serde_json::{json, Value};
use std::io::{BufRead, BufReader, Write};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

fn bin() -> Command {
    Command::new(env!("CARGO_BIN_EXE_clippings"))
}

#[test]
fn lsp_over_stdio_initializes_scans_and_shuts_down() {
    let t = tempfile::tempdir().unwrap();
    let root = dunce::canonicalize(t.path()).unwrap();
    std::fs::write(root.join("a.ts"), "// TODO over stdio\n").unwrap();
    let uri = clippings_core::uri::file_uri(&root);
    let mut child = bin()
        .arg("lsp")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    let mut stdin = child.stdin.take().unwrap();
    let mut stdout = BufReader::new(child.stdout.take().unwrap());
    let mut send = |m: Message| m.write(&mut stdin).unwrap();

    send(Request::new(RequestId::from(1), "initialize".into(), json!({
        "workspaceFolders": [{ "uri": uri, "name": "w" }],
        "capabilities": {},
        "initializationOptions": { "protocolVersion": 1, "settings": { "tree": { "showCurrentScanMode": false } } }
    })).into());
    let init = Message::read(&mut stdout).unwrap().unwrap();
    assert!(
        matches!(init, Message::Response(ref r) if r.response_result.is_ok()),
        "{init:?}"
    );
    send(Notification::new("initialized".into(), json!({})).into());

    // Wait until a scan finishes, then ask for the top level.
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        assert!(Instant::now() < deadline, "no finished scan");
        if let Some(Message::Notification(n)) = Message::read(&mut stdout).unwrap() {
            if n.method == "clippings/status" && n.params["scanning"] == false {
                break;
            }
        }
    }
    std::thread::sleep(Duration::from_millis(200));
    send(
        Request::new(
            RequestId::from(2),
            "clippings/children".into(),
            json!({ "parent": null }),
        )
        .into(),
    );
    let nodes = loop {
        if let Some(Message::Response(r)) = Message::read(&mut stdout).unwrap() {
            break r.response_result.unwrap();
        }
    };
    assert_eq!(nodes["nodes"].as_array().unwrap().len(), 1);

    send(Request::new(RequestId::from(3), "shutdown".into(), Value::Null).into());
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        assert!(Instant::now() < deadline, "no shutdown response");
        if let Some(Message::Response(r)) = Message::read(&mut stdout).unwrap() {
            assert_eq!(r.id, RequestId::from(3));
            break;
        }
    }
    send(Notification::new("exit".into(), Value::Null).into());
    drop(stdin);
    assert!(child.wait().unwrap().success());
}

#[test]
fn lsp_rejects_a_mismatched_protocol_version() {
    let mut child = bin()
        .arg("lsp")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    let mut stdin = child.stdin.take().unwrap();
    let mut stdout = BufReader::new(child.stdout.take().unwrap());
    let init: Message = Request::new(
        RequestId::from(1),
        "initialize".into(),
        json!({ "capabilities": {}, "initializationOptions": { "protocolVersion": 0 } }),
    )
    .into();
    init.write(&mut stdin).unwrap();
    let resp = Message::read(&mut stdout).unwrap().unwrap();
    assert!(matches!(resp, Message::Response(ref r) if r.response_result.is_err()));
    drop(stdin);
    assert!(!child.wait().unwrap().success());
}

#[test]
fn watch_prints_changes() {
    let t = tempfile::tempdir().unwrap();
    let root = dunce::canonicalize(t.path()).unwrap();
    std::fs::write(root.join("a.ts"), "// TODO first\n").unwrap();
    let mut child = bin()
        .arg("watch")
        .arg(&root)
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    let mut lines = BufReader::new(child.stdout.take().unwrap()).lines();
    let ready: Value = serde_json::from_str(&lines.next().unwrap().unwrap()).unwrap();
    assert_eq!(ready, json!({ "event": "ready", "files": 1, "todos": 1 }));
    std::thread::sleep(Duration::from_millis(300));
    std::fs::write(root.join("b.ts"), "// FIXME second\n").unwrap();
    let mut seen = false;
    for line in lines.by_ref().take(20) {
        let v: Value = serde_json::from_str(&line.unwrap()).unwrap();
        if v["event"] == "updated" && v["path"] == "b.ts" {
            assert_eq!(v["todos"][0]["tag"], "FIXME");
            seen = true;
            break;
        }
    }
    child.kill().unwrap();
    let _ = child.wait();
    assert!(seen);
    let _ = std::io::stdout().flush();
}

#[test]
fn watch_drops_events_under_a_never_index_directory() {
    let t = tempfile::tempdir().unwrap();
    let root = dunce::canonicalize(t.path()).unwrap();
    std::fs::create_dir_all(root.join("node_modules/pkg")).unwrap();
    let mut child = bin()
        .arg("watch")
        .arg(&root)
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    let mut lines = BufReader::new(child.stdout.take().unwrap()).lines();
    let ready: Value = serde_json::from_str(&lines.next().unwrap().unwrap()).unwrap();
    assert_eq!(ready["event"], "ready");
    std::thread::sleep(Duration::from_millis(300));

    // A created, then changed, then deleted file under node_modules, and
    // finally the whole node_modules directory removed with it.
    let nested = root.join("node_modules/pkg/a.ts");
    std::fs::write(&nested, "// TODO in node_modules\n").unwrap();
    std::thread::sleep(Duration::from_millis(150));
    std::fs::write(&nested, "// FIXME in node_modules\n").unwrap();
    std::thread::sleep(Duration::from_millis(150));
    std::fs::remove_file(&nested).unwrap();
    std::thread::sleep(Duration::from_millis(150));
    std::fs::remove_dir_all(root.join("node_modules")).unwrap();
    std::thread::sleep(Duration::from_millis(150));

    // The barrier: a change to a top-level file. Once its `updated` line
    // arrives, every notify event queued before it has already been
    // processed and either printed or correctly dropped, so the test fails
    // deterministically instead of timing out.
    std::fs::write(root.join("sentinel.ts"), "// TODO sentinel\n").unwrap();

    let mut saw_node_modules_line = false;
    let mut saw_sentinel = false;
    for line in lines.by_ref().take(50) {
        let v: Value = serde_json::from_str(&line.unwrap()).unwrap();
        let path = v["path"].as_str().unwrap_or_default();
        if path == "node_modules" || path.starts_with("node_modules/") {
            saw_node_modules_line = true;
        }
        if v["event"] == "updated" && v["path"] == "sentinel.ts" {
            saw_sentinel = true;
            break;
        }
    }
    child.kill().unwrap();
    let _ = child.wait();
    assert!(saw_sentinel, "the sentinel change was never reported");
    assert!(
        !saw_node_modules_line,
        "an event for a path under node_modules was printed"
    );
}

/// Runs a short `clippings lsp` session with `CLIPPINGS_LOG` set to `level`
/// and an unreadable configuration, and returns what it wrote to stderr.
fn lsp_stderr(level: &str) -> String {
    let t = tempfile::tempdir().unwrap();
    let uri = clippings_core::uri::file_uri(&dunce::canonicalize(t.path()).unwrap());
    let mut child = bin()
        .arg("lsp")
        .env("CLIPPINGS_LOG", level)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let mut stdin = child.stdin.take().unwrap();
    let mut stdout = BufReader::new(child.stdout.take().unwrap());
    let mut send = |m: Message| m.write(&mut stdin).unwrap();
    send(
        Request::new(
            RequestId::from(1),
            "initialize".into(),
            json!({
                "workspaceFolders": [{ "uri": uri, "name": "w" }],
                "capabilities": {},
                "initializationOptions": { "protocolVersion": 1, "settings": {} }
            }),
        )
        .into(),
    );
    send(Notification::new("initialized".into(), json!({})).into());
    send(
        Notification::new(
            "clippings/configure".into(),
            json!({ "general": { "tags": 5 } }),
        )
        .into(),
    );
    send(Request::new(RequestId::from(2), "shutdown".into(), Value::Null).into());
    loop {
        if let Some(Message::Response(r)) = Message::read(&mut stdout).unwrap() {
            if r.id == RequestId::from(2) {
                break;
            }
        }
    }
    send(Notification::new("exit".into(), Value::Null).into());
    drop(stdin);
    let output = child.wait_with_output().unwrap();
    assert!(output.status.success());
    String::from_utf8(output.stderr).unwrap()
}

#[test]
fn lsp_logs_to_stderr_at_the_level_in_clippings_log() {
    let info = lsp_stderr("info");
    assert!(info.contains("serving 1 workspace folder(s)"), "{info}");
    assert!(info.contains("bad configuration"), "{info}");
    assert!(info.contains("WARN"), "{info}");

    let error = lsp_stderr("error");
    assert!(!error.contains("serving"), "{error}");
    assert!(!error.contains("bad configuration"), "{error}");
}
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test -p clippings --test lsp lsp_logs_to_stderr`
Expected: FAIL. `lsp_logs_to_stderr_at_the_level_in_clippings_log` panics at its first assertion with an empty message: nothing reached stderr.

- [ ] **Step 3: Write the implementation**

The `Cargo.lock` diff pins the versions the prototype resolved. `cargo build` alone would add the same packages, possibly at newer patch versions.

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
notify = "8.2"
regex = "1.13"
regex-syntax = "0.8.11"
serde = { version = "1", features = ["derive"] }
serde_json = { version = "1", features = ["preserve_order"] }
tempfile = "3"
thiserror = "2"
tracing = "0.1"
tracing-subscriber = { version = "0.3", default-features = false, features = ["fmt", "std"] }

[profile.release]
lto = "fat"
codegen-units = 1
panic = "unwind"
strip = "symbols"
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

[dev-dependencies]
insta.workspace = true
lsp-server.workspace = true
tempfile.workspace = true
```

Apply this diff to `Cargo.lock` with `git apply`:

```diff
diff --git a/Cargo.lock b/Cargo.lock
index be760ff..bf378fe 100644
--- a/Cargo.lock
+++ b/Cargo.lock
@@ -199,6 +199,7 @@ dependencies = [
  "lsp-server",
  "serde_json",
  "tempfile",
+ "tracing-subscriber",
 ]
 
 [[package]]
@@ -598,6 +599,12 @@ dependencies = [
  "libc",
 ]
 
+[[package]]
+name = "lazy_static"
+version = "1.5.0"
+source = "registry+https://github.com/rust-lang/crates.io-index"
+checksum = "bbd2bcb4c963f2ddae06a2efc7e9f3591312473c50c6685e1f298068316e66fe"
+
 [[package]]
 name = "libc"
 version = "0.2.189"
@@ -868,6 +875,15 @@ dependencies = [
  "zmij",
 ]
 
+[[package]]
+name = "sharded-slab"
+version = "0.1.7"
+source = "registry+https://github.com/rust-lang/crates.io-index"
+checksum = "f40ca3c46823713e0d4209592e8d6e826aa57e928f09752619fc696c499637f6"
+dependencies = [
+ "lazy_static",
+]
+
 [[package]]
 name = "shlex"
 version = "2.0.1"
@@ -953,6 +969,15 @@ dependencies = [
  "syn 3.0.6",
 ]
 
+[[package]]
+name = "thread_local"
+version = "1.1.10"
+source = "registry+https://github.com/rust-lang/crates.io-index"
+checksum = "1ad99c4c6d32803332c548b1af0540b357b3f5fc0be8f6c6bfe8b2e6ae784070"
+dependencies = [
+ "cfg-if",
+]
+
 [[package]]
 name = "tracing"
 version = "0.1.44"
@@ -984,6 +1009,17 @@ dependencies = [
  "once_cell",
 ]
 
+[[package]]
+name = "tracing-subscriber"
+version = "0.3.23"
+source = "registry+https://github.com/rust-lang/crates.io-index"
+checksum = "cb7f578e5945fb242538965c2d0b04418d38ec25c79d160cd279bf0731c8d319"
+dependencies = [
+ "sharded-slab",
+ "thread_local",
+ "tracing-core",
+]
+
 [[package]]
 name = "unicode-ident"
 version = "1.0.26"
```

Replace `crates/clippings-core/src/server/main_loop.rs` with:

```rust
//! The server's event loop over an `lsp-server` connection.

use super::{Env, Server};
use crate::fs::{Fs, NativeFs};
use crate::protocol::InitializeParams;
use crate::PROTOCOL_VERSION;
use crossbeam_channel::{after, never, select};
use lsp_server::{Connection, ErrorCode, Message, Response};
use serde_json::json;
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::sync::Arc;
use std::time::Instant;

fn capabilities() -> serde_json::Value {
    json!({
        "capabilities": {
            "textDocumentSync": { "openClose": true, "change": 2 },
            "workspace": { "workspaceFolders": { "supported": true, "changeNotifications": true } }
        },
        "serverInfo": { "name": "clippings", "version": env!("CARGO_PKG_VERSION") }
    })
}

/// Runs the handshake and the event loop until `exit`.
pub fn run(connection: Connection, fs: Arc<dyn Fs>, env: Env) -> Result<(), String> {
    let (id, raw) = connection.initialize_start().map_err(|e| e.to_string())?;
    let params: InitializeParams = serde_json::from_value(raw).map_err(|e| e.to_string())?;
    let version = params
        .initialization_options
        .as_ref()
        .map(|o| o.protocol_version);
    if version != Some(PROTOCOL_VERSION) {
        let message =
            format!("protocol version mismatch: client {version:?}, server {PROTOCOL_VERSION}");
        let _ = connection
            .sender
            .send(Response::new_err(id, ErrorCode::InvalidRequest as i32, message.clone()).into());
        return Err(message);
    }
    connection
        .initialize_finish(id, capabilities())
        .map_err(|e| e.to_string())?;
    tracing::info!(
        "clippings {} serving {} workspace folder(s), protocol version {PROTOCOL_VERSION}",
        env!("CARGO_PKG_VERSION"),
        params.workspace_folders.as_ref().map_or(0, Vec::len),
    );
    let mut server = Server::new(connection.sender.clone(), fs, env, &params);
    server.start(Instant::now());
    let work = server.work_rx.clone();
    loop {
        let timeout = match server.next_deadline() {
            Some(d) => after(d.saturating_duration_since(Instant::now())),
            None => never(),
        };
        select! {
            recv(connection.receiver) -> msg => {
                let Ok(msg) = msg else { break };
                match msg {
                    Message::Request(req) => {
                        if connection.handle_shutdown(&req).map_err(|e| e.to_string())? {
                            break;
                        }
                        let id = req.id.clone();
                        let resp = catch_unwind(AssertUnwindSafe(|| server.handle_request(req)))
                            .unwrap_or_else(|_| Response::new_err(id, ErrorCode::InternalError as i32, "request panicked".into()));
                        let _ = connection.sender.send(resp.into());
                    }
                    Message::Notification(n) => {
                        if n.method == "exit" {
                            break;
                        }
                        if catch_unwind(AssertUnwindSafe(|| server.handle_notification(n, Instant::now()))).is_err() {
                            server.recover(Instant::now());
                        }
                    }
                    Message::Response(_) => {}
                }
            }
            recv(work) -> w => {
                if let Ok(w) = w {
                    if catch_unwind(AssertUnwindSafe(|| server.handle_work(w, Instant::now()))).is_err() {
                        server.recover(Instant::now());
                    }
                }
            }
            recv(timeout) -> _ => {
                if catch_unwind(AssertUnwindSafe(|| server.tick(Instant::now()))).is_err() {
                    server.recover(Instant::now());
                }
            }
        }
    }
    Ok(())
}

/// `clippings lsp`: the server over stdin and stdout.
pub fn run_stdio() -> Result<(), String> {
    let (connection, io) = Connection::stdio();
    let env: Env = Arc::new(|n| std::env::var(n).ok());
    let result = run(connection, Arc::new(NativeFs), env);
    io.join().map_err(|e| e.to_string())?;
    result
}
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

/// Sends log records to stderr at the level named by `CLIPPINGS_LOG`
/// (`error`, `warn`, `info`, `debug`, `trace` or `off`), `info` by default.
/// The extension shows stderr in its output channel (spec 10.3); stdout
/// carries JSON-RPC only.
fn init_logging() {
    let level = std::env::var("CLIPPINGS_LOG")
        .ok()
        .and_then(|v| v.trim().parse::<LevelFilter>().ok())
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

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test -p clippings --test lsp`
Expected: 5 tests pass.
Then run: `cargo fmt --all && cargo clippy --all-targets -- -D warnings && INSTA_UPDATE=no cargo test --all`
Expected: no diffs, no warnings, and every test passes (200 passed, 2 ignored).

Then run: `pnpm -C extension dev && pnpm -C extension build && pnpm -C extension test:unit`
Expected: `31 passing`, including the protocol conformance tests against the rebuilt server.

- [ ] **Step 5: Commit**

```bash
git add -A
git commit -F - <<'EOF'
feat(server): log to stderr at the level in CLIPPINGS_LOG

Co-Authored-By: <your model attribution>
EOF
```

### Task 5: First status before the first styles reset

Spec 6.2 tells the client to discard its style generation when it sees a new server instance, and the instance arrives in `clippings/status`. `Server::start` sent the generation-0 styles reset before the first status, so a client following the spec dropped it and then ignored every decoration from a restarted server (ruling 19). `start` now sends the first status, from the full rescan or directly when `tree.scanAtStartup` is off, before the reset.

**Files:**
- Modify: `crates/clippings-core/src/server/mod.rs` (diff)
- Test: `crates/clippings-core/src/server/tests.rs` (modified, diff)

**Interfaces:**
- Produces: after `initialized`, the server's first notification carrying `instance` (`clippings/status`) precedes its first `clippings/styles` reset. Task 7's instance tracking and Task 14's decoration manager rely on it.

- [ ] **Step 1: Write the failing test**

Apply this diff to `crates/clippings-core/src/server/tests.rs` with `git apply`:

```diff
diff --git a/crates/clippings-core/src/server/tests.rs b/crates/clippings-core/src/server/tests.rs
index c16392e..29086ce 100644
--- a/crates/clippings-core/src/server/tests.rs
+++ b/crates/clippings-core/src/server/tests.rs
@@ -807,3 +807,33 @@ fn switching_the_scan_mode_re_registers_watchers() {
     mode(&mut s, "workspace only");
     assert_eq!(watcher_requests(&rx), vec![]);
 }
+
+/// Methods of the notifications sent so far.
+fn sent_methods(rx: &Receiver<Message>) -> Vec<String> {
+    rx.try_iter()
+        .filter_map(|m| match m {
+            Message::Notification(n) => Some(n.method),
+            _ => None,
+        })
+        .collect()
+}
+
+#[test]
+fn the_first_status_precedes_the_first_styles_reset() {
+    for scan_at_startup in [true, false] {
+        let (_t, root) = workspace();
+        let (mut s, rx) = server(
+            &root,
+            json!({ "tree": { "scanAtStartup": scan_at_startup } }),
+            Arc::new(NativeFs),
+        );
+        s.start(Instant::now());
+        let methods = sent_methods(&rx);
+        let status = methods.iter().position(|m| m == method::STATUS);
+        let styles = methods.iter().position(|m| m == method::STYLES);
+        assert!(
+            status.is_some() && styles.is_some() && status < styles,
+            "scanAtStartup {scan_at_startup}: {methods:?}"
+        );
+    }
+}
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test -p clippings-core --lib the_first_status_precedes_the_first_styles_reset`
Expected: FAIL. The test panics with `scanAtStartup true: ["clippings/styles", "clippings/status"]`.

- [ ] **Step 3: Write the implementation**

Apply this diff to `crates/clippings-core/src/server/mod.rs` with `git apply`:

```diff
diff --git a/crates/clippings-core/src/server/mod.rs b/crates/clippings-core/src/server/mod.rs
index 4cf2b12..bbe0f3c 100644
--- a/crates/clippings-core/src/server/mod.rs
+++ b/crates/clippings-core/src/server/mod.rs
@@ -234,9 +234,17 @@ impl Server {
             .send(Request::new(id, method.to_string(), params).into());
     }
 
-    /// Called once the client has sent `initialized`.
+    /// Called once the client has sent `initialized`. The first status goes
+    /// out before the first styles reset: a client discards its style
+    /// generation when it sees a new instance, so the reset must follow it.
     pub fn start(&mut self, now: Instant) {
         self.update_watchers();
+        if self.settings.tree.scan_at_startup {
+            self.full_rescan();
+        } else {
+            self.needs_scan = true;
+            self.send_status();
+        }
         self.send_notification(
             method::STYLES,
             p::StylesParams {
@@ -245,14 +253,8 @@ impl Server {
                 styles: BTreeMap::new(),
             },
         );
-        if self.settings.tree.scan_at_startup {
-            self.full_rescan();
-        } else {
-            self.needs_scan = true;
-        }
         self.reset_timers(now);
         self.schedule_view(now, true);
-        self.send_status();
     }
 
     fn rebuild_scan_state(&mut self) {
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test -p clippings-core --lib server::tests`
Expected: every server unit test passes, including the new one.
Then run: `cargo fmt --all && cargo clippy --all-targets -- -D warnings && INSTA_UPDATE=no cargo test --all`
Expected: no diffs, no warnings, and every test passes (201 passed, 2 ignored).

Then run: `pnpm -C extension dev && pnpm -C extension build && pnpm -C extension test:unit`
Expected: `31 passing`, including the protocol conformance tests against the rebuilt server.

- [ ] **Step 5: Commit**

```bash
git add -A
git commit -F - <<'EOF'
fix(server): send the first status before the first styles reset

Co-Authored-By: <your model attribution>
EOF
```

### Task 6: Language client, crash restarts and server commands

`ServerConnection` owns the server's lifecycle (spec 7.4): it resolves the binary, spawns `clippings lsp` through `vscode-languageclient` with `RUST_BACKTRACE=1` and `CLIPPINGS_LOG`, sends the configuration object in `initializationOptions`, and turns the four server notifications into events. `CrashHistory` counts crashes across every client of the connection, five within three minutes, and a server that exits before it is running, or does not finish starting within `START_TIMEOUT_MS` (10 seconds), counts too: the start settles at once, the server process is killed, that client is disposed and a fresh one starts, until the limit brings the notice with Restart and Show Log (ruling 28, Review Focus 5). `clippings.restartServer` and `clippings.showLog` are registered; a change to `general.schemes`, `server.path`, `server.logLevel` or `trace.server` restarts the client, and granting workspace trust restarts it when a workspace `server.path` exists. The integration tests kill the server process, and point `CLIPPINGS_SERVER_PATH` at scripts that pass the probe and then exit on `initialize` or never answer it, with the timeout shortened through a test hook, to prove the restart policy and that no server process is left behind.

**Files:**
- Create: `extension/src/commands/server.ts`, `extension/src/server/connection.ts`, `extension/src/server/crashHistory.ts`, `extension/src/server/crashPolicy.ts`, `extension/src/server/messages.ts`, `extension/src/testApi.ts`
- Modify: `extension/src/extension.ts`
- Test: `extension/src/test/integration/helpers.ts`, `extension/src/test/integration/server.test.ts`, `extension/src/test/unit/crashHistory.test.ts`

**Interfaces:**
- Consumes: `config/read.ts` (Task 2): `affectsServer`, `readConfiguration`; `protocol.ts` (Task 2): `ActiveEditorParams`, `ChildrenParams`, `ChildrenResult`, `DecorationsParams`, `Direction`, `ExportResult`, `FindParams`, `FindResult`, `Method`, `NavigateParams`, `NavigateResult`, `PROTOCOL_VERSION`, `Position`, `Range`, `Settings`, `StatusParams`, `StylesParams`, `TreeChangedParams`, `ViewNode`; `server/locate.ts` (Task 3): `locateServer`; `server/resolve.ts` (Task 3): `ResolutionError`; `state/viewState.ts` (Task 2): `ViewStateStore`.
- Produces, in `commands/server.ts`: `function registerServerCommands(connection: ServerConnection, log: vscode.LogOutputChannel): vscode.Disposable[]`; `function needsRestart(e: vscode.ConfigurationChangeEvent): boolean`.
- Produces, in `server/connection.ts`: `START_TIMEOUT_MS` (const); `ConnectionHost` (interface); `class ServerConnection implements vscode.Disposable` with `startTimeoutMs = START_TIMEOUT_MS`, `readonly onStatus: vscode.Event<StatusParams>`, `readonly onTreeChanged: vscode.Event<TreeChangedParams>`, `readonly onStyles: vscode.Event<StylesParams>`, `readonly onDecorations: vscode.Event<DecorationsParams>`, `readonly onGaveUp: vscode.Event<string>`, `readonly onRunning: vscode.Event<void>`, `constructor(private readonly host: ConnectionHost)`, `get running(): boolean`, `get pid(): number | undefined`, `start(): Promise<void>`, `async restart(): Promise<void>`, `async stop(): Promise<void>`, `configure(settings: Settings): void`, `activeEditor(uri: string | null): void`, `rescan(): void`, `stopScan(): void`, `async children(parent: string | null): Promise<ViewNode[]>`, `async find(uri: string, line: number | null): Promise<ViewNode[][]>`, `async navigate(uri: string, positions: Position[], direction: Direction): Promise<Range[] | null>`, `async export(): Promise<ExportResult | undefined>`, `dispose(): void`.
- Produces, in `server/crashHistory.ts`: `MAX_CRASHES` (const); `CRASH_WINDOW_MS` (const); `GIVE_UP_MESSAGE` (const); `class CrashHistory` with `record(now: number): 'restart' | 'give up'`, `clear(): void`.
- Produces, in `server/crashPolicy.ts`: `CrashEvents` (interface); `class CrashPolicy implements ErrorHandler` with `constructor(private readonly history: CrashHistory, private readonly events: CrashEvents)`, `setRunning(running: boolean): void`, `error(): ErrorHandlerResult`, `closed(): CloseHandlerResult`.
- Produces, in `server/messages.ts`: `Configure` (const); `ActiveEditor` (const); `Rescan` (const); `StopScan` (const); `Children` (const); `Find` (const); `Navigate` (const); `Export` (const); `TreeChanged` (const); `Styles` (const); `Decorations` (const); `Status` (const).
- Produces, in `testApi.ts`: `ServerHooks` (interface); `TestHooks` (interface); `ClippingsApi` (interface).
- Produces, in `extension.ts`: `async function deactivate(): Promise<void>`.

- [ ] **Step 1: Write the failing tests**

Create `extension/src/test/integration/helpers.ts`:

```ts
// Shared helpers for the integration tests. Waits are driven by real
// signals from the extension, never by fixed sleeps.

import * as vscode from 'vscode';
import type { ClippingsApi } from '../../testApi';

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

export function workspacePath(...parts: string[]): string {
  const root = process.env['CLIPPINGS_TEST_WORKSPACE'];
  if (!root) throw new Error('CLIPPINGS_TEST_WORKSPACE is not set');
  return [root, ...parts].join('/');
}
```

Create `extension/src/test/integration/server.test.ts`:

```ts
import * as assert from 'node:assert/strict';
import { chmodSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import * as vscode from 'vscode';
import { START_TIMEOUT_MS } from '../../server/connection';
import type { ClippingsApi } from '../../testApi';
import { getApi, waitFor, whenIdle, withTimeout } from './helpers';

describe('server lifecycle', () => {
  let api: ClippingsApi;

  before(async () => {
    api = await getApi();
    await whenIdle(api);
  });

  it('starts the locally built server and receives its status', () => {
    const status = api.test.server.status();
    assert.ok(status);
    assert.match(status.instance, /^[0-9a-f]+-[0-9a-f]+$/);
    assert.equal(status.error, null);
    assert.ok(api.test.server.pid);
  });

  it('restarts on demand with a new server instance', async () => {
    const before = api.test.server.status()?.instance;
    await vscode.commands.executeCommand('clippings.restartServer');
    const s = api.test.server;
    await waitFor('a new instance', () => s.running && s.status()?.instance !== before, [s.onStatus, s.onRunning]);
    await whenIdle(api);
  });

  it('restarts a crashed server', async () => {
    const s = api.test.server;
    const instance = s.status()?.instance;
    const pid = s.pid;
    assert.ok(pid);
    process.kill(pid, 'SIGKILL');
    await waitFor(
      'a restarted server',
      () => s.running && s.pid !== pid && s.status()?.instance !== instance,
      [s.onStatus, s.onRunning],
    );
    await whenIdle(api);
  });

  it('restarts when a setting fixed at start changes', async () => {
    const s = api.test.server;
    const instance = s.status()?.instance;
    await vscode.workspace
      .getConfiguration('clippings')
      .update('server.logLevel', 'debug', vscode.ConfigurationTarget.Global);
    try {
      await waitFor('a restart', () => s.running && s.status()?.instance !== instance, [s.onStatus, s.onRunning]);
    } finally {
      await vscode.workspace
        .getConfiguration('clippings')
        .update('server.logLevel', undefined, vscode.ConfigurationTarget.Global);
    }
    await whenIdle(api);
  });

  it('stops restarting after five crashes in three minutes and restarts on demand', async function () {
    this.timeout(60_000);
    const s = api.test.server;
    await vscode.commands.executeCommand('clippings.restartServer');
    await whenIdle(api);
    let notice: string | undefined;
    const subscription = s.onGaveUp((message) => (notice = message));
    try {
      for (let crash = 1; crash <= 5; crash++) {
        const pid = s.pid;
        assert.ok(pid, `a running server before crash ${crash}`);
        process.kill(pid, 'SIGKILL');
        if (crash < 5) {
          await waitFor(`a restart after crash ${crash}`, () => s.running && s.pid !== pid, [s.onStatus, s.onRunning]);
        }
      }
      const message = await waitFor('the crash notice', () => notice, [s.onGaveUp]);
      assert.match(message, /crashed 5 times in the last 3 minutes/);
    } finally {
      subscription.dispose();
    }
    await vscode.commands.executeCommand('clippings.restartServer');
    await whenIdle(api);
    assert.ok(s.running);
  });

  it('counts a server that exits while starting as a crash and restarts on demand', async function () {
    if (process.platform === 'win32') this.skip();
    this.timeout(60_000);
    const s = api.test.server;
    // Passes the probe, then exits as soon as `initialize` arrives.
    const dir = mkdtempSync(join(tmpdir(), 'clippings-fake-'));
    const fake = join(dir, 'clippings');
    writeFileSync(
      fake,
      [
        '#!/bin/sh',
        'if [ "$1" = probe ]; then',
        '  echo \'{"version":"0.0.0","target":"fake","protocolVersion":1}\'',
        '  exit 0',
        'fi',
        'head -c 1 > /dev/null',
        'exit 1',
        '',
      ].join('\n'),
    );
    chmodSync(fake, 0o755);
    const real = process.env['CLIPPINGS_SERVER_PATH'];
    let notice: string | undefined;
    const subscription = s.onGaveUp((message) => (notice = message));
    try {
      process.env['CLIPPINGS_SERVER_PATH'] = fake;
      const restart = vscode.commands.executeCommand('clippings.restartServer');
      await withTimeout('Restart Server with a server that exits', restart);
      const message = await waitFor('the crash notice', () => notice, [s.onGaveUp]);
      assert.match(message, /crashed 5 times in the last 3 minutes/);
      assert.equal(s.running, false);
    } finally {
      process.env['CLIPPINGS_SERVER_PATH'] = real;
      subscription.dispose();
      rmSync(dir, { recursive: true, force: true });
    }
    const restart = vscode.commands.executeCommand('clippings.restartServer');
    await withTimeout('Restart Server with the real server', restart);
    await whenIdle(api);
    assert.ok(s.running);
  });

  it('counts a server that does not finish starting in time as a crash and kills it', async function () {
    if (process.platform === 'win32') this.skip();
    this.timeout(60_000);
    const s = api.test.server;
    // Passes the probe, then reads `initialize` and never answers.
    const dir = mkdtempSync(join(tmpdir(), 'clippings-fake-'));
    const fake = join(dir, 'clippings');
    const pids = join(dir, 'pids');
    writeFileSync(
      fake,
      [
        '#!/bin/sh',
        'if [ "$1" = probe ]; then',
        '  echo \'{"version":"0.0.0","target":"fake","protocolVersion":1}\'',
        '  exit 0',
        'fi',
        `echo $$ >> '${pids}'`,
        // Keeps stdout open on fd 3 while discarding the requests.
        'exec cat 3>&1 > /dev/null',
        '',
      ].join('\n'),
    );
    chmodSync(fake, 0o755);
    const real = process.env['CLIPPINGS_SERVER_PATH'];
    let notice: string | undefined;
    const subscription = s.onGaveUp((message) => (notice = message));
    try {
      process.env['CLIPPINGS_SERVER_PATH'] = fake;
      s.setStartTimeout(500);
      const restart = vscode.commands.executeCommand('clippings.restartServer');
      await withTimeout('Restart Server with a server that hangs', restart);
      const message = await waitFor('the crash notice', () => notice, [s.onGaveUp]);
      assert.match(message, /crashed 5 times in the last 3 minutes/);
      assert.equal(s.running, false);
      const started = readFileSync(pids, 'utf8').trim().split('\n').map(Number);
      assert.equal(started.length, 5, 'five attempts');
      for (const pid of started) assert.throws(() => process.kill(pid, 0), /ESRCH/, `server process ${pid} is gone`);
    } finally {
      process.env['CLIPPINGS_SERVER_PATH'] = real;
      s.setStartTimeout(START_TIMEOUT_MS);
      subscription.dispose();
      rmSync(dir, { recursive: true, force: true });
    }
    const restart = vscode.commands.executeCommand('clippings.restartServer');
    await withTimeout('Restart Server with the real server', restart);
    await whenIdle(api);
    assert.ok(s.running);
  });

  it('shows the log', async () => {
    await vscode.commands.executeCommand('clippings.showLog');
  });
});
```

Create `extension/src/test/unit/crashHistory.test.ts`:

```ts
import * as assert from 'node:assert/strict';
import { CRASH_WINDOW_MS, CrashHistory } from '../../server/crashHistory';

describe('crash history', () => {
  it('restarts four times and gives up on the fifth crash within three minutes', () => {
    const history = new CrashHistory();
    const results = [0, 1000, 2000, 3000, 4000].map((t) => history.record(t));
    assert.deepEqual(results, ['restart', 'restart', 'restart', 'restart', 'give up']);
    assert.equal(history.record(5000), 'restart', 'giving up starts a new count');
  });

  it('forgets crashes older than the window', () => {
    const history = new CrashHistory();
    for (const t of [0, 1, 2, 3]) history.record(t);
    assert.equal(history.record(CRASH_WINDOW_MS + 10), 'restart');
  });

  it('starts over when cleared', () => {
    const history = new CrashHistory();
    for (const t of [0, 1, 2, 3]) history.record(t);
    history.clear();
    assert.equal(history.record(4), 'restart');
  });
});
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `pnpm -C extension typecheck`
Expected: FAIL. `error TS2307: Cannot find module '../../testApi'` in `helpers.ts` and `server.test.ts`, and `error TS2307: Cannot find module` for `../../server/connection` and `../../server/crashHistory`.

- [ ] **Step 3: Write the implementation**

Create `extension/src/commands/server.ts`:

```ts
// Server commands (spec 7.4, 10.3): restart on demand and show the log.

import * as vscode from 'vscode';
import type { ServerConnection } from '../server/connection';

export function registerServerCommands(connection: ServerConnection, log: vscode.LogOutputChannel): vscode.Disposable[] {
  return [
    vscode.commands.registerCommand('clippings.restartServer', () => connection.restart()),
    vscode.commands.registerCommand('clippings.showLog', () => log.show(true)),
  ];
}

/** Settings fixed when the client starts: changing one restarts it (spec 7.4). */
const RESTART_SETTINGS = [
  'clippings.general.schemes',
  'clippings.server.path',
  'clippings.server.logLevel',
  'clippings.trace.server',
];

export function needsRestart(e: vscode.ConfigurationChangeEvent): boolean {
  return RESTART_SETTINGS.some((s) => e.affectsConfiguration(s));
}
```

Create `extension/src/server/connection.ts`:

```ts
// The server's lifecycle (spec 7.4): resolve the binary, spawn `clippings
// lsp` through the language client, relay its notifications, and restart on
// demand or after a crash. Everything else talks to the server through this
// class.

import { spawn, type ChildProcess } from 'node:child_process';
import * as vscode from 'vscode';
import { LanguageClient, State, type LanguageClientOptions, type ServerOptions } from 'vscode-languageclient/node';
import {
  PROTOCOL_VERSION,
  type DecorationsParams,
  type Direction,
  type ExportResult,
  type Position,
  type Range,
  type Settings,
  type StatusParams,
  type StylesParams,
  type TreeChangedParams,
  type ViewNode,
} from '../protocol';
import { CrashHistory, GIVE_UP_MESSAGE } from './crashHistory';
import { CrashPolicy } from './crashPolicy';
import { locateServer } from './locate';
import * as m from './messages';
import { ResolutionError } from './resolve';

/** How long a server may take to finish starting before it counts as a crash. */
export const START_TIMEOUT_MS = 10_000;
/** How long to wait for a killed server process to exit, per signal. */
const KILL_WAIT_MS = 2000;

export interface ConnectionHost {
  readonly context: vscode.ExtensionContext;
  readonly log: vscode.LogOutputChannel;
  /** The configuration object to send, read fresh on each call. */
  settings(): Settings;
  /** The active editor's document URI, for `clippings/activeEditor`. */
  activeUri(): string | null;
  /** Shows an error with action buttons and resolves with the chosen one. */
  error(message: string, ...actions: string[]): PromiseLike<string | undefined>;
}

export class ServerConnection implements vscode.Disposable {
  private client: LanguageClient | undefined;
  /**
   * The current client's server process. Clippings spawns it itself: the
   * language client forgets its process once the connection closes, and a
   * server that closed its output can still be alive and must be killed.
   */
  private process: ChildProcess | undefined;
  private starting: Promise<void> | undefined;
  /** Crashes and start failures across every client of this connection. */
  private readonly crashes = new CrashHistory();
  /** `START_TIMEOUT_MS`; tests shorten it. */
  startTimeoutMs = START_TIMEOUT_MS;
  private readonly emitters = {
    gaveUp: new vscode.EventEmitter<string>(),
    status: new vscode.EventEmitter<StatusParams>(),
    treeChanged: new vscode.EventEmitter<TreeChangedParams>(),
    styles: new vscode.EventEmitter<StylesParams>(),
    decorations: new vscode.EventEmitter<DecorationsParams>(),
    running: new vscode.EventEmitter<void>(),
  };
  readonly onStatus = this.emitters.status.event;
  readonly onTreeChanged = this.emitters.treeChanged.event;
  readonly onStyles = this.emitters.styles.event;
  readonly onDecorations = this.emitters.decorations.event;
  /** Fires with the notice when repeated crashes stop the automatic restarts. */
  readonly onGaveUp = this.emitters.gaveUp.event;
  /** Fires each time a server, new or restarted, is ready for requests. */
  readonly onRunning = this.emitters.running.event;

  private readonly trust: vscode.Disposable;

  constructor(private readonly host: ConnectionHost) {
    // A workspace `server.path` counts only once the workspace is trusted.
    this.trust = vscode.workspace.onDidGrantWorkspaceTrust(() => {
      if (vscode.workspace.getConfiguration('clippings').inspect('server.path')?.workspaceValue) void this.restart();
    });
  }

  get running(): boolean {
    return this.client?.state === State.Running;
  }

  /** The server process ID, for tests. */
  get pid(): number | undefined {
    return this.client ? this.process?.pid : undefined;
  }

  start(): Promise<void> {
    this.starting ??= this.doStart().finally(() => (this.starting = undefined));
    return this.starting;
  }

  /** Restarts on demand: forgets past crashes, then starts a fresh client. */
  async restart(): Promise<void> {
    this.crashes.clear();
    await this.starting;
    await this.stop();
    await this.start();
  }

  async stop(): Promise<void> {
    const client = this.client;
    const child = this.process;
    this.client = undefined;
    this.process = undefined;
    if (client) {
      try {
        await client.dispose(2000);
      } catch (err) {
        this.host.log.warn(`Stopping the server failed: ${String(err)}`);
      }
    }
    await killProcess(child);
  }

  /**
   * Starts a client, and starts a fresh one after each start failure until
   * the crash limit. Settles once a server runs, resolution fails, or the
   * limit is reached, so `restart` never waits on a server that died.
   */
  private async doStart(): Promise<void> {
    while ((await this.startOnce()) === 'failed') {
      if (this.crashes.record(Date.now()) === 'give up') {
        void this.showCrash(GIVE_UP_MESSAGE);
        return;
      }
    }
  }

  private async startOnce(): Promise<'started' | 'failed' | 'error'> {
    const { log, context } = this.host;
    let serverPath: string;
    try {
      serverPath = (await locateServer(context, (msg) => log.info(msg))).candidate.path;
    } catch (err) {
      const message = err instanceof Error ? err.message : String(err);
      log.error(message);
      void this.showError(message, err instanceof ResolutionError);
      return 'error';
    }
    const settings = this.host.settings();
    const env = {
      ...process.env,
      RUST_BACKTRACE: '1',
      CLIPPINGS_LOG: vscode.workspace.getConfiguration('clippings').get<string>('server.logLevel', 'info'),
    };
    // Called for the first start and for each restart after a crash.
    let child: ChildProcess | undefined;
    const serverOptions: ServerOptions = () => {
      child = spawn(serverPath, ['lsp'], { env, windowsHide: true });
      if (this.client === client) this.process = child;
      return Promise.resolve(child);
    };
    // Set while `startOnce` waits for this client; a start failure after that
    // comes from the client restarting a crashed server.
    let waiting: ((outcome: 'failed') => void) | undefined;
    const failed = new Promise<'failed'>((resolve) => (waiting = resolve));
    // Each start, first or after a crash, must reach the running state in time.
    let watchdog: NodeJS.Timeout | undefined;
    let abandoned = false;
    const startFailed = (reason: string) => {
      if (abandoned) return;
      abandoned = true;
      clearTimeout(watchdog);
      log.error(reason);
      if (waiting) waiting('failed');
      else void this.afterStartFailure(client, child);
    };
    const policy = new CrashPolicy(this.crashes, {
      startFailed: () => startFailed('The server exited while starting.'),
      gaveUp: (message) => void this.showCrash(message),
    });
    const clientOptions: LanguageClientOptions = {
      documentSelector: settings.general.schemes.map((scheme) => ({ scheme })),
      initializationOptions: () => ({ protocolVersion: PROTOCOL_VERSION, settings: this.host.settings() }),
      outputChannel: log,
      errorHandler: policy,
    };
    const client = new LanguageClient('clippings', 'Clippings', serverOptions, clientOptions);
    client.onNotification(m.Status, (p) => this.emitters.status.fire(p));
    client.onNotification(m.TreeChanged, (p) => this.emitters.treeChanged.fire(p));
    client.onNotification(m.Styles, (p) => this.emitters.styles.fire(p));
    client.onNotification(m.Decorations, (p) => this.emitters.decorations.fire(p));
    client.onDidChangeState((e) => {
      clearTimeout(watchdog);
      if (e.newState === State.Starting) {
        const seconds = this.startTimeoutMs / 1000;
        watchdog = setTimeout(
          () => startFailed(`The server did not finish starting within ${seconds} s.`),
          this.startTimeoutMs,
        );
      }
      policy.setRunning(e.newState === State.Running);
      if (e.newState === State.Running && this.client === client) this.onClientRunning(client);
    });
    this.client = client;
    // When the server exits or hangs during `initialize`, `start()` may never
    // settle; a start failure settles the race instead.
    const started = client.start().then(() => 'started' as const);
    started.catch(() => undefined);
    try {
      const outcome = await Promise.race([started, failed]);
      if (outcome === 'failed') await this.discard(client, child);
      return outcome;
    } catch (err) {
      // `initialize` failed or the connection closed under it: a start failure.
      abandoned = true;
      clearTimeout(watchdog);
      log.error(`The server failed to start: ${String(err)}`);
      await this.discard(client, child);
      return 'failed';
    } finally {
      waiting = undefined;
    }
  }

  /** A crashed server that the client restarted, then failed to start. */
  private async afterStartFailure(client: LanguageClient, child: ChildProcess | undefined): Promise<void> {
    await this.discard(client, child);
    if (this.crashes.record(Date.now()) === 'give up') void this.showCrash(GIVE_UP_MESSAGE);
    else void this.start();
  }

  /** Forgets a client whose server failed; it cannot be started again. */
  private async discard(client: LanguageClient, child: ChildProcess | undefined): Promise<void> {
    if (this.client === client) {
      this.client = undefined;
      this.process = undefined;
    }
    await killProcess(child);
    try {
      await client.dispose(2000);
    } catch {
      // A client that never ran cannot be stopped; there is nothing to stop.
    }
  }

  private onClientRunning(client: LanguageClient): void {
    // Settings may have changed between `initialize` and now.
    void client.sendNotification(m.Configure, this.host.settings());
    void client.sendNotification(m.ActiveEditor, { uri: this.host.activeUri() });
    this.emitters.running.fire();
  }

  private async showError(message: string, offerSetting: boolean): Promise<void> {
    const actions = offerSetting ? ['Show Log', 'Open Setting'] : ['Show Log'];
    const choice = await this.host.error(message, ...actions);
    if (choice === 'Show Log') this.host.log.show(true);
    if (choice === 'Open Setting') {
      await vscode.commands.executeCommand('workbench.action.openSettings', 'clippings.server.path');
    }
  }

  private async showCrash(message: string): Promise<void> {
    this.host.log.error(message);
    this.emitters.gaveUp.fire(message);
    const choice = await this.host.error(message, 'Restart', 'Show Log');
    if (choice === 'Restart') await this.restart();
    if (choice === 'Show Log') this.host.log.show(true);
  }

  // ---- client to server ----

  configure(settings: Settings): void {
    if (this.running) void this.client?.sendNotification(m.Configure, settings);
  }

  activeEditor(uri: string | null): void {
    if (this.running) void this.client?.sendNotification(m.ActiveEditor, { uri });
  }

  rescan(): void {
    if (this.running) void this.client?.sendNotification(m.Rescan, {});
  }

  stopScan(): void {
    if (this.running) void this.client?.sendNotification(m.StopScan, {});
  }

  async children(parent: string | null): Promise<ViewNode[]> {
    if (!this.running || !this.client) return [];
    return (await this.client.sendRequest(m.Children, { parent })).nodes;
  }

  async find(uri: string, line: number | null): Promise<ViewNode[][]> {
    if (!this.running || !this.client) return [];
    return (await this.client.sendRequest(m.Find, { uri, line })).paths;
  }

  async navigate(uri: string, positions: Position[], direction: Direction): Promise<Range[] | null> {
    if (!this.running || !this.client) return null;
    return (await this.client.sendRequest(m.Navigate, { uri, positions, direction })).ranges;
  }

  async export(): Promise<ExportResult | undefined> {
    if (!this.running || !this.client) return undefined;
    return this.client.sendRequest(m.Export, {});
  }

  dispose(): void {
    this.trust.dispose();
    void this.stop();
    for (const e of Object.values(this.emitters)) e.dispose();
  }
}

/** Ends a server process that did not exit by itself: SIGTERM, then SIGKILL. */
async function killProcess(child: ChildProcess | undefined): Promise<void> {
  if (!child) return;
  const exited = () => child.exitCode !== null || child.signalCode !== null;
  for (const signal of ['SIGTERM', 'SIGKILL'] as const) {
    if (exited()) return;
    const exit = new Promise<void>((resolve) => child.once('exit', () => resolve()));
    child.kill(signal);
    let timer: NodeJS.Timeout | undefined;
    await Promise.race([exit, new Promise<void>((resolve) => (timer = setTimeout(resolve, KILL_WAIT_MS)))]);
    clearTimeout(timer);
  }
}
```

Create `extension/src/server/crashHistory.ts`:

```ts
// The crash count behind automatic restarts (spec 7.4). Pure. A server that
// exits while starting counts as a crash, so one that always fails at startup
// stops after five attempts instead of looping.

export const MAX_CRASHES = 5;
export const CRASH_WINDOW_MS = 3 * 60 * 1000;

export const GIVE_UP_MESSAGE =
  'The Clippings server crashed 5 times in the last 3 minutes. The server will not be restarted. ' +
  'See the output for more information.';

export class CrashHistory {
  private times: number[] = [];

  /** Records a crash at `now`: `restart` while under the limit, else `give up`. */
  record(now: number): 'restart' | 'give up' {
    this.times = [...this.times.filter((t) => now - t <= CRASH_WINDOW_MS), now];
    if (this.times.length < MAX_CRASHES) return 'restart';
    this.times = [];
    return 'give up';
  }

  /** Forgets every crash, as when the user restarts the server. */
  clear(): void {
    this.times = [];
  }
}
```

Create `extension/src/server/crashPolicy.ts`:

```ts
// Crash handling (spec 7.4) for one language client. A crash of a running
// server counts in the connection's `CrashHistory`: under the limit the client
// restarts it, at the limit Clippings shows its own notice with Restart and
// Show Log. A server that exits before it is running is a start failure,
// which the connection handles, so the client never restarts it itself.

import {
  CloseAction,
  ErrorAction,
  type CloseHandlerResult,
  type ErrorHandler,
  type ErrorHandlerResult,
} from 'vscode-languageclient/node';
import { GIVE_UP_MESSAGE, type CrashHistory } from './crashHistory';

export interface CrashEvents {
  /** The server exited before the client reached the running state. */
  startFailed(): void;
  /** Crashes reached the limit; `message` says so. */
  gaveUp(message: string): void;
}

export class CrashPolicy implements ErrorHandler {
  private running = false;

  constructor(
    private readonly history: CrashHistory,
    private readonly events: CrashEvents,
  ) {}

  /** Follows the client's state: only a running server can crash. */
  setRunning(running: boolean): void {
    this.running = running;
  }

  /**
   * Always continues. Writes to a server that just died fail before the
   * connection closes; shutting down on them, as the language client's
   * default does after three, would stop the client without `closed`
   * ever deciding on a restart.
   */
  error(): ErrorHandlerResult {
    return { action: ErrorAction.Continue };
  }

  closed(): CloseHandlerResult {
    if (!this.running) {
      this.events.startFailed();
      return { action: CloseAction.DoNotRestart, handled: true };
    }
    this.running = false;
    if (this.history.record(Date.now()) === 'restart') return { action: CloseAction.Restart, handled: true };
    this.events.gaveUp(GIVE_UP_MESSAGE);
    return { action: CloseAction.DoNotRestart, handled: true };
  }
}
```

Create `extension/src/server/messages.ts`:

```ts
// Typed JSON-RPC message types for the custom `clippings/*` messages.

import { NotificationType, RequestType } from 'vscode-languageclient/node';
import {
  Method,
  type ActiveEditorParams,
  type ChildrenParams,
  type ChildrenResult,
  type DecorationsParams,
  type ExportResult,
  type FindParams,
  type FindResult,
  type NavigateParams,
  type NavigateResult,
  type Settings,
  type StatusParams,
  type StylesParams,
  type TreeChangedParams,
} from '../protocol';

export const Configure = new NotificationType<Settings>(Method.configure);
export const ActiveEditor = new NotificationType<ActiveEditorParams>(Method.activeEditor);
export const Rescan = new NotificationType<Record<string, never>>(Method.rescan);
export const StopScan = new NotificationType<Record<string, never>>(Method.stopScan);
export const Children = new RequestType<ChildrenParams, ChildrenResult, void>(Method.children);
export const Find = new RequestType<FindParams, FindResult, void>(Method.find);
export const Navigate = new RequestType<NavigateParams, NavigateResult, void>(Method.navigate);
export const Export = new RequestType<Record<string, never>, ExportResult, void>(Method.export);
export const TreeChanged = new NotificationType<TreeChangedParams>(Method.treeChanged);
export const Styles = new NotificationType<StylesParams>(Method.styles);
export const Decorations = new NotificationType<DecorationsParams>(Method.decorations);
export const Status = new NotificationType<StatusParams>(Method.status);
```

Create `extension/src/testApi.ts`:

```ts
// What `activate` returns. The `test` hooks let the integration tests
// observe state VS Code offers no API to read (spec 12.5).

import type * as vscode from 'vscode';
import type { StatusParams } from './protocol';

export interface ServerHooks {
  readonly running: boolean;
  readonly pid: number | undefined;
  readonly onStatus: vscode.Event<StatusParams>;
  readonly onRunning: vscode.Event<void>;
  /** Fires when repeated crashes stop the automatic restarts (spec 7.4). */
  readonly onGaveUp: vscode.Event<string>;
  /** Shortens the start timeout (`START_TIMEOUT_MS`) so tests need not wait for it. */
  setStartTimeout(ms: number): void;
  status(): StatusParams | undefined;
}

export interface TestHooks {
  readonly server: ServerHooks;
}

export interface ClippingsApi {
  readonly version: string;
  readonly test: TestHooks;
}
```

Replace `extension/src/extension.ts` with:

```ts
// Composition root: builds each part of the extension and wires them to the
// server connection.

import * as vscode from 'vscode';
import { needsRestart, registerServerCommands } from './commands/server';
import { affectsServer, readConfiguration } from './config/read';
import type { StatusParams } from './protocol';
import { ServerConnection } from './server/connection';
import { ViewStateStore } from './state/viewState';
import type { ClippingsApi } from './testApi';

let connection: ServerConnection | undefined;

export function activate(context: vscode.ExtensionContext): ClippingsApi {
  const log = vscode.window.createOutputChannel('Clippings', { log: true });
  const store = new ViewStateStore(context.workspaceState);
  const settings = () => readConfiguration(store.snapshot());
  const activeUri = () => vscode.window.activeTextEditor?.document.uri.toString() ?? null;
  connection = new ServerConnection({
    context,
    log,
    settings,
    activeUri,
    error: (message, ...actions) => vscode.window.showErrorMessage(message, ...actions),
  });
  const server = connection;
  let lastStatus: StatusParams | undefined;

  context.subscriptions.push(
    log,
    server,
    ...registerServerCommands(server, log),
    server.onStatus((s) => (lastStatus = s)),
    vscode.workspace.onDidChangeConfiguration((e) => {
      if (needsRestart(e)) void server.restart();
      else if (affectsServer(e)) server.configure(settings());
    }),
    vscode.window.onDidChangeActiveTextEditor(() => server.activeEditor(activeUri())),
  );
  void server.start();

  const manifest = context.extension.packageJSON as { version: string };
  return {
    version: manifest.version,
    test: {
      server: {
        get running() {
          return server.running;
        },
        get pid() {
          return server.pid;
        },
        onStatus: server.onStatus,
        onRunning: server.onRunning,
        onGaveUp: server.onGaveUp,
        setStartTimeout: (ms) => (server.startTimeoutMs = ms),
        status: () => lastStatus,
      },
    },
  };
}

export async function deactivate(): Promise<void> {
  await connection?.stop();
  connection = undefined;
}
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `pnpm -C extension dev && pnpm -C extension typecheck && pnpm -C extension build`
Expected: no errors.

Run: `pnpm -C extension test:unit`
Expected: `34 passing`.

Run: `pnpm -C extension test:integration`
Expected: `9 passing`. A VS Code test window opens, runs the suite and closes.

- [ ] **Step 5: Commit**

```bash
git add -A
git commit -F - <<'EOF'
feat(extension): run the server through the language client with crash restarts

Co-Authored-By: <your model attribution>
EOF
```

### Task 7: Tree provider over node IDs

The tree view of spec 7.5: the element type is the node ID string, `NodeCache` keeps the last node received for each ID and each node's parent, and `treeItem` maps a `ViewNode` onto a `TreeItem`, with codicons as theme icons. `clippings/treeChanged` refreshes the listed parents and ignores IDs never loaded, and a new server instance clears the cache (spec 6.2). The task adds the tree test hooks, quiet user settings for test runs (ruling 14), and the bundled default icon that Task 15 uses.

**Files:**
- Create: `extension/resources/todo-default.svg`, `extension/src/icons/resolver.ts`, `extension/src/tree/items.ts`, `extension/src/tree/nodeCache.ts`, `extension/src/tree/provider.ts`, `extension/src/tree/testItems.ts`
- Modify: `extension/.vscode-test.mjs`, `extension/src/server/connection.ts`, `extension/src/testApi.ts`, `extension/src/extension.ts`
- Test: `extension/src/test/integration/helpers.ts` (modified), `extension/src/test/integration/tree.test.ts`, `extension/src/test/unit/nodeCache.test.ts`

**Interfaces:**
- Consumes: `protocol.ts` (Task 2): `IconDescriptor`, `ViewNode`.
- Produces, in `icons/resolver.ts`: `TreeIcon` (type); `class IconResolver` with `treeIcon(icon: IconDescriptor | null): TreeIcon | undefined`.
- Produces, in `tree/items.ts`: `ItemContext` (interface); `function treeItem(node: ViewNode, ctx: ItemContext): vscode.TreeItem`.
- Produces, in `tree/nodeCache.ts`: `class NodeCache` with `get(id: string): ViewNode | undefined`, `has(id: string): boolean`, `parent(id: string): string | null | undefined`, `get size(): number`, `record(parent: string | null, children: readonly ViewNode[]): void`, `recordPath(path: readonly ViewNode[]): void`, `ownKey(id: string): string`, `clear(): void`.
- Produces, in `tree/provider.ts`: `ChildrenSource` (interface); `class TreeProvider implements vscode.TreeDataProvider<string>, vscode.Disposable` with `readonly onDidChangeTreeData: vscode.Event<string | string[] | undefined>`, `constructor(private readonly source: ChildrenSource, private readonly cache: NodeCache, private readonly ctx: ItemContext)`, `async getChildren(element?: string): Promise<string[]>`, `getTreeItem(element: string): vscode.TreeItem`, `getParent(element: string): string | undefined`, `refresh(parents: readonly (string | null)[]): void`, `refreshAll(): void`, `reset(): void`, `dispose(): void`.
- Produces, in `tree/testItems.ts`: `TestItem` (interface); `function testItem(id: string, item: vscode.TreeItem): TestItem`.
- Produces, in `server/connection.ts`: `ServerConnection` gains `readonly onNewInstance: vscode.Event<string>`.
- Produces, in `testApi.ts`: `TreeHooks` (interface); `TestHooks` gains `readonly tree: TreeHooks`.

- [ ] **Step 1: Write the failing tests**

Replace `extension/src/test/integration/helpers.ts` with:

```ts
// Shared helpers for the integration tests. Waits are driven by real
// signals from the extension, never by fixed sleeps.

import * as vscode from 'vscode';
import type { ClippingsApi } from '../../testApi';

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

export function workspacePath(...parts: string[]): string {
  const root = process.env['CLIPPINGS_TEST_WORKSPACE'];
  if (!root) throw new Error('CLIPPINGS_TEST_WORKSPACE is not set');
  return [root, ...parts].join('/');
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
```

Create `extension/src/test/integration/tree.test.ts`:

```ts
import * as assert from 'node:assert/strict';
import { writeFileSync, readFileSync } from 'node:fs';
import type { ClippingsApi } from '../../testApi';
import { getApi, outline, treeBecomes, whenIdle, workspacePath } from './helpers';

export const DEFAULT_TREE = [
  '(Scan mode: workspace and open files)',
  'workspace',
  '  docs',
  '    plan.md',
  '      [ ] write the guide',
  '      [x] pick a name',
  '  lib',
  '    notes.rs',
  '      TODO first line of a long note',
  '  src',
  '    util',
  '      strings.py',
  '        TODO normalise unicode before comparing',
  '        BUG (bob) drops leading tabs too',
  '    app.ts',
  '      TODO (alice) wire up the router',
  '      FIXME handle the error path',
  '      HACK temporary shim until the router lands',
];

describe('tree provider', () => {
  let api: ClippingsApi;

  before(async () => {
    api = await getApi();
    await whenIdle(api);
  });

  it('shows the fixture workspace as a tree', async () => {
    await treeBecomes(api, DEFAULT_TREE);
  });

  it('maps node fields onto tree items', async () => {
    const [status, root] = await api.test.tree.items();
    assert.ok(status && root);
    assert.equal(status.id, 'status:scan-mode');
    assert.equal(status.icon, '$(search)');
    assert.equal(root.contextValue, 'folder');
    assert.equal(root.icon, '$(window)');
    assert.equal(root.state, 'collapsed');
    const [docs] = await api.test.tree.items(root.id);
    const [plan] = await api.test.tree.items(docs?.id);
    assert.equal(plan?.contextValue, 'file');
    const [todo] = await api.test.tree.items(plan?.id);
    assert.ok(todo);
    assert.equal(todo.state, 'none');
    assert.equal(todo.tooltip, `${workspacePath('docs', 'plan.md')}, line 3`);
    assert.equal(todo.command?.command, 'clippings.revealInFile');
    assert.deepEqual(todo.command?.arguments[1], { line: 2, character: 0 });
  });

  it('refreshes when a file changes on disk', async () => {
    const path = workspacePath('lib', 'notes.rs');
    const original = readFileSync(path, 'utf8');
    try {
      writeFileSync(path, original + '// FIXME appended on disk\n');
      const expected = [...DEFAULT_TREE];
      expected.splice(expected.indexOf('      TODO first line of a long note') + 1, 0, '      FIXME appended on disk');
      await treeBecomes(api, expected);
    } finally {
      writeFileSync(path, original);
    }
    await treeBecomes(api, DEFAULT_TREE);
    assert.ok((await outline(api, 1)).length === 2);
  });
});
```

Create `extension/src/test/unit/nodeCache.test.ts`:

```ts
import * as assert from 'node:assert/strict';
import type { ViewNode } from '../../protocol';
import { NodeCache } from '../../tree/nodeCache';

function node(id: string): ViewNode {
  return {
    id,
    label: id,
    description: null,
    tooltip: null,
    icon: null,
    hasChildren: false,
    defaultExpanded: false,
    contextValue: null,
    resourceUri: null,
    command: null,
  };
}

describe('node cache', () => {
  it('records children and their parents', () => {
    const cache = new NodeCache();
    cache.record(null, [node('w:file:///w')]);
    cache.record('w:file:///w', [node('w:file:///w/d:/w/src')]);
    assert.equal(cache.parent('w:file:///w'), null);
    assert.equal(cache.parent('w:file:///w/d:/w/src'), 'w:file:///w');
    assert.equal(cache.parent('unknown'), undefined);
    assert.equal(cache.size, 2);
  });

  it('records a find path top down', () => {
    const cache = new NodeCache();
    cache.recordPath([node('a'), node('a/b'), node('a/b/c')]);
    assert.equal(cache.parent('a'), null);
    assert.equal(cache.parent('a/b/c'), 'a/b');
  });

  it('derives own keys that contain slashes from the recorded parent', () => {
    const cache = new NodeCache();
    const root = 'w:file:///w';
    const folder = `${root}/d:/w/app/[slug]`;
    const file = `${folder}/f:/w/app/[slug]/page.ts`;
    cache.recordPath([node(root), node(folder), node(file)]);
    assert.equal(cache.ownKey(root), root);
    assert.equal(cache.ownKey(folder), 'd:/w/app/[slug]');
    assert.equal(cache.ownKey(file), 'f:/w/app/[slug]/page.ts');
    assert.equal(cache.ownKey('never-seen'), 'never-seen');
  });

  it('clears everything', () => {
    const cache = new NodeCache();
    cache.record(null, [node('a')]);
    cache.clear();
    assert.equal(cache.size, 0);
    assert.equal(cache.get('a'), undefined);
  });
});
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `pnpm -C extension typecheck`
Expected: FAIL. `error TS2339: Property 'tree' does not exist on type 'TestHooks'` in the integration files, and `error TS2307: Cannot find module '../../tree/nodeCache'`.

- [ ] **Step 3: Write the implementation**

Create `extension/resources/todo-default.svg`:

```xml
<svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" viewBox="0 0 16 16">
  <rect x="1" y="1" width="14" height="14" rx="3.5" fill="#2ea043"/>
  <path d="M4.5 8.25l2.25 2.25 4.75-5" fill="none" stroke="#ffffff" stroke-width="1.75" stroke-linecap="round" stroke-linejoin="round"/>
</svg>
```

Create `extension/src/icons/resolver.ts`:

```ts
// Maps the server's icon descriptors onto VS Code icons (spec 7.7).

import * as vscode from 'vscode';
import type { IconDescriptor } from '../protocol';

export type TreeIcon = vscode.ThemeIcon | vscode.Uri;

export class IconResolver {
  /** An icon for a tree item, or undefined for none. */
  treeIcon(icon: IconDescriptor | null): TreeIcon | undefined {
    if (!icon) return undefined;
    switch (icon.kind) {
      case 'codicon':
        return new vscode.ThemeIcon(icon.name, icon.colour ? new vscode.ThemeColor(icon.colour) : undefined);
      case 'folder':
        return vscode.ThemeIcon.Folder;
      case 'file':
        return vscode.ThemeIcon.File;
      default:
        return undefined;
    }
  }
}
```

Create `extension/src/tree/items.ts`:

```ts
// Turns a rendered `ViewNode` into a VS Code tree item (spec 5.12, 7.5).

import * as vscode from 'vscode';
import type { IconResolver } from '../icons/resolver';
import type { ViewNode } from '../protocol';

export interface ItemContext {
  icons: IconResolver;
  /** The tree item ID for a node ID. */
  itemId(id: string): string;
  /** Whether a node with children shows expanded. */
  expanded(node: ViewNode): boolean;
}

export function treeItem(node: ViewNode, ctx: ItemContext): vscode.TreeItem {
  const state = !node.hasChildren
    ? vscode.TreeItemCollapsibleState.None
    : ctx.expanded(node)
      ? vscode.TreeItemCollapsibleState.Expanded
      : vscode.TreeItemCollapsibleState.Collapsed;
  const item = new vscode.TreeItem(node.label, state);
  item.id = ctx.itemId(node.id);
  if (node.description !== null) item.description = node.description;
  if (node.tooltip !== null) item.tooltip = node.tooltip;
  if (node.contextValue !== null) item.contextValue = node.contextValue;
  if (node.resourceUri !== null) item.resourceUri = vscode.Uri.parse(node.resourceUri);
  const icon = ctx.icons.treeIcon(node.icon);
  if (icon) item.iconPath = icon;
  const command = node.command;
  if (command?.kind === 'reveal') {
    item.command = {
      title: 'Reveal In File',
      command: 'clippings.revealInFile',
      arguments: [command.uri, command.position],
    };
  } else if (command?.kind === 'openUrl') {
    item.command = { title: 'Open URL', command: 'clippings.openUrl', arguments: [command.url] };
  }
  return item;
}
```

Create `extension/src/tree/nodeCache.ts`:

```ts
// The last node received for each ID and each node's parent (spec 7.5).
// Pure, so the tree logic can be tested without VS Code.

import type { ViewNode } from '../protocol';

export class NodeCache {
  private readonly nodes = new Map<string, ViewNode>();
  private readonly parents = new Map<string, string | null>();

  get(id: string): ViewNode | undefined {
    return this.nodes.get(id);
  }

  has(id: string): boolean {
    return this.nodes.has(id);
  }

  /** The node's parent ID, `null` for a top-level node, `undefined` if unknown. */
  parent(id: string): string | null | undefined {
    return this.parents.get(id);
  }

  get size(): number {
    return this.nodes.size;
  }

  /** Records the children of `parent` (`null` for the top level). */
  record(parent: string | null, children: readonly ViewNode[]): void {
    for (const node of children) {
      this.nodes.set(node.id, node);
      this.parents.set(node.id, parent);
    }
  }

  /** Records a path from `clippings/find`: each node is the parent of the next. */
  recordPath(path: readonly ViewNode[]): void {
    let parent: string | null = null;
    for (const node of path) {
      this.record(parent, [node]);
      parent = node.id;
    }
  }

  /**
   * The node's own key: its ID without the parent's ID and the `/` after it
   * (spec 5.12). Keys can contain `/`, so this needs the recorded parent.
   */
  ownKey(id: string): string {
    const parent = this.parents.get(id);
    return parent && id.startsWith(parent + '/') ? id.slice(parent.length + 1) : id;
  }

  clear(): void {
    this.nodes.clear();
    this.parents.clear();
  }
}
```

Create `extension/src/tree/provider.ts`:

```ts
// The tree data provider over node ID strings (spec 7.5). Children come from
// `clippings/children`; `clippings/treeChanged` names the parents to refetch.

import * as vscode from 'vscode';
import type { ViewNode } from '../protocol';
import { treeItem, type ItemContext } from './items';
import type { NodeCache } from './nodeCache';

export interface ChildrenSource {
  children(parent: string | null): Promise<ViewNode[]>;
}

export class TreeProvider implements vscode.TreeDataProvider<string>, vscode.Disposable {
  private readonly changed = new vscode.EventEmitter<string | string[] | undefined>();
  readonly onDidChangeTreeData = this.changed.event;

  constructor(
    private readonly source: ChildrenSource,
    private readonly cache: NodeCache,
    private readonly ctx: ItemContext,
  ) {}

  async getChildren(element?: string): Promise<string[]> {
    const parent = element ?? null;
    const nodes = await this.source.children(parent);
    this.cache.record(parent, nodes);
    return nodes.map((n) => n.id);
  }

  getTreeItem(element: string): vscode.TreeItem {
    const node = this.cache.get(element);
    if (!node) {
      const item = new vscode.TreeItem('');
      item.id = this.ctx.itemId(element);
      return item;
    }
    return treeItem(node, this.ctx);
  }

  getParent(element: string): string | undefined {
    return this.cache.parent(element) ?? undefined;
  }

  /** Applies `clippings/treeChanged`: `null` is the root; unknown IDs are ignored. */
  refresh(parents: readonly (string | null)[]): void {
    if (parents.includes(null)) return this.changed.fire(undefined);
    const known = parents.filter((p): p is string => p !== null && this.cache.has(p));
    if (known.length > 0) this.changed.fire(known);
  }

  /** Refetches the whole tree. */
  refreshAll(): void {
    this.changed.fire(undefined);
  }

  /** A new server instance: forget every node and refetch. */
  reset(): void {
    this.cache.clear();
    this.changed.fire(undefined);
  }

  dispose(): void {
    this.changed.dispose();
  }
}
```

Create `extension/src/tree/testItems.ts`:

```ts
// A plain snapshot of tree items for the test hooks.

import * as vscode from 'vscode';

export interface TestItem {
  id: string;
  itemId: string;
  label: string;
  description: string | undefined;
  tooltip: string | undefined;
  contextValue: string | undefined;
  state: 'none' | 'collapsed' | 'expanded';
  icon: string | undefined;
  command: { command: string; arguments: unknown[] } | undefined;
}

function iconName(icon: vscode.TreeItem['iconPath']): string | undefined {
  if (icon instanceof vscode.ThemeIcon) return `$(${icon.id})`;
  if (icon instanceof vscode.Uri) return icon.fsPath;
  return undefined;
}

export function testItem(id: string, item: vscode.TreeItem): TestItem {
  const states = { 0: 'none', 1: 'collapsed', 2: 'expanded' } as const;
  return {
    id,
    itemId: item.id ?? '',
    label: typeof item.label === 'string' ? item.label : (item.label?.label ?? ''),
    description: typeof item.description === 'string' ? item.description : undefined,
    tooltip: typeof item.tooltip === 'string' ? item.tooltip : undefined,
    contextValue: item.contextValue,
    state: states[item.collapsibleState ?? 0],
    icon: iconName(item.iconPath),
    command: item.command ? { command: item.command.command, arguments: item.command.arguments ?? [] } : undefined,
  };
}
```

Replace `extension/.vscode-test.mjs` with:

```js
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
  files: 'out/test/integration/**/*.test.js',
  version: process.env.CLIPPINGS_TEST_VSCODE ?? 'stable',
  extensionDevelopmentPath: here,
  workspaceFolder: workspace,
  launchArgs: ['--disable-extensions', `--user-data-dir=${join(scratch, 'user-data')}`, '--disable-workspace-trust'],
  env: { CLIPPINGS_SERVER_PATH: server, CLIPPINGS_TEST_WORKSPACE: workspace },
  mocha: { ui: 'bdd', timeout: 20000, slow: 2000 },
});
```

Replace `extension/src/server/connection.ts` with:

```ts
// The server's lifecycle (spec 7.4): resolve the binary, spawn `clippings
// lsp` through the language client, relay its notifications, and restart on
// demand or after a crash. Everything else talks to the server through this
// class.

import { spawn, type ChildProcess } from 'node:child_process';
import * as vscode from 'vscode';
import { LanguageClient, State, type LanguageClientOptions, type ServerOptions } from 'vscode-languageclient/node';
import {
  PROTOCOL_VERSION,
  type DecorationsParams,
  type Direction,
  type ExportResult,
  type Position,
  type Range,
  type Settings,
  type StatusParams,
  type StylesParams,
  type TreeChangedParams,
  type ViewNode,
} from '../protocol';
import { CrashHistory, GIVE_UP_MESSAGE } from './crashHistory';
import { CrashPolicy } from './crashPolicy';
import { locateServer } from './locate';
import * as m from './messages';
import { ResolutionError } from './resolve';

/** How long a server may take to finish starting before it counts as a crash. */
export const START_TIMEOUT_MS = 10_000;
/** How long to wait for a killed server process to exit, per signal. */
const KILL_WAIT_MS = 2000;

export interface ConnectionHost {
  readonly context: vscode.ExtensionContext;
  readonly log: vscode.LogOutputChannel;
  /** The configuration object to send, read fresh on each call. */
  settings(): Settings;
  /** The active editor's document URI, for `clippings/activeEditor`. */
  activeUri(): string | null;
  /** Shows an error with action buttons and resolves with the chosen one. */
  error(message: string, ...actions: string[]): PromiseLike<string | undefined>;
}

export class ServerConnection implements vscode.Disposable {
  private client: LanguageClient | undefined;
  /**
   * The current client's server process. Clippings spawns it itself: the
   * language client forgets its process once the connection closes, and a
   * server that closed its output can still be alive and must be killed.
   */
  private process: ChildProcess | undefined;
  private starting: Promise<void> | undefined;
  /** Crashes and start failures across every client of this connection. */
  private readonly crashes = new CrashHistory();
  /** `START_TIMEOUT_MS`; tests shorten it. */
  startTimeoutMs = START_TIMEOUT_MS;
  private readonly emitters = {
    gaveUp: new vscode.EventEmitter<string>(),
    status: new vscode.EventEmitter<StatusParams>(),
    treeChanged: new vscode.EventEmitter<TreeChangedParams>(),
    styles: new vscode.EventEmitter<StylesParams>(),
    decorations: new vscode.EventEmitter<DecorationsParams>(),
    running: new vscode.EventEmitter<void>(),
    instance: new vscode.EventEmitter<string>(),
  };
  private instance: string | undefined;
  readonly onStatus = this.emitters.status.event;
  readonly onTreeChanged = this.emitters.treeChanged.event;
  readonly onStyles = this.emitters.styles.event;
  readonly onDecorations = this.emitters.decorations.event;
  /** Fires with the notice when repeated crashes stop the automatic restarts. */
  readonly onGaveUp = this.emitters.gaveUp.event;
  /** Fires each time a server, new or restarted, is ready for requests. */
  readonly onRunning = this.emitters.running.event;
  /**
   * Fires before the first status of a new server instance, so listeners
   * discard their node, decoration and style state (spec 6.2).
   */
  readonly onNewInstance = this.emitters.instance.event;

  private readonly trust: vscode.Disposable;

  constructor(private readonly host: ConnectionHost) {
    // A workspace `server.path` counts only once the workspace is trusted.
    this.trust = vscode.workspace.onDidGrantWorkspaceTrust(() => {
      if (vscode.workspace.getConfiguration('clippings').inspect('server.path')?.workspaceValue) void this.restart();
    });
  }

  get running(): boolean {
    return this.client?.state === State.Running;
  }

  /** The server process ID, for tests. */
  get pid(): number | undefined {
    return this.client ? this.process?.pid : undefined;
  }

  start(): Promise<void> {
    this.starting ??= this.doStart().finally(() => (this.starting = undefined));
    return this.starting;
  }

  /** Restarts on demand: forgets past crashes, then starts a fresh client. */
  async restart(): Promise<void> {
    this.crashes.clear();
    await this.starting;
    await this.stop();
    await this.start();
  }

  async stop(): Promise<void> {
    const client = this.client;
    const child = this.process;
    this.client = undefined;
    this.process = undefined;
    if (client) {
      try {
        await client.dispose(2000);
      } catch (err) {
        this.host.log.warn(`Stopping the server failed: ${String(err)}`);
      }
    }
    await killProcess(child);
  }

  /**
   * Starts a client, and starts a fresh one after each start failure until
   * the crash limit. Settles once a server runs, resolution fails, or the
   * limit is reached, so `restart` never waits on a server that died.
   */
  private async doStart(): Promise<void> {
    while ((await this.startOnce()) === 'failed') {
      if (this.crashes.record(Date.now()) === 'give up') {
        void this.showCrash(GIVE_UP_MESSAGE);
        return;
      }
    }
  }

  private async startOnce(): Promise<'started' | 'failed' | 'error'> {
    const { log, context } = this.host;
    let serverPath: string;
    try {
      serverPath = (await locateServer(context, (msg) => log.info(msg))).candidate.path;
    } catch (err) {
      const message = err instanceof Error ? err.message : String(err);
      log.error(message);
      void this.showError(message, err instanceof ResolutionError);
      return 'error';
    }
    const settings = this.host.settings();
    const env = {
      ...process.env,
      RUST_BACKTRACE: '1',
      CLIPPINGS_LOG: vscode.workspace.getConfiguration('clippings').get<string>('server.logLevel', 'info'),
    };
    // Called for the first start and for each restart after a crash.
    let child: ChildProcess | undefined;
    const serverOptions: ServerOptions = () => {
      child = spawn(serverPath, ['lsp'], { env, windowsHide: true });
      if (this.client === client) this.process = child;
      return Promise.resolve(child);
    };
    // Set while `startOnce` waits for this client; a start failure after that
    // comes from the client restarting a crashed server.
    let waiting: ((outcome: 'failed') => void) | undefined;
    const failed = new Promise<'failed'>((resolve) => (waiting = resolve));
    // Each start, first or after a crash, must reach the running state in time.
    let watchdog: NodeJS.Timeout | undefined;
    let abandoned = false;
    const startFailed = (reason: string) => {
      if (abandoned) return;
      abandoned = true;
      clearTimeout(watchdog);
      log.error(reason);
      if (waiting) waiting('failed');
      else void this.afterStartFailure(client, child);
    };
    const policy = new CrashPolicy(this.crashes, {
      startFailed: () => startFailed('The server exited while starting.'),
      gaveUp: (message) => void this.showCrash(message),
    });
    const clientOptions: LanguageClientOptions = {
      documentSelector: settings.general.schemes.map((scheme) => ({ scheme })),
      initializationOptions: () => ({ protocolVersion: PROTOCOL_VERSION, settings: this.host.settings() }),
      outputChannel: log,
      errorHandler: policy,
    };
    const client = new LanguageClient('clippings', 'Clippings', serverOptions, clientOptions);
    client.onNotification(m.Status, (p) => {
      if (p.instance !== this.instance) {
        this.instance = p.instance;
        this.emitters.instance.fire(p.instance);
      }
      this.emitters.status.fire(p);
    });
    client.onNotification(m.TreeChanged, (p) => this.emitters.treeChanged.fire(p));
    client.onNotification(m.Styles, (p) => this.emitters.styles.fire(p));
    client.onNotification(m.Decorations, (p) => this.emitters.decorations.fire(p));
    client.onDidChangeState((e) => {
      clearTimeout(watchdog);
      if (e.newState === State.Starting) {
        const seconds = this.startTimeoutMs / 1000;
        watchdog = setTimeout(
          () => startFailed(`The server did not finish starting within ${seconds} s.`),
          this.startTimeoutMs,
        );
      }
      policy.setRunning(e.newState === State.Running);
      if (e.newState === State.Running && this.client === client) this.onClientRunning(client);
    });
    this.client = client;
    // When the server exits or hangs during `initialize`, `start()` may never
    // settle; a start failure settles the race instead.
    const started = client.start().then(() => 'started' as const);
    started.catch(() => undefined);
    try {
      const outcome = await Promise.race([started, failed]);
      if (outcome === 'failed') await this.discard(client, child);
      return outcome;
    } catch (err) {
      // `initialize` failed or the connection closed under it: a start failure.
      abandoned = true;
      clearTimeout(watchdog);
      log.error(`The server failed to start: ${String(err)}`);
      await this.discard(client, child);
      return 'failed';
    } finally {
      waiting = undefined;
    }
  }

  /** A crashed server that the client restarted, then failed to start. */
  private async afterStartFailure(client: LanguageClient, child: ChildProcess | undefined): Promise<void> {
    await this.discard(client, child);
    if (this.crashes.record(Date.now()) === 'give up') void this.showCrash(GIVE_UP_MESSAGE);
    else void this.start();
  }

  /** Forgets a client whose server failed; it cannot be started again. */
  private async discard(client: LanguageClient, child: ChildProcess | undefined): Promise<void> {
    if (this.client === client) {
      this.client = undefined;
      this.process = undefined;
    }
    await killProcess(child);
    try {
      await client.dispose(2000);
    } catch {
      // A client that never ran cannot be stopped; there is nothing to stop.
    }
  }

  private onClientRunning(client: LanguageClient): void {
    // Settings may have changed between `initialize` and now.
    void client.sendNotification(m.Configure, this.host.settings());
    void client.sendNotification(m.ActiveEditor, { uri: this.host.activeUri() });
    this.emitters.running.fire();
  }

  private async showError(message: string, offerSetting: boolean): Promise<void> {
    const actions = offerSetting ? ['Show Log', 'Open Setting'] : ['Show Log'];
    const choice = await this.host.error(message, ...actions);
    if (choice === 'Show Log') this.host.log.show(true);
    if (choice === 'Open Setting') {
      await vscode.commands.executeCommand('workbench.action.openSettings', 'clippings.server.path');
    }
  }

  private async showCrash(message: string): Promise<void> {
    this.host.log.error(message);
    this.emitters.gaveUp.fire(message);
    const choice = await this.host.error(message, 'Restart', 'Show Log');
    if (choice === 'Restart') await this.restart();
    if (choice === 'Show Log') this.host.log.show(true);
  }

  // ---- client to server ----

  configure(settings: Settings): void {
    if (this.running) void this.client?.sendNotification(m.Configure, settings);
  }

  activeEditor(uri: string | null): void {
    if (this.running) void this.client?.sendNotification(m.ActiveEditor, { uri });
  }

  rescan(): void {
    if (this.running) void this.client?.sendNotification(m.Rescan, {});
  }

  stopScan(): void {
    if (this.running) void this.client?.sendNotification(m.StopScan, {});
  }

  async children(parent: string | null): Promise<ViewNode[]> {
    if (!this.running || !this.client) return [];
    return (await this.client.sendRequest(m.Children, { parent })).nodes;
  }

  async find(uri: string, line: number | null): Promise<ViewNode[][]> {
    if (!this.running || !this.client) return [];
    return (await this.client.sendRequest(m.Find, { uri, line })).paths;
  }

  async navigate(uri: string, positions: Position[], direction: Direction): Promise<Range[] | null> {
    if (!this.running || !this.client) return null;
    return (await this.client.sendRequest(m.Navigate, { uri, positions, direction })).ranges;
  }

  async export(): Promise<ExportResult | undefined> {
    if (!this.running || !this.client) return undefined;
    return this.client.sendRequest(m.Export, {});
  }

  dispose(): void {
    this.trust.dispose();
    void this.stop();
    for (const e of Object.values(this.emitters)) e.dispose();
  }
}

/** Ends a server process that did not exit by itself: SIGTERM, then SIGKILL. */
async function killProcess(child: ChildProcess | undefined): Promise<void> {
  if (!child) return;
  const exited = () => child.exitCode !== null || child.signalCode !== null;
  for (const signal of ['SIGTERM', 'SIGKILL'] as const) {
    if (exited()) return;
    const exit = new Promise<void>((resolve) => child.once('exit', () => resolve()));
    child.kill(signal);
    let timer: NodeJS.Timeout | undefined;
    await Promise.race([exit, new Promise<void>((resolve) => (timer = setTimeout(resolve, KILL_WAIT_MS)))]);
    clearTimeout(timer);
  }
}
```

Replace `extension/src/testApi.ts` with:

```ts
// What `activate` returns. The `test` hooks let the integration tests
// observe state VS Code offers no API to read (spec 12.5).

import type * as vscode from 'vscode';
import type { StatusParams } from './protocol';
import type { TestItem } from './tree/testItems';

export interface ServerHooks {
  readonly running: boolean;
  readonly pid: number | undefined;
  readonly onStatus: vscode.Event<StatusParams>;
  readonly onRunning: vscode.Event<void>;
  /** Fires when repeated crashes stop the automatic restarts (spec 7.4). */
  readonly onGaveUp: vscode.Event<string>;
  /** Shortens the start timeout (`START_TIMEOUT_MS`) so tests need not wait for it. */
  setStartTimeout(ms: number): void;
  status(): StatusParams | undefined;
}

export interface TreeHooks {
  readonly view: vscode.TreeView<string>;
  /** Fires whenever the provider asks VS Code to refetch. */
  readonly onDidChange: vscode.Event<unknown>;
  /** The provider's children of `parent` (top level when omitted), as tree items. */
  items(parent?: string): Promise<TestItem[]>;
}

export interface TestHooks {
  readonly server: ServerHooks;
  readonly tree: TreeHooks;
}

export interface ClippingsApi {
  readonly version: string;
  readonly test: TestHooks;
}
```

Replace `extension/src/extension.ts` with:

```ts
// Composition root: builds each part of the extension and wires them to the
// server connection.

import * as vscode from 'vscode';
import { needsRestart, registerServerCommands } from './commands/server';
import { affectsServer, readConfiguration } from './config/read';
import type { StatusParams } from './protocol';
import { IconResolver } from './icons/resolver';
import { ServerConnection } from './server/connection';
import { ViewStateStore } from './state/viewState';
import type { ClippingsApi } from './testApi';
import { NodeCache } from './tree/nodeCache';
import { TreeProvider } from './tree/provider';
import { testItem } from './tree/testItems';

let connection: ServerConnection | undefined;

export function activate(context: vscode.ExtensionContext): ClippingsApi {
  const log = vscode.window.createOutputChannel('Clippings', { log: true });
  const store = new ViewStateStore(context.workspaceState);
  const settings = () => readConfiguration(store.snapshot());
  const activeUri = () => vscode.window.activeTextEditor?.document.uri.toString() ?? null;
  connection = new ServerConnection({
    context,
    log,
    settings,
    activeUri,
    error: (message, ...actions) => vscode.window.showErrorMessage(message, ...actions),
  });
  const server = connection;
  let lastStatus: StatusParams | undefined;

  const cache = new NodeCache();
  const icons = new IconResolver();
  const provider = new TreeProvider(server, cache, {
    icons,
    itemId: (id) => id,
    expanded: (node) => node.defaultExpanded,
  });
  const treeView = vscode.window.createTreeView('clippings-view', { treeDataProvider: provider });

  context.subscriptions.push(
    log,
    server,
    ...registerServerCommands(server, log),
    server.onStatus((s) => (lastStatus = s)),
    provider,
    treeView,
    server.onNewInstance(() => provider.reset()),
    server.onTreeChanged((p) => provider.refresh(p.refresh)),
    vscode.workspace.onDidChangeConfiguration((e) => {
      if (needsRestart(e)) void server.restart();
      else if (affectsServer(e)) server.configure(settings());
    }),
    vscode.window.onDidChangeActiveTextEditor(() => server.activeEditor(activeUri())),
  );
  void server.start();

  const manifest = context.extension.packageJSON as { version: string };
  return {
    version: manifest.version,
    test: {
      server: {
        get running() {
          return server.running;
        },
        get pid() {
          return server.pid;
        },
        onStatus: server.onStatus,
        onRunning: server.onRunning,
        onGaveUp: server.onGaveUp,
        setStartTimeout: (ms) => (server.startTimeoutMs = ms),
        status: () => lastStatus,
      },
      tree: {
        view: treeView,
        onDidChange: provider.onDidChangeTreeData,
        items: async (parent) => {
          const ids = await provider.getChildren(parent);
          return ids.map((id) => testItem(id, provider.getTreeItem(id)));
        },
      },
    },
  };
}

export async function deactivate(): Promise<void> {
  await connection?.stop();
  connection = undefined;
}
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `pnpm -C extension dev && pnpm -C extension typecheck && pnpm -C extension build`
Expected: no errors.

Run: `pnpm -C extension test:unit`
Expected: `38 passing`.

Run: `pnpm -C extension test:integration`
Expected: `12 passing`. A VS Code test window opens, runs the suite and closes.

- [ ] **Step 5: Commit**

```bash
git add -A
git commit -F - <<'EOF'
feat(extension): show the server's view in a tree provider over node IDs

Co-Authored-By: <your model attribution>
EOF
```

### Task 8: Client-side expansion and the epoch

Expansion lives on the client (spec 7.5): `Expansion` keeps a map from node ID to expanded in workspace storage, falls back to each node's `defaultExpanded`, and prefixes tree item IDs with an epoch so VS Code forgets its own expansion state when the whole tree resets. `ConfigurationSync` re-reads and pushes the configuration and says whether the change makes the server replace the tree, which decides when the epoch bumps (rulings 8 and 9). Expand Tree, Collapse Tree and a `tree.expanded` change use it. The fixture's expected outlines move to `fixture.ts` for the later suites.

**Files:**
- Create: `extension/src/commands/expand.ts`, `extension/src/config/sync.ts`, `extension/src/tree/expansion.ts`
- Modify: `extension/src/testApi.ts`, `extension/src/extension.ts`
- Test: `extension/src/test/integration/fixture.ts`, `extension/src/test/integration/expansion.test.ts`, `extension/src/test/integration/tree.test.ts` (modified), `extension/src/test/unit/expansion.test.ts`

**Interfaces:**
- Consumes: `config/configuration.ts` (Task 2): `effectiveView`; `protocol.ts` (Task 2): `Settings`, `ViewNode`; `state/viewState.ts` (Task 2): `ViewStateStore`; `tree/provider.ts` (Task 7): `TreeProvider`.
- Produces, in `commands/expand.ts`: `ExpandDeps` (interface); `function resetExpansion(deps: Pick<ExpandDeps, 'expansion' | 'provider'>, replaced: boolean): void`; `function registerExpandCommands(deps: ExpandDeps): vscode.Disposable[]`.
- Produces, in `config/sync.ts`: `Push` (interface); `function replacesTree({ before, after }: Push): boolean`; `class ConfigurationSync` with `constructor(private readonly read: () => Settings, private readonly send: (settings: Settings) => void)`, `get current(): Settings`, `push(): Push`, `onPush(listener: (push: Push) => void): void`.
- Produces, in `tree/expansion.ts`: `class Expansion` with `constructor(private readonly store: ViewStateStore)`, `get currentEpoch(): number`, `itemId(id: string): string`, `expanded(node: ViewNode): boolean`, `set(id: string, expanded: boolean): void`, `reset(awaitServer: boolean): void`, `onRootRefresh(): void`.
- Produces, in `testApi.ts`: `TreeHooks` gains `readonly epoch: number`.

- [ ] **Step 1: Write the failing tests**

Create `extension/src/test/integration/fixture.ts`:

```ts
// Expected views of tests/fixtures/workspace.

/** The default tree view, as `outline` prints it. */
export const DEFAULT_TREE = [
  '(Scan mode: workspace and open files)',
  'workspace',
  '  docs',
  '    plan.md',
  '      [ ] write the guide',
  '      [x] pick a name',
  '  lib',
  '    notes.rs',
  '      TODO first line of a long note',
  '  src',
  '    util',
  '      strings.py',
  '        TODO normalise unicode before comparing',
  '        BUG (bob) drops leading tabs too',
  '    app.ts',
  '      TODO (alice) wire up the router',
  '      FIXME handle the error path',
  '      HACK temporary shim until the router lands',
];
```

Create `extension/src/test/integration/expansion.test.ts`:

```ts
import * as assert from 'node:assert/strict';
import { readFileSync, writeFileSync } from 'node:fs';
import * as vscode from 'vscode';
import type { ClippingsApi } from '../../testApi';
import type { TestItem } from '../../tree/testItems';
import { getApi, treeBecomes, waitFor, whenIdle, workspacePath } from './helpers';
import { DEFAULT_TREE } from './fixture';

/** Every item with children, depth first. */
async function containers(api: ClippingsApi, parent?: string): Promise<TestItem[]> {
  const out: TestItem[] = [];
  for (const item of await api.test.tree.items(parent)) {
    if (item.state === 'none') continue;
    out.push(item, ...(await containers(api, item.id)));
  }
  return out;
}

async function allContainers(api: ClippingsApi, state: 'expanded' | 'collapsed'): Promise<TestItem[]> {
  return waitFor(
    `every container ${state}`,
    async () => {
      const items = await containers(api);
      return items.length > 0 && items.every((i) => i.state === state) ? items : undefined;
    },
    [api.test.tree.onDidChange],
  );
}

describe('expand and collapse', () => {
  let api: ClippingsApi;

  before(async () => {
    api = await getApi();
    await whenIdle(api);
    await treeBecomes(api, DEFAULT_TREE);
    await vscode.commands.executeCommand('clippings-view.focus');
  });

  after(async () => {
    await vscode.commands.executeCommand('clippings.collapse');
    await allContainers(api, 'collapsed');
  });

  it('Expand Tree expands every node under a new epoch', async () => {
    const epoch = api.test.tree.epoch;
    await vscode.commands.executeCommand('clippings.expand');
    const items = await allContainers(api, 'expanded');
    assert.ok(api.test.tree.epoch > epoch);
    for (const item of items) assert.equal(item.itemId, `${api.test.tree.epoch}/${item.id}`);
  });

  it('Collapse Tree collapses every node under a new epoch', async () => {
    const epoch = api.test.tree.epoch;
    await vscode.commands.executeCommand('clippings.collapse');
    await allContainers(api, 'collapsed');
    assert.ok(api.test.tree.epoch > epoch);
  });

  it('records expanding and collapsing one node on the client', async () => {
    const [, root] = await api.test.tree.items();
    assert.ok(root);
    await api.test.tree.view.reveal(root.id, { expand: true, focus: false, select: false });
    const [docs, lib] = await waitFor(
      'the root expanded',
      async () => {
        const [, r] = await api.test.tree.items();
        return r?.state === 'expanded' ? api.test.tree.items(r.id) : undefined;
      },
      [api.test.tree.view.onDidExpandElement],
    );
    assert.ok(docs && lib);
    assert.equal(docs.state, 'collapsed', 'siblings keep the default');
    await api.test.tree.view.reveal(docs.id, { expand: true, focus: false, select: false });
    await waitFor(
      'docs expanded',
      async () => (await api.test.tree.items(root.id))[0]?.state === 'expanded',
      [api.test.tree.view.onDidExpandElement],
    );
  });

  it('keeps the epoch across ordinary changes', async () => {
    const epoch = api.test.tree.epoch;
    const path = workspacePath('docs', 'plan.md');
    const original = readFileSync(path, 'utf8');
    const expected = [...DEFAULT_TREE];
    expected.splice(expected.indexOf('      [x] pick a name') + 1, 0, '      [ ] another item');
    try {
      writeFileSync(path, original + '- [ ] another item\n');
      await treeBecomes(api, expected);
      assert.equal(api.test.tree.epoch, epoch);
    } finally {
      writeFileSync(path, original);
    }
    await treeBecomes(api, DEFAULT_TREE);
  });
});
```

Replace `extension/src/test/integration/tree.test.ts` with:

```ts
import * as assert from 'node:assert/strict';
import { writeFileSync, readFileSync } from 'node:fs';
import type { ClippingsApi } from '../../testApi';
import { DEFAULT_TREE } from './fixture';
import { getApi, outline, treeBecomes, whenIdle, workspacePath } from './helpers';


describe('tree provider', () => {
  let api: ClippingsApi;

  before(async () => {
    api = await getApi();
    await whenIdle(api);
  });

  it('shows the fixture workspace as a tree', async () => {
    await treeBecomes(api, DEFAULT_TREE);
  });

  it('maps node fields onto tree items', async () => {
    const [status, root] = await api.test.tree.items();
    assert.ok(status && root);
    assert.equal(status.id, 'status:scan-mode');
    assert.equal(status.icon, '$(search)');
    assert.equal(root.contextValue, 'folder');
    assert.equal(root.icon, '$(window)');
    assert.equal(root.state, 'collapsed');
    const [docs] = await api.test.tree.items(root.id);
    const [plan] = await api.test.tree.items(docs?.id);
    assert.equal(plan?.contextValue, 'file');
    const [todo] = await api.test.tree.items(plan?.id);
    assert.ok(todo);
    assert.equal(todo.state, 'none');
    assert.equal(todo.tooltip, `${workspacePath('docs', 'plan.md')}, line 3`);
    assert.equal(todo.command?.command, 'clippings.revealInFile');
    assert.deepEqual(todo.command?.arguments[1], { line: 2, character: 0 });
  });

  it('refreshes when a file changes on disk', async () => {
    const path = workspacePath('lib', 'notes.rs');
    const original = readFileSync(path, 'utf8');
    try {
      writeFileSync(path, original + '// FIXME appended on disk\n');
      const expected = [...DEFAULT_TREE];
      expected.splice(expected.indexOf('      TODO first line of a long note') + 1, 0, '      FIXME appended on disk');
      await treeBecomes(api, expected);
    } finally {
      writeFileSync(path, original);
    }
    await treeBecomes(api, DEFAULT_TREE);
    assert.ok((await outline(api, 1)).length === 2);
  });
});
```

Create `extension/src/test/unit/expansion.test.ts`:

```ts
import * as assert from 'node:assert/strict';
import { buildConfiguration } from '../../config/configuration';
import { replacesTree } from '../../config/sync';
import type { Settings, ViewNode } from '../../protocol';
import { ViewStateStore, type PersistedViewState } from '../../state/viewState';
import { Expansion } from '../../tree/expansion';
import { MemoryMemento } from './memento';

function node(id: string, defaultExpanded: boolean): ViewNode {
  return {
    id,
    label: id,
    description: null,
    tooltip: null,
    icon: null,
    hasChildren: true,
    defaultExpanded,
    contextValue: null,
    resourceUri: null,
    command: null,
  };
}

describe('expansion state', () => {
  it('falls back to the node default and prefers the recorded state', () => {
    const expansion = new Expansion(new ViewStateStore(new MemoryMemento()));
    assert.equal(expansion.expanded(node('a', true)), true);
    expansion.set('a', false);
    assert.equal(expansion.expanded(node('a', true)), false);
    assert.equal(expansion.expanded(node('b', false)), false);
  });

  it('persists the map and the epoch across instances', async () => {
    const memento = new MemoryMemento();
    const first = new Expansion(new ViewStateStore(memento));
    first.set('a', true);
    first.reset(false);
    first.set('b', true);
    await Promise.resolve();
    const second = new Expansion(new ViewStateStore(memento));
    assert.equal(second.currentEpoch, 1);
    assert.equal(second.expanded(node('a', false)), false, 'reset forgot a');
    assert.equal(second.expanded(node('b', false)), true);
    assert.equal(second.itemId('x/y'), '1/x/y');
  });

  it('bumps the epoch at the next root refresh when the server re-renders', () => {
    const expansion = new Expansion(new ViewStateStore(new MemoryMemento()));
    expansion.set('a', true);
    expansion.reset(true);
    assert.equal(expansion.currentEpoch, 0);
    assert.equal(expansion.expanded(node('a', false)), false);
    expansion.onRootRefresh();
    assert.equal(expansion.currentEpoch, 1);
    expansion.onRootRefresh();
    assert.equal(expansion.currentEpoch, 1, 'ordinary root refreshes keep the epoch');
  });
});

describe('whole-tree changes', () => {
  const empty: PersistedViewState = { currentFilter: '', filtered: false, includeGlobs: [], excludeGlobs: [] };
  const settings = (tree: object, viewState: Partial<PersistedViewState> = {}): Settings =>
    buildConfiguration({
      groups: {
        general: {},
        highlights: {},
        filtering: {},
        tree: { flat: false, tagsOnly: false, expanded: false, groupedByTag: false, groupedBySubTag: false, scanMode: 'workspace', ...tree },
        regex: {},
      },
      filesExclude: {},
      searchExclude: {},
      explorerCompactFolders: false,
      viewState: { ...empty, ...viewState },
    });

  it('are view mode, grouping, expansion default and scan mode changes', () => {
    const base = settings({});
    assert.equal(replacesTree({ before: base, after: settings({}, { expanded: true }) }), true);
    assert.equal(replacesTree({ before: base, after: settings({ groupedByTag: true }) }), true);
    assert.equal(replacesTree({ before: base, after: settings({ scanMode: 'open files' }) }), true);
    assert.equal(replacesTree({ before: base, after: settings({ showBadges: false }) }), false);
  });

  it('ignore a setting that clicked view state overrides', () => {
    const before = settings({ expanded: false }, { expanded: true });
    const after = settings({ expanded: true }, { expanded: true });
    assert.equal(replacesTree({ before, after }), false);
  });
});
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `pnpm -C extension typecheck`
Expected: FAIL. `error TS2339: Property 'epoch' does not exist on type 'TreeHooks'`, and `error TS2307: Cannot find module` for `../../config/sync` and `../../tree/expansion`.

- [ ] **Step 3: Write the implementation**

Create `extension/src/commands/expand.ts`:

```ts
// Expand Tree and Collapse Tree (spec 7.5).

import * as vscode from 'vscode';
import { replacesTree, type ConfigurationSync } from '../config/sync';
import type { ViewStateStore } from '../state/viewState';
import type { Expansion } from '../tree/expansion';
import type { TreeProvider } from '../tree/provider';

export interface ExpandDeps {
  store: ViewStateStore;
  sync: ConfigurationSync;
  expansion: Expansion;
  provider: TreeProvider;
}

/** Resets expansion after a push: at the server's root refresh if one is coming. */
export function resetExpansion(deps: Pick<ExpandDeps, 'expansion' | 'provider'>, replaced: boolean): void {
  deps.expansion.reset(replaced);
  if (!replaced) deps.provider.refreshAll();
}

export function registerExpandCommands(deps: ExpandDeps): vscode.Disposable[] {
  const setExpanded = async (expanded: boolean) => {
    await deps.store.setFlags({ expanded });
    resetExpansion(deps, replacesTree(deps.sync.push()));
  };
  return [
    vscode.commands.registerCommand('clippings.expand', () => setExpanded(true)),
    vscode.commands.registerCommand('clippings.collapse', () => setExpanded(false)),
  ];
}
```

Create `extension/src/config/sync.ts`:

```ts
// Keeps the server's configuration in step with settings and view state:
// each push reads the configuration afresh and sends it.

import type { Settings } from '../protocol';
import { effectiveView } from './configuration';

export interface Push {
  before: Settings;
  after: Settings;
}

/**
 * Whether the server rebuilds the whole tree for this change and reports it
 * as a root refresh: a view mode, grouping or scan mode change (spec 5.12).
 */
export function replacesTree({ before, after }: Push): boolean {
  const a = effectiveView(before.tree, before.viewState);
  const b = effectiveView(after.tree, after.viewState);
  return (
    (Object.keys(a) as (keyof typeof a)[]).some((k) => a[k] !== b[k]) || before.tree.scanMode !== after.tree.scanMode
  );
}

export class ConfigurationSync {
  private last: Settings;
  private readonly listeners: ((push: Push) => void)[] = [];

  constructor(
    private readonly read: () => Settings,
    private readonly send: (settings: Settings) => void,
  ) {
    this.last = read();
  }

  /** The configuration most recently read. */
  get current(): Settings {
    return this.last;
  }

  /** Reads and sends the configuration, and tells listeners what changed. */
  push(): Push {
    const push = { before: this.last, after: this.read() };
    this.last = push.after;
    this.send(push.after);
    for (const l of this.listeners) l(push);
    return push;
  }

  onPush(listener: (push: Push) => void): void {
    this.listeners.push(listener);
  }
}
```

Create `extension/src/tree/expansion.ts`:

```ts
// Client-side expansion state (spec 7.5): a map from node ID to expanded,
// persisted in workspace storage, and the epoch that prefixes tree item IDs
// so VS Code forgets its own expansion state when the whole tree resets.

import type { ViewNode } from '../protocol';
import type { ViewStateStore } from '../state/viewState';

export class Expansion {
  private nodes: Record<string, boolean>;
  private epoch: number;
  /** Bump the epoch at the next root refresh, once the server has re-rendered. */
  private bumpAtRoot = false;

  constructor(private readonly store: ViewStateStore) {
    this.nodes = store.expandedNodes();
    this.epoch = store.epoch();
  }

  get currentEpoch(): number {
    return this.epoch;
  }

  /** The tree item ID for a node: the node ID prefixed with the epoch. */
  itemId(id: string): string {
    return `${this.epoch}/${id}`;
  }

  expanded(node: ViewNode): boolean {
    return this.nodes[node.id] ?? node.defaultExpanded;
  }

  set(id: string, expanded: boolean): void {
    this.nodes[id] = expanded;
    void this.store.setExpandedNodes(this.nodes);
  }

  /**
   * Forgets every node's state and starts a new epoch. When the server is
   * about to re-render the tree with new defaults (`awaitServer`), the epoch
   * changes at that root refresh, so VS Code reads the new defaults under
   * new IDs; otherwise it changes now and the caller refreshes the root.
   */
  reset(awaitServer: boolean): void {
    this.nodes = {};
    void this.store.setExpandedNodes(this.nodes);
    if (awaitServer) this.bumpAtRoot = true;
    else this.bump();
  }

  /** Called before the provider refetches the root for `clippings/treeChanged`. */
  onRootRefresh(): void {
    if (!this.bumpAtRoot) return;
    this.bumpAtRoot = false;
    this.bump();
  }

  private bump(): void {
    this.epoch += 1;
    void this.store.bumpEpoch();
  }
}
```

Replace `extension/src/testApi.ts` with:

```ts
// What `activate` returns. The `test` hooks let the integration tests
// observe state VS Code offers no API to read (spec 12.5).

import type * as vscode from 'vscode';
import type { StatusParams } from './protocol';
import type { TestItem } from './tree/testItems';

export interface ServerHooks {
  readonly running: boolean;
  readonly pid: number | undefined;
  readonly onStatus: vscode.Event<StatusParams>;
  readonly onRunning: vscode.Event<void>;
  /** Fires when repeated crashes stop the automatic restarts (spec 7.4). */
  readonly onGaveUp: vscode.Event<string>;
  /** Shortens the start timeout (`START_TIMEOUT_MS`) so tests need not wait for it. */
  setStartTimeout(ms: number): void;
  status(): StatusParams | undefined;
}

export interface TreeHooks {
  readonly view: vscode.TreeView<string>;
  /** Fires whenever the provider asks VS Code to refetch. */
  readonly onDidChange: vscode.Event<unknown>;
  /** The provider's children of `parent` (top level when omitted), as tree items. */
  items(parent?: string): Promise<TestItem[]>;
  /** The current tree item epoch (spec 7.5). */
  readonly epoch: number;
}

export interface TestHooks {
  readonly server: ServerHooks;
  readonly tree: TreeHooks;
}

export interface ClippingsApi {
  readonly version: string;
  readonly test: TestHooks;
}
```

Replace `extension/src/extension.ts` with:

```ts
// Composition root: builds each part of the extension and wires them to the
// server connection.

import * as vscode from 'vscode';
import { registerExpandCommands, resetExpansion } from './commands/expand';
import { needsRestart, registerServerCommands } from './commands/server';
import { affectsServer, readConfiguration } from './config/read';
import { ConfigurationSync, replacesTree } from './config/sync';
import { IconResolver } from './icons/resolver';
import type { StatusParams } from './protocol';
import { ServerConnection } from './server/connection';
import { ViewStateStore } from './state/viewState';
import type { ClippingsApi } from './testApi';
import { Expansion } from './tree/expansion';
import { NodeCache } from './tree/nodeCache';
import { TreeProvider } from './tree/provider';
import { testItem } from './tree/testItems';

let connection: ServerConnection | undefined;

export function activate(context: vscode.ExtensionContext): ClippingsApi {
  const log = vscode.window.createOutputChannel('Clippings', { log: true });
  const store = new ViewStateStore(context.workspaceState);
  const activeUri = () => vscode.window.activeTextEditor?.document.uri.toString() ?? null;
  const server = new ServerConnection({
    context,
    log,
    settings: () => sync.current,
    activeUri,
    error: (message, ...actions) => vscode.window.showErrorMessage(message, ...actions),
  });
  connection = server;
  const sync = new ConfigurationSync(
    () => readConfiguration(store.snapshot()),
    (settings) => server.configure(settings),
  );
  let lastStatus: StatusParams | undefined;

  const cache = new NodeCache();
  const expansion = new Expansion(store);
  const provider = new TreeProvider(server, cache, {
    icons: new IconResolver(),
    itemId: (id) => expansion.itemId(id),
    expanded: (node) => expansion.expanded(node),
  });
  const treeView = vscode.window.createTreeView('clippings-view', { treeDataProvider: provider });

  context.subscriptions.push(
    log,
    server,
    provider,
    treeView,
    ...registerServerCommands(server, log),
    ...registerExpandCommands({ store, sync, expansion, provider }),
    server.onStatus((s) => (lastStatus = s)),
    server.onNewInstance(() => provider.reset()),
    server.onTreeChanged((p) => {
      if (p.refresh.includes(null)) expansion.onRootRefresh();
      provider.refresh(p.refresh);
    }),
    treeView.onDidExpandElement((e) => expansion.set(e.element, true)),
    treeView.onDidCollapseElement((e) => expansion.set(e.element, false)),
    vscode.workspace.onDidChangeConfiguration((e) => {
      if (needsRestart(e)) {
        sync.push();
        void server.restart();
        return;
      }
      if (!affectsServer(e)) return;
      const push = sync.push();
      if (e.affectsConfiguration('clippings.tree.expanded')) {
        resetExpansion({ expansion, provider }, replacesTree(push));
      }
    }),
    vscode.window.onDidChangeActiveTextEditor(() => server.activeEditor(activeUri())),
  );
  void server.start();

  const manifest = context.extension.packageJSON as { version: string };
  return {
    version: manifest.version,
    test: {
      server: {
        get running() {
          return server.running;
        },
        get pid() {
          return server.pid;
        },
        onStatus: server.onStatus,
        onRunning: server.onRunning,
        onGaveUp: server.onGaveUp,
        setStartTimeout: (ms) => (server.startTimeoutMs = ms),
        status: () => lastStatus,
      },
      tree: {
        view: treeView,
        onDidChange: provider.onDidChangeTreeData,
        items: async (parent) => {
          const ids = await provider.getChildren(parent);
          return ids.map((id) => testItem(id, provider.getTreeItem(id)));
        },
        get epoch() {
          return expansion.currentEpoch;
        },
      },
    },
  };
}

export async function deactivate(): Promise<void> {
  await connection?.stop();
  connection = undefined;
}
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `pnpm -C extension dev && pnpm -C extension typecheck && pnpm -C extension build`
Expected: no errors.

Run: `pnpm -C extension test:unit`
Expected: `43 passing`.

Run: `pnpm -C extension test:integration`
Expected: `16 passing`. A VS Code test window opens, runs the suite and closes.

- [ ] **Step 5: Commit**

```bash
git add -A
git commit -F - <<'EOF'
feat(extension): keep expansion state on the client with an epoch for whole-tree resets

Co-Authored-By: <your model attribution>
EOF
```

### Task 9: Reveal, track file, todo clicks and the first-scan prompt

`Revealer` implements Reveal Current File In Tree and track file (spec 7.5): it calls `clippings/find`, records the returned paths in the node cache and reveals the file node without taking focus. Track file waits 500 ms after an editor change and checks `tree.autoRefresh`, `tree.trackFile`, the scheme and the view's visibility (rulings 6 and 7). A todo click opens the document at the node's position and flashes the line for 150 ms with one reused decoration type. `clippings.openUrl`, `clippings.refresh` and `clippings.stopScan` are registered, and the view shows `Click the refresh button to scan...` while `needsScan` is set.

**Files:**
- Create: `extension/src/commands/navigation.ts`, `extension/src/commands/scan.ts`, `extension/src/tree/open.ts`, `extension/src/tree/reveal.ts`
- Modify: `extension/src/testApi.ts`, `extension/src/extension.ts`
- Test: `extension/src/test/integration/helpers.ts` (modified), `extension/src/test/integration/reveal.test.ts`

**Interfaces:**
- Consumes: `protocol.ts` (Task 2): `Position`, `Settings`, `ViewNode`; `server/connection.ts` (Task 6): `ServerConnection`; `tree/nodeCache.ts` (Task 7): `NodeCache`.
- Produces, in `commands/navigation.ts`: `function registerNavigationCommands(revealer: Revealer, flash: LineFlash): vscode.Disposable[]`.
- Produces, in `commands/scan.ts`: `function registerScanCommands(server: ServerConnection): vscode.Disposable[]`; `NEEDS_SCAN_MESSAGE` (const).
- Produces, in `tree/open.ts`: `FLASH_MS` (const); `class LineFlash implements vscode.Disposable` with `last: { uri: string; line: number } | undefined`, `flash(editor: vscode.TextEditor): void`, `dispose(): void`; `async function revealInFile(flash: LineFlash, uri: unknown, position: unknown): Promise<void>`.
- Produces, in `tree/reveal.ts`: `RevealDeps` (interface); `TRACK_DELAY_MS` (const); `class Revealer implements vscode.Disposable` with `constructor(private readonly deps: RevealDeps)`, `async reveal(uri: vscode.Uri, focus: boolean): Promise<boolean>`, `async revealActive(): Promise<void>`, `onActiveEditor(editor: vscode.TextEditor | undefined): void`, `dispose(): void`.
- Produces, in `testApi.ts`: `TreeHooks` gains `lastFlash(): { uri: string; line: number } | undefined`.

- [ ] **Step 1: Write the failing tests**

Replace `extension/src/test/integration/helpers.ts` with:

```ts
// Shared helpers for the integration tests. Waits are driven by real
// signals from the extension, never by fixed sleeps.

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

export function workspacePath(...parts: string[]): string {
  const root = process.env['CLIPPINGS_TEST_WORKSPACE'];
  if (!root) throw new Error('CLIPPINGS_TEST_WORKSPACE is not set');
  return [root, ...parts].join('/');
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
```

Create `extension/src/test/integration/reveal.test.ts`:

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
    await waitFor('strings.py selected', () => selected(api)?.endsWith(`/f:${path}`), [
      api.test.tree.view.onDidChangeSelection,
    ]);
  });

  it('reveals the current file on demand when tracking is off', async () => {
    await setSetting('tree.trackFile', false);
    try {
      const path = workspacePath('docs', 'plan.md');
      await open(path);
      assert.ok(!selected(api)?.endsWith(`/f:${path}`));
      await vscode.commands.executeCommand('clippings.reveal');
      await waitFor('plan.md selected', () => selected(api)?.endsWith(`/f:${path}`), [
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

- [ ] **Step 2: Run the tests to verify they fail**

Run: `pnpm -C extension typecheck`
Expected: FAIL. `error TS2307: Cannot find module '../../commands/scan'` and `error TS2339: Property 'lastFlash' does not exist on type 'TreeHooks'`.

- [ ] **Step 3: Write the implementation**

Create `extension/src/commands/navigation.ts`:

```ts
// Commands that open things: reveal in the tree, reveal in the file, open a
// sub-tag URL (spec 7.5, 7.6).

import * as vscode from 'vscode';
import type { Revealer } from '../tree/reveal';
import { revealInFile, type LineFlash } from '../tree/open';

export function registerNavigationCommands(revealer: Revealer, flash: LineFlash): vscode.Disposable[] {
  return [
    vscode.commands.registerCommand('clippings.reveal', () => revealer.revealActive()),
    vscode.commands.registerCommand('clippings.revealInFile', (uri: unknown, position: unknown) =>
      revealInFile(flash, uri, position),
    ),
    vscode.commands.registerCommand('clippings.openUrl', (url: unknown) =>
      typeof url === 'string' ? vscode.env.openExternal(vscode.Uri.parse(url)) : false,
    ),
  ];
}
```

Create `extension/src/commands/scan.ts`:

```ts
// Refresh and stop scan (spec 5.9, 7.7).

import * as vscode from 'vscode';
import type { ServerConnection } from '../server/connection';

export function registerScanCommands(server: ServerConnection): vscode.Disposable[] {
  return [
    vscode.commands.registerCommand('clippings.refresh', () => server.rescan()),
    vscode.commands.registerCommand('clippings.stopScan', () => server.stopScan()),
  ];
}

export const NEEDS_SCAN_MESSAGE = 'Click the refresh button to scan...';
```

Create `extension/src/tree/open.ts`:

```ts
// Todo clicks (spec 7.5): open the document at the node's position and
// flash the line for 150 ms with one reused decoration type.

import * as vscode from 'vscode';
import type { Position } from '../protocol';

export const FLASH_MS = 150;

export class LineFlash implements vscode.Disposable {
  private readonly type = vscode.window.createTextEditorDecorationType({
    isWholeLine: true,
    backgroundColor: new vscode.ThemeColor('editor.rangeHighlightBackground'),
  });
  private timer: NodeJS.Timeout | undefined;
  /** The last flashed line, for the tests. */
  last: { uri: string; line: number } | undefined;

  flash(editor: vscode.TextEditor): void {
    const line = editor.selection.active.line;
    clearTimeout(this.timer);
    editor.setDecorations(this.type, [editor.document.lineAt(line).range]);
    this.last = { uri: editor.document.uri.toString(), line };
    this.timer = setTimeout(() => editor.setDecorations(this.type, []), FLASH_MS);
  }

  dispose(): void {
    clearTimeout(this.timer);
    this.type.dispose();
  }
}

export async function revealInFile(flash: LineFlash, uri: unknown, position: unknown): Promise<void> {
  const target = typeof uri === 'string' ? vscode.Uri.parse(uri) : uri instanceof vscode.Uri ? uri : undefined;
  if (!target) return;
  const p = position as Partial<Position> | undefined;
  const options: vscode.TextDocumentShowOptions = {};
  if (typeof p?.line === 'number' && typeof p.character === 'number') {
    const at = new vscode.Position(p.line, p.character);
    options.selection = new vscode.Range(at, at);
  }
  await vscode.commands.executeCommand('vscode.open', target, options);
  const editor = vscode.window.activeTextEditor;
  if (editor && editor.document.uri.toString() === target.toString()) flash.flash(editor);
}
```

Create `extension/src/tree/reveal.ts`:

```ts
// Reveal Current File In Tree and track file (spec 7.5).

import * as vscode from 'vscode';
import type { Settings, ViewNode } from '../protocol';
import type { NodeCache } from './nodeCache';

export interface RevealDeps {
  find(uri: string, line: number | null): Promise<ViewNode[][]>;
  cache: NodeCache;
  view: vscode.TreeView<string>;
  settings(): Settings;
}

export const TRACK_DELAY_MS = 500;

export class Revealer implements vscode.Disposable {
  private timer: NodeJS.Timeout | undefined;

  constructor(private readonly deps: RevealDeps) {}

  /** Reveals the document's file node; false when the tree has none. */
  async reveal(uri: vscode.Uri, focus: boolean): Promise<boolean> {
    const paths = await this.deps.find(uri.toString(), null);
    const first = paths[0];
    const target = first?.at(-1);
    if (!target) return false;
    for (const path of paths) this.deps.cache.recordPath(path);
    await this.deps.view.reveal(target.id, { select: true, focus, expand: false });
    return true;
  }

  /** The `clippings.reveal` command: the active editor's file, if the view is visible. */
  async revealActive(): Promise<void> {
    const editor = vscode.window.activeTextEditor;
    if (editor && this.deps.view.visible) await this.reveal(editor.document.uri, false);
  }

  /** Track file: reveal the new active editor's file after a pause. */
  onActiveEditor(editor: vscode.TextEditor | undefined): void {
    clearTimeout(this.timer);
    if (!editor) return;
    const uri = editor.document.uri;
    this.timer = setTimeout(() => {
      const { tree, general } = this.deps.settings();
      if (tree.autoRefresh && tree.trackFile && general.schemes.includes(uri.scheme) && this.deps.view.visible) {
        void this.reveal(uri, false);
      }
    }, TRACK_DELAY_MS);
  }

  dispose(): void {
    clearTimeout(this.timer);
  }
}
```

Replace `extension/src/testApi.ts` with:

```ts
// What `activate` returns. The `test` hooks let the integration tests
// observe state VS Code offers no API to read (spec 12.5).

import type * as vscode from 'vscode';
import type { StatusParams } from './protocol';
import type { TestItem } from './tree/testItems';

export interface ServerHooks {
  readonly running: boolean;
  readonly pid: number | undefined;
  readonly onStatus: vscode.Event<StatusParams>;
  readonly onRunning: vscode.Event<void>;
  /** Fires when repeated crashes stop the automatic restarts (spec 7.4). */
  readonly onGaveUp: vscode.Event<string>;
  /** Shortens the start timeout (`START_TIMEOUT_MS`) so tests need not wait for it. */
  setStartTimeout(ms: number): void;
  status(): StatusParams | undefined;
}

export interface TreeHooks {
  readonly view: vscode.TreeView<string>;
  /** Fires whenever the provider asks VS Code to refetch. */
  readonly onDidChange: vscode.Event<unknown>;
  /** The provider's children of `parent` (top level when omitted), as tree items. */
  items(parent?: string): Promise<TestItem[]>;
  /** The current tree item epoch (spec 7.5). */
  readonly epoch: number;
  /** The line the last todo click flashed. */
  lastFlash(): { uri: string; line: number } | undefined;
}

export interface TestHooks {
  readonly server: ServerHooks;
  readonly tree: TreeHooks;
}

export interface ClippingsApi {
  readonly version: string;
  readonly test: TestHooks;
}
```

Replace `extension/src/extension.ts` with:

```ts
// Composition root: builds each part of the extension and wires them to the
// server connection.

import * as vscode from 'vscode';
import { registerExpandCommands, resetExpansion } from './commands/expand';
import { registerNavigationCommands } from './commands/navigation';
import { NEEDS_SCAN_MESSAGE, registerScanCommands } from './commands/scan';
import { needsRestart, registerServerCommands } from './commands/server';
import { affectsServer, readConfiguration } from './config/read';
import { ConfigurationSync, replacesTree } from './config/sync';
import { IconResolver } from './icons/resolver';
import type { StatusParams } from './protocol';
import { ServerConnection } from './server/connection';
import { ViewStateStore } from './state/viewState';
import type { ClippingsApi } from './testApi';
import { Expansion } from './tree/expansion';
import { NodeCache } from './tree/nodeCache';
import { LineFlash } from './tree/open';
import { TreeProvider } from './tree/provider';
import { Revealer } from './tree/reveal';
import { testItem } from './tree/testItems';

let connection: ServerConnection | undefined;

export function activate(context: vscode.ExtensionContext): ClippingsApi {
  const log = vscode.window.createOutputChannel('Clippings', { log: true });
  const store = new ViewStateStore(context.workspaceState);
  const activeUri = () => vscode.window.activeTextEditor?.document.uri.toString() ?? null;
  const server = new ServerConnection({
    context,
    log,
    settings: () => sync.current,
    activeUri,
    error: (message, ...actions) => vscode.window.showErrorMessage(message, ...actions),
  });
  connection = server;
  const sync = new ConfigurationSync(
    () => readConfiguration(store.snapshot()),
    (settings) => server.configure(settings),
  );
  let lastStatus: StatusParams | undefined;

  const cache = new NodeCache();
  const expansion = new Expansion(store);
  const provider = new TreeProvider(server, cache, {
    icons: new IconResolver(),
    itemId: (id) => expansion.itemId(id),
    expanded: (node) => expansion.expanded(node),
  });
  const treeView = vscode.window.createTreeView('clippings-view', { treeDataProvider: provider });
  const revealer = new Revealer({
    find: (uri, line) => server.find(uri, line),
    cache,
    view: treeView,
    settings: () => sync.current,
  });
  const flash = new LineFlash();

  context.subscriptions.push(
    log,
    server,
    provider,
    treeView,
    ...registerServerCommands(server, log),
    revealer,
    flash,
    ...registerExpandCommands({ store, sync, expansion, provider }),
    ...registerNavigationCommands(revealer, flash),
    ...registerScanCommands(server),
    server.onStatus((s) => {
      lastStatus = s;
      treeView.message = s.needsScan && !s.scanning ? NEEDS_SCAN_MESSAGE : undefined;
    }),
    server.onNewInstance(() => provider.reset()),
    server.onTreeChanged((p) => {
      if (p.refresh.includes(null)) expansion.onRootRefresh();
      provider.refresh(p.refresh);
    }),
    treeView.onDidExpandElement((e) => expansion.set(e.element, true)),
    treeView.onDidCollapseElement((e) => expansion.set(e.element, false)),
    vscode.workspace.onDidChangeConfiguration((e) => {
      if (needsRestart(e)) {
        sync.push();
        void server.restart();
        return;
      }
      if (!affectsServer(e)) return;
      const push = sync.push();
      if (e.affectsConfiguration('clippings.tree.expanded')) {
        resetExpansion({ expansion, provider }, replacesTree(push));
      }
    }),
    vscode.window.onDidChangeActiveTextEditor((editor) => {
      server.activeEditor(activeUri());
      revealer.onActiveEditor(editor);
    }),
  );
  void server.start();

  const manifest = context.extension.packageJSON as { version: string };
  return {
    version: manifest.version,
    test: {
      server: {
        get running() {
          return server.running;
        },
        get pid() {
          return server.pid;
        },
        onStatus: server.onStatus,
        onRunning: server.onRunning,
        onGaveUp: server.onGaveUp,
        setStartTimeout: (ms) => (server.startTimeoutMs = ms),
        status: () => lastStatus,
      },
      tree: {
        view: treeView,
        onDidChange: provider.onDidChangeTreeData,
        items: async (parent) => {
          const ids = await provider.getChildren(parent);
          return ids.map((id) => testItem(id, provider.getTreeItem(id)));
        },
        get epoch() {
          return expansion.currentEpoch;
        },
        lastFlash: () => flash.last,
      },
    },
  };
}

export async function deactivate(): Promise<void> {
  await connection?.stop();
  connection = undefined;
}
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `pnpm -C extension dev && pnpm -C extension typecheck && pnpm -C extension build`
Expected: no errors.

Run: `pnpm -C extension test:unit`
Expected: `43 passing`.

Run: `pnpm -C extension test:integration`
Expected: `20 passing`. A VS Code test window opens, runs the suite and closes.

- [ ] **Step 5: Commit**

```bash
git add -A
git commit -F - <<'EOF'
feat(extension): reveal and track files, open clicked todos and prompt for a first scan

Co-Authored-By: <your model attribution>
EOF
```

### Task 10: View style, grouping, filter and scope commands

The view-state commands of spec 7.6: flat, tags-only and tree views, grouping by tag and sub-tag, the tree filter, Reset Cache, the folder and file filters, scopes, Remove Filter and Reset All Filters (ruling 2). Each writes workspace storage and pushes the configuration. `filters/globs.ts` turns a node's own key into a path and the path into an escaped glob (rulings 10 and 11, Review Focus 3), and labels Remove Filter's choices as todo-tree does. `ui/prompts.ts` is the one route for prompts and messages, which the tests script (ruling 12); the server connection's error and crash notices now go through it too, and the start-failure test checks that notice and that the tree loads after the restart.

**Files:**
- Create: `extension/src/commands/filters.ts`, `extension/src/commands/view.ts`, `extension/src/filters/globs.ts`, `extension/src/ui/prompts.ts`
- Modify: `extension/src/testApi.ts`, `extension/src/extension.ts`
- Test: `extension/src/test/integration/fixture.ts` (modified), `extension/src/test/integration/filters.test.ts`, `extension/src/test/integration/server.test.ts` (modified), `extension/src/test/integration/views.test.ts`, `extension/src/test/unit/globs.test.ts`

**Interfaces:**
- Consumes: `commands/expand.ts` (Task 8): `resetExpansion`; `config/sync.ts` (Task 8): `ConfigurationSync`, `replacesTree`; `state/viewState.ts` (Task 2): `PersistedViewState`, `ViewFlags`, `ViewStateStore`; `tree/expansion.ts` (Task 8): `Expansion`; `tree/nodeCache.ts` (Task 7): `NodeCache`; `tree/provider.ts` (Task 7): `TreeProvider`.
- Produces, in `commands/filters.ts`: `FilterDeps` (interface); `function registerFilterCommands(deps: FilterDeps): vscode.Disposable[]`.
- Produces, in `commands/view.ts`: `ViewDeps` (interface); `function registerViewCommands(deps: ViewDeps): vscode.Disposable[]`.
- Produces, in `filters/globs.ts`: `function escapeGlob(path: string): string`; `function slashPath(path: string, platform: NodeJS.Platform = process.platform): string`; `function folderGlob(path: string, recursive: boolean): string`; `function fileGlob(path: string): string`; `function nodePath(ownKey: string): string | undefined`; `function toGlobArray(value: unknown): string[]`; `Scope` (interface); `function scopesOf(value: unknown): Scope[]`; `function isCurrentScope(scope: Scope, include: string[], exclude: string[]): boolean`; `Removal` (type); `CLEAR_TREE_FILTER` (const); `function removalChoices(filter: string, include: string[], exclude: string[]): Map<string, Removal>`.
- Produces, in `ui/prompts.ts`: `Shown` (type); `class Prompts` with `readonly shown: Shown[]`, `script(...answers: unknown[]): void`, `async input(options: vscode.InputBoxOptions): Promise<string | undefined>`, `async pick(items: string[], options: vscode.QuickPickOptions & { canPickMany: true }): Promise<string[] | undefined>`, `async pick(items: string[], options: vscode.QuickPickOptions): Promise<string | undefined>`, `async pick(items: string[], options: vscode.QuickPickOptions): Promise<string | string[] | undefined>`, `async pickItem(items: vscode.QuickPickItem[], options: vscode.QuickPickOptions): Promise<string | undefined>`, `async message(kind: 'info' | 'warning' | 'error', message: string, ...actions: string[]): Promise<string | undefined>`.
- Produces, in `testApi.ts`: `TestHooks` gains `readonly prompts: Prompts`, `viewState(): PersistedViewState`.

- [ ] **Step 1: Write the failing tests**

Replace `extension/src/test/integration/fixture.ts` with:

```ts
// Expected views of tests/fixtures/workspace.

/** The default tree view, as `outline` prints it. */
export const DEFAULT_TREE = [
  '(Scan mode: workspace and open files)',
  'workspace',
  '  docs',
  '    plan.md',
  '      [ ] write the guide',
  '      [x] pick a name',
  '  lib',
  '    notes.rs',
  '      TODO first line of a long note',
  '  src',
  '    util',
  '      strings.py',
  '        TODO normalise unicode before comparing',
  '        BUG (bob) drops leading tabs too',
  '    app.ts',
  '      TODO (alice) wire up the router',
  '      FIXME handle the error path',
  '      HACK temporary shim until the router lands',
];

export const FLAT_VIEW = [
  '(Scan mode: workspace and open files)',
  'workspace',
  '  plan.md (docs)',
  '    [ ] write the guide',
  '    [x] pick a name',
  '  notes.rs (lib)',
  '    TODO first line of a long note',
  '  app.ts (src)',
  '    TODO (alice) wire up the router',
  '    FIXME handle the error path',
  '    HACK temporary shim until the router lands',
  '  strings.py (src/util)',
  '    TODO normalise unicode before comparing',
  '    BUG (bob) drops leading tabs too',
];

export const TAGS_ONLY_VIEW = [
  '(Scan mode: workspace and open files)',
  'BUG (bob) drops leading tabs too',
  'HACK temporary shim until the router lands',
  'FIXME handle the error path',
  'TODO first line of a long note',
  'TODO (alice) wire up the router',
  'TODO normalise unicode before comparing',
  '[ ] write the guide',
  '[x] pick a name',
];

/** The tree view grouped by tag. */
export const TREE_GROUPED_BY_TAG = [
  '(Scan mode: workspace and open files)',
  'workspace',
  '  BUG',
  '    src/util',
  '      strings.py',
  '        BUG (bob) drops leading tabs too',
  '  HACK',
  '    src',
  '      app.ts',
  '        HACK temporary shim until the router lands',
  '  FIXME',
  '    src',
  '      app.ts',
  '        FIXME handle the error path',
  '  TODO',
  '    lib',
  '      notes.rs',
  '        TODO first line of a long note',
  '    src',
  '      util',
  '        strings.py',
  '          TODO normalise unicode before comparing',
  '      app.ts',
  '        TODO (alice) wire up the router',
  '  [ ]',
  '    docs',
  '      plan.md',
  '        [ ] write the guide',
  '  [x]',
  '    docs',
  '      plan.md',
  '        [x] pick a name',
];

export const FLAT_GROUPED_BY_TAG = [
  '(Scan mode: workspace and open files)',
  'workspace',
  '  BUG',
  '    strings.py (src/util)',
  '      BUG (bob) drops leading tabs too',
  '  HACK',
  '    app.ts (src)',
  '      HACK temporary shim until the router lands',
  '  FIXME',
  '    app.ts (src)',
  '      FIXME handle the error path',
  '  TODO',
  '    notes.rs (lib)',
  '      TODO first line of a long note',
  '    app.ts (src)',
  '      TODO (alice) wire up the router',
  '    strings.py (src/util)',
  '      TODO normalise unicode before comparing',
  '  [ ]',
  '    plan.md (docs)',
  '      [ ] write the guide',
  '  [x]',
  '    plan.md (docs)',
  '      [x] pick a name',
];

/** Tags only, grouped by tag: a tag with one todo is compacted away. */
export const TAGS_ONLY_GROUPED_BY_TAG = [
  '(Scan mode: workspace and open files)',
  'BUG (bob) drops leading tabs too',
  'HACK temporary shim until the router lands',
  'FIXME handle the error path',
  'TODO',
  '  TODO first line of a long note',
  '  TODO (alice) wire up the router',
  '  TODO normalise unicode before comparing',
  '[ ] write the guide',
  '[x] pick a name',
];
```

Create `extension/src/test/integration/filters.test.ts`:

```ts
import * as assert from 'node:assert/strict';
import { mkdirSync, rmSync, writeFileSync } from 'node:fs';
import * as vscode from 'vscode';
import type { ClippingsApi } from '../../testApi';
import { DEFAULT_TREE } from './fixture';
import { getApi, itemAt, setSetting, treeBecomes, whenIdle, workspacePath } from './helpers';

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
    assert.deepEqual(api.test.viewState().excludeGlobs, [`${workspacePath('src')}/**/*`]);
    api.test.prompts.script([`Exclude Folder: ${workspacePath('src')}`]);
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
    assert.deepEqual(api.test.viewState().excludeGlobs, [workspacePath('docs', 'plan.md')]);
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
    assert.deepEqual(api.test.viewState().includeGlobs, [`${workspacePath('src')}/**/*`]);
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
      assert.deepEqual(api.test.viewState().includeGlobs, [`${workspacePath('app [[]slug[]] (old)')}/*`]);
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

Replace `extension/src/test/integration/server.test.ts` with:

```ts
import * as assert from 'node:assert/strict';
import { chmodSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import * as vscode from 'vscode';
import { START_TIMEOUT_MS } from '../../server/connection';
import type { ClippingsApi } from '../../testApi';
import { DEFAULT_TREE } from './fixture';
import { getApi, treeBecomes, waitFor, whenIdle, withTimeout } from './helpers';

describe('server lifecycle', () => {
  let api: ClippingsApi;

  before(async () => {
    api = await getApi();
    await whenIdle(api);
  });

  it('starts the locally built server and receives its status', () => {
    const status = api.test.server.status();
    assert.ok(status);
    assert.match(status.instance, /^[0-9a-f]+-[0-9a-f]+$/);
    assert.equal(status.error, null);
    assert.ok(api.test.server.pid);
  });

  it('restarts on demand with a new server instance', async () => {
    const before = api.test.server.status()?.instance;
    await vscode.commands.executeCommand('clippings.restartServer');
    const s = api.test.server;
    await waitFor('a new instance', () => s.running && s.status()?.instance !== before, [s.onStatus, s.onRunning]);
    await whenIdle(api);
  });

  it('restarts a crashed server', async () => {
    const s = api.test.server;
    const instance = s.status()?.instance;
    const pid = s.pid;
    assert.ok(pid);
    process.kill(pid, 'SIGKILL');
    await waitFor(
      'a restarted server',
      () => s.running && s.pid !== pid && s.status()?.instance !== instance,
      [s.onStatus, s.onRunning],
    );
    await whenIdle(api);
  });

  it('restarts when a setting fixed at start changes', async () => {
    const s = api.test.server;
    const instance = s.status()?.instance;
    await vscode.workspace
      .getConfiguration('clippings')
      .update('server.logLevel', 'debug', vscode.ConfigurationTarget.Global);
    try {
      await waitFor('a restart', () => s.running && s.status()?.instance !== instance, [s.onStatus, s.onRunning]);
    } finally {
      await vscode.workspace
        .getConfiguration('clippings')
        .update('server.logLevel', undefined, vscode.ConfigurationTarget.Global);
    }
    await whenIdle(api);
  });

  it('stops restarting after five crashes in three minutes and restarts on demand', async function () {
    this.timeout(60_000);
    const s = api.test.server;
    await vscode.commands.executeCommand('clippings.restartServer');
    await whenIdle(api);
    let notice: string | undefined;
    const subscription = s.onGaveUp((message) => (notice = message));
    try {
      for (let crash = 1; crash <= 5; crash++) {
        const pid = s.pid;
        assert.ok(pid, `a running server before crash ${crash}`);
        process.kill(pid, 'SIGKILL');
        if (crash < 5) {
          await waitFor(`a restart after crash ${crash}`, () => s.running && s.pid !== pid, [s.onStatus, s.onRunning]);
        }
      }
      const message = await waitFor('the crash notice', () => notice, [s.onGaveUp]);
      assert.match(message, /crashed 5 times in the last 3 minutes/);
    } finally {
      subscription.dispose();
    }
    await vscode.commands.executeCommand('clippings.restartServer');
    await whenIdle(api);
    assert.ok(s.running);
  });

  it('counts a server that exits while starting as a crash and restarts on demand', async function () {
    if (process.platform === 'win32') this.skip();
    this.timeout(60_000);
    const s = api.test.server;
    // Passes the probe, then exits as soon as `initialize` arrives.
    const dir = mkdtempSync(join(tmpdir(), 'clippings-fake-'));
    const fake = join(dir, 'clippings');
    writeFileSync(
      fake,
      [
        '#!/bin/sh',
        'if [ "$1" = probe ]; then',
        '  echo \'{"version":"0.0.0","target":"fake","protocolVersion":1}\'',
        '  exit 0',
        'fi',
        'head -c 1 > /dev/null',
        'exit 1',
        '',
      ].join('\n'),
    );
    chmodSync(fake, 0o755);
    const real = process.env['CLIPPINGS_SERVER_PATH'];
    let notice: string | undefined;
    const subscription = s.onGaveUp((message) => (notice = message));
    try {
      process.env['CLIPPINGS_SERVER_PATH'] = fake;
      const restart = vscode.commands.executeCommand('clippings.restartServer');
      await withTimeout('Restart Server with a server that exits', restart);
      const message = await waitFor('the crash notice', () => notice, [s.onGaveUp]);
      assert.match(message, /crashed 5 times in the last 3 minutes/);
      assert.equal(s.running, false);
      const shown = api.test.prompts.shown.at(-1);
      assert.deepEqual(shown, { kind: 'error', message, actions: ['Restart', 'Show Log'] });
    } finally {
      process.env['CLIPPINGS_SERVER_PATH'] = real;
      subscription.dispose();
      rmSync(dir, { recursive: true, force: true });
    }
    const restart = vscode.commands.executeCommand('clippings.restartServer');
    await withTimeout('Restart Server with the real server', restart);
    await whenIdle(api);
    assert.ok(s.running);
    await treeBecomes(api, DEFAULT_TREE);
  });

  it('counts a server that does not finish starting in time as a crash and kills it', async function () {
    if (process.platform === 'win32') this.skip();
    this.timeout(60_000);
    const s = api.test.server;
    // Passes the probe, then reads `initialize` and never answers.
    const dir = mkdtempSync(join(tmpdir(), 'clippings-fake-'));
    const fake = join(dir, 'clippings');
    const pids = join(dir, 'pids');
    writeFileSync(
      fake,
      [
        '#!/bin/sh',
        'if [ "$1" = probe ]; then',
        '  echo \'{"version":"0.0.0","target":"fake","protocolVersion":1}\'',
        '  exit 0',
        'fi',
        `echo $$ >> '${pids}'`,
        // Keeps stdout open on fd 3 while discarding the requests.
        'exec cat 3>&1 > /dev/null',
        '',
      ].join('\n'),
    );
    chmodSync(fake, 0o755);
    const real = process.env['CLIPPINGS_SERVER_PATH'];
    let notice: string | undefined;
    const subscription = s.onGaveUp((message) => (notice = message));
    try {
      process.env['CLIPPINGS_SERVER_PATH'] = fake;
      s.setStartTimeout(500);
      const restart = vscode.commands.executeCommand('clippings.restartServer');
      await withTimeout('Restart Server with a server that hangs', restart);
      const message = await waitFor('the crash notice', () => notice, [s.onGaveUp]);
      assert.match(message, /crashed 5 times in the last 3 minutes/);
      assert.equal(s.running, false);
      const started = readFileSync(pids, 'utf8').trim().split('\n').map(Number);
      assert.equal(started.length, 5, 'five attempts');
      for (const pid of started) assert.throws(() => process.kill(pid, 0), /ESRCH/, `server process ${pid} is gone`);
      const shown = api.test.prompts.shown.at(-1);
      assert.deepEqual(shown, { kind: 'error', message, actions: ['Restart', 'Show Log'] });
    } finally {
      process.env['CLIPPINGS_SERVER_PATH'] = real;
      s.setStartTimeout(START_TIMEOUT_MS);
      subscription.dispose();
      rmSync(dir, { recursive: true, force: true });
    }
    const restart = vscode.commands.executeCommand('clippings.restartServer');
    await withTimeout('Restart Server with the real server', restart);
    await whenIdle(api);
    assert.ok(s.running);
    await treeBecomes(api, DEFAULT_TREE);
  });

  it('shows the log', async () => {
    await vscode.commands.executeCommand('clippings.showLog');
  });
});
```

Create `extension/src/test/integration/views.test.ts`:

```ts
import * as assert from 'node:assert/strict';
import * as vscode from 'vscode';
import type { ClippingsApi } from '../../testApi';
import {
  DEFAULT_TREE,
  FLAT_GROUPED_BY_TAG,
  FLAT_VIEW,
  TAGS_ONLY_GROUPED_BY_TAG,
  TAGS_ONLY_VIEW,
  TREE_GROUPED_BY_TAG,
} from './fixture';
import { getApi, treeBecomes, whenIdle } from './helpers';

async function run(...commands: string[]): Promise<void> {
  for (const command of commands) await vscode.commands.executeCommand(command);
}

describe('view modes', () => {
  let api: ClippingsApi;

  before(async () => {
    api = await getApi();
    await whenIdle(api);
    await treeBecomes(api, DEFAULT_TREE);
  });

  after(async () => {
    await run('clippings.resetCache');
    await treeBecomes(api, DEFAULT_TREE);
  });

  it('shows the flat view', async () => {
    await run('clippings.showFlatView');
    await treeBecomes(api, FLAT_VIEW);
    assert.equal(api.test.viewState().flat, true);
    assert.equal(api.test.viewState().tagsOnly, false);
  });

  it('shows the tags only view', async () => {
    await run('clippings.showTagsOnlyView');
    await treeBecomes(api, TAGS_ONLY_VIEW);
  });

  it('shows the tree view grouped by tag', async () => {
    await run('clippings.showTreeView', 'clippings.groupByTag');
    await treeBecomes(api, TREE_GROUPED_BY_TAG);
  });

  it('shows the flat view grouped by tag', async () => {
    await run('clippings.showFlatView');
    await treeBecomes(api, FLAT_GROUPED_BY_TAG);
  });

  it('shows the tags only view grouped by tag', async () => {
    await run('clippings.showTagsOnlyView');
    await treeBecomes(api, TAGS_ONLY_GROUPED_BY_TAG);
  });

  it('ungroups and returns to the tree view', async () => {
    await run('clippings.ungroupByTag', 'clippings.showTreeView');
    await treeBecomes(api, DEFAULT_TREE);
    assert.equal(api.test.viewState().groupedByTag, false);
  });

  it('groups and ungroups by sub tag', async () => {
    await vscode.workspace
      .getConfiguration('clippings.regex')
      .update('subTagRegex', '^\\s*\\((\\w+)\\)', vscode.ConfigurationTarget.Global);
    try {
      await run('clippings.groupBySubTag');
      await treeBecomes(api, [
        '(Scan mode: workspace and open files)',
        'workspace',
        '  alice',
        '    src',
        '      app.ts',
        '        TODO wire up the router',
        '  bob',
        '    src/util',
        '      strings.py',
        '        BUG drops leading tabs too',
        '  docs',
        '    plan.md',
        '      [ ] write the guide',
        '      [x] pick a name',
        '  lib',
        '    notes.rs',
        '      TODO first line of a long note',
        '  src',
        '    util',
        '      strings.py',
        '        TODO normalise unicode before comparing',
        '    app.ts',
        '      FIXME handle the error path',
        '      HACK temporary shim until the router lands',
      ]);
      await run('clippings.ungroupBySubTag');
      assert.equal(api.test.viewState().groupedBySubTag, false);
    } finally {
      await vscode.workspace
        .getConfiguration('clippings.regex')
        .update('subTagRegex', undefined, vscode.ConfigurationTarget.Global);
    }
    await treeBecomes(api, DEFAULT_TREE);
  });

  it('Reset Cache clears the clicked view state', async () => {
    await run('clippings.showFlatView', 'clippings.groupByTag');
    await treeBecomes(api, FLAT_GROUPED_BY_TAG);
    const epoch = api.test.tree.epoch;
    await run('clippings.resetCache');
    await treeBecomes(api, DEFAULT_TREE);
    assert.deepEqual(api.test.viewState(), { currentFilter: '', filtered: false, includeGlobs: [], excludeGlobs: [] });
    assert.ok(api.test.tree.epoch > epoch);
  });
});
```

Create `extension/src/test/unit/globs.test.ts`:

```ts
import * as assert from 'node:assert/strict';
import { pathToFileURL } from 'node:url';
import {
  escapeGlob,
  fileGlob,
  folderGlob,
  isCurrentScope,
  nodePath,
  removalChoices,
  scopesOf,
  slashPath,
  toGlobArray,
} from '../../filters/globs';

describe('temporary globs', () => {
  it('escapes metacharacters as globset does', () => {
    assert.equal(escapeGlob('/w/app/[slug]/a*b?{c}'), '/w/app/[[]slug[]]/a[*]b[?][{]c[}]');
    assert.equal(escapeGlob('/plain/path!'), '/plain/path!');
  });

  it('builds folder and file globs', () => {
    assert.equal(folderGlob('/w/src', false), '/w/src/*');
    assert.equal(folderGlob('/w/[id]', true), '/w/[[]id[]]/**/*');
    assert.equal(fileGlob('/w/a.ts'), '/w/a.ts');
  });

  it('uses forward slashes on Windows only', () => {
    assert.equal(slashPath('C:\\w\\src', 'win32'), 'C:/w/src');
    assert.equal(slashPath('/w/a\\b', 'linux'), '/w/a\\b');
  });

  it('reads node paths from own keys', () => {
    const root = process.platform === 'win32' ? 'C:\\work\\space' : '/work/space';
    assert.equal(nodePath(`w:${pathToFileURL(root).href}`), root);
    assert.equal(nodePath('d:/w/src'), '/w/src');
    assert.equal(nodePath('f:/w/src/a.ts'), '/w/src/a.ts');
    assert.equal(nodePath('f:C:\\w\\a.ts'), 'C:\\w\\a.ts');
    assert.equal(nodePath('f:untitled:Untitled-1'), undefined);
    assert.equal(nodePath('g:TODO'), undefined);
    assert.equal(nodePath('w:not a uri'), undefined);
  });
});

describe('scopes', () => {
  it('accepts comma-separated strings or arrays', () => {
    assert.deepEqual(toGlobArray(' a/** , b/*,,'), ['a/**', 'b/*']);
    assert.deepEqual(toGlobArray(['x', 1]), ['x']);
    assert.deepEqual(toGlobArray(undefined), []);
  });

  it('reads named scopes and spots the current one', () => {
    const scopes = scopesOf([{ name: 'src', includeGlobs: 'src/**' }, { excludeGlobs: [] }, 'bad', { name: 'all' }]);
    assert.deepEqual(scopes, [
      { name: 'src', includeGlobs: ['src/**'], excludeGlobs: [] },
      { name: 'all', includeGlobs: [], excludeGlobs: [] },
    ]);
    assert.equal(isCurrentScope(scopes[0]!, ['src/**'], []), true);
    assert.equal(isCurrentScope(scopes[0]!, [], []), false);
  });
});

describe('remove filter choices', () => {
  it('labels each filter as todo-tree does', () => {
    const choices = removalChoices('fix', ['/w/a/**/*', '/w/b/*', '*.ts'], ['/w/c/**/*', '/w/d.ts', '**/*.md']);
    assert.deepEqual(
      [...choices.keys()],
      [
        'Clear Tree Filter',
        'Exclude Folder: /w/c',
        'Exclude File: /w/d.ts',
        'Exclude: **/*.md',
        'Include Folder and Subfolders: /w/a',
        'Include Folder: /w/b',
        'Include: *.ts',
      ],
    );
    assert.deepEqual(choices.get('Include Folder: /w/b'), { kind: 'include', glob: '/w/b/*' });
    assert.ok(!removalChoices('', [], []).has('Clear Tree Filter'));
  });
});
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `pnpm -C extension typecheck`
Expected: FAIL. `error TS2339` for `prompts` and `viewState` on `TestHooks`, and `error TS2307: Cannot find module '../../filters/globs'`.

- [ ] **Step 3: Write the implementation**

Create `extension/src/commands/filters.ts`:

```ts
// Folder and file filters, scopes, Remove Filter and Reset All Filters
// (spec 5.3, 7.6; inventory 6.2).

import * as vscode from 'vscode';
import type { ConfigurationSync } from '../config/sync';
import {
  fileGlob,
  folderGlob,
  isCurrentScope,
  nodePath,
  removalChoices,
  scopesOf,
} from '../filters/globs';
import type { ViewStateStore } from '../state/viewState';
import type { NodeCache } from '../tree/nodeCache';
import type { Prompts } from '../ui/prompts';

export interface FilterDeps {
  store: ViewStateStore;
  sync: ConfigurationSync;
  cache: NodeCache;
  prompts: Prompts;
}

export function registerFilterCommands(deps: FilterDeps): vscode.Disposable[] {
  const { store, sync, cache, prompts } = deps;
  const pathOf = (element: unknown) => (typeof element === 'string' ? nodePath(cache.ownKey(element)) : undefined);
  const setGlobs = async (include: string[], exclude: string[]) => {
    await store.setGlobs(include, exclude);
    sync.push();
  };
  const onlyFolder = (recursive: boolean) => async (element: unknown) => {
    const path = pathOf(element);
    if (path === undefined) return;
    await setGlobs([folderGlob(path, recursive)], store.snapshot().excludeGlobs);
  };
  const exclude = (glob: (path: string) => string) => async (element: unknown) => {
    const path = pathOf(element);
    if (path === undefined) return;
    const { includeGlobs, excludeGlobs } = store.snapshot();
    const g = glob(path);
    if (!excludeGlobs.includes(g)) await setGlobs(includeGlobs, [...excludeGlobs, g]);
  };

  const commands: Record<string, (...args: unknown[]) => unknown> = {
    'clippings.showOnlyThisFolder': onlyFolder(false),
    'clippings.showOnlyThisFolderAndSubfolders': onlyFolder(true),
    'clippings.excludeThisFolder': exclude((p) => folderGlob(p, true)),
    'clippings.excludeThisFile': exclude(fileGlob),
    'clippings.switchScope': async () => {
      const scopes = scopesOf(vscode.workspace.getConfiguration('clippings.filtering').get('scopes'));
      if (scopes.length === 0) {
        const choice = await prompts.message(
          'warning',
          'Clippings: No scopes configured (see clippings.filtering.scopes setting)',
          'Open Settings',
          'OK',
        );
        if (choice === 'Open Settings') {
          await vscode.workspace
            .getConfiguration('clippings.filtering')
            .update('scopes', [], vscode.ConfigurationTarget.Global);
          await vscode.commands.executeCommand('workbench.action.openSettingsJson', 'clippings.filtering.scopes');
        }
        return;
      }
      const { includeGlobs, excludeGlobs } = store.snapshot();
      const name = await prompts.pickItem(
        scopes.map((s) => ({
          label: s.name,
          description: isCurrentScope(s, includeGlobs, excludeGlobs) ? '$(check)' : undefined,
        })),
        { placeHolder: 'Select scope...' },
      );
      const scope = scopes.find((s) => s.name === name);
      if (scope) await setGlobs(scope.includeGlobs, scope.excludeGlobs);
    },
    'clippings.removeFilter': async () => {
      const state = store.snapshot();
      const filter = state.filtered ? state.currentFilter : '';
      const choices = removalChoices(filter, state.includeGlobs, state.excludeGlobs);
      const picked = await prompts.pick([...choices.keys()], {
        canPickMany: true,
        matchOnDescription: true,
        matchOnDetail: true,
        placeHolder: 'Select filters to remove',
      });
      if (!picked || picked.length === 0) return;
      const removals = picked.map((label) => choices.get(label));
      if (removals.some((r) => r?.kind === 'clear')) await store.setFilter(undefined);
      const include = state.includeGlobs.filter((g) => !removals.some((r) => r?.kind === 'include' && r.glob === g));
      const exclude = state.excludeGlobs.filter((g) => !removals.some((r) => r?.kind === 'exclude' && r.glob === g));
      await setGlobs(include, exclude);
    },
    'clippings.resetAllFilters': async () => {
      await store.setFilter(undefined);
      await setGlobs([], []);
    },
  };
  return Object.entries(commands).map(([id, run]) => vscode.commands.registerCommand(id, run));
}
```

Create `extension/src/commands/view.ts`:

```ts
// View style, grouping, the tree filter and Reset Cache (spec 7.6). Each
// updates workspace storage and sends the configuration.

import * as vscode from 'vscode';
import { replacesTree, type ConfigurationSync } from '../config/sync';
import type { ViewFlags, ViewStateStore } from '../state/viewState';
import type { Expansion } from '../tree/expansion';
import type { TreeProvider } from '../tree/provider';
import type { Prompts } from '../ui/prompts';
import { resetExpansion } from './expand';

export interface ViewDeps {
  store: ViewStateStore;
  sync: ConfigurationSync;
  expansion: Expansion;
  provider: TreeProvider;
  prompts: Prompts;
}

export function registerViewCommands(deps: ViewDeps): vscode.Disposable[] {
  const { store, sync, prompts } = deps;
  const flags = (value: ViewFlags) => async () => {
    await store.setFlags(value);
    sync.push();
  };
  const commands: Record<string, (...args: unknown[]) => unknown> = {
    'clippings.showFlatView': flags({ tagsOnly: false, flat: true }),
    'clippings.showTagsOnlyView': flags({ flat: false, tagsOnly: true }),
    'clippings.showTreeView': flags({ flat: false, tagsOnly: false }),
    'clippings.groupByTag': flags({ groupedByTag: true }),
    'clippings.ungroupByTag': flags({ groupedByTag: false }),
    'clippings.groupBySubTag': flags({ groupedBySubTag: true }),
    'clippings.ungroupBySubTag': flags({ groupedBySubTag: false }),
    'clippings.filter': async () => {
      const text = await prompts.input({ prompt: 'Filter tree' });
      if (!text) return;
      await store.setFilter(text);
      sync.push();
    },
    'clippings.filterClear': async () => {
      await store.setFilter(undefined);
      sync.push();
    },
    'clippings.resetCache': async () => {
      await store.reset();
      resetExpansion(deps, replacesTree(sync.push()));
    },
  };
  return Object.entries(commands).map(([id, run]) => vscode.commands.registerCommand(id, run));
}
```

Create `extension/src/filters/globs.ts`:

```ts
// Temporary filter globs from the folder and file context menus, scopes and
// Remove Filter (spec 5.3, inventory 6.2). Pure.

import { fileURLToPath } from 'node:url';

/** globset::escape: wraps each glob metacharacter in brackets. */
export function escapeGlob(path: string): string {
  return path.replace(/[?*[\]{}]/g, (c) => `[${c}]`);
}

/** A path with `/` separators, as the server matches globs. */
export function slashPath(path: string, platform: NodeJS.Platform = process.platform): string {
  return platform === 'win32' ? path.replaceAll('\\', '/') : path;
}

/** Only Show This Folder (`/*`), or And Subfolders and Hide This Folder (`/**\/*`). */
export function folderGlob(path: string, recursive: boolean): string {
  return escapeGlob(slashPath(path)) + (recursive ? '/**/*' : '/*');
}

/** Hide This File. */
export function fileGlob(path: string): string {
  return escapeGlob(slashPath(path));
}

/**
 * The filesystem path of a tree root, folder or file node, from its own key
 * (spec 5.12): `w:<folder uri>`, `d:<path>` or `f:<path>`. A file node of a
 * non-`file` document has no path.
 */
export function nodePath(ownKey: string): string | undefined {
  const value = ownKey.slice(2);
  if (ownKey.startsWith('w:')) {
    try {
      return fileURLToPath(value);
    } catch {
      return undefined;
    }
  }
  if (ownKey.startsWith('d:')) return value;
  // A scheme has two or more characters; a Windows drive letter has one.
  if (ownKey.startsWith('f:')) return /^[a-z][a-z0-9+.-]+:/i.test(value) ? undefined : value;
  return undefined;
}

/** A scope's globs, a comma-separated string or an array (todo-tree's `toGlobArray`). */
export function toGlobArray(value: unknown): string[] {
  if (typeof value === 'string') {
    return value
      .split(',')
      .map((g) => g.trim())
      .filter((g) => g.length > 0);
  }
  return Array.isArray(value) ? value.filter((g): g is string => typeof g === 'string') : [];
}

export interface Scope {
  name: string;
  includeGlobs: string[];
  excludeGlobs: string[];
}

export function scopesOf(value: unknown): Scope[] {
  if (!Array.isArray(value)) return [];
  return value
    .filter((s): s is Record<string, unknown> => !!s && typeof s === 'object' && typeof s['name'] === 'string')
    .map((s) => ({
      name: s['name'] as string,
      includeGlobs: toGlobArray(s['includeGlobs']),
      excludeGlobs: toGlobArray(s['excludeGlobs']),
    }));
}

/** Whether a scope's globs equal the current temporary globs. */
export function isCurrentScope(scope: Scope, include: string[], exclude: string[]): boolean {
  return (
    JSON.stringify(scope.includeGlobs) === JSON.stringify(include) &&
    JSON.stringify(scope.excludeGlobs) === JSON.stringify(exclude)
  );
}

export type Removal = { kind: 'clear' } | { kind: 'include'; glob: string } | { kind: 'exclude'; glob: string };

export const CLEAR_TREE_FILTER = 'Clear Tree Filter';

/** Remove Filter's choices, labelled as todo-tree labels them. */
export function removalChoices(filter: string, include: string[], exclude: string[]): Map<string, Removal> {
  const choices = new Map<string, Removal>();
  if (filter) choices.set(CLEAR_TREE_FILTER, { kind: 'clear' });
  for (const glob of exclude) {
    const label = glob.endsWith('/**/*')
      ? `Exclude Folder: ${glob.slice(0, -5)}`
      : glob.includes('*')
        ? `Exclude: ${glob}`
        : `Exclude File: ${glob}`;
    choices.set(label, { kind: 'exclude', glob });
  }
  for (const glob of include) {
    const label = glob.endsWith('/**/*')
      ? `Include Folder and Subfolders: ${glob.slice(0, -5)}`
      : glob.endsWith('/*')
        ? `Include Folder: ${glob.slice(0, -2)}`
        : `Include: ${glob}`;
    choices.set(label, { kind: 'include', glob });
  }
  return choices;
}
```

Create `extension/src/ui/prompts.ts`:

```ts
// Every prompt and message the commands show goes through here, so the
// integration tests can answer prompts and read messages (spec 12.5).

import * as vscode from 'vscode';

export type Shown =
  | { kind: 'input'; options: vscode.InputBoxOptions }
  | { kind: 'pick'; items: string[]; options: vscode.QuickPickOptions }
  | { kind: 'info' | 'warning' | 'error'; message: string; actions: string[] };

export class Prompts {
  private readonly answers: unknown[] = [];
  /** Everything shown, most recent last. */
  readonly shown: Shown[] = [];

  /** Test hook: queues answers for the next prompts or message actions. */
  script(...answers: unknown[]): void {
    this.answers.push(...answers);
  }

  private scripted<T>(): { answer: T | undefined } | undefined {
    return this.answers.length > 0 ? { answer: this.answers.shift() as T | undefined } : undefined;
  }

  async input(options: vscode.InputBoxOptions): Promise<string | undefined> {
    this.shown.push({ kind: 'input', options });
    const s = this.scripted<string>();
    return s ? s.answer : vscode.window.showInputBox(options);
  }

  async pick(items: string[], options: vscode.QuickPickOptions & { canPickMany: true }): Promise<string[] | undefined>;
  async pick(items: string[], options: vscode.QuickPickOptions): Promise<string | undefined>;
  async pick(items: string[], options: vscode.QuickPickOptions): Promise<string | string[] | undefined> {
    this.shown.push({ kind: 'pick', items, options });
    const s = this.scripted<string | string[]>();
    return s ? s.answer : vscode.window.showQuickPick(items, options);
  }

  /** A quick pick of labelled items, answering with the chosen label. */
  async pickItem(items: vscode.QuickPickItem[], options: vscode.QuickPickOptions): Promise<string | undefined> {
    this.shown.push({ kind: 'pick', items: items.map((i) => i.label), options });
    const s = this.scripted<string>();
    if (s) return s.answer;
    return (await vscode.window.showQuickPick(items, options))?.label;
  }

  async message(kind: 'info' | 'warning' | 'error', message: string, ...actions: string[]): Promise<string | undefined> {
    this.shown.push({ kind, message, actions });
    if (actions.length > 0) {
      const s = this.scripted<string>();
      if (s) return s.answer;
    }
    const show =
      kind === 'info'
        ? vscode.window.showInformationMessage
        : kind === 'warning'
          ? vscode.window.showWarningMessage
          : vscode.window.showErrorMessage;
    return show(message, ...actions);
  }
}
```

Replace `extension/src/testApi.ts` with:

```ts
// What `activate` returns. The `test` hooks let the integration tests
// observe state VS Code offers no API to read (spec 12.5).

import type * as vscode from 'vscode';
import type { StatusParams } from './protocol';
import type { PersistedViewState } from './state/viewState';
import type { TestItem } from './tree/testItems';
import type { Prompts } from './ui/prompts';

export interface ServerHooks {
  readonly running: boolean;
  readonly pid: number | undefined;
  readonly onStatus: vscode.Event<StatusParams>;
  readonly onRunning: vscode.Event<void>;
  /** Fires when repeated crashes stop the automatic restarts (spec 7.4). */
  readonly onGaveUp: vscode.Event<string>;
  /** Shortens the start timeout (`START_TIMEOUT_MS`) so tests need not wait for it. */
  setStartTimeout(ms: number): void;
  status(): StatusParams | undefined;
}

export interface TreeHooks {
  readonly view: vscode.TreeView<string>;
  /** Fires whenever the provider asks VS Code to refetch. */
  readonly onDidChange: vscode.Event<unknown>;
  /** The provider's children of `parent` (top level when omitted), as tree items. */
  items(parent?: string): Promise<TestItem[]>;
  /** The current tree item epoch (spec 7.5). */
  readonly epoch: number;
  /** The line the last todo click flashed. */
  lastFlash(): { uri: string; line: number } | undefined;
}

export interface TestHooks {
  /** Answers prompts and records messages. */
  readonly prompts: Prompts;
  viewState(): PersistedViewState;
  readonly server: ServerHooks;
  readonly tree: TreeHooks;
}

export interface ClippingsApi {
  readonly version: string;
  readonly test: TestHooks;
}
```

Replace `extension/src/extension.ts` with:

```ts
// Composition root: builds each part of the extension and wires them to the
// server connection.

import * as vscode from 'vscode';
import { registerExpandCommands, resetExpansion } from './commands/expand';
import { registerFilterCommands } from './commands/filters';
import { registerNavigationCommands } from './commands/navigation';
import { NEEDS_SCAN_MESSAGE, registerScanCommands } from './commands/scan';
import { needsRestart, registerServerCommands } from './commands/server';
import { registerViewCommands } from './commands/view';
import { affectsServer, readConfiguration } from './config/read';
import { ConfigurationSync, replacesTree } from './config/sync';
import { IconResolver } from './icons/resolver';
import type { StatusParams } from './protocol';
import { ServerConnection } from './server/connection';
import { ViewStateStore } from './state/viewState';
import type { ClippingsApi } from './testApi';
import { Expansion } from './tree/expansion';
import { NodeCache } from './tree/nodeCache';
import { LineFlash } from './tree/open';
import { TreeProvider } from './tree/provider';
import { Revealer } from './tree/reveal';
import { Prompts } from './ui/prompts';
import { testItem } from './tree/testItems';

let connection: ServerConnection | undefined;

export function activate(context: vscode.ExtensionContext): ClippingsApi {
  const log = vscode.window.createOutputChannel('Clippings', { log: true });
  const store = new ViewStateStore(context.workspaceState);
  const prompts = new Prompts();
  const activeUri = () => vscode.window.activeTextEditor?.document.uri.toString() ?? null;
  const server = new ServerConnection({
    context,
    log,
    settings: () => sync.current,
    activeUri,
    error: (message, ...actions) => prompts.message('error', message, ...actions),
  });
  connection = server;
  const sync = new ConfigurationSync(
    () => readConfiguration(store.snapshot()),
    (settings) => server.configure(settings),
  );
  let lastStatus: StatusParams | undefined;

  const cache = new NodeCache();
  const expansion = new Expansion(store);
  const provider = new TreeProvider(server, cache, {
    icons: new IconResolver(),
    itemId: (id) => expansion.itemId(id),
    expanded: (node) => expansion.expanded(node),
  });
  const treeView = vscode.window.createTreeView('clippings-view', { treeDataProvider: provider });
  const revealer = new Revealer({
    find: (uri, line) => server.find(uri, line),
    cache,
    view: treeView,
    settings: () => sync.current,
  });
  const flash = new LineFlash();

  context.subscriptions.push(
    log,
    server,
    provider,
    treeView,
    ...registerServerCommands(server, log),
    revealer,
    flash,
    ...registerExpandCommands({ store, sync, expansion, provider }),
    ...registerNavigationCommands(revealer, flash),
    ...registerViewCommands({ store, sync, expansion, provider, prompts }),
    ...registerFilterCommands({ store, sync, cache, prompts }),
    ...registerScanCommands(server),
    server.onStatus((s) => {
      lastStatus = s;
      treeView.message = s.needsScan && !s.scanning ? NEEDS_SCAN_MESSAGE : undefined;
    }),
    server.onNewInstance(() => provider.reset()),
    server.onTreeChanged((p) => {
      if (p.refresh.includes(null)) expansion.onRootRefresh();
      provider.refresh(p.refresh);
    }),
    treeView.onDidExpandElement((e) => expansion.set(e.element, true)),
    treeView.onDidCollapseElement((e) => expansion.set(e.element, false)),
    vscode.workspace.onDidChangeConfiguration((e) => {
      if (needsRestart(e)) {
        sync.push();
        void server.restart();
        return;
      }
      if (!affectsServer(e)) return;
      const push = sync.push();
      if (e.affectsConfiguration('clippings.tree.expanded')) {
        resetExpansion({ expansion, provider }, replacesTree(push));
      }
    }),
    vscode.window.onDidChangeActiveTextEditor((editor) => {
      server.activeEditor(activeUri());
      revealer.onActiveEditor(editor);
    }),
  );
  void server.start();

  const manifest = context.extension.packageJSON as { version: string };
  return {
    version: manifest.version,
    test: {
      prompts,
      viewState: () => store.snapshot(),
      server: {
        get running() {
          return server.running;
        },
        get pid() {
          return server.pid;
        },
        onStatus: server.onStatus,
        onRunning: server.onRunning,
        onGaveUp: server.onGaveUp,
        setStartTimeout: (ms) => (server.startTimeoutMs = ms),
        status: () => lastStatus,
      },
      tree: {
        view: treeView,
        onDidChange: provider.onDidChangeTreeData,
        items: async (parent) => {
          const ids = await provider.getChildren(parent);
          return ids.map((id) => testItem(id, provider.getTreeItem(id)));
        },
        get epoch() {
          return expansion.currentEpoch;
        },
        lastFlash: () => flash.last,
      },
    },
  };
}

export async function deactivate(): Promise<void> {
  await connection?.stop();
  connection = undefined;
}
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `pnpm -C extension dev && pnpm -C extension typecheck && pnpm -C extension build`
Expected: no errors.

Run: `pnpm -C extension test:unit`
Expected: `50 passing`.

Run: `pnpm -C extension test:integration`
Expected: `36 passing`. A VS Code test window opens, runs the suite and closes.

- [ ] **Step 5: Commit**

```bash
git add -A
git commit -F - <<'EOF'
feat(extension): add the view style, grouping, filter and scope commands

Co-Authored-By: <your model attribution>
EOF
```

### Task 11: Scan mode, tag, toggle and go-to commands

The setting commands of spec 7.6. The four scan mode commands and the item count, badge and compact folder toggles write to the workspace scope when a folder is open, else to the global scope; Add Tag and Remove Tag write where `general.tags` already has a value. `SettingWriter` shows a failed write as a warning (spec 10.2). Go To Next and Go To Previous ask the server's `clippings/navigate` from every cursor and move them all, or none (spec 5.15).

**Files:**
- Create: `extension/src/commands/goTo.ts`, `extension/src/commands/settings.ts`, `extension/src/config/targets.ts`, `extension/src/config/writes.ts`
- Modify: `extension/src/tree/testItems.ts`, `extension/src/extension.ts`
- Test: `extension/src/test/integration/settings.test.ts`, `extension/src/test/unit/writes.test.ts`

**Interfaces:**
- Consumes: `protocol.ts` (Task 2): `Direction`, `ScanMode`; `server/connection.ts` (Task 6): `ServerConnection`; `ui/prompts.ts` (Task 10): `Prompts`.
- Produces, in `commands/goTo.ts`: `function registerGoToCommands(server: ServerConnection): vscode.Disposable[]`.
- Produces, in `commands/settings.ts`: `function registerSettingCommands(writer: SettingWriter, prompts: Prompts): vscode.Disposable[]`.
- Produces, in `config/targets.ts`: `Target` (type); `function folderTarget(hasFolder: boolean): Target`; `function valueTarget(inspected: { workspaceValue?: unknown } | undefined): Target`.
- Produces, in `config/writes.ts`: `class SettingWriter` with `constructor(private readonly prompts: Prompts)`, `folderTarget(): Target`, `valueTarget(key: string): Target`, `get<T>(key: string): T | undefined`, `async write(key: string, value: unknown, target: Target): Promise<boolean>`.
- Produces, in `tree/testItems.ts`: `TestItem` gains `resourceUri: string | undefined`.

- [ ] **Step 1: Write the failing tests**

Create `extension/src/test/integration/settings.test.ts`:

```ts
import * as assert from 'node:assert/strict';
import * as vscode from 'vscode';
import type { ClippingsApi } from '../../testApi';
import { DEFAULT_TREE } from './fixture';
import { getApi, setSetting, treeBecomes, waitFor, whenIdle, workspacePath } from './helpers';

const APP_TODOS = [
  '      TODO (alice) wire up the router',
  '      FIXME handle the error path',
  '      HACK temporary shim until the router lands',
];

function inspect(key: string) {
  return vscode.workspace.getConfiguration('clippings').inspect(key);
}

describe('setting commands', () => {
  let api: ClippingsApi;

  before(async () => {
    api = await getApi();
    await whenIdle(api);
    await treeBecomes(api, DEFAULT_TREE);
  });

  afterEach(async () => {
    await vscode.commands.executeCommand('workbench.action.closeAllEditors');
  });

  it('switches scan modes in workspace settings', async () => {
    await vscode.window.showTextDocument(vscode.Uri.file(workspacePath('src', 'app.ts')));
    try {
      await vscode.commands.executeCommand('clippings.scanOpenFilesOnly');
      assert.equal(inspect('tree.scanMode')?.workspaceValue, 'open files');
      await treeBecomes(api, ['(Scan mode: open files)', 'workspace', '  src', '    app.ts', ...APP_TODOS]);

      await vscode.commands.executeCommand('clippings.scanCurrentFileOnly');
      await treeBecomes(api, ['(Scan mode: current file)', 'workspace', '  src', '    app.ts', ...APP_TODOS]);
      await vscode.window.showTextDocument(vscode.Uri.file(workspacePath('docs', 'plan.md')));
      await treeBecomes(api, [
        '(Scan mode: current file)',
        'workspace',
        '  docs',
        '    plan.md',
        '      [ ] write the guide',
        '      [x] pick a name',
      ]);

      await vscode.commands.executeCommand('clippings.scanWorkspaceOnly');
      await treeBecomes(api, ['(Scan mode: workspace only)', ...DEFAULT_TREE.slice(1)]);

      await vscode.commands.executeCommand('clippings.scanWorkspaceAndOpenFiles');
      assert.equal(inspect('tree.scanMode')?.workspaceValue, 'workspace');
      await treeBecomes(api, DEFAULT_TREE);
    } finally {
      await setSetting('tree.scanMode', undefined, vscode.ConfigurationTarget.Workspace);
    }
  });

  it('adds and removes tags where the tags are set', async () => {
    api.test.prompts.script('NOTE');
    await vscode.commands.executeCommand('clippings.addTag');
    assert.deepEqual(api.test.prompts.shown.at(-1), {
      kind: 'input',
      options: { prompt: 'New tag', placeHolder: 'e.g. FIXME' },
    });
    assert.deepEqual(inspect('general.tags')?.globalValue, ['BUG', 'HACK', 'FIXME', 'TODO', 'XXX', '[ ]', '[x]', 'NOTE']);

    await setSetting('general.tags', ['TODO'], vscode.ConfigurationTarget.Workspace);
    try {
      api.test.prompts.script('HACK');
      await vscode.commands.executeCommand('clippings.addTag');
      assert.deepEqual(inspect('general.tags')?.workspaceValue, ['TODO', 'HACK']);
      api.test.prompts.script(['TODO']);
      await vscode.commands.executeCommand('clippings.removeTag');
      assert.deepEqual(inspect('general.tags')?.workspaceValue, ['HACK']);
    } finally {
      await setSetting('general.tags', undefined, vscode.ConfigurationTarget.Workspace);
    }
    api.test.prompts.script(['NOTE']);
    await vscode.commands.executeCommand('clippings.removeTag');
    assert.deepEqual(inspect('general.tags')?.globalValue, ['BUG', 'HACK', 'FIXME', 'TODO', 'XXX', '[ ]', '[x]']);
    await setSetting('general.tags', undefined);
    await treeBecomes(api, DEFAULT_TREE);
  });

  it('toggles item counts, badges and compact folders in workspace settings', async () => {
    const root = async () => (await api.test.tree.items())[1];
    await vscode.commands.executeCommand('clippings.toggleItemCounts');
    assert.equal(inspect('tree.showCountsInTree')?.workspaceValue, true);
    await waitFor('counts', async () => (await root())?.description === '8', [api.test.tree.onDidChange]);

    assert.ok((await root())?.resourceUri);
    await vscode.commands.executeCommand('clippings.toggleBadges');
    assert.equal(inspect('tree.showBadges')?.workspaceValue, false);
    await waitFor('no badges', async () => (await root())?.resourceUri === undefined, [api.test.tree.onDidChange]);

    await vscode.commands.executeCommand('clippings.toggleCompactFolders');
    assert.equal(inspect('tree.disableCompactFolders')?.workspaceValue, true);

    for (const key of ['tree.showCountsInTree', 'tree.showBadges', 'tree.disableCompactFolders']) {
      await setSetting(key, undefined, vscode.ConfigurationTarget.Workspace);
    }
    await waitFor('defaults again', async () => (await root())?.description === undefined, [api.test.tree.onDidChange]);
  });

  it('goes to the next and previous todo without wrapping', async () => {
    const editor = await vscode.window.showTextDocument(vscode.Uri.file(workspacePath('src', 'app.ts')));
    const at = () => [editor.selection.active.line, editor.selection.active.character];
    editor.selection = new vscode.Selection(0, 0, 0, 0);
    await vscode.commands.executeCommand('clippings.goToNext');
    assert.deepEqual(at(), [1, 2]);
    await vscode.commands.executeCommand('clippings.goToNext');
    assert.deepEqual(at(), [3, 2]);
    await vscode.commands.executeCommand('clippings.goToNext');
    await vscode.commands.executeCommand('clippings.goToNext');
    assert.deepEqual(at(), [7, 2], 'stops at the last todo');
    await vscode.commands.executeCommand('clippings.goToPrevious');
    assert.deepEqual(at(), [3, 2]);
    await vscode.commands.executeCommand('clippings.goToPrevious');
    await vscode.commands.executeCommand('clippings.goToPrevious');
    assert.deepEqual(at(), [1, 2], 'stops at the first todo');
  });
});
```

Create `extension/src/test/unit/writes.test.ts`:

```ts
import * as assert from 'node:assert/strict';
import { folderTarget, valueTarget } from '../../config/targets';

describe('setting write targets', () => {
  it('writes folder-scoped commands to the workspace only when a folder is open', () => {
    assert.equal(folderTarget(true), 'workspace');
    assert.equal(folderTarget(false), 'global');
  });

  it('writes value-scoped commands where the value already lives', () => {
    assert.equal(valueTarget({ workspaceValue: ['TODO'] }), 'workspace');
    assert.equal(valueTarget({ workspaceValue: false }), 'workspace');
    assert.equal(valueTarget({}), 'global');
    assert.equal(valueTarget(undefined), 'global');
  });
});
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `pnpm -C extension typecheck`
Expected: FAIL. `error TS2339: Property 'resourceUri' does not exist on type 'TestItem'`, and `error TS2307: Cannot find module '../../config/targets'`.

- [ ] **Step 3: Write the implementation**

Create `extension/src/commands/goTo.ts`:

```ts
// Go To Next and Go To Previous (spec 5.15): the server searches the open
// buffer with the same matcher as the scan.

import * as vscode from 'vscode';
import type { Direction } from '../protocol';
import type { ServerConnection } from '../server/connection';

async function goTo(server: ServerConnection, direction: Direction): Promise<void> {
  const editor = vscode.window.activeTextEditor;
  if (!editor) return;
  const positions = editor.selections.map((s) => ({ line: s.start.line, character: s.start.character }));
  const ranges = await server.navigate(editor.document.uri.toString(), positions, direction);
  if (!ranges || ranges.length === 0) return;
  editor.selections = ranges.map((r) => {
    const at = new vscode.Position(r.start.line, r.start.character);
    return new vscode.Selection(at, at);
  });
  const first = editor.selections[0];
  if (first) editor.revealRange(new vscode.Range(first.start, first.start));
}

export function registerGoToCommands(server: ServerConnection): vscode.Disposable[] {
  return [
    vscode.commands.registerCommand('clippings.goToNext', () => goTo(server, 'next')),
    vscode.commands.registerCommand('clippings.goToPrevious', () => goTo(server, 'previous')),
  ];
}
```

Create `extension/src/commands/settings.ts`:

```ts
// Commands that change settings: scan modes, tags and the tree toggles
// (spec 7.6).

import * as vscode from 'vscode';
import type { SettingWriter } from '../config/writes';
import type { ScanMode } from '../protocol';
import type { Prompts } from '../ui/prompts';

export function registerSettingCommands(writer: SettingWriter, prompts: Prompts): vscode.Disposable[] {
  const scanMode = (mode: ScanMode) => () => writer.write('tree.scanMode', mode, writer.folderTarget());
  const toggle = (key: string) => () => writer.write(key, !writer.get<boolean>(key), writer.folderTarget());
  const tags = () => writer.get<string[]>('general.tags') ?? [];

  const commands: Record<string, (...args: unknown[]) => unknown> = {
    'clippings.scanOpenFilesOnly': scanMode('open files'),
    'clippings.scanCurrentFileOnly': scanMode('current file'),
    'clippings.scanWorkspaceAndOpenFiles': scanMode('workspace'),
    'clippings.scanWorkspaceOnly': scanMode('workspace only'),
    'clippings.toggleItemCounts': toggle('tree.showCountsInTree'),
    'clippings.toggleBadges': toggle('tree.showBadges'),
    'clippings.toggleCompactFolders': toggle('tree.disableCompactFolders'),
    'clippings.addTag': async () => {
      const tag = await prompts.input({ prompt: 'New tag', placeHolder: 'e.g. FIXME' });
      if (!tag || tags().includes(tag)) return;
      await writer.write('general.tags', [...tags(), tag], writer.valueTarget('general.tags'));
    },
    'clippings.removeTag': async () => {
      const remove = await prompts.pick(tags(), {
        canPickMany: true,
        matchOnDescription: true,
        matchOnDetail: true,
        placeHolder: 'Select tags to remove',
      });
      if (!remove || remove.length === 0) return;
      await writer.write(
        'general.tags',
        tags().filter((t) => !remove.includes(t)),
        writer.valueTarget('general.tags'),
      );
    },
  };
  return Object.entries(commands).map(([id, run]) => vscode.commands.registerCommand(id, run));
}
```

Create `extension/src/config/targets.ts`:

```ts
// Where commands write settings (spec 7.6). Pure.

export type Target = 'global' | 'workspace';

/** Scan mode commands and the count, badge and compact folder toggles. */
export function folderTarget(hasFolder: boolean): Target {
  return hasFolder ? 'workspace' : 'global';
}

/** addTag, removeTag and the status bar clicks: where the value already lives. */
export function valueTarget(inspected: { workspaceValue?: unknown } | undefined): Target {
  return inspected?.workspaceValue !== undefined ? 'workspace' : 'global';
}
```

Create `extension/src/config/writes.ts`:

```ts
// Setting writes that report failure as a warning instead of throwing
// (spec 10.2), at the targets in targets.ts.

import * as vscode from 'vscode';
import type { Prompts } from '../ui/prompts';
import { folderTarget, valueTarget, type Target } from './targets';

export class SettingWriter {
  constructor(private readonly prompts: Prompts) {}

  /** Target for a folder-scoped command. */
  folderTarget(): Target {
    return folderTarget((vscode.workspace.workspaceFolders?.length ?? 0) > 0);
  }

  /** Target for `clippings.<key>` by where its value lives. */
  valueTarget(key: string): Target {
    return valueTarget(vscode.workspace.getConfiguration('clippings').inspect(key));
  }

  get<T>(key: string): T | undefined {
    return vscode.workspace.getConfiguration('clippings').get<T>(key);
  }

  /** Writes `clippings.<key>`; a failure is shown as a warning. Returns success. */
  async write(key: string, value: unknown, target: Target): Promise<boolean> {
    const t = target === 'workspace' ? vscode.ConfigurationTarget.Workspace : vscode.ConfigurationTarget.Global;
    try {
      await vscode.workspace.getConfiguration('clippings').update(key, value, t);
      return true;
    } catch (err) {
      void this.prompts.message('warning', `Clippings: could not update clippings.${key}: ${String(err)}`);
      return false;
    }
  }
}
```

Replace `extension/src/tree/testItems.ts` with:

```ts
// A plain snapshot of tree items for the test hooks.

import * as vscode from 'vscode';

export interface TestItem {
  id: string;
  itemId: string;
  label: string;
  description: string | undefined;
  tooltip: string | undefined;
  contextValue: string | undefined;
  resourceUri: string | undefined;
  state: 'none' | 'collapsed' | 'expanded';
  icon: string | undefined;
  command: { command: string; arguments: unknown[] } | undefined;
}

function iconName(icon: vscode.TreeItem['iconPath']): string | undefined {
  if (icon instanceof vscode.ThemeIcon) return `$(${icon.id})`;
  if (icon instanceof vscode.Uri) return icon.fsPath;
  return undefined;
}

export function testItem(id: string, item: vscode.TreeItem): TestItem {
  const states = { 0: 'none', 1: 'collapsed', 2: 'expanded' } as const;
  return {
    id,
    itemId: item.id ?? '',
    label: typeof item.label === 'string' ? item.label : (item.label?.label ?? ''),
    description: typeof item.description === 'string' ? item.description : undefined,
    tooltip: typeof item.tooltip === 'string' ? item.tooltip : undefined,
    contextValue: item.contextValue,
    resourceUri: item.resourceUri?.toString(),
    state: states[item.collapsibleState ?? 0],
    icon: iconName(item.iconPath),
    command: item.command ? { command: item.command.command, arguments: item.command.arguments ?? [] } : undefined,
  };
}
```

Replace `extension/src/extension.ts` with:

```ts
// Composition root: builds each part of the extension and wires them to the
// server connection.

import * as vscode from 'vscode';
import { registerExpandCommands, resetExpansion } from './commands/expand';
import { registerFilterCommands } from './commands/filters';
import { registerGoToCommands } from './commands/goTo';
import { registerNavigationCommands } from './commands/navigation';
import { NEEDS_SCAN_MESSAGE, registerScanCommands } from './commands/scan';
import { needsRestart, registerServerCommands } from './commands/server';
import { registerSettingCommands } from './commands/settings';
import { registerViewCommands } from './commands/view';
import { affectsServer, readConfiguration } from './config/read';
import { ConfigurationSync, replacesTree } from './config/sync';
import { SettingWriter } from './config/writes';
import { IconResolver } from './icons/resolver';
import type { StatusParams } from './protocol';
import { ServerConnection } from './server/connection';
import { ViewStateStore } from './state/viewState';
import type { ClippingsApi } from './testApi';
import { Expansion } from './tree/expansion';
import { NodeCache } from './tree/nodeCache';
import { LineFlash } from './tree/open';
import { TreeProvider } from './tree/provider';
import { Revealer } from './tree/reveal';
import { Prompts } from './ui/prompts';
import { testItem } from './tree/testItems';

let connection: ServerConnection | undefined;

export function activate(context: vscode.ExtensionContext): ClippingsApi {
  const log = vscode.window.createOutputChannel('Clippings', { log: true });
  const store = new ViewStateStore(context.workspaceState);
  const prompts = new Prompts();
  const activeUri = () => vscode.window.activeTextEditor?.document.uri.toString() ?? null;
  const server = new ServerConnection({
    context,
    log,
    settings: () => sync.current,
    activeUri,
    error: (message, ...actions) => prompts.message('error', message, ...actions),
  });
  connection = server;
  const sync = new ConfigurationSync(
    () => readConfiguration(store.snapshot()),
    (settings) => server.configure(settings),
  );
  let lastStatus: StatusParams | undefined;

  const cache = new NodeCache();
  const expansion = new Expansion(store);
  const provider = new TreeProvider(server, cache, {
    icons: new IconResolver(),
    itemId: (id) => expansion.itemId(id),
    expanded: (node) => expansion.expanded(node),
  });
  const treeView = vscode.window.createTreeView('clippings-view', { treeDataProvider: provider });
  const revealer = new Revealer({
    find: (uri, line) => server.find(uri, line),
    cache,
    view: treeView,
    settings: () => sync.current,
  });
  const flash = new LineFlash();
  const writer = new SettingWriter(prompts);

  context.subscriptions.push(
    log,
    server,
    provider,
    treeView,
    ...registerServerCommands(server, log),
    revealer,
    flash,
    ...registerExpandCommands({ store, sync, expansion, provider }),
    ...registerNavigationCommands(revealer, flash),
    ...registerViewCommands({ store, sync, expansion, provider, prompts }),
    ...registerFilterCommands({ store, sync, cache, prompts }),
    ...registerSettingCommands(writer, prompts),
    ...registerGoToCommands(server),
    ...registerScanCommands(server),
    server.onStatus((s) => {
      lastStatus = s;
      treeView.message = s.needsScan && !s.scanning ? NEEDS_SCAN_MESSAGE : undefined;
    }),
    server.onNewInstance(() => provider.reset()),
    server.onTreeChanged((p) => {
      if (p.refresh.includes(null)) expansion.onRootRefresh();
      provider.refresh(p.refresh);
    }),
    treeView.onDidExpandElement((e) => expansion.set(e.element, true)),
    treeView.onDidCollapseElement((e) => expansion.set(e.element, false)),
    vscode.workspace.onDidChangeConfiguration((e) => {
      if (needsRestart(e)) {
        sync.push();
        void server.restart();
        return;
      }
      if (!affectsServer(e)) return;
      const push = sync.push();
      if (e.affectsConfiguration('clippings.tree.expanded')) {
        resetExpansion({ expansion, provider }, replacesTree(push));
      }
    }),
    vscode.window.onDidChangeActiveTextEditor((editor) => {
      server.activeEditor(activeUri());
      revealer.onActiveEditor(editor);
    }),
  );
  void server.start();

  const manifest = context.extension.packageJSON as { version: string };
  return {
    version: manifest.version,
    test: {
      prompts,
      viewState: () => store.snapshot(),
      server: {
        get running() {
          return server.running;
        },
        get pid() {
          return server.pid;
        },
        onStatus: server.onStatus,
        onRunning: server.onRunning,
        onGaveUp: server.onGaveUp,
        setStartTimeout: (ms) => (server.startTimeoutMs = ms),
        status: () => lastStatus,
      },
      tree: {
        view: treeView,
        onDidChange: provider.onDidChangeTreeData,
        items: async (parent) => {
          const ids = await provider.getChildren(parent);
          return ids.map((id) => testItem(id, provider.getTreeItem(id)));
        },
        get epoch() {
          return expansion.currentEpoch;
        },
        lastFlash: () => flash.last,
      },
    },
  };
}

export async function deactivate(): Promise<void> {
  await connection?.stop();
  connection = undefined;
}
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `pnpm -C extension dev && pnpm -C extension typecheck && pnpm -C extension build`
Expected: no errors.

Run: `pnpm -C extension test:unit`
Expected: `52 passing`.

Run: `pnpm -C extension test:integration`
Expected: `40 passing`. A VS Code test window opens, runs the suite and closes.

- [ ] **Step 5: Commit**

```bash
git add -A
git commit -F - <<'EOF'
feat(extension): add scan mode, tag, toggle and go-to commands with setting write scopes

Co-Authored-By: <your model attribution>
EOF
```

### Task 12: Context keys

`contextValues` computes the 22 context keys of spec 7.6 from the configuration object and the last status, reading `tree.buttons.*` with todo-tree's defaults, and `ContextKeys` sets only the keys that changed. They are recomputed on every configuration push and every `clippings/status`. `config/clientSettings.ts` reads the settings only the client uses: the buttons and `general.statusBarClickBehaviour`.

**Files:**
- Create: `extension/src/config/clientSettings.ts`, `extension/src/context/apply.ts`, `extension/src/context/keys.ts`
- Modify: `extension/src/testApi.ts`, `extension/src/extension.ts`
- Test: `extension/src/test/integration/contextKeys.test.ts`, `extension/src/test/unit/contextKeys.test.ts`

**Interfaces:**
- Consumes: `config/configuration.ts` (Task 2): `effectiveView`; `protocol.ts` (Task 2): `Settings`, `StatusParams`.
- Produces, in `config/clientSettings.ts`: `Buttons` (interface); `DEFAULT_BUTTONS` (const); `function buttons(settings: Settings): Buttons`; `ClickBehaviour` (type); `function statusBarClickBehaviour(settings: Settings): ClickBehaviour`.
- Produces, in `context/apply.ts`: `class ContextKeys` with `get values(): Readonly<ContextValues>`, `async apply(next: ContextValues): Promise<void>`.
- Produces, in `context/keys.ts`: `ContextValues` (type); `function contextValues(settings: Settings, status: Pick<StatusParams, 'hasSubTags' | 'isEmpty'> | undefined): ContextValues`; `function changedValues(previous: ContextValues, next: ContextValues): ContextValues`.
- Produces, in `testApi.ts`: `TestHooks` gains `contextKeys(): Readonly<Record<string, boolean | string>>`.

- [ ] **Step 1: Write the failing tests**

Create `extension/src/test/integration/contextKeys.test.ts`:

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
    api.test.prompts.script('guide');
    await vscode.commands.executeCommand('clippings.filter');
    assert.equal(key('filtered'), true);
    assert.equal(key('global-filter-active'), 'guide');
    const lib = await itemAt(api, 'workspace', 'lib');
    await vscode.commands.executeCommand('clippings.excludeThisFolder', lib.id);
    assert.equal(key('folder-filter-active'), true);
    await vscode.commands.executeCommand('clippings.resetAllFilters');
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

Create `extension/src/test/unit/contextKeys.test.ts`:

```ts
import * as assert from 'node:assert/strict';
import { buttons, DEFAULT_BUTTONS, statusBarClickBehaviour } from '../../config/clientSettings';
import { buildConfiguration } from '../../config/configuration';
import { changedValues, contextValues } from '../../context/keys';
import type { PersistedViewState } from '../../state/viewState';

const empty: PersistedViewState = { currentFilter: '', filtered: false, includeGlobs: [], excludeGlobs: [] };

function settings(tree: object = {}, viewState: Partial<PersistedViewState> = {}, general: object = {}) {
  return buildConfiguration({
    groups: {
      general,
      highlights: {},
      filtering: {},
      tree: {
        flat: false,
        tagsOnly: false,
        expanded: false,
        groupedByTag: false,
        groupedBySubTag: false,
        trackFile: true,
        hideTreeWhenEmpty: false,
        scanMode: 'workspace',
        ...tree,
      },
      regex: {},
    },
    filesExclude: {},
    searchExclude: {},
    explorerCompactFolders: true,
    viewState: { ...empty, ...viewState },
  });
}

describe('context keys', () => {
  it('reflects the default settings', () => {
    const v = contextValues(settings(), undefined);
    assert.equal(Object.keys(v).length, 22);
    assert.equal(v['clippings-show-reveal-button'], false, 'track file hides the reveal button');
    assert.equal(v['clippings-show-view-style-button'], true);
    assert.equal(v['clippings-show-export-button'], false);
    assert.equal(v['clippings-collapsible'], true);
    assert.equal(v['clippings-filtered'], false);
    assert.equal(v['clippings-global-filter-active'], '');
    assert.equal(v['clippings-can-toggle-compact-folders'], true);
    assert.equal(v['clippings-scan-mode'], 'workspace');
    assert.equal(v['clippings-has-sub-tags'], false);
    assert.equal(v['clippings-is-empty'], false);
  });

  it('reflects buttons, view state, filters and status', () => {
    const v = contextValues(
      settings(
        { trackFile: false, hideTreeWhenEmpty: true, buttons: { reveal: true, export: true } },
        { tagsOnly: true, currentFilter: 'fix', filtered: true, excludeGlobs: ['/w/**/*'] },
      ),
      { hasSubTags: true, isEmpty: true },
    );
    assert.equal(v['clippings-show-reveal-button'], true);
    assert.equal(v['clippings-show-export-button'], true);
    assert.equal(v['clippings-tags-only'], true);
    assert.equal(v['clippings-collapsible'], false, 'an ungrouped tags only view has nothing to collapse');
    assert.equal(v['clippings-filtered'], true);
    assert.equal(v['clippings-global-filter-active'], 'fix');
    assert.equal(v['clippings-folder-filter-active'], true);
    assert.equal(v['clippings-has-sub-tags'], true);
    assert.equal(v['clippings-is-empty'], true);
  });

  it('only reports keys whose values changed', () => {
    assert.deepEqual(changedValues({ a: true, b: 'x' }, { a: true, b: 'y', c: false }), { b: 'y', c: false });
  });
});

describe('client-only settings', () => {
  it('fills missing buttons with todo-tree defaults and ignores bad values', () => {
    assert.deepEqual(buttons(settings()), DEFAULT_BUTTONS);
    assert.equal(buttons(settings({ buttons: { scanMode: true, filter: 'no' } })).scanMode, true);
    assert.equal(buttons(settings({ buttons: { filter: 'no' } })).filter, true);
  });

  it('reads the status bar click behaviour', () => {
    assert.equal(statusBarClickBehaviour(settings()), 'reveal');
    assert.equal(statusBarClickBehaviour(settings({}, {}, { statusBarClickBehaviour: 'cycle' })), 'cycle');
    assert.equal(statusBarClickBehaviour(settings({}, {}, { statusBarClickBehaviour: 'bogus' })), 'reveal');
  });
});
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `pnpm -C extension typecheck`
Expected: FAIL. `error TS2339: Property 'contextKeys' does not exist on type 'TestHooks'`, and `error TS2307: Cannot find module` for `../../config/clientSettings` and `../../context/keys`.

- [ ] **Step 3: Write the implementation**

Create `extension/src/config/clientSettings.ts`:

```ts
// Settings only the client reads. They travel inside the configuration
// object's groups, which the server ignores. Pure.

import type { Settings } from '../protocol';

export interface Buttons {
  reveal: boolean;
  scanMode: boolean;
  viewStyle: boolean;
  groupByTag: boolean;
  groupBySubTag: boolean;
  filter: boolean;
  refresh: boolean;
  expand: boolean;
  export: boolean;
}

/** todo-tree's defaults for `tree.buttons.*`. */
export const DEFAULT_BUTTONS: Buttons = {
  reveal: true,
  scanMode: false,
  viewStyle: true,
  groupByTag: true,
  groupBySubTag: false,
  filter: true,
  refresh: true,
  expand: true,
  export: false,
};

export function buttons(settings: Settings): Buttons {
  const raw = (settings.tree as unknown as { buttons?: Partial<Record<keyof Buttons, unknown>> }).buttons ?? {};
  const out = { ...DEFAULT_BUTTONS };
  for (const key of Object.keys(out) as (keyof Buttons)[]) {
    if (typeof raw[key] === 'boolean') out[key] = raw[key];
  }
  return out;
}

export type ClickBehaviour = 'cycle' | 'reveal' | 'toggle highlights';

export function statusBarClickBehaviour(settings: Settings): ClickBehaviour {
  const value = (settings.general as unknown as { statusBarClickBehaviour?: unknown }).statusBarClickBehaviour;
  return value === 'cycle' || value === 'toggle highlights' ? value : 'reveal';
}
```

Create `extension/src/context/apply.ts`:

```ts
// Sets the context keys that changed.

import * as vscode from 'vscode';
import { changedValues, type ContextValues } from './keys';

export class ContextKeys {
  private applied: ContextValues = {};

  /** The values last set, for the tests. */
  get values(): Readonly<ContextValues> {
    return this.applied;
  }

  async apply(next: ContextValues): Promise<void> {
    const changed = changedValues(this.applied, next);
    this.applied = { ...next };
    await Promise.all(Object.entries(changed).map(([k, v]) => vscode.commands.executeCommand('setContext', k, v)));
  }
}
```

Create `extension/src/context/keys.ts`:

```ts
// Context keys for menus and view visibility (spec 7.6). Pure.

import { buttons } from '../config/clientSettings';
import { effectiveView } from '../config/configuration';
import type { Settings, StatusParams } from '../protocol';

export type ContextValues = Record<string, boolean | string>;

export function contextValues(settings: Settings, status: Pick<StatusParams, 'hasSubTags' | 'isEmpty'> | undefined): ContextValues {
  const b = buttons(settings);
  const tree = settings.tree;
  const state = settings.viewState;
  const view = effectiveView(tree, state);
  return {
    'clippings-show-reveal-button': b.reveal && !tree.trackFile,
    'clippings-show-scan-mode-button': b.scanMode,
    'clippings-show-view-style-button': b.viewStyle,
    'clippings-show-group-by-tag-button': b.groupByTag,
    'clippings-show-group-by-sub-tag-button': b.groupBySubTag,
    'clippings-show-filter-button': b.filter,
    'clippings-show-refresh-button': b.refresh,
    'clippings-show-expand-button': b.expand,
    'clippings-show-export-button': b.export,
    'clippings-expanded': view.expanded,
    'clippings-flat': view.flat,
    'clippings-tags-only': view.tagsOnly,
    'clippings-grouped-by-tag': view.groupedByTag,
    'clippings-grouped-by-sub-tag': view.groupedBySubTag,
    'clippings-filtered': state.filter !== '',
    'clippings-collapsible': !view.tagsOnly || view.groupedByTag || view.groupedBySubTag,
    'clippings-folder-filter-active': state.includeGlobs.length + state.excludeGlobs.length > 0,
    'clippings-global-filter-active': state.filter,
    'clippings-can-toggle-compact-folders': settings.explorerCompactFolders,
    'clippings-has-sub-tags': status?.hasSubTags ?? false,
    'clippings-scan-mode': tree.scanMode,
    'clippings-is-empty': tree.hideTreeWhenEmpty && (status?.isEmpty ?? false),
  };
}

/** The keys whose values differ from `previous`. */
export function changedValues(previous: ContextValues, next: ContextValues): ContextValues {
  return Object.fromEntries(Object.entries(next).filter(([k, v]) => previous[k] !== v));
}
```

Replace `extension/src/testApi.ts` with:

```ts
// What `activate` returns. The `test` hooks let the integration tests
// observe state VS Code offers no API to read (spec 12.5).

import type * as vscode from 'vscode';
import type { StatusParams } from './protocol';
import type { PersistedViewState } from './state/viewState';
import type { TestItem } from './tree/testItems';
import type { Prompts } from './ui/prompts';

export interface ServerHooks {
  readonly running: boolean;
  readonly pid: number | undefined;
  readonly onStatus: vscode.Event<StatusParams>;
  readonly onRunning: vscode.Event<void>;
  /** Fires when repeated crashes stop the automatic restarts (spec 7.4). */
  readonly onGaveUp: vscode.Event<string>;
  /** Shortens the start timeout (`START_TIMEOUT_MS`) so tests need not wait for it. */
  setStartTimeout(ms: number): void;
  status(): StatusParams | undefined;
}

export interface TreeHooks {
  readonly view: vscode.TreeView<string>;
  /** Fires whenever the provider asks VS Code to refetch. */
  readonly onDidChange: vscode.Event<unknown>;
  /** The provider's children of `parent` (top level when omitted), as tree items. */
  items(parent?: string): Promise<TestItem[]>;
  /** The current tree item epoch (spec 7.5). */
  readonly epoch: number;
  /** The line the last todo click flashed. */
  lastFlash(): { uri: string; line: number } | undefined;
}

export interface TestHooks {
  /** Answers prompts and records messages. */
  readonly prompts: Prompts;
  viewState(): PersistedViewState;
  /** The context keys last set (spec 7.6). */
  contextKeys(): Readonly<Record<string, boolean | string>>;
  readonly server: ServerHooks;
  readonly tree: TreeHooks;
}

export interface ClippingsApi {
  readonly version: string;
  readonly test: TestHooks;
}
```

Replace `extension/src/extension.ts` with:

```ts
// Composition root: builds each part of the extension and wires them to the
// server connection.

import * as vscode from 'vscode';
import { registerExpandCommands, resetExpansion } from './commands/expand';
import { registerFilterCommands } from './commands/filters';
import { registerGoToCommands } from './commands/goTo';
import { registerNavigationCommands } from './commands/navigation';
import { NEEDS_SCAN_MESSAGE, registerScanCommands } from './commands/scan';
import { needsRestart, registerServerCommands } from './commands/server';
import { registerSettingCommands } from './commands/settings';
import { registerViewCommands } from './commands/view';
import { affectsServer, readConfiguration } from './config/read';
import { ConfigurationSync, replacesTree } from './config/sync';
import { ContextKeys } from './context/apply';
import { contextValues } from './context/keys';
import { SettingWriter } from './config/writes';
import { IconResolver } from './icons/resolver';
import type { StatusParams } from './protocol';
import { ServerConnection } from './server/connection';
import { ViewStateStore } from './state/viewState';
import type { ClippingsApi } from './testApi';
import { Expansion } from './tree/expansion';
import { NodeCache } from './tree/nodeCache';
import { LineFlash } from './tree/open';
import { TreeProvider } from './tree/provider';
import { Revealer } from './tree/reveal';
import { Prompts } from './ui/prompts';
import { testItem } from './tree/testItems';

let connection: ServerConnection | undefined;

export function activate(context: vscode.ExtensionContext): ClippingsApi {
  const log = vscode.window.createOutputChannel('Clippings', { log: true });
  const store = new ViewStateStore(context.workspaceState);
  const prompts = new Prompts();
  const activeUri = () => vscode.window.activeTextEditor?.document.uri.toString() ?? null;
  const server = new ServerConnection({
    context,
    log,
    settings: () => sync.current,
    activeUri,
    error: (message, ...actions) => prompts.message('error', message, ...actions),
  });
  connection = server;
  const sync = new ConfigurationSync(
    () => readConfiguration(store.snapshot()),
    (settings) => server.configure(settings),
  );
  let lastStatus: StatusParams | undefined;
  const contextKeys = new ContextKeys();
  const updateContext = () => void contextKeys.apply(contextValues(sync.current, lastStatus));
  sync.onPush(updateContext);
  updateContext();

  const cache = new NodeCache();
  const expansion = new Expansion(store);
  const provider = new TreeProvider(server, cache, {
    icons: new IconResolver(),
    itemId: (id) => expansion.itemId(id),
    expanded: (node) => expansion.expanded(node),
  });
  const treeView = vscode.window.createTreeView('clippings-view', { treeDataProvider: provider });
  const revealer = new Revealer({
    find: (uri, line) => server.find(uri, line),
    cache,
    view: treeView,
    settings: () => sync.current,
  });
  const flash = new LineFlash();
  const writer = new SettingWriter(prompts);

  context.subscriptions.push(
    log,
    server,
    provider,
    treeView,
    ...registerServerCommands(server, log),
    revealer,
    flash,
    ...registerExpandCommands({ store, sync, expansion, provider }),
    ...registerNavigationCommands(revealer, flash),
    ...registerViewCommands({ store, sync, expansion, provider, prompts }),
    ...registerFilterCommands({ store, sync, cache, prompts }),
    ...registerSettingCommands(writer, prompts),
    ...registerGoToCommands(server),
    ...registerScanCommands(server),
    server.onStatus((s) => {
      lastStatus = s;
      updateContext();
      treeView.message = s.needsScan && !s.scanning ? NEEDS_SCAN_MESSAGE : undefined;
    }),
    server.onNewInstance(() => provider.reset()),
    server.onTreeChanged((p) => {
      if (p.refresh.includes(null)) expansion.onRootRefresh();
      provider.refresh(p.refresh);
    }),
    treeView.onDidExpandElement((e) => expansion.set(e.element, true)),
    treeView.onDidCollapseElement((e) => expansion.set(e.element, false)),
    vscode.workspace.onDidChangeConfiguration((e) => {
      if (needsRestart(e)) {
        sync.push();
        void server.restart();
        return;
      }
      if (!affectsServer(e)) return;
      const push = sync.push();
      if (e.affectsConfiguration('clippings.tree.expanded')) {
        resetExpansion({ expansion, provider }, replacesTree(push));
      }
    }),
    vscode.window.onDidChangeActiveTextEditor((editor) => {
      server.activeEditor(activeUri());
      revealer.onActiveEditor(editor);
    }),
  );
  void server.start();

  const manifest = context.extension.packageJSON as { version: string };
  return {
    version: manifest.version,
    test: {
      prompts,
      viewState: () => store.snapshot(),
      contextKeys: () => contextKeys.values,
      server: {
        get running() {
          return server.running;
        },
        get pid() {
          return server.pid;
        },
        onStatus: server.onStatus,
        onRunning: server.onRunning,
        onGaveUp: server.onGaveUp,
        setStartTimeout: (ms) => (server.startTimeoutMs = ms),
        status: () => lastStatus,
      },
      tree: {
        view: treeView,
        onDidChange: provider.onDidChangeTreeData,
        items: async (parent) => {
          const ids = await provider.getChildren(parent);
          return ids.map((id) => testItem(id, provider.getTreeItem(id)));
        },
        get epoch() {
          return expansion.currentEpoch;
        },
        lastFlash: () => flash.last,
      },
    },
  };
}

export async function deactivate(): Promise<void> {
  await connection?.stop();
  connection = undefined;
}
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `pnpm -C extension dev && pnpm -C extension typecheck && pnpm -C extension build`
Expected: no errors.

Run: `pnpm -C extension test:unit`
Expected: `57 passing`.

Run: `pnpm -C extension test:integration`
Expected: `44 passing`. A VS Code test window opens, runs the suite and closes.

- [ ] **Step 5: Commit**

```bash
git add -A
git commit -F - <<'EOF'
feat(extension): set the context keys for menus and view visibility

Co-Authored-By: <your model attribution>
EOF
```

### Task 13: Status bar, badge, title and notices

`StatusController` applies each `clippings/status` (spec 7.5 and 7.7): the status bar item on the left at priority 0, the scanning and interrupted states whose clicks stop or restart the scan (ruling 25), the view's title, badge and first-scan message, and configuration warnings and errors shown once per distinct set. `clippings.onStatusBarClicked` follows `general.statusBarClickBehaviour`: reveal the view, toggle highlights, or cycle the status bar mode with todo-tree's four messages, writing where the value lives (ruling 3).

**Files:**
- Create: `extension/src/commands/statusBar.ts`, `extension/src/status/controller.ts`, `extension/src/status/presentation.ts`
- Modify: `extension/src/testApi.ts`, `extension/src/extension.ts`
- Test: `extension/src/test/integration/status.test.ts`, `extension/src/test/unit/statusBar.test.ts`

**Interfaces:**
- Consumes: `commands/scan.ts` (Task 9): `NEEDS_SCAN_MESSAGE`; `config/clientSettings.ts` (Task 12): `statusBarClickBehaviour`; `config/sync.ts` (Task 8): `ConfigurationSync`; `config/writes.ts` (Task 11): `SettingWriter`; `protocol.ts` (Task 2): `StatusBarMode`, `StatusParams`; `ui/prompts.ts` (Task 10): `Prompts`.
- Produces, in `commands/statusBar.ts`: `StatusBarDeps` (interface); `function registerStatusBarCommand({ sync, writer, prompts, view }: StatusBarDeps): vscode.Disposable`.
- Produces, in `status/controller.ts`: `class StatusController implements vscode.Disposable` with `shown: StatusBarView | undefined`, `constructor(private readonly view: vscode.TreeView<string>, private readonly prompts: Prompts)`, `update(status: StatusParams): void`, `dispose(): void`.
- Produces, in `status/presentation.ts`: `StatusBarView` (interface); `function statusBarView(status: StatusParams): StatusBarView`; `function nextStatusBarMode(current: StatusBarMode): { mode: StatusBarMode; message: string }`; `class OnceNotice` with `next(items: readonly string[]): string | undefined`.
- Produces, in `testApi.ts`: `TestHooks` gains `statusBar(): StatusBarView | undefined`.

- [ ] **Step 1: Write the failing tests**

Create `extension/src/test/integration/status.test.ts`:

```ts
import * as assert from 'node:assert/strict';
import * as vscode from 'vscode';
import type { ClippingsApi } from '../../testApi';
import { DEFAULT_TREE } from './fixture';
import { getApi, setSetting, treeBecomes, waitFor, whenIdle } from './helpers';

function inspect(key: string) {
  return vscode.workspace.getConfiguration('clippings').inspect(key);
}

describe('status bar, badge and title', () => {
  let api: ClippingsApi;
  const bar = () => api.test.statusBar();
  const badge = () => api.test.tree.view.badge;

  before(async () => {
    api = await getApi();
    await whenIdle(api);
    await treeBecomes(api, DEFAULT_TREE);
  });

  it('is hidden in the default none mode and shows scanning during a scan', async () => {
    assert.equal(bar()?.visible, false);
    const scanning = waitFor('a scanning status bar', () => (bar()?.text === 'Clippings: Scanning...' ? bar() : undefined), [
      api.test.server.onStatus,
    ]);
    await vscode.commands.executeCommand('clippings.refresh');
    const shown = await scanning;
    assert.equal(shown.command, 'clippings.stopScan');
    assert.equal(shown.visible, true);
    await whenIdle(api);
    assert.equal(bar()?.visible, false);
  });

  it('shows the total and tag counts', async () => {
    await setSetting('general.statusBar', 'total');
    await waitFor('the total', () => bar()?.text === '$(check) 8', [api.test.server.onStatus]);
    assert.equal(bar()?.tooltip, 'Clippings total');
    assert.equal(bar()?.command, 'clippings.onStatusBarClicked');
    await setSetting('general.statusBar', 'tags');
    await waitFor(
      'tag counts',
      // todo-tree's spacing: each item ends in a space and items are joined with another.
      () => bar()?.text === '$(check) BUG: 1  HACK: 1  FIXME: 1  TODO: 3  [ ]: 1  [x]: 1',
      [api.test.server.onStatus],
    );
    await setSetting('general.statusBar', undefined);
  });

  it('cycles the mode where it is set, with an information message', async () => {
    await setSetting('general.statusBarClickBehaviour', 'cycle');
    await setSetting('general.statusBar', 'total', vscode.ConfigurationTarget.Workspace);
    try {
      await vscode.commands.executeCommand('clippings.onStatusBarClicked');
      assert.equal(inspect('general.statusBar')?.workspaceValue, 'tags');
      assert.deepEqual(api.test.prompts.shown.at(-1), {
        kind: 'info',
        message: 'Clippings: Now showing tag counts',
        actions: [],
      });
      await vscode.commands.executeCommand('clippings.onStatusBarClicked');
      assert.equal(inspect('general.statusBar')?.workspaceValue, 'top three');
    } finally {
      await setSetting('general.statusBar', undefined, vscode.ConfigurationTarget.Workspace);
      await setSetting('general.statusBarClickBehaviour', undefined);
    }
  });

  it('toggles highlights on click', async () => {
    await setSetting('general.statusBarClickBehaviour', 'toggle highlights');
    try {
      await vscode.commands.executeCommand('clippings.onStatusBarClicked');
      assert.equal(inspect('highlights.enabled')?.globalValue, false);
      await vscode.commands.executeCommand('clippings.onStatusBarClicked');
      assert.equal(inspect('highlights.enabled')?.globalValue, true);
    } finally {
      await setSetting('highlights.enabled', undefined);
      await setSetting('general.statusBarClickBehaviour', undefined);
    }
  });

  it('reveals the view on click', async () => {
    await vscode.commands.executeCommand('workbench.view.explorer');
    await waitFor('a hidden view', () => !api.test.tree.view.visible, [api.test.tree.view.onDidChangeVisibility]);
    await vscode.commands.executeCommand('clippings.onStatusBarClicked');
    await waitFor('a visible view', () => api.test.tree.view.visible, [api.test.tree.view.onDidChangeVisibility]);
  });

  it('sets the badge and the view title', async () => {
    assert.equal(api.test.tree.view.title, 'Tree');
    assert.equal(badge(), undefined);
    await setSetting('general.showActivityBarBadge', true);
    await setSetting('tree.showCountsInTree', true);
    try {
      await waitFor('a badge', () => badge()?.value === 8, [api.test.server.onStatus]);
      assert.equal(badge()?.tooltip, '8 todos');
      await waitFor('a counted title', () => api.test.tree.view.title === 'Tree (8)', [api.test.server.onStatus]);
    } finally {
      await setSetting('general.showActivityBarBadge', undefined);
      await setSetting('tree.showCountsInTree', undefined);
    }
    await waitFor('no badge', () => badge() === undefined, [api.test.server.onStatus]);
  });

  it('shows each distinct configuration warning once', async () => {
    const warnings = () =>
      api.test.prompts.shown.filter((s) => (s.kind === 'warning' ? s.message.includes('notacolour') : false));
    await setSetting('highlights.customHighlight', { TODO: { foreground: 'notacolour' } });
    try {
      await waitFor('a colour warning', () => warnings().length === 1, [api.test.server.onStatus]);
      assert.deepEqual(warnings()[0], {
        kind: 'warning',
        message: 'Clippings: Invalid colour settings: customHighlight.TODO.foreground (notacolour)',
        actions: [],
      });
      await setSetting('tree.showCountsInTree', true);
      await waitFor('another status', () => api.test.tree.view.title === 'Tree (8)', [api.test.server.onStatus]);
      assert.equal(warnings().length, 1);
    } finally {
      await setSetting('tree.showCountsInTree', undefined);
      await setSetting('highlights.customHighlight', undefined);
    }
  });

  it('shows a regex error with an Open Settings action and keeps the last tree', async () => {
    api.test.prompts.script(undefined);
    await setSetting('regex.regex', '(unclosed');
    try {
      const shown = await waitFor(
        'an error',
        () => api.test.prompts.shown.find((s) => (s.kind === 'warning' ? s.message.includes('(unclosed') : false)),
        [api.test.server.onStatus],
      );
      assert.deepEqual(shown.kind === 'warning' && shown.actions, ['Open Settings']);
      assert.ok(api.test.server.status()?.error);
      await treeBecomes(api, DEFAULT_TREE);
    } finally {
      await setSetting('regex.regex', undefined);
    }
    await waitFor('no error', () => api.test.server.status()?.error === null, [api.test.server.onStatus]);
  });
});
```

Create `extension/src/test/unit/statusBar.test.ts`:

```ts
import * as assert from 'node:assert/strict';
import type { StatusParams } from '../../protocol';
import { nextStatusBarMode, OnceNotice, statusBarView } from '../../status/presentation';

const idle: StatusParams = {
  instance: 'i',
  scanning: false,
  interrupted: false,
  needsScan: false,
  error: null,
  warnings: [],
  statusBar: { text: '$(check) 3', tooltip: 'Clippings total', visible: true },
  badge: { value: 0, tooltip: '0 todos' },
  viewTitle: 'Tree',
  hasSubTags: false,
  isEmpty: false,
};

describe('status bar', () => {
  it('shows the server text and clicks through to the click behaviour', () => {
    assert.deepEqual(statusBarView(idle), {
      text: '$(check) 3',
      tooltip: 'Clippings total',
      visible: true,
      command: 'clippings.onStatusBarClicked',
    });
  });

  it('shows scanning, clickable to stop, even when the mode is none', () => {
    const view = statusBarView({ ...idle, scanning: true, statusBar: { text: '', tooltip: '', visible: false } });
    assert.deepEqual(view, {
      text: 'Clippings: Scanning...',
      tooltip: 'Click to interrupt scan',
      command: 'clippings.stopScan',
      visible: true,
    });
  });

  it('shows an interrupted scan, clickable to refresh', () => {
    const view = statusBarView({ ...idle, interrupted: true });
    assert.equal(view.text, 'Clippings: Scanning interrupted.');
    assert.equal(view.command, 'clippings.refresh');
  });

  it('cycles total, tags, top three, current file with todo-tree’s messages', () => {
    const seen: string[] = [];
    let mode = nextStatusBarMode('none');
    seen.push(mode.message);
    for (let i = 0; i < 4; i++) {
      mode = nextStatusBarMode(mode.mode);
      seen.push(`${mode.mode}: ${mode.message}`);
    }
    assert.deepEqual(seen, [
      'Clippings: Now showing total tags',
      'tags: Clippings: Now showing tag counts',
      'top three: Clippings: Now showing top three tag counts',
      'current file: Clippings: Now showing total tags in current file',
      'total: Clippings: Now showing total tags',
    ]);
  });
});

describe('once notices', () => {
  it('shows each distinct set once, in any order', () => {
    const notice = new OnceNotice();
    assert.equal(notice.next([]), undefined);
    assert.equal(notice.next(['b', 'a']), 'Clippings: b; a');
    assert.equal(notice.next(['a', 'b']), undefined);
    assert.equal(notice.next(['a']), 'Clippings: a');
    assert.equal(notice.next([]), undefined);
    assert.equal(notice.next(['a']), 'Clippings: a', 'shown again after it cleared');
  });
});
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `pnpm -C extension typecheck`
Expected: FAIL. `error TS2339: Property 'statusBar' does not exist on type 'TestHooks'`, and `error TS2307: Cannot find module '../../status/presentation'`.

- [ ] **Step 3: Write the implementation**

Create `extension/src/commands/statusBar.ts`:

```ts
// The status bar click (spec 7.7): reveal the view, toggle highlights, or
// cycle the status bar mode.

import * as vscode from 'vscode';
import { statusBarClickBehaviour } from '../config/clientSettings';
import type { ConfigurationSync } from '../config/sync';
import type { SettingWriter } from '../config/writes';
import { nextStatusBarMode } from '../status/presentation';
import type { Prompts } from '../ui/prompts';

export interface StatusBarDeps {
  sync: ConfigurationSync;
  writer: SettingWriter;
  prompts: Prompts;
  view: vscode.TreeView<string>;
}

export function registerStatusBarCommand({ sync, writer, prompts, view }: StatusBarDeps): vscode.Disposable {
  return vscode.commands.registerCommand('clippings.onStatusBarClicked', async () => {
    const settings = sync.current;
    switch (statusBarClickBehaviour(settings)) {
      case 'reveal':
        if (!view.visible) await vscode.commands.executeCommand('clippings-view.focus');
        return;
      case 'toggle highlights':
        await writer.write('highlights.enabled', !settings.highlights.enabled, writer.valueTarget('highlights.enabled'));
        return;
      case 'cycle': {
        const { mode, message } = nextStatusBarMode(settings.general.statusBar);
        if (await writer.write('general.statusBar', mode, writer.valueTarget('general.statusBar'))) {
          void prompts.message('info', message);
        }
        return;
      }
    }
  });
}
```

Create `extension/src/status/controller.ts`:

```ts
// The status bar item, the view's title, badge and message, and the
// configuration notices, all driven by `clippings/status` (spec 7.5, 7.7).

import * as vscode from 'vscode';
import { NEEDS_SCAN_MESSAGE } from '../commands/scan';
import type { StatusParams } from '../protocol';
import type { Prompts } from '../ui/prompts';
import { OnceNotice, statusBarView, type StatusBarView } from './presentation';

export class StatusController implements vscode.Disposable {
  private readonly item = vscode.window.createStatusBarItem('clippings.status', vscode.StatusBarAlignment.Left, 0);
  private readonly warnings = new OnceNotice();
  private readonly errors = new OnceNotice();
  /** What the item shows, for the tests: VS Code cannot report it. */
  shown: StatusBarView | undefined;

  constructor(
    private readonly view: vscode.TreeView<string>,
    private readonly prompts: Prompts,
  ) {
    this.item.name = 'Clippings';
  }

  update(status: StatusParams): void {
    const bar = statusBarView(status);
    this.item.text = bar.text;
    this.item.tooltip = bar.tooltip;
    this.item.command = bar.command;
    if (bar.visible) this.item.show();
    else this.item.hide();
    this.shown = bar;

    this.view.title = status.viewTitle;
    this.view.badge = status.badge.value > 0 ? { value: status.badge.value, tooltip: status.badge.tooltip } : undefined;
    this.view.message = status.needsScan && !status.scanning ? NEEDS_SCAN_MESSAGE : undefined;

    const warning = this.warnings.next(status.warnings);
    if (warning) void this.prompts.message('warning', warning);
    const error = this.errors.next(status.error ? [status.error] : []);
    if (error) void this.showError(error);
  }

  private async showError(message: string): Promise<void> {
    const choice = await this.prompts.message('warning', message, 'Open Settings');
    if (choice === 'Open Settings') await vscode.commands.executeCommand('workbench.action.openSettings', 'clippings');
  }

  dispose(): void {
    this.item.dispose();
  }
}
```

Create `extension/src/status/presentation.ts`:

```ts
// What the status bar item shows for a server status, the status bar cycle,
// and one-time configuration notices (spec 5.14, 7.7). Pure.

import type { StatusBarMode, StatusParams } from '../protocol';

export interface StatusBarView {
  text: string;
  tooltip: string;
  command: string;
  visible: boolean;
}

export function statusBarView(status: StatusParams): StatusBarView {
  if (status.scanning) {
    return { text: 'Clippings: Scanning...', tooltip: 'Click to interrupt scan', command: 'clippings.stopScan', visible: true };
  }
  if (status.interrupted) {
    return { text: 'Clippings: Scanning interrupted.', tooltip: 'Click to restart', command: 'clippings.refresh', visible: true };
  }
  return { ...status.statusBar, command: 'clippings.onStatusBarClicked' };
}

/** The `cycle` click: total, tags, top three, current file, total. */
export function nextStatusBarMode(current: StatusBarMode): { mode: StatusBarMode; message: string } {
  switch (current) {
    case 'total':
      return { mode: 'tags', message: 'Clippings: Now showing tag counts' };
    case 'tags':
      return { mode: 'top three', message: 'Clippings: Now showing top three tag counts' };
    case 'top three':
      return { mode: 'current file', message: 'Clippings: Now showing total tags in current file' };
    default:
      return { mode: 'total', message: 'Clippings: Now showing total tags' };
  }
}

/** Remembers the last notice shown, so each distinct one is shown once. */
export class OnceNotice {
  private last = '';

  /** The notice to show for `items`, or undefined when there is none or it was just shown. */
  next(items: readonly string[]): string | undefined {
    const key = [...items].sort().join('\n');
    if (key === this.last) return undefined;
    this.last = key;
    return items.length > 0 ? `Clippings: ${items.join('; ')}` : undefined;
  }
}
```

Replace `extension/src/testApi.ts` with:

```ts
// What `activate` returns. The `test` hooks let the integration tests
// observe state VS Code offers no API to read (spec 12.5).

import type * as vscode from 'vscode';
import type { StatusParams } from './protocol';
import type { PersistedViewState } from './state/viewState';
import type { StatusBarView } from './status/presentation';
import type { TestItem } from './tree/testItems';
import type { Prompts } from './ui/prompts';

export interface ServerHooks {
  readonly running: boolean;
  readonly pid: number | undefined;
  readonly onStatus: vscode.Event<StatusParams>;
  readonly onRunning: vscode.Event<void>;
  /** Fires when repeated crashes stop the automatic restarts (spec 7.4). */
  readonly onGaveUp: vscode.Event<string>;
  /** Shortens the start timeout (`START_TIMEOUT_MS`) so tests need not wait for it. */
  setStartTimeout(ms: number): void;
  status(): StatusParams | undefined;
}

export interface TreeHooks {
  readonly view: vscode.TreeView<string>;
  /** Fires whenever the provider asks VS Code to refetch. */
  readonly onDidChange: vscode.Event<unknown>;
  /** The provider's children of `parent` (top level when omitted), as tree items. */
  items(parent?: string): Promise<TestItem[]>;
  /** The current tree item epoch (spec 7.5). */
  readonly epoch: number;
  /** The line the last todo click flashed. */
  lastFlash(): { uri: string; line: number } | undefined;
}

export interface TestHooks {
  /** Answers prompts and records messages. */
  readonly prompts: Prompts;
  viewState(): PersistedViewState;
  /** The context keys last set (spec 7.6). */
  contextKeys(): Readonly<Record<string, boolean | string>>;
  /** What the status bar item shows. */
  statusBar(): StatusBarView | undefined;
  readonly server: ServerHooks;
  readonly tree: TreeHooks;
}

export interface ClippingsApi {
  readonly version: string;
  readonly test: TestHooks;
}
```

Replace `extension/src/extension.ts` with:

```ts
// Composition root: builds each part of the extension and wires them to the
// server connection.

import * as vscode from 'vscode';
import { registerExpandCommands, resetExpansion } from './commands/expand';
import { registerFilterCommands } from './commands/filters';
import { registerGoToCommands } from './commands/goTo';
import { registerNavigationCommands } from './commands/navigation';
import { registerScanCommands } from './commands/scan';
import { needsRestart, registerServerCommands } from './commands/server';
import { registerSettingCommands } from './commands/settings';
import { registerStatusBarCommand } from './commands/statusBar';
import { registerViewCommands } from './commands/view';
import { affectsServer, readConfiguration } from './config/read';
import { ConfigurationSync, replacesTree } from './config/sync';
import { ContextKeys } from './context/apply';
import { contextValues } from './context/keys';
import { SettingWriter } from './config/writes';
import { IconResolver } from './icons/resolver';
import type { StatusParams } from './protocol';
import { ServerConnection } from './server/connection';
import { ViewStateStore } from './state/viewState';
import { StatusController } from './status/controller';
import type { ClippingsApi } from './testApi';
import { Expansion } from './tree/expansion';
import { NodeCache } from './tree/nodeCache';
import { LineFlash } from './tree/open';
import { TreeProvider } from './tree/provider';
import { Revealer } from './tree/reveal';
import { Prompts } from './ui/prompts';
import { testItem } from './tree/testItems';

let connection: ServerConnection | undefined;

export function activate(context: vscode.ExtensionContext): ClippingsApi {
  const log = vscode.window.createOutputChannel('Clippings', { log: true });
  const store = new ViewStateStore(context.workspaceState);
  const prompts = new Prompts();
  const activeUri = () => vscode.window.activeTextEditor?.document.uri.toString() ?? null;
  const server = new ServerConnection({
    context,
    log,
    settings: () => sync.current,
    activeUri,
    error: (message, ...actions) => prompts.message('error', message, ...actions),
  });
  connection = server;
  const sync = new ConfigurationSync(
    () => readConfiguration(store.snapshot()),
    (settings) => server.configure(settings),
  );
  let lastStatus: StatusParams | undefined;
  const contextKeys = new ContextKeys();
  const updateContext = () => void contextKeys.apply(contextValues(sync.current, lastStatus));
  sync.onPush(updateContext);
  updateContext();

  const cache = new NodeCache();
  const expansion = new Expansion(store);
  const provider = new TreeProvider(server, cache, {
    icons: new IconResolver(),
    itemId: (id) => expansion.itemId(id),
    expanded: (node) => expansion.expanded(node),
  });
  const treeView = vscode.window.createTreeView('clippings-view', { treeDataProvider: provider });
  const revealer = new Revealer({
    find: (uri, line) => server.find(uri, line),
    cache,
    view: treeView,
    settings: () => sync.current,
  });
  const flash = new LineFlash();
  const writer = new SettingWriter(prompts);
  const statusController = new StatusController(treeView, prompts);

  context.subscriptions.push(
    log,
    server,
    provider,
    treeView,
    ...registerServerCommands(server, log),
    revealer,
    flash,
    ...registerExpandCommands({ store, sync, expansion, provider }),
    ...registerNavigationCommands(revealer, flash),
    ...registerViewCommands({ store, sync, expansion, provider, prompts }),
    ...registerFilterCommands({ store, sync, cache, prompts }),
    ...registerSettingCommands(writer, prompts),
    ...registerGoToCommands(server),
    statusController,
    registerStatusBarCommand({ sync, writer, prompts, view: treeView }),
    ...registerScanCommands(server),
    server.onStatus((s) => {
      lastStatus = s;
      updateContext();
      statusController.update(s);
    }),
    server.onNewInstance(() => provider.reset()),
    server.onTreeChanged((p) => {
      if (p.refresh.includes(null)) expansion.onRootRefresh();
      provider.refresh(p.refresh);
    }),
    treeView.onDidExpandElement((e) => expansion.set(e.element, true)),
    treeView.onDidCollapseElement((e) => expansion.set(e.element, false)),
    vscode.workspace.onDidChangeConfiguration((e) => {
      if (needsRestart(e)) {
        sync.push();
        void server.restart();
        return;
      }
      if (!affectsServer(e)) return;
      const push = sync.push();
      if (e.affectsConfiguration('clippings.tree.expanded')) {
        resetExpansion({ expansion, provider }, replacesTree(push));
      }
    }),
    vscode.window.onDidChangeActiveTextEditor((editor) => {
      server.activeEditor(activeUri());
      revealer.onActiveEditor(editor);
    }),
  );
  void server.start();

  const manifest = context.extension.packageJSON as { version: string };
  return {
    version: manifest.version,
    test: {
      prompts,
      viewState: () => store.snapshot(),
      contextKeys: () => contextKeys.values,
      statusBar: () => statusController.shown,
      server: {
        get running() {
          return server.running;
        },
        get pid() {
          return server.pid;
        },
        onStatus: server.onStatus,
        onRunning: server.onRunning,
        onGaveUp: server.onGaveUp,
        setStartTimeout: (ms) => (server.startTimeoutMs = ms),
        status: () => lastStatus,
      },
      tree: {
        view: treeView,
        onDidChange: provider.onDidChangeTreeData,
        items: async (parent) => {
          const ids = await provider.getChildren(parent);
          return ids.map((id) => testItem(id, provider.getTreeItem(id)));
        },
        get epoch() {
          return expansion.currentEpoch;
        },
        lastFlash: () => flash.last,
      },
    },
  };
}

export async function deactivate(): Promise<void> {
  await connection?.stop();
  connection = undefined;
}
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `pnpm -C extension dev && pnpm -C extension typecheck && pnpm -C extension build`
Expected: no errors.

Run: `pnpm -C extension test:unit`
Expected: `62 passing`.

Run: `pnpm -C extension test:integration`
Expected: `52 passing`. A VS Code test window opens, runs the suite and closes.

- [ ] **Step 5: Commit**

```bash
git add -A
git commit -F - <<'EOF'
feat(extension): drive the status bar, badge, title and notices from the server status

Co-Authored-By: <your model attribution>
EOF
```

### Task 14: Decorations by generation and version

`DecorationManager` applies `clippings/styles` and `clippings/decorations` under the rules of spec 6.2 and 7.7: decoration types are created synchronously per key, a reset starts a new generation, and a decorations message from another generation or for an older document version is dropped. The last applied entry per document is reapplied when an editor becomes visible and dropped when the document closes. The rules and the mapping to render options are pure and unit-tested; the integration tests cover edits, a highlight toggle, reapplication from the cache and notebook cells (Review Focus 2).

**Files:**
- Create: `extension/src/decorations/manager.ts`, `extension/src/decorations/renderOptions.ts`, `extension/src/decorations/rules.ts`
- Modify: `extension/src/icons/resolver.ts`, `extension/src/testApi.ts`, `extension/src/extension.ts`
- Test: `extension/src/test/integration/helpers.ts` (modified), `extension/src/test/integration/decorations.test.ts`, `extension/src/test/integration/notebook.test.ts`, `extension/src/test/unit/decorations.test.ts`

**Interfaces:**
- Consumes: `icons/resolver.ts` (Task 7): `IconResolver`; `protocol.ts` (Task 2): `Colour`, `DecorationStyle`, `DecorationsParams`, `Range`, `StylesParams`.
- Produces, in `decorations/manager.ts`: `ApplySource` (type); `class DecorationManager implements vscode.Disposable` with `readonly onApplied: vscode.Event<{ uri: string; source: ApplySource }>`, `constructor(private readonly icons: IconResolver)`, `get currentGeneration(): number | undefined`, `get styleKeys(): string[]`, `entry(uri: string): DecorationsParams | undefined`, `onStyles(msg: StylesParams): void`, `onDecorations(msg: DecorationsParams): void`, `onVisibleEditors(editors: readonly vscode.TextEditor[]): void`, `onDocumentClosed(document: vscode.TextDocument): void`, `reset(): void`, `dispose(): void`.
- Produces, in `decorations/renderOptions.ts`: `RenderDeps` (interface); `function renderOptions(style: DecorationStyle, deps: RenderDeps): DecorationRenderOptions`.
- Produces, in `decorations/rules.ts`: `function stylesAction(current: number | undefined, msg: StylesParams): 'reset' | 'add' | 'drop'`; `function acceptDecorations(current: number | undefined, msg: Pick<DecorationsParams, 'generation' | 'version'>, documentVersion: number | undefined): boolean`; `function reusable(current: number | undefined, entry: Pick<DecorationsParams, 'generation' | 'version'>, documentVersion: number): boolean`; `function keysToSet(previous: Iterable<string>, ranges: Record<string, unknown>): string[]`.
- Produces, in `icons/resolver.ts`: `IconResolver` gains `gutterIcon(_icon: IconDescriptor): vscode.Uri | undefined`.
- Produces, in `testApi.ts`: `DecorationHooks` (interface); `TestHooks` gains `readonly decorations: DecorationHooks`.

- [ ] **Step 1: Write the failing tests**

Replace `extension/src/test/integration/helpers.ts` with:

```ts
// Shared helpers for the integration tests. Waits are driven by real
// signals from the extension, never by fixed sleeps.

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

export function workspacePath(...parts: string[]): string {
  const root = process.env['CLIPPINGS_TEST_WORKSPACE'];
  if (!root) throw new Error('CLIPPINGS_TEST_WORKSPACE is not set');
  return [root, ...parts].join('/');
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

Create `extension/src/test/integration/decorations.test.ts`:

```ts
import * as assert from 'node:assert/strict';
import * as vscode from 'vscode';
import type { Range } from '../../protocol';
import type { ClippingsApi } from '../../testApi';
import { getApi, nextEvent, setSetting, waitFor, whenIdle, workspacePath } from './helpers';

function r(line: number, start: number, end: number): Range {
  return { start: { line, character: start }, end: { line, character: end } };
}

describe('decorations', () => {
  let api: ClippingsApi;
  let editor: vscode.TextEditor;
  let uri: string;
  const entry = () => api.test.decorations.entry(uri);

  before(async () => {
    api = await getApi();
    await whenIdle(api);
  });

  beforeEach(async () => {
    editor = await vscode.window.showTextDocument(vscode.Uri.file(workspacePath('src', 'app.ts')));
    uri = editor.document.uri.toString();
  });

  afterEach(async () => {
    await vscode.commands.executeCommand('workbench.action.files.revert');
    await vscode.commands.executeCommand('workbench.action.closeAllEditors');
  });

  it('applies the server ranges per tag to an opened document', async () => {
    const applied = await waitFor('decorations', () => entry(), [api.test.decorations.onApplied]);
    assert.equal(applied.version, editor.document.version);
    assert.equal(applied.generation, api.test.decorations.generation);
    assert.deepEqual(applied.ranges, { TODO: [r(1, 5, 9)], FIXME: [r(3, 5, 10)], HACK: [r(7, 5, 9)] });
    for (const key of ['TODO', 'FIXME', 'HACK']) assert.ok(api.test.decorations.styleKeys.includes(key), key);
  });

  it('follows edits with the new document version', async () => {
    await waitFor('decorations', () => entry(), [api.test.decorations.onApplied]);
    await editor.edit((e) => e.insert(new vscode.Position(0, 0), '// BUG inserted at the top\n'));
    const version = editor.document.version;
    const applied = await waitFor(
      'decorations for the edit',
      () => (entry()?.version === version ? entry() : undefined),
      [api.test.decorations.onApplied],
    );
    assert.deepEqual(applied.ranges, { BUG: [r(0, 3, 6)], TODO: [r(2, 5, 9)], FIXME: [r(4, 5, 10)], HACK: [r(8, 5, 9)] });
  });

  it('clears when highlights are disabled and restyles when attributes change', async () => {
    await waitFor('decorations', () => entry(), [api.test.decorations.onApplied]);
    const generation = api.test.decorations.generation ?? -1;
    await setSetting('highlights.enabled', false);
    try {
      await waitFor('no decorations', () => entry() && Object.keys(entry()?.ranges ?? {}).length === 0, [
        api.test.decorations.onApplied,
      ]);
      assert.ok((api.test.decorations.generation ?? -1) > generation, 'a new style generation');
    } finally {
      await setSetting('highlights.enabled', undefined);
    }
    await waitFor('decorations again', () => Object.keys(entry()?.ranges ?? {}).length === 3, [
      api.test.decorations.onApplied,
    ]);
  });

  it('reapplies the cached decorations when the editor becomes visible again', async () => {
    await waitFor('decorations', () => entry(), [api.test.decorations.onApplied]);
    await vscode.window.showTextDocument(vscode.Uri.file(workspacePath('docs', 'plan.md')));
    const reapplied = nextEvent('a reapply from the cache', api.test.decorations.onApplied, (e) => e.uri === uri);
    await vscode.window.showTextDocument(editor.document);
    assert.equal((await reapplied).source, 'cache');
  });
});
```

Create `extension/src/test/integration/notebook.test.ts`:

```ts
import * as assert from 'node:assert/strict';
import { rmSync, writeFileSync } from 'node:fs';
import * as vscode from 'vscode';
import type { ClippingsApi } from '../../testApi';
import { DEFAULT_TREE } from './fixture';
import { getApi, treeBecomes, waitFor, whenIdle, workspacePath } from './helpers';

// Notebook cells reach the server through the `vscode-notebook-cell` scheme
// (spec 6.1), are indexed per cell and grouped under their notebook (spec
// 11.2), and are decorated in each cell's editor (spec 7.7).
const NOTEBOOK = {
  cells: [
    { cell_type: 'code', execution_count: null, metadata: {}, outputs: [], source: ['# TODO cell one\n', 'x = 1\n'] },
    { cell_type: 'markdown', metadata: {}, source: ['FIXME in markdown'] },
  ],
  metadata: { language_info: { name: 'python' } },
  nbformat: 4,
  nbformat_minor: 5,
};

describe('notebook cells', () => {
  let api: ClippingsApi;
  const path = workspacePath('notes.ipynb');

  before(async () => {
    api = await getApi();
    await whenIdle(api);
  });

  after(async () => {
    rmSync(path, { force: true });
    await treeBecomes(api, DEFAULT_TREE);
  });

  it('lists and decorates the todos in notebook cells', async () => {
    writeFileSync(path, JSON.stringify(NOTEBOOK));
    const notebook = await vscode.workspace.openNotebookDocument(vscode.Uri.file(path));
    await vscode.window.showNotebookDocument(notebook);
    const cell = notebook.cellAt(0).document.uri;
    assert.equal(cell.scheme, 'vscode-notebook-cell');
    const applied = await waitFor('decorations in the first cell', () => api.test.decorations.entry(cell.toString()), [
      api.test.decorations.onApplied,
    ]);
    assert.deepEqual(applied.ranges, {
      TODO: [{ start: { line: 0, character: 2 }, end: { line: 0, character: 6 } }],
    });
    await treeBecomes(api, [...DEFAULT_TREE, '  notes.ipynb', '    TODO cell one', '    FIXME in markdown']);

    // Empty the cells so the still-open cell buffers hold no todos, then save
    // and close; `after` deletes the file.
    const edit = new vscode.WorkspaceEdit();
    for (const c of notebook.getCells()) {
      edit.replace(c.document.uri, new vscode.Range(0, 0, c.document.lineCount, 0), '');
    }
    assert.ok(await vscode.workspace.applyEdit(edit));
    assert.ok(await notebook.save());
    await vscode.commands.executeCommand('workbench.action.closeAllEditors');
  });
});
```

Create `extension/src/test/unit/decorations.test.ts`:

```ts
import * as assert from 'node:assert/strict';
import type { ThemeColor, Uri } from 'vscode';
import { renderOptions } from '../../decorations/renderOptions';
import { acceptDecorations, keysToSet, reusable, stylesAction } from '../../decorations/rules';
import type { DecorationStyle } from '../../protocol';

describe('decoration rules', () => {
  it('starts a generation on reset and adds only to the current one', () => {
    assert.equal(stylesAction(undefined, { generation: 0, reset: true, styles: {} }), 'reset');
    assert.equal(stylesAction(3, { generation: 3, reset: false, styles: {} }), 'add');
    assert.equal(stylesAction(3, { generation: 2, reset: false, styles: {} }), 'drop');
    assert.equal(stylesAction(undefined, { generation: 0, reset: false, styles: {} }), 'drop');
  });

  it('drops decorations from another generation or an older version', () => {
    assert.equal(acceptDecorations(2, { generation: 2, version: 5 }, 5), true);
    assert.equal(acceptDecorations(2, { generation: 2, version: 6 }, 5), true, 'newer than the document is fine');
    assert.equal(acceptDecorations(2, { generation: 1, version: 5 }, 5), false);
    assert.equal(acceptDecorations(2, { generation: 2, version: 4 }, 5), false);
    assert.equal(acceptDecorations(2, { generation: 2, version: 4 }, undefined), false, 'closed document');
  });

  it('reuses a cached entry only for the same version and generation', () => {
    assert.equal(reusable(1, { generation: 1, version: 3 }, 3), true);
    assert.equal(reusable(1, { generation: 1, version: 3 }, 4), false);
    assert.equal(reusable(2, { generation: 1, version: 3 }, 3), false);
  });

  it('clears keys that were applied before and are now absent', () => {
    assert.deepEqual(keysToSet(['TODO', 'BUG'], { TODO: [], FIXME: [] }).sort(), ['BUG', 'FIXME', 'TODO']);
  });
});

describe('decoration render options', () => {
  const theme = (id: string) => ({ id }) as ThemeColor;
  const base: DecorationStyle = {
    color: { theme: 'editor.background' },
    backgroundColor: { css: 'rgba(255,0,0,0.5)' },
    overviewRulerColor: { css: '#ff0000' },
    overviewRulerLane: 4,
    borderRadius: '0.2em',
    fontStyle: 'normal',
    fontWeight: 'bold',
    textDecoration: '',
    isWholeLine: false,
    gutterIcon: null,
  };

  it('maps colours, ruler and font attributes', () => {
    const options = renderOptions(base, { themeColor: theme, gutterIcon: () => undefined });
    assert.deepEqual(options, {
      borderRadius: '0.2em',
      fontStyle: 'normal',
      fontWeight: 'bold',
      isWholeLine: false,
      color: { id: 'editor.background' },
      backgroundColor: 'rgba(255,0,0,0.5)',
      overviewRulerColor: '#ff0000',
      overviewRulerLane: 4,
    });
  });

  it('omits the ruler without a lane and adds whole-line, decoration and gutter icon', () => {
    const icon = { fsPath: '/icons/todo.svg' } as Uri;
    const options = renderOptions(
      { ...base, overviewRulerLane: null, isWholeLine: true, textDecoration: 'underline', gutterIcon: { kind: 'default' } },
      { themeColor: theme, gutterIcon: () => icon },
    );
    assert.equal(options.overviewRulerColor, undefined);
    assert.equal(options.overviewRulerLane, undefined);
    assert.equal(options.isWholeLine, true);
    assert.equal(options.textDecoration, 'underline');
    assert.equal(options.gutterIconPath, icon);
  });
});
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `pnpm -C extension typecheck`
Expected: FAIL. `error TS2339: Property 'decorations' does not exist on type 'TestHooks'`, and `error TS2307: Cannot find module` for `../../decorations/renderOptions` and `../../decorations/rules`.

- [ ] **Step 3: Write the implementation**

Create `extension/src/decorations/manager.ts`:

```ts
// Applies `clippings/styles` and `clippings/decorations` (spec 6.2, 7.7):
// decoration types per key and generation, the last decorations per
// document, and reapplication when an editor becomes visible.

import * as vscode from 'vscode';
import type { DecorationsParams, Range, StylesParams } from '../protocol';
import type { IconResolver } from '../icons/resolver';
import { renderOptions } from './renderOptions';
import { acceptDecorations, keysToSet, reusable, stylesAction } from './rules';

export type ApplySource = 'message' | 'cache';

function toRange(r: Range): vscode.Range {
  return new vscode.Range(r.start.line, r.start.character, r.end.line, r.end.character);
}

export class DecorationManager implements vscode.Disposable {
  private generation: number | undefined;
  private readonly types = new Map<string, vscode.TextEditorDecorationType>();
  /** The last applied decorations per document URI. */
  private readonly cache = new Map<string, DecorationsParams>();
  /** Keys applied per document, so a later message can clear the absent ones. */
  private readonly applied = new Map<string, Set<string>>();
  private readonly appliedEmitter = new vscode.EventEmitter<{ uri: string; source: ApplySource }>();
  /** Fires after decorations are applied to a document's editors. */
  readonly onApplied = this.appliedEmitter.event;

  constructor(private readonly icons: IconResolver) {}

  get currentGeneration(): number | undefined {
    return this.generation;
  }

  get styleKeys(): string[] {
    return [...this.types.keys()];
  }

  entry(uri: string): DecorationsParams | undefined {
    return this.cache.get(uri);
  }

  /** Creates each decoration type before returning, so later decorations find it. */
  onStyles(msg: StylesParams): void {
    const action = stylesAction(this.generation, msg);
    if (action === 'drop') return;
    if (action === 'reset') {
      this.disposeTypes();
      this.applied.clear();
      this.cache.clear();
      this.generation = msg.generation;
    }
    for (const [key, style] of Object.entries(msg.styles)) {
      this.types.get(key)?.dispose();
      const options = renderOptions(style, {
        themeColor: (id) => new vscode.ThemeColor(id),
        gutterIcon: (s) => (s.gutterIcon ? this.icons.gutterIcon(s.gutterIcon) : undefined),
      });
      this.types.set(key, vscode.window.createTextEditorDecorationType(options));
    }
  }

  onDecorations(msg: DecorationsParams): void {
    const document = vscode.workspace.textDocuments.find((d) => d.uri.toString() === msg.uri);
    if (!acceptDecorations(this.generation, msg, document?.version)) return;
    this.cache.set(msg.uri, msg);
    const editors = vscode.window.visibleTextEditors.filter((e) => e.document.uri.toString() === msg.uri);
    for (const editor of editors) this.apply(editor, msg);
    this.appliedEmitter.fire({ uri: msg.uri, source: 'message' });
  }

  /** Reapplies cached decorations to editors that became visible. */
  onVisibleEditors(editors: readonly vscode.TextEditor[]): void {
    for (const editor of editors) {
      const uri = editor.document.uri.toString();
      const entry = this.cache.get(uri);
      if (!entry || !reusable(this.generation, entry, editor.document.version)) continue;
      this.apply(editor, entry);
      this.appliedEmitter.fire({ uri, source: 'cache' });
    }
  }

  onDocumentClosed(document: vscode.TextDocument): void {
    const uri = document.uri.toString();
    this.cache.delete(uri);
    this.applied.delete(uri);
  }

  /** A new server instance: forget every type, generation and cached entry. */
  reset(): void {
    this.disposeTypes();
    this.generation = undefined;
    this.cache.clear();
    this.applied.clear();
  }

  private apply(editor: vscode.TextEditor, msg: DecorationsParams): void {
    const previous = this.applied.get(msg.uri) ?? new Set<string>();
    for (const key of keysToSet(previous, msg.ranges)) {
      const type = this.types.get(key);
      if (type) editor.setDecorations(type, (msg.ranges[key] ?? []).map(toRange));
    }
    this.applied.set(msg.uri, new Set(Object.keys(msg.ranges)));
  }

  private disposeTypes(): void {
    for (const type of this.types.values()) type.dispose();
    this.types.clear();
  }

  dispose(): void {
    this.disposeTypes();
    this.appliedEmitter.dispose();
  }
}
```

Create `extension/src/decorations/renderOptions.ts`:

```ts
// A server decoration style as VS Code decoration options (spec 5.13).
// Pure: the caller supplies theme colours and the gutter icon.

import type { DecorationRenderOptions, ThemeColor, Uri } from 'vscode';
import type { Colour, DecorationStyle } from '../protocol';

export interface RenderDeps {
  themeColor(id: string): ThemeColor;
  /** The gutter icon file, when the descriptor renders to SVG. */
  gutterIcon(style: DecorationStyle): Uri | undefined;
}

function colour(value: Colour | null, deps: RenderDeps): string | ThemeColor | undefined {
  if (!value) return undefined;
  return 'theme' in value ? deps.themeColor(value.theme) : value.css;
}

export function renderOptions(style: DecorationStyle, deps: RenderDeps): DecorationRenderOptions {
  const options: DecorationRenderOptions = {
    borderRadius: style.borderRadius,
    fontStyle: style.fontStyle,
    fontWeight: style.fontWeight,
    isWholeLine: style.isWholeLine,
  };
  const fg = colour(style.color, deps);
  const bg = colour(style.backgroundColor, deps);
  if (fg !== undefined) options.color = fg;
  if (bg !== undefined) options.backgroundColor = bg;
  if (style.textDecoration) options.textDecoration = style.textDecoration;
  if (style.overviewRulerLane !== null) {
    const ruler = colour(style.overviewRulerColor, deps);
    if (ruler !== undefined) options.overviewRulerColor = ruler;
    // The server's lane numbers are VS Code's OverviewRulerLane values.
    options.overviewRulerLane = style.overviewRulerLane;
  }
  const gutter = deps.gutterIcon(style);
  if (gutter) options.gutterIconPath = gutter;
  return options;
}
```

Create `extension/src/decorations/rules.ts`:

```ts
// The styles and decorations rules of spec 6.2. Pure.

import type { DecorationsParams, StylesParams } from '../protocol';

/**
 * What to do with a `clippings/styles` message: a reset starts a new
 * generation; other messages add keys to the current generation only.
 */
export function stylesAction(current: number | undefined, msg: StylesParams): 'reset' | 'add' | 'drop' {
  if (msg.reset) return 'reset';
  return msg.generation === current ? 'add' : 'drop';
}

/**
 * Whether to apply a `clippings/decorations` message: its generation must be
 * current and its version no older than the open document's.
 */
export function acceptDecorations(
  current: number | undefined,
  msg: Pick<DecorationsParams, 'generation' | 'version'>,
  documentVersion: number | undefined,
): boolean {
  return msg.generation === current && documentVersion !== undefined && msg.version >= documentVersion;
}

/** Whether a cached entry can be reapplied to an editor that became visible. */
export function reusable(
  current: number | undefined,
  entry: Pick<DecorationsParams, 'generation' | 'version'>,
  documentVersion: number,
): boolean {
  return entry.generation === current && entry.version === documentVersion;
}

/** Keys to set on an editor: the new keys, plus previously applied keys to clear. */
export function keysToSet(previous: Iterable<string>, ranges: Record<string, unknown>): string[] {
  return [...new Set([...previous, ...Object.keys(ranges)])];
}
```

Replace `extension/src/icons/resolver.ts` with:

```ts
// Maps the server's icon descriptors onto VS Code icons (spec 7.7).

import * as vscode from 'vscode';
import type { IconDescriptor } from '../protocol';

export type TreeIcon = vscode.ThemeIcon | vscode.Uri;

export class IconResolver {
  /** An icon for a tree item, or undefined for none. */
  treeIcon(icon: IconDescriptor | null): TreeIcon | undefined {
    if (!icon) return undefined;
    switch (icon.kind) {
      case 'codicon':
        return new vscode.ThemeIcon(icon.name, icon.colour ? new vscode.ThemeColor(icon.colour) : undefined);
      case 'folder':
        return vscode.ThemeIcon.Folder;
      case 'file':
        return vscode.ThemeIcon.File;
      default:
        return undefined;
    }
  }

  /** A gutter icon file; codicons have none. */
  gutterIcon(_icon: IconDescriptor): vscode.Uri | undefined {
    return undefined;
  }
}
```

Replace `extension/src/testApi.ts` with:

```ts
// What `activate` returns. The `test` hooks let the integration tests
// observe state VS Code offers no API to read (spec 12.5).

import type * as vscode from 'vscode';
import type { ApplySource } from './decorations/manager';
import type { DecorationsParams, StatusParams } from './protocol';
import type { PersistedViewState } from './state/viewState';
import type { StatusBarView } from './status/presentation';
import type { TestItem } from './tree/testItems';
import type { Prompts } from './ui/prompts';

export interface ServerHooks {
  readonly running: boolean;
  readonly pid: number | undefined;
  readonly onStatus: vscode.Event<StatusParams>;
  readonly onRunning: vscode.Event<void>;
  /** Fires when repeated crashes stop the automatic restarts (spec 7.4). */
  readonly onGaveUp: vscode.Event<string>;
  /** Shortens the start timeout (`START_TIMEOUT_MS`) so tests need not wait for it. */
  setStartTimeout(ms: number): void;
  status(): StatusParams | undefined;
}

export interface TreeHooks {
  readonly view: vscode.TreeView<string>;
  /** Fires whenever the provider asks VS Code to refetch. */
  readonly onDidChange: vscode.Event<unknown>;
  /** The provider's children of `parent` (top level when omitted), as tree items. */
  items(parent?: string): Promise<TestItem[]>;
  /** The current tree item epoch (spec 7.5). */
  readonly epoch: number;
  /** The line the last todo click flashed. */
  lastFlash(): { uri: string; line: number } | undefined;
}

export interface DecorationHooks {
  /** Fires after decorations are applied to a document's visible editors. */
  readonly onApplied: vscode.Event<{ uri: string; source: ApplySource }>;
  /** The decorations last applied to a document. */
  entry(uri: string): DecorationsParams | undefined;
  readonly generation: number | undefined;
  readonly styleKeys: string[];
}

export interface TestHooks {
  /** Answers prompts and records messages. */
  readonly prompts: Prompts;
  viewState(): PersistedViewState;
  /** The context keys last set (spec 7.6). */
  contextKeys(): Readonly<Record<string, boolean | string>>;
  /** What the status bar item shows. */
  statusBar(): StatusBarView | undefined;
  readonly decorations: DecorationHooks;
  readonly server: ServerHooks;
  readonly tree: TreeHooks;
}

export interface ClippingsApi {
  readonly version: string;
  readonly test: TestHooks;
}
```

Replace `extension/src/extension.ts` with:

```ts
// Composition root: builds each part of the extension and wires them to the
// server connection.

import * as vscode from 'vscode';
import { registerExpandCommands, resetExpansion } from './commands/expand';
import { registerFilterCommands } from './commands/filters';
import { registerGoToCommands } from './commands/goTo';
import { registerNavigationCommands } from './commands/navigation';
import { registerScanCommands } from './commands/scan';
import { needsRestart, registerServerCommands } from './commands/server';
import { registerSettingCommands } from './commands/settings';
import { registerStatusBarCommand } from './commands/statusBar';
import { registerViewCommands } from './commands/view';
import { affectsServer, readConfiguration } from './config/read';
import { ConfigurationSync, replacesTree } from './config/sync';
import { ContextKeys } from './context/apply';
import { contextValues } from './context/keys';
import { SettingWriter } from './config/writes';
import { DecorationManager } from './decorations/manager';
import { IconResolver } from './icons/resolver';
import type { StatusParams } from './protocol';
import { ServerConnection } from './server/connection';
import { ViewStateStore } from './state/viewState';
import { StatusController } from './status/controller';
import type { ClippingsApi } from './testApi';
import { Expansion } from './tree/expansion';
import { NodeCache } from './tree/nodeCache';
import { LineFlash } from './tree/open';
import { TreeProvider } from './tree/provider';
import { Revealer } from './tree/reveal';
import { Prompts } from './ui/prompts';
import { testItem } from './tree/testItems';

let connection: ServerConnection | undefined;

export function activate(context: vscode.ExtensionContext): ClippingsApi {
  const log = vscode.window.createOutputChannel('Clippings', { log: true });
  const store = new ViewStateStore(context.workspaceState);
  const prompts = new Prompts();
  const activeUri = () => vscode.window.activeTextEditor?.document.uri.toString() ?? null;
  const server = new ServerConnection({
    context,
    log,
    settings: () => sync.current,
    activeUri,
    error: (message, ...actions) => prompts.message('error', message, ...actions),
  });
  connection = server;
  const sync = new ConfigurationSync(
    () => readConfiguration(store.snapshot()),
    (settings) => server.configure(settings),
  );
  let lastStatus: StatusParams | undefined;
  const contextKeys = new ContextKeys();
  const updateContext = () => void contextKeys.apply(contextValues(sync.current, lastStatus));
  sync.onPush(updateContext);
  updateContext();

  const icons = new IconResolver();
  const decorations = new DecorationManager(icons);
  const cache = new NodeCache();
  const expansion = new Expansion(store);
  const provider = new TreeProvider(server, cache, {
    icons,
    itemId: (id) => expansion.itemId(id),
    expanded: (node) => expansion.expanded(node),
  });
  const treeView = vscode.window.createTreeView('clippings-view', { treeDataProvider: provider });
  const revealer = new Revealer({
    find: (uri, line) => server.find(uri, line),
    cache,
    view: treeView,
    settings: () => sync.current,
  });
  const flash = new LineFlash();
  const writer = new SettingWriter(prompts);
  const statusController = new StatusController(treeView, prompts);

  context.subscriptions.push(
    log,
    server,
    provider,
    treeView,
    ...registerServerCommands(server, log),
    revealer,
    flash,
    ...registerExpandCommands({ store, sync, expansion, provider }),
    ...registerNavigationCommands(revealer, flash),
    ...registerViewCommands({ store, sync, expansion, provider, prompts }),
    ...registerFilterCommands({ store, sync, cache, prompts }),
    ...registerSettingCommands(writer, prompts),
    ...registerGoToCommands(server),
    statusController,
    registerStatusBarCommand({ sync, writer, prompts, view: treeView }),
    ...registerScanCommands(server),
    server.onStatus((s) => {
      lastStatus = s;
      updateContext();
      statusController.update(s);
    }),
    decorations,
    server.onNewInstance(() => {
      provider.reset();
      decorations.reset();
    }),
    server.onStyles((p) => decorations.onStyles(p)),
    server.onDecorations((p) => decorations.onDecorations(p)),
    vscode.window.onDidChangeVisibleTextEditors((editors) => decorations.onVisibleEditors(editors)),
    vscode.workspace.onDidCloseTextDocument((d) => decorations.onDocumentClosed(d)),
    server.onTreeChanged((p) => {
      if (p.refresh.includes(null)) expansion.onRootRefresh();
      provider.refresh(p.refresh);
    }),
    treeView.onDidExpandElement((e) => expansion.set(e.element, true)),
    treeView.onDidCollapseElement((e) => expansion.set(e.element, false)),
    vscode.workspace.onDidChangeConfiguration((e) => {
      if (needsRestart(e)) {
        sync.push();
        void server.restart();
        return;
      }
      if (!affectsServer(e)) return;
      const push = sync.push();
      if (e.affectsConfiguration('clippings.tree.expanded')) {
        resetExpansion({ expansion, provider }, replacesTree(push));
      }
    }),
    vscode.window.onDidChangeActiveTextEditor((editor) => {
      server.activeEditor(activeUri());
      revealer.onActiveEditor(editor);
    }),
  );
  void server.start();

  const manifest = context.extension.packageJSON as { version: string };
  return {
    version: manifest.version,
    test: {
      prompts,
      viewState: () => store.snapshot(),
      contextKeys: () => contextKeys.values,
      statusBar: () => statusController.shown,
      decorations: {
        onApplied: decorations.onApplied,
        entry: (uri) => decorations.entry(uri),
        get generation() {
          return decorations.currentGeneration;
        },
        get styleKeys() {
          return decorations.styleKeys;
        },
      },
      server: {
        get running() {
          return server.running;
        },
        get pid() {
          return server.pid;
        },
        onStatus: server.onStatus,
        onRunning: server.onRunning,
        onGaveUp: server.onGaveUp,
        setStartTimeout: (ms) => (server.startTimeoutMs = ms),
        status: () => lastStatus,
      },
      tree: {
        view: treeView,
        onDidChange: provider.onDidChangeTreeData,
        items: async (parent) => {
          const ids = await provider.getChildren(parent);
          return ids.map((id) => testItem(id, provider.getTreeItem(id)));
        },
        get epoch() {
          return expansion.currentEpoch;
        },
        lastFlash: () => flash.last,
      },
    },
  };
}

export async function deactivate(): Promise<void> {
  await connection?.stop();
  connection = undefined;
}
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `pnpm -C extension dev && pnpm -C extension typecheck && pnpm -C extension build`
Expected: no errors.

Run: `pnpm -C extension test:unit`
Expected: `68 passing`.

Run: `pnpm -C extension test:integration`
Expected: `57 passing`. A VS Code test window opens, runs the suite and closes.

- [ ] **Step 5: Commit**

```bash
git add -A
git commit -F - <<'EOF'
feat(extension): apply decoration styles and ranges by generation and version

Co-Authored-By: <your model attribution>
EOF
```

### Task 15: Icons rendered to SVG

Octicons, the `todo-tree` and `todo-tree-filled` icons and the check-circle are rendered to SVG in their colour, written once under global storage with deterministic, filesystem-safe names, and used for tree items and gutter icons (spec 7.7). The todo icons are Clippings' own drawings (ruling 4). Icon names that are not octicons, the two todo names or `$(codicon)` names are reported once as a warning (ruling 5).

**Files:**
- Create: `extension/src/icons/files.ts`, `extension/src/icons/svg.ts`
- Modify: `extension/src/decorations/manager.ts`, `extension/src/icons/resolver.ts`, `extension/src/testApi.ts`, `extension/src/extension.ts`
- Test: `extension/src/test/integration/icons.test.ts`, `extension/src/test/unit/icons.test.ts`

**Interfaces:**
- Consumes: `protocol.ts` (Task 2): `IconDescriptor`, `Settings`; `status/presentation.ts` (Task 13): `OnceNotice`.
- Produces, in `icons/files.ts`: `class IconFiles` with `constructor(private readonly dir: string)`, `path(icon: SvgIcon): string`.
- Produces, in `icons/svg.ts`: `SvgIcon` (type); `function isSvgIcon(icon: IconDescriptor): icon is SvgIcon`; `function isOcticon(name: string): boolean`; `function renderSvg(icon: SvgIcon): string`; `function iconFileName(icon: SvgIcon): string`; `function invalidIcons(settings: Settings): string[]`.
- Produces, in `decorations/manager.ts`: `DecorationManager` gains `options(key: string): vscode.DecorationRenderOptions | undefined`.
- Produces, in `icons/resolver.ts`: `IconResolver` gains `constructor(private readonly files: IconFiles, private readonly defaultIcon: vscode.Uri)`, `gutterIcon(icon: IconDescriptor): vscode.Uri | undefined`.
- Produces, in `testApi.ts`: `DecorationHooks` gains `options(key: string): vscode.DecorationRenderOptions | undefined`.

- [ ] **Step 1: Write the failing tests**

Create `extension/src/test/integration/icons.test.ts`:

```ts
import * as assert from 'node:assert/strict';
import { existsSync, readFileSync } from 'node:fs';
import { basename } from 'node:path';
import * as vscode from 'vscode';
import type { ClippingsApi } from '../../testApi';
import { DEFAULT_TREE } from './fixture';
import { getApi, itemAt, setSetting, treeBecomes, waitFor, whenIdle, workspacePath } from './helpers';

describe('icons', () => {
  let api: ClippingsApi;
  const todoIcon = async () => (await itemAt(api, 'workspace', 'lib', 'notes.rs', 'TODO first line of a long note')).icon;

  before(async () => {
    api = await getApi();
    await whenIdle(api);
    await treeBecomes(api, DEFAULT_TREE);
  });

  afterEach(async () => {
    await setSetting('highlights.customHighlight', undefined);
    await vscode.commands.executeCommand('workbench.action.closeAllEditors');
  });

  it('renders octicons and the check-circle to SVG files in the icon colour', async () => {
    const bug = await itemAt(api, 'workspace', 'src', 'util', 'strings.py', 'BUG (bob) drops leading tabs too');
    assert.ok(bug.icon);
    assert.match(basename(bug.icon), /^octicon-bug-green-[0-9a-f]{8}\.svg$/);
    assert.match(readFileSync(bug.icon, 'utf8'), /octicon-bug.*fill="green"|fill="green".*octicon-bug/);
    const todo = await todoIcon();
    assert.ok(todo);
    assert.match(basename(todo), /^check-check-green-/);
    assert.ok(existsSync(todo));
  });

  it('renders the todo-tree icon names and codicons', async () => {
    await setSetting('highlights.customHighlight', { TODO: { icon: 'todo-tree-filled', iconColour: '#123456' } });
    const filled = await waitFor('a filled todo icon', async () => ((await todoIcon())?.includes('todoTree-filled') ? todoIcon() : undefined), [
      api.test.tree.onDidChange,
    ]);
    assert.match(readFileSync(filled, 'utf8'), /fill="#123456"/);

    await setSetting('highlights.customHighlight', { TODO: { icon: '$(beaker)' } });
    await waitFor('a codicon', async () => (await todoIcon()) === '$(beaker)', [api.test.tree.onDidChange]);
  });

  it('gives decorations a gutter icon file when asked', async () => {
    await setSetting('highlights.customHighlight', { TODO: { gutterIcon: true, icon: 'flame' } });
    const editor = await vscode.window.showTextDocument(vscode.Uri.file(workspacePath('src', 'app.ts')));
    const uri = editor.document.uri.toString();
    await waitFor('decorations with a gutter icon', () => api.test.decorations.options('TODO')?.gutterIconPath && api.test.decorations.entry(uri), [
      api.test.decorations.onApplied,
    ]);
    const gutter = api.test.decorations.options('TODO')?.gutterIconPath;
    assert.ok(gutter instanceof vscode.Uri);
    assert.match(basename(gutter.fsPath), /^octicon-flame-green-/);
    assert.ok(existsSync(gutter.fsPath));
    assert.equal(api.test.decorations.options('FIXME')?.gutterIconPath, undefined);
  });

  it('warns once about icon names it does not know', async () => {
    const shown = () => api.test.prompts.shown.filter((s) => s.kind === 'warning' && s.message.includes('Invalid icons'));
    const before = shown().length;
    await setSetting('highlights.customHighlight', { TODO: { icon: 'not-an-octicon' } });
    await waitFor('an icon warning', () => shown().length === before + 1, [api.test.server.onStatus]);
    const last = shown().at(-1);
    assert.equal(last?.kind === 'warning' ? last.message : '', 'Clippings: Invalid icons: not-an-octicon');
  });
});
```

Create `extension/src/test/unit/icons.test.ts`:

```ts
import * as assert from 'node:assert/strict';
import { mkdtempSync, readFileSync, rmSync, statSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { buildConfiguration } from '../../config/configuration';
import { IconFiles } from '../../icons/files';
import { iconFileName, invalidIcons, isOcticon, renderSvg } from '../../icons/svg';

describe('icon svgs', () => {
  it('renders octicons in the colour, falling back to check', () => {
    const bug = renderSvg({ kind: 'octicon', name: 'bug', colour: '#ff0000' });
    assert.match(bug, /^<svg /);
    assert.match(bug, /fill="#ff0000"/);
    assert.match(bug, /octicon-bug/);
    const unknown = renderSvg({ kind: 'octicon', name: 'no-such-icon', colour: 'red' });
    assert.match(unknown, /octicon-check"/);
  });

  it('renders the todo icons and the check-circle', () => {
    const outline = renderSvg({ kind: 'todoTree', filled: false, colour: 'blue' });
    const filled = renderSvg({ kind: 'todoTree', filled: true, colour: 'blue' });
    assert.notEqual(outline, filled);
    assert.match(outline, /stroke="blue"/);
    assert.match(filled, /<rect[^>]*fill="blue"/);
    assert.match(renderSvg({ kind: 'check', colour: 'green' }), /octicon-check-circle-fill.*fill="green"|fill="green".*octicon-check-circle-fill/);
  });

  it('escapes colours for the attribute', () => {
    assert.match(renderSvg({ kind: 'check', colour: 'x" onload="y' }), /fill="x&#34; onload=&#34;y"/);
  });

  it('names files deterministically, safely and per colour', () => {
    const a = iconFileName({ kind: 'octicon', name: 'bug', colour: 'rgba(255, 0, 0, 0.5)' });
    assert.equal(a, iconFileName({ kind: 'octicon', name: 'bug', colour: 'rgba(255, 0, 0, 0.5)' }));
    assert.match(a, /^octicon-bug-rgba_255_0_0_0_5_-[0-9a-f]{8}\.svg$/);
    assert.notEqual(a, iconFileName({ kind: 'octicon', name: 'bug', colour: 'rgba(255,0,0,0.5)' }));
    assert.notEqual(
      iconFileName({ kind: 'todoTree', filled: true, colour: 'red' }),
      iconFileName({ kind: 'todoTree', filled: false, colour: 'red' }),
    );
    assert.doesNotMatch(iconFileName({ kind: 'check', colour: '../../etc' }), /\.\.|\//);
  });

  it('knows the octicon set', () => {
    assert.ok(isOcticon('flame'));
    assert.ok(!isOcticon('constructor'));
    assert.ok(!isOcticon('flames'));
  });
});

describe('icon files', () => {
  it('writes each icon once and remembers it', () => {
    const dir = mkdtempSync(join(tmpdir(), 'clippings-icons-'));
    try {
      const files = new IconFiles(join(dir, 'icons'));
      const path = files.path({ kind: 'octicon', name: 'flame', colour: 'orange' });
      assert.match(readFileSync(path, 'utf8'), /fill="orange"/);
      const written = statSync(path).mtimeMs;
      assert.equal(files.path({ kind: 'octicon', name: 'flame', colour: 'orange' }), path);
      assert.equal(new IconFiles(join(dir, 'icons')).path({ kind: 'octicon', name: 'flame', colour: 'orange' }), path);
      assert.equal(statSync(path).mtimeMs, written);
    } finally {
      rmSync(dir, { recursive: true, force: true });
    }
  });
});

describe('icon validation', () => {
  it('reports names that are not octicons, codicons or todo-tree icons', () => {
    const settings = buildConfiguration({
      groups: {
        general: {},
        highlights: {
          defaultHighlight: { icon: 'nope' },
          customHighlight: {
            A: { icon: 'bug' },
            B: { icon: '$(beaker)' },
            C: { icon: 'todo-tree' },
            D: { icon: 'todo-tree-filled' },
            E: { icon: 'nope' },
            F: { icon: 'also-bad' },
            G: {},
          },
        },
        filtering: {},
        tree: {},
        regex: {},
      },
      filesExclude: {},
      searchExclude: {},
      explorerCompactFolders: false,
      viewState: { currentFilter: '', filtered: false, includeGlobs: [], excludeGlobs: [] },
    });
    assert.deepEqual(invalidIcons(settings), ['nope', 'also-bad']);
  });
});
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `pnpm -C extension typecheck`
Expected: FAIL. `error TS2339: Property 'options' does not exist on type 'DecorationHooks'`, and `error TS2307: Cannot find module` for `../../icons/files` and `../../icons/svg`.

- [ ] **Step 3: Write the implementation**

Create `extension/src/icons/files.ts`:

```ts
// Rendered icons on disk (spec 7.7): one file per icon and colour under
// global storage, written synchronously the first time it is needed and
// remembered in memory.

import { existsSync, mkdirSync, writeFileSync } from 'node:fs';
import { join } from 'node:path';
import { iconFileName, renderSvg, type SvgIcon } from './svg';

export class IconFiles {
  private readonly known = new Map<string, string>();

  constructor(private readonly dir: string) {}

  /** The path of the icon's SVG file, writing it if missing. */
  path(icon: SvgIcon): string {
    const name = iconFileName(icon);
    const cached = this.known.get(name);
    if (cached) return cached;
    const path = join(this.dir, name);
    if (!existsSync(path)) {
      mkdirSync(this.dir, { recursive: true });
      writeFileSync(path, renderSvg(icon));
    }
    this.known.set(name, path);
    return path;
  }
}
```

Create `extension/src/icons/svg.ts`:

```ts
// SVG icons rendered in a colour (spec 5.13, 7.7): octicons from
// @primer/octicons, Clippings' own todo icons, and the check-circle. Pure.

import octicons from '@primer/octicons';
import type { IconDescriptor, Settings } from '../protocol';

/** The descriptors that render to an SVG file. */
export type SvgIcon = Extract<IconDescriptor, { kind: 'octicon' | 'todoTree' | 'check' }>;

export function isSvgIcon(icon: IconDescriptor): icon is SvgIcon {
  return icon.kind === 'octicon' || icon.kind === 'todoTree' || icon.kind === 'check';
}

export function isOcticon(name: string): boolean {
  return Object.hasOwn(octicons, name);
}

function attr(value: string): string {
  return value.replace(/[&<>"']/g, (c) => `&#${c.charCodeAt(0)};`);
}

function octicon(name: string, colour: string): string {
  const icon = octicons[isOcticon(name) ? name : 'check'];
  if (!icon) throw new Error('@primer/octicons has no check icon');
  return icon.toSVG({ xmlns: 'http://www.w3.org/2000/svg', fill: attr(colour) });
}

/** Clippings' own todo icon: a rounded square with a tick, outline or filled. */
function todo(filled: boolean, colour: string): string {
  const c = attr(colour);
  const box = filled
    ? `<rect x="1" y="1" width="14" height="14" rx="3.5" fill="${c}"/>`
    : `<rect x="1.75" y="1.75" width="12.5" height="12.5" rx="3" fill="none" stroke="${c}" stroke-width="1.5"/>`;
  const tick = filled ? '#ffffff' : c;
  return (
    '<svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" viewBox="0 0 16 16">' +
    box +
    `<path d="M4.5 8.25l2.25 2.25 4.75-5" fill="none" stroke="${tick}" stroke-width="1.75" stroke-linecap="round" stroke-linejoin="round"/>` +
    '</svg>'
  );
}

export function renderSvg(icon: SvgIcon): string {
  switch (icon.kind) {
    case 'octicon':
      return octicon(icon.name, icon.colour);
    case 'todoTree':
      return todo(icon.filled, icon.colour);
    case 'check':
      return octicon('check-circle-fill', icon.colour);
  }
}

/** FNV-1a, 32 bits, as eight hex digits. */
function hash(text: string): string {
  let h = 0x811c9dc5;
  for (let i = 0; i < text.length; i++) {
    h ^= text.charCodeAt(i);
    h = Math.imul(h, 0x01000193) >>> 0;
  }
  return h.toString(16).padStart(8, '0');
}

/**
 * A deterministic, filesystem-safe file name per icon and colour. The
 * readable part helps debugging; the hash keeps distinct colours apart.
 */
export function iconFileName(icon: SvgIcon): string {
  const name = icon.kind === 'octicon' ? icon.name : icon.kind === 'todoTree' ? (icon.filled ? 'filled' : 'outline') : 'check';
  const key = `${icon.kind}|${name}|${icon.colour}`;
  const readable = `${icon.kind}-${name}-${icon.colour}`.replace(/[^A-Za-z0-9_-]+/g, '_').slice(0, 48);
  return `${readable}-${hash(key)}.svg`;
}

/**
 * Icon names that are neither octicons, todo-tree's own names, nor codicons
 * (`$(name)`, which VS Code checks itself).
 */
export function invalidIcons(settings: Settings): string[] {
  const names = [
    settings.highlights.defaultHighlight?.icon,
    ...Object.values(settings.highlights.customHighlight ?? {}).map((a) => a?.icon),
  ];
  const bad = names.filter(
    (n): n is string =>
      typeof n === 'string' &&
      n.length > 0 &&
      !n.trim().startsWith('$(') &&
      n !== 'todo-tree' &&
      n !== 'todo-tree-filled' &&
      !isOcticon(n),
  );
  return [...new Set(bad)];
}
```

Replace `extension/src/decorations/manager.ts` with:

```ts
// Applies `clippings/styles` and `clippings/decorations` (spec 6.2, 7.7):
// decoration types per key and generation, the last decorations per
// document, and reapplication when an editor becomes visible.

import * as vscode from 'vscode';
import type { DecorationsParams, Range, StylesParams } from '../protocol';
import type { IconResolver } from '../icons/resolver';
import { renderOptions } from './renderOptions';
import { acceptDecorations, keysToSet, reusable, stylesAction } from './rules';

export type ApplySource = 'message' | 'cache';

function toRange(r: Range): vscode.Range {
  return new vscode.Range(r.start.line, r.start.character, r.end.line, r.end.character);
}

export class DecorationManager implements vscode.Disposable {
  private generation: number | undefined;
  private readonly types = new Map<string, vscode.TextEditorDecorationType>();
  private readonly renderOptions = new Map<string, vscode.DecorationRenderOptions>();
  /** The last applied decorations per document URI. */
  private readonly cache = new Map<string, DecorationsParams>();
  /** Keys applied per document, so a later message can clear the absent ones. */
  private readonly applied = new Map<string, Set<string>>();
  private readonly appliedEmitter = new vscode.EventEmitter<{ uri: string; source: ApplySource }>();
  /** Fires after decorations are applied to a document's editors. */
  readonly onApplied = this.appliedEmitter.event;

  constructor(private readonly icons: IconResolver) {}

  get currentGeneration(): number | undefined {
    return this.generation;
  }

  get styleKeys(): string[] {
    return [...this.types.keys()];
  }

  /** The options a key's decoration type was created with, for the tests. */
  options(key: string): vscode.DecorationRenderOptions | undefined {
    return this.renderOptions.get(key);
  }

  entry(uri: string): DecorationsParams | undefined {
    return this.cache.get(uri);
  }

  /** Creates each decoration type before returning, so later decorations find it. */
  onStyles(msg: StylesParams): void {
    const action = stylesAction(this.generation, msg);
    if (action === 'drop') return;
    if (action === 'reset') {
      this.disposeTypes();
      this.applied.clear();
      this.cache.clear();
      this.generation = msg.generation;
    }
    for (const [key, style] of Object.entries(msg.styles)) {
      this.types.get(key)?.dispose();
      const options = renderOptions(style, {
        themeColor: (id) => new vscode.ThemeColor(id),
        gutterIcon: (s) => (s.gutterIcon ? this.icons.gutterIcon(s.gutterIcon) : undefined),
      });
      this.types.set(key, vscode.window.createTextEditorDecorationType(options));
      this.renderOptions.set(key, options);
    }
  }

  onDecorations(msg: DecorationsParams): void {
    const document = vscode.workspace.textDocuments.find((d) => d.uri.toString() === msg.uri);
    if (!acceptDecorations(this.generation, msg, document?.version)) return;
    this.cache.set(msg.uri, msg);
    const editors = vscode.window.visibleTextEditors.filter((e) => e.document.uri.toString() === msg.uri);
    for (const editor of editors) this.apply(editor, msg);
    this.appliedEmitter.fire({ uri: msg.uri, source: 'message' });
  }

  /** Reapplies cached decorations to editors that became visible. */
  onVisibleEditors(editors: readonly vscode.TextEditor[]): void {
    for (const editor of editors) {
      const uri = editor.document.uri.toString();
      const entry = this.cache.get(uri);
      if (!entry || !reusable(this.generation, entry, editor.document.version)) continue;
      this.apply(editor, entry);
      this.appliedEmitter.fire({ uri, source: 'cache' });
    }
  }

  onDocumentClosed(document: vscode.TextDocument): void {
    const uri = document.uri.toString();
    this.cache.delete(uri);
    this.applied.delete(uri);
  }

  /** A new server instance: forget every type, generation and cached entry. */
  reset(): void {
    this.disposeTypes();
    this.generation = undefined;
    this.cache.clear();
    this.applied.clear();
  }

  private apply(editor: vscode.TextEditor, msg: DecorationsParams): void {
    const previous = this.applied.get(msg.uri) ?? new Set<string>();
    for (const key of keysToSet(previous, msg.ranges)) {
      const type = this.types.get(key);
      if (type) editor.setDecorations(type, (msg.ranges[key] ?? []).map(toRange));
    }
    this.applied.set(msg.uri, new Set(Object.keys(msg.ranges)));
  }

  private disposeTypes(): void {
    for (const type of this.types.values()) type.dispose();
    this.types.clear();
    this.renderOptions.clear();
  }

  dispose(): void {
    this.disposeTypes();
    this.appliedEmitter.dispose();
  }
}
```

Replace `extension/src/icons/resolver.ts` with:

```ts
// Maps the server's icon descriptors onto VS Code icons (spec 5.13, 7.7):
// codicons become theme icons; octicons, the todo icons and the check-circle
// become SVG files; the default is the bundled icon.

import * as vscode from 'vscode';
import type { IconDescriptor } from '../protocol';
import type { IconFiles } from './files';
import { isSvgIcon } from './svg';

export type TreeIcon = vscode.ThemeIcon | vscode.Uri;

export class IconResolver {
  constructor(
    private readonly files: IconFiles,
    private readonly defaultIcon: vscode.Uri,
  ) {}

  /** An icon for a tree item, or undefined for none. */
  treeIcon(icon: IconDescriptor | null): TreeIcon | undefined {
    if (!icon) return undefined;
    switch (icon.kind) {
      case 'codicon':
        return new vscode.ThemeIcon(icon.name, icon.colour ? new vscode.ThemeColor(icon.colour) : undefined);
      case 'folder':
        return vscode.ThemeIcon.Folder;
      case 'file':
        return vscode.ThemeIcon.File;
      default:
        return this.gutterIcon(icon);
    }
  }

  /** A gutter icon file: SVG icons only, as in todo-tree; codicons have none. */
  gutterIcon(icon: IconDescriptor): vscode.Uri | undefined {
    if (icon.kind === 'default') return this.defaultIcon;
    return isSvgIcon(icon) ? vscode.Uri.file(this.files.path(icon)) : undefined;
  }
}
```

Replace `extension/src/testApi.ts` with:

```ts
// What `activate` returns. The `test` hooks let the integration tests
// observe state VS Code offers no API to read (spec 12.5).

import type * as vscode from 'vscode';
import type { ApplySource } from './decorations/manager';
import type { DecorationsParams, StatusParams } from './protocol';
import type { PersistedViewState } from './state/viewState';
import type { StatusBarView } from './status/presentation';
import type { TestItem } from './tree/testItems';
import type { Prompts } from './ui/prompts';

export interface ServerHooks {
  readonly running: boolean;
  readonly pid: number | undefined;
  readonly onStatus: vscode.Event<StatusParams>;
  readonly onRunning: vscode.Event<void>;
  /** Fires when repeated crashes stop the automatic restarts (spec 7.4). */
  readonly onGaveUp: vscode.Event<string>;
  /** Shortens the start timeout (`START_TIMEOUT_MS`) so tests need not wait for it. */
  setStartTimeout(ms: number): void;
  status(): StatusParams | undefined;
}

export interface TreeHooks {
  readonly view: vscode.TreeView<string>;
  /** Fires whenever the provider asks VS Code to refetch. */
  readonly onDidChange: vscode.Event<unknown>;
  /** The provider's children of `parent` (top level when omitted), as tree items. */
  items(parent?: string): Promise<TestItem[]>;
  /** The current tree item epoch (spec 7.5). */
  readonly epoch: number;
  /** The line the last todo click flashed. */
  lastFlash(): { uri: string; line: number } | undefined;
}

export interface DecorationHooks {
  /** Fires after decorations are applied to a document's visible editors. */
  readonly onApplied: vscode.Event<{ uri: string; source: ApplySource }>;
  /** The decorations last applied to a document. */
  entry(uri: string): DecorationsParams | undefined;
  readonly generation: number | undefined;
  readonly styleKeys: string[];
  /** The options a key's decoration type was created with. */
  options(key: string): vscode.DecorationRenderOptions | undefined;
}

export interface TestHooks {
  /** Answers prompts and records messages. */
  readonly prompts: Prompts;
  viewState(): PersistedViewState;
  /** The context keys last set (spec 7.6). */
  contextKeys(): Readonly<Record<string, boolean | string>>;
  /** What the status bar item shows. */
  statusBar(): StatusBarView | undefined;
  readonly decorations: DecorationHooks;
  readonly server: ServerHooks;
  readonly tree: TreeHooks;
}

export interface ClippingsApi {
  readonly version: string;
  readonly test: TestHooks;
}
```

Replace `extension/src/extension.ts` with:

```ts
// Composition root: builds each part of the extension and wires them to the
// server connection.

import * as vscode from 'vscode';
import { registerExpandCommands, resetExpansion } from './commands/expand';
import { registerFilterCommands } from './commands/filters';
import { registerGoToCommands } from './commands/goTo';
import { registerNavigationCommands } from './commands/navigation';
import { registerScanCommands } from './commands/scan';
import { needsRestart, registerServerCommands } from './commands/server';
import { registerSettingCommands } from './commands/settings';
import { registerStatusBarCommand } from './commands/statusBar';
import { registerViewCommands } from './commands/view';
import { affectsServer, readConfiguration } from './config/read';
import { ConfigurationSync, replacesTree } from './config/sync';
import { ContextKeys } from './context/apply';
import { contextValues } from './context/keys';
import { SettingWriter } from './config/writes';
import { DecorationManager } from './decorations/manager';
import { IconFiles } from './icons/files';
import { IconResolver } from './icons/resolver';
import { invalidIcons } from './icons/svg';
import type { StatusParams } from './protocol';
import { ServerConnection } from './server/connection';
import { ViewStateStore } from './state/viewState';
import { StatusController } from './status/controller';
import { OnceNotice } from './status/presentation';
import type { ClippingsApi } from './testApi';
import { Expansion } from './tree/expansion';
import { NodeCache } from './tree/nodeCache';
import { LineFlash } from './tree/open';
import { TreeProvider } from './tree/provider';
import { Revealer } from './tree/reveal';
import { Prompts } from './ui/prompts';
import { testItem } from './tree/testItems';

let connection: ServerConnection | undefined;

export function activate(context: vscode.ExtensionContext): ClippingsApi {
  const log = vscode.window.createOutputChannel('Clippings', { log: true });
  const store = new ViewStateStore(context.workspaceState);
  const prompts = new Prompts();
  const activeUri = () => vscode.window.activeTextEditor?.document.uri.toString() ?? null;
  const server = new ServerConnection({
    context,
    log,
    settings: () => sync.current,
    activeUri,
    error: (message, ...actions) => prompts.message('error', message, ...actions),
  });
  connection = server;
  const sync = new ConfigurationSync(
    () => readConfiguration(store.snapshot()),
    (settings) => server.configure(settings),
  );
  let lastStatus: StatusParams | undefined;
  const contextKeys = new ContextKeys();
  const updateContext = () => void contextKeys.apply(contextValues(sync.current, lastStatus));
  sync.onPush(updateContext);
  updateContext();

  const icons = new IconResolver(
    new IconFiles(vscode.Uri.joinPath(context.globalStorageUri, 'icons').fsPath),
    vscode.Uri.joinPath(context.extensionUri, 'resources', 'todo-default.svg'),
  );
  const decorations = new DecorationManager(icons);
  const cache = new NodeCache();
  const expansion = new Expansion(store);
  const provider = new TreeProvider(server, cache, {
    icons,
    itemId: (id) => expansion.itemId(id),
    expanded: (node) => expansion.expanded(node),
  });
  const treeView = vscode.window.createTreeView('clippings-view', { treeDataProvider: provider });
  const revealer = new Revealer({
    find: (uri, line) => server.find(uri, line),
    cache,
    view: treeView,
    settings: () => sync.current,
  });
  const flash = new LineFlash();
  const writer = new SettingWriter(prompts);
  const statusController = new StatusController(treeView, prompts);
  const iconWarnings = new OnceNotice();
  const checkIcons = () => {
    const bad = invalidIcons(sync.current);
    const notice = iconWarnings.next(bad.length > 0 ? [`Invalid icons: ${bad.join(', ')}`] : []);
    if (notice) void prompts.message('warning', notice);
  };
  sync.onPush(checkIcons);
  checkIcons();

  context.subscriptions.push(
    log,
    server,
    provider,
    treeView,
    ...registerServerCommands(server, log),
    revealer,
    flash,
    ...registerExpandCommands({ store, sync, expansion, provider }),
    ...registerNavigationCommands(revealer, flash),
    ...registerViewCommands({ store, sync, expansion, provider, prompts }),
    ...registerFilterCommands({ store, sync, cache, prompts }),
    ...registerSettingCommands(writer, prompts),
    ...registerGoToCommands(server),
    statusController,
    registerStatusBarCommand({ sync, writer, prompts, view: treeView }),
    ...registerScanCommands(server),
    server.onStatus((s) => {
      lastStatus = s;
      updateContext();
      statusController.update(s);
    }),
    decorations,
    server.onNewInstance(() => {
      provider.reset();
      decorations.reset();
    }),
    server.onStyles((p) => decorations.onStyles(p)),
    server.onDecorations((p) => decorations.onDecorations(p)),
    vscode.window.onDidChangeVisibleTextEditors((editors) => decorations.onVisibleEditors(editors)),
    vscode.workspace.onDidCloseTextDocument((d) => decorations.onDocumentClosed(d)),
    server.onTreeChanged((p) => {
      if (p.refresh.includes(null)) expansion.onRootRefresh();
      provider.refresh(p.refresh);
    }),
    treeView.onDidExpandElement((e) => expansion.set(e.element, true)),
    treeView.onDidCollapseElement((e) => expansion.set(e.element, false)),
    vscode.workspace.onDidChangeConfiguration((e) => {
      if (needsRestart(e)) {
        sync.push();
        void server.restart();
        return;
      }
      if (!affectsServer(e)) return;
      const push = sync.push();
      if (e.affectsConfiguration('clippings.tree.expanded')) {
        resetExpansion({ expansion, provider }, replacesTree(push));
      }
    }),
    vscode.window.onDidChangeActiveTextEditor((editor) => {
      server.activeEditor(activeUri());
      revealer.onActiveEditor(editor);
    }),
  );
  void server.start();

  const manifest = context.extension.packageJSON as { version: string };
  return {
    version: manifest.version,
    test: {
      prompts,
      viewState: () => store.snapshot(),
      contextKeys: () => contextKeys.values,
      statusBar: () => statusController.shown,
      decorations: {
        onApplied: decorations.onApplied,
        entry: (uri) => decorations.entry(uri),
        get generation() {
          return decorations.currentGeneration;
        },
        get styleKeys() {
          return decorations.styleKeys;
        },
        options: (key) => decorations.options(key),
      },
      server: {
        get running() {
          return server.running;
        },
        get pid() {
          return server.pid;
        },
        onStatus: server.onStatus,
        onRunning: server.onRunning,
        onGaveUp: server.onGaveUp,
        setStartTimeout: (ms) => (server.startTimeoutMs = ms),
        status: () => lastStatus,
      },
      tree: {
        view: treeView,
        onDidChange: provider.onDidChangeTreeData,
        items: async (parent) => {
          const ids = await provider.getChildren(parent);
          return ids.map((id) => testItem(id, provider.getTreeItem(id)));
        },
        get epoch() {
          return expansion.currentEpoch;
        },
        lastFlash: () => flash.last,
      },
    },
  };
}

export async function deactivate(): Promise<void> {
  await connection?.stop();
  connection = undefined;
}
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `pnpm -C extension dev && pnpm -C extension typecheck && pnpm -C extension build`
Expected: no errors.

Run: `pnpm -C extension test:unit`
Expected: `75 passing`.

Run: `pnpm -C extension test:integration`
Expected: `61 passing`. A VS Code test window opens, runs the suite and closes.

- [ ] **Step 5: Commit**

```bash
git add -A
git commit -F - <<'EOF'
feat(extension): render octicon, todo and check icons to cached SVG files

Co-Authored-By: <your model attribution>
EOF
```

### Task 16: Export as a read-only virtual document

Export Tree (spec 7.6) asks the server's `clippings/export` and opens the content as a read-only document under the `clippings-export` scheme, named by the formatted export path. The server decides the format from the path, text tree or JSON (spec 5.15).

**Files:**
- Create: `extension/src/export/documents.ts`
- Modify: `extension/src/extension.ts`
- Test: `extension/src/test/integration/export.test.ts`

**Interfaces:**
- Consumes: `server/connection.ts` (Task 6): `ServerConnection`.
- Produces, in `export/documents.ts`: `EXPORT_SCHEME` (const); `function exportUri(path: string): vscode.Uri`; `class ExportDocuments implements vscode.TextDocumentContentProvider, vscode.Disposable` with `readonly onDidChange: vscode.Event<vscode.Uri>`, `provideTextDocumentContent(uri: vscode.Uri): string`, `set(uri: vscode.Uri, content: string): void`, `dispose(): void`; `function registerExport(server: ServerConnection): vscode.Disposable[]`.

- [ ] **Step 1: Write the failing tests**

Create `extension/src/test/integration/export.test.ts`:

```ts
import * as assert from 'node:assert/strict';
import { homedir } from 'node:os';
import * as vscode from 'vscode';
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
    assert.ok(path.startsWith(homedir().replaceAll('\\', '/')) || path.startsWith('/' + homedir()), path);
    assert.match(path, /\/todo-tree-\d{8}-\d{4}\.txt$/);
    const text = document.getText();
    assert.match(text, /^└─ workspace\n/);
    assert.ok(text.includes('line 2: TODO (alice) wire up the router'), text);
    assert.ok(!text.includes('Scan mode'), 'status nodes are not exported');
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

- [ ] **Step 2: Run the tests to verify they fail**

Run: `pnpm -C extension dev && pnpm -C extension typecheck && pnpm -C extension build && pnpm -C extension test:integration`
Expected: FAIL. `61 passing`, `2 failing`, each with `Error: command 'clippings.exportTree' not found`. A VS Code test window opens, runs the suite and closes.

- [ ] **Step 3: Write the implementation**

Create `extension/src/export/documents.ts`:

```ts
// Export Tree (spec 5.15, 7.6): the server's export shown as a read-only
// virtual document under the `clippings-export` scheme, named by the
// formatted export path.

import * as vscode from 'vscode';
import type { ServerConnection } from '../server/connection';

export const EXPORT_SCHEME = 'clippings-export';

/** The virtual document URI for an export path. */
export function exportUri(path: string): vscode.Uri {
  const slashed = path.replaceAll('\\', '/');
  return vscode.Uri.from({ scheme: EXPORT_SCHEME, path: slashed.startsWith('/') ? slashed : `/${slashed}` });
}

export class ExportDocuments implements vscode.TextDocumentContentProvider, vscode.Disposable {
  private readonly contents = new Map<string, string>();
  private readonly changed = new vscode.EventEmitter<vscode.Uri>();
  readonly onDidChange = this.changed.event;

  provideTextDocumentContent(uri: vscode.Uri): string {
    return this.contents.get(uri.toString()) ?? '';
  }

  set(uri: vscode.Uri, content: string): void {
    this.contents.set(uri.toString(), content);
    this.changed.fire(uri);
  }

  dispose(): void {
    this.changed.dispose();
  }
}

export function registerExport(server: ServerConnection): vscode.Disposable[] {
  const documents = new ExportDocuments();
  return [
    documents,
    vscode.workspace.registerTextDocumentContentProvider(EXPORT_SCHEME, documents),
    vscode.commands.registerCommand('clippings.exportTree', async () => {
      const result = await server.export();
      if (!result) return;
      const uri = exportUri(result.path);
      documents.set(uri, result.content);
      const document = await vscode.workspace.openTextDocument(uri);
      await vscode.window.showTextDocument(document, { preview: true });
    }),
  ];
}
```

Replace `extension/src/extension.ts` with:

```ts
// Composition root: builds each part of the extension and wires them to the
// server connection.

import * as vscode from 'vscode';
import { registerExpandCommands, resetExpansion } from './commands/expand';
import { registerFilterCommands } from './commands/filters';
import { registerGoToCommands } from './commands/goTo';
import { registerNavigationCommands } from './commands/navigation';
import { registerScanCommands } from './commands/scan';
import { needsRestart, registerServerCommands } from './commands/server';
import { registerSettingCommands } from './commands/settings';
import { registerStatusBarCommand } from './commands/statusBar';
import { registerViewCommands } from './commands/view';
import { affectsServer, readConfiguration } from './config/read';
import { ConfigurationSync, replacesTree } from './config/sync';
import { ContextKeys } from './context/apply';
import { contextValues } from './context/keys';
import { SettingWriter } from './config/writes';
import { DecorationManager } from './decorations/manager';
import { registerExport } from './export/documents';
import { IconFiles } from './icons/files';
import { IconResolver } from './icons/resolver';
import { invalidIcons } from './icons/svg';
import type { StatusParams } from './protocol';
import { ServerConnection } from './server/connection';
import { ViewStateStore } from './state/viewState';
import { StatusController } from './status/controller';
import { OnceNotice } from './status/presentation';
import type { ClippingsApi } from './testApi';
import { Expansion } from './tree/expansion';
import { NodeCache } from './tree/nodeCache';
import { LineFlash } from './tree/open';
import { TreeProvider } from './tree/provider';
import { Revealer } from './tree/reveal';
import { Prompts } from './ui/prompts';
import { testItem } from './tree/testItems';

let connection: ServerConnection | undefined;

export function activate(context: vscode.ExtensionContext): ClippingsApi {
  const log = vscode.window.createOutputChannel('Clippings', { log: true });
  const store = new ViewStateStore(context.workspaceState);
  const prompts = new Prompts();
  const activeUri = () => vscode.window.activeTextEditor?.document.uri.toString() ?? null;
  const server = new ServerConnection({
    context,
    log,
    settings: () => sync.current,
    activeUri,
    error: (message, ...actions) => prompts.message('error', message, ...actions),
  });
  connection = server;
  const sync = new ConfigurationSync(
    () => readConfiguration(store.snapshot()),
    (settings) => server.configure(settings),
  );
  let lastStatus: StatusParams | undefined;
  const contextKeys = new ContextKeys();
  const updateContext = () => void contextKeys.apply(contextValues(sync.current, lastStatus));
  sync.onPush(updateContext);
  updateContext();

  const icons = new IconResolver(
    new IconFiles(vscode.Uri.joinPath(context.globalStorageUri, 'icons').fsPath),
    vscode.Uri.joinPath(context.extensionUri, 'resources', 'todo-default.svg'),
  );
  const decorations = new DecorationManager(icons);
  const cache = new NodeCache();
  const expansion = new Expansion(store);
  const provider = new TreeProvider(server, cache, {
    icons,
    itemId: (id) => expansion.itemId(id),
    expanded: (node) => expansion.expanded(node),
  });
  const treeView = vscode.window.createTreeView('clippings-view', { treeDataProvider: provider });
  const revealer = new Revealer({
    find: (uri, line) => server.find(uri, line),
    cache,
    view: treeView,
    settings: () => sync.current,
  });
  const flash = new LineFlash();
  const writer = new SettingWriter(prompts);
  const statusController = new StatusController(treeView, prompts);
  const iconWarnings = new OnceNotice();
  const checkIcons = () => {
    const bad = invalidIcons(sync.current);
    const notice = iconWarnings.next(bad.length > 0 ? [`Invalid icons: ${bad.join(', ')}`] : []);
    if (notice) void prompts.message('warning', notice);
  };
  sync.onPush(checkIcons);
  checkIcons();

  context.subscriptions.push(
    log,
    server,
    provider,
    treeView,
    ...registerServerCommands(server, log),
    revealer,
    flash,
    ...registerExpandCommands({ store, sync, expansion, provider }),
    ...registerNavigationCommands(revealer, flash),
    ...registerViewCommands({ store, sync, expansion, provider, prompts }),
    ...registerFilterCommands({ store, sync, cache, prompts }),
    ...registerSettingCommands(writer, prompts),
    ...registerGoToCommands(server),
    ...registerExport(server),
    statusController,
    registerStatusBarCommand({ sync, writer, prompts, view: treeView }),
    ...registerScanCommands(server),
    server.onStatus((s) => {
      lastStatus = s;
      updateContext();
      statusController.update(s);
    }),
    decorations,
    server.onNewInstance(() => {
      provider.reset();
      decorations.reset();
    }),
    server.onStyles((p) => decorations.onStyles(p)),
    server.onDecorations((p) => decorations.onDecorations(p)),
    vscode.window.onDidChangeVisibleTextEditors((editors) => decorations.onVisibleEditors(editors)),
    vscode.workspace.onDidCloseTextDocument((d) => decorations.onDocumentClosed(d)),
    server.onTreeChanged((p) => {
      if (p.refresh.includes(null)) expansion.onRootRefresh();
      provider.refresh(p.refresh);
    }),
    treeView.onDidExpandElement((e) => expansion.set(e.element, true)),
    treeView.onDidCollapseElement((e) => expansion.set(e.element, false)),
    vscode.workspace.onDidChangeConfiguration((e) => {
      if (needsRestart(e)) {
        sync.push();
        void server.restart();
        return;
      }
      if (!affectsServer(e)) return;
      const push = sync.push();
      if (e.affectsConfiguration('clippings.tree.expanded')) {
        resetExpansion({ expansion, provider }, replacesTree(push));
      }
    }),
    vscode.window.onDidChangeActiveTextEditor((editor) => {
      server.activeEditor(activeUri());
      revealer.onActiveEditor(editor);
    }),
  );
  void server.start();

  const manifest = context.extension.packageJSON as { version: string };
  return {
    version: manifest.version,
    test: {
      prompts,
      viewState: () => store.snapshot(),
      contextKeys: () => contextKeys.values,
      statusBar: () => statusController.shown,
      decorations: {
        onApplied: decorations.onApplied,
        entry: (uri) => decorations.entry(uri),
        get generation() {
          return decorations.currentGeneration;
        },
        get styleKeys() {
          return decorations.styleKeys;
        },
        options: (key) => decorations.options(key),
      },
      server: {
        get running() {
          return server.running;
        },
        get pid() {
          return server.pid;
        },
        onStatus: server.onStatus,
        onRunning: server.onRunning,
        onGaveUp: server.onGaveUp,
        setStartTimeout: (ms) => (server.startTimeoutMs = ms),
        status: () => lastStatus,
      },
      tree: {
        view: treeView,
        onDidChange: provider.onDidChangeTreeData,
        items: async (parent) => {
          const ids = await provider.getChildren(parent);
          return ids.map((id) => testItem(id, provider.getTreeItem(id)));
        },
        get epoch() {
          return expansion.currentEpoch;
        },
        lastFlash: () => flash.last,
      },
    },
  };
}

export async function deactivate(): Promise<void> {
  await connection?.stop();
  connection = undefined;
}
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `pnpm -C extension dev && pnpm -C extension typecheck && pnpm -C extension build`
Expected: no errors.

Run: `pnpm -C extension test:unit`
Expected: `75 passing`.

Run: `pnpm -C extension test:integration`
Expected: `63 passing`. A VS Code test window opens, runs the suite and closes.

- [ ] **Step 5: Commit**

```bash
git add -A
git commit -F - <<'EOF'
feat(extension): export the tree as a read-only virtual document

Co-Authored-By: <your model attribution>
EOF
```

### Task 17: todo-tree settings import

The import of spec 7.3. On activation, when a `todo-tree.*` key has a global or workspace value, no `clippings.*` key has one and the user has not chosen Never, a notification offers Import, Not Now and Never (ruling 16). The import copies the global and workspace values of the 61 carried keys to the same scopes, maps `general.debug: true` to `server.logLevel: debug`, and logs what it skips. `clippings.importTodoTreeSettings` runs it on demand. The key lists and the import plan are pure.

**Files:**
- Create: `extension/src/importer/keys.ts`, `extension/src/importer/plan.ts`, `extension/src/importer/run.ts`
- Modify: `extension/src/testApi.ts`, `extension/src/extension.ts`
- Test: `extension/src/test/integration/importer.test.ts`, `extension/src/test/unit/importer.test.ts`

**Interfaces:**
- Consumes: `ui/prompts.ts` (Task 10): `Prompts`.
- Produces, in `importer/keys.ts`: `CARRIED_KEYS` (const); `NEW_KEYS` (const); `DROPPED_KEYS` (const); `TODO_TREE_KEYS` (const); `CLIPPINGS_KEYS` (const); `function mapSetting(key: string, value: unknown): { key: string; value: unknown } | { skip: string }`.
- Produces, in `importer/plan.ts`: `Inspected` (interface); `Scope` (type); `Write` (interface); `ImportPlan` (interface); `function shouldOffer(inspect: (section: 'todo-tree' | 'clippings', key: string) => Inspected | undefined, decision: unknown): boolean`; `function importPlan(inspect: (key: string) => Inspected | undefined): ImportPlan`.
- Produces, in `importer/run.ts`: `IMPORT_OFFER_KEY` (const); `class TodoTreeImporter` with `constructor(private readonly context: vscode.ExtensionContext, private readonly prompts: Prompts, private readonly log: vscode.LogOutputChannel)`, `async offer(): Promise<void>`, `async run(): Promise<number>`.
- Produces, in `testApi.ts`: `TestHooks` gains `offerImport(): Promise<void>`.

- [ ] **Step 1: Write the failing tests**

Create `extension/src/test/integration/importer.test.ts`:

```ts
import * as assert from 'node:assert/strict';
import { mkdirSync, rmSync, writeFileSync } from 'node:fs';
import * as vscode from 'vscode';
import type { ClippingsApi } from '../../testApi';
import { DEFAULT_TREE } from './fixture';
import { getApi, nextEvent, treeBecomes, whenIdle, workspacePath } from './helpers';

/** Replaces the workspace settings file and waits for VS Code to read it. */
async function writeWorkspaceSettings(settings: Record<string, unknown> | undefined, section: string): Promise<void> {
  const changed = nextEvent('a settings change', vscode.workspace.onDidChangeConfiguration, (e) =>
    e.affectsConfiguration(section),
  );
  const path = workspacePath('.vscode', 'settings.json');
  if (settings) {
    mkdirSync(workspacePath('.vscode'), { recursive: true });
    writeFileSync(path, JSON.stringify(settings, null, 2));
  } else {
    rmSync(path, { force: true });
  }
  await changed;
}

const TODO_TREE_SETTINGS = {
  'todo-tree.general.tags': ['BUG', 'TODO'],
  'todo-tree.general.debug': true,
  'todo-tree.ripgrep.ripgrepArgs': '--max-columns=1000',
  'todo-tree.tree.buttons.export': true,
};

function offers(api: ClippingsApi): number {
  return api.test.prompts.shown.filter((s) => s.kind === 'info' && s.message.includes('found Todo Tree settings')).length;
}

describe('todo-tree settings import', () => {
  let api: ClippingsApi;

  before(async () => {
    api = await getApi();
    await whenIdle(api);
    await writeWorkspaceSettings(TODO_TREE_SETTINGS, 'todo-tree');
  });

  after(async () => {
    await writeWorkspaceSettings(undefined, 'clippings');
    await whenIdle(api);
    await treeBecomes(api, DEFAULT_TREE);
  });

  it('offers the import until the user chooses Never', async () => {
    const before = offers(api);
    api.test.prompts.script('Not Now');
    await api.test.offerImport();
    assert.equal(offers(api), before + 1);
    assert.equal(vscode.workspace.getConfiguration('clippings').inspect('general.tags')?.workspaceValue, undefined);
    api.test.prompts.script('Never');
    await api.test.offerImport();
    assert.equal(offers(api), before + 2);
    await api.test.offerImport();
    assert.equal(offers(api), before + 2, 'no offer after Never');
  });

  it('imports carried values to the same scope and maps debug to a log level', async () => {
    await vscode.commands.executeCommand('clippings.importTodoTreeSettings');
    const clippings = vscode.workspace.getConfiguration('clippings');
    assert.deepEqual(clippings.inspect('general.tags')?.workspaceValue, ['BUG', 'TODO']);
    assert.equal(clippings.inspect('tree.buttons.export')?.workspaceValue, true);
    assert.equal(clippings.inspect('server.logLevel')?.workspaceValue, 'debug');
    const done = api.test.prompts.shown.at(-1);
    assert.deepEqual(done, { kind: 'info', message: 'Clippings: imported 3 settings from Todo Tree.', actions: [] });
    assert.equal(api.test.contextKeys()['clippings-show-export-button'], true);
  });
});
```

Create `extension/src/test/unit/importer.test.ts`:

```ts
import * as assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';
import { CARRIED_KEYS, CLIPPINGS_KEYS, DROPPED_KEYS, mapSetting, NEW_KEYS } from '../../importer/keys';
import { importPlan, shouldOffer, type Inspected } from '../../importer/plan';

const manifest = JSON.parse(readFileSync(resolve(__dirname, '../../../package.json'), 'utf8')) as {
  contributes: { configuration: { properties: Record<string, unknown> }[] };
};

describe('importer keys', () => {
  it('cover exactly the manifest settings: 61 carried and 4 new', () => {
    const declared = manifest.contributes.configuration.flatMap((g) => Object.keys(g.properties)).sort();
    assert.equal(CARRIED_KEYS.length, 61);
    assert.equal(NEW_KEYS.length, 4);
    assert.deepEqual(CLIPPINGS_KEYS.map((k) => `clippings.${k}`).sort(), declared);
    assert.equal(DROPPED_KEYS.length, 9);
  });

  it('maps carried keys unchanged, debug to a log level and drops the rest', () => {
    assert.deepEqual(mapSetting('general.tags', ['A']), { key: 'general.tags', value: ['A'] });
    assert.deepEqual(mapSetting('general.debug', true), { key: 'server.logLevel', value: 'debug' });
    assert.ok('skip' in mapSetting('general.debug', false));
    assert.deepEqual(mapSetting('ripgrep.ripgrepArgs', '--x'), {
      skip: 'todo-tree.ripgrep.ripgrepArgs has no Clippings equivalent',
    });
  });
});

describe('import offer', () => {
  const values = (map: Record<string, Inspected>) => (section: string, key: string) => map[`${section}.${key}`];

  it('needs a todo-tree value, no clippings value and no Never', () => {
    const todoTree = { 'todo-tree.general.tags': { globalValue: ['A'] } };
    assert.equal(shouldOffer(values(todoTree), undefined), true);
    assert.equal(shouldOffer(values(todoTree), 'never'), false);
    assert.equal(shouldOffer(values({}), undefined), false);
    assert.equal(
      shouldOffer(values({ ...todoTree, 'clippings.server.logLevel': { workspaceValue: 'info' } }), undefined),
      false,
    );
    assert.equal(
      shouldOffer(values({ 'todo-tree.general.tags': { workspaceFolderValue: ['A'] } }), undefined),
      false,
      'folder values do not count',
    );
  });
});

describe('import plan', () => {
  it('copies values to the same scope and skips what cannot move', () => {
    const plan = importPlan(
      (key) =>
        ({
          'general.tags': { globalValue: ['A'], workspaceValue: ['B'], workspaceFolderValue: ['C'] },
          'general.debug': { workspaceValue: true },
          'ripgrep.ripgrepArgs': { globalValue: '--foo' },
          'tree.buttons.export': { globalValue: true },
        })[key],
    );
    assert.deepEqual(plan.writes, [
      { key: 'general.tags', value: ['A'], scope: 'global' },
      { key: 'general.tags', value: ['B'], scope: 'workspace' },
      { key: 'tree.buttons.export', value: true, scope: 'global' },
      { key: 'server.logLevel', value: 'debug', scope: 'workspace' },
    ]);
    assert.deepEqual(plan.skipped, [
      'todo-tree.general.tags: skipped the workspace folder value, which todo-tree ignored',
      'todo-tree.ripgrep.ripgrepArgs has no Clippings equivalent; skipped its global value',
    ]);
  });
});
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `pnpm -C extension typecheck`
Expected: FAIL. `error TS2339: Property 'offerImport' does not exist on type 'TestHooks'`, and `error TS2307: Cannot find module` for `../../importer/keys` and `../../importer/plan`.

- [ ] **Step 3: Write the implementation**

Create `extension/src/importer/keys.ts`:

```ts
// The todo-tree settings the importer knows (spec 7.2, 7.3). Pure.

/** Settings carried unchanged in name, type and default: 61 keys. */
export const CARRIED_KEYS = [
  'general.automaticGitRefreshInterval',
  'general.periodicRefreshInterval',
  'general.revealBehaviour',
  'general.exportPath',
  'general.rootFolder',
  'general.schemes',
  'general.statusBar',
  'general.showIconsInsteadOfTagsInStatusBar',
  'general.statusBarClickBehaviour',
  'general.tagGroups',
  'general.tags',
  'general.showActivityBarBadge',
  'highlights.customHighlight',
  'highlights.defaultHighlight',
  'highlights.enabled',
  'highlights.highlightDelay',
  'highlights.useColourScheme',
  'highlights.foregroundColourScheme',
  'highlights.backgroundColourScheme',
  'filtering.excludedWorkspaces',
  'filtering.excludeGlobs',
  'filtering.ignoreGitSubmodules',
  'filtering.includedWorkspaces',
  'filtering.includeGlobs',
  'filtering.includeHiddenFiles',
  'filtering.scopes',
  'filtering.useBuiltInExcludes',
  'tree.autoRefresh',
  'tree.disableCompactFolders',
  'tree.expanded',
  'tree.filterCaseSensitive',
  'tree.flat',
  'tree.groupedByTag',
  'tree.groupedBySubTag',
  'tree.hideIconsWhenGroupedByTag',
  'tree.hideTreeWhenEmpty',
  'tree.labelFormat',
  'tree.scanAtStartup',
  'tree.scanMode',
  'tree.showBadges',
  'tree.showCountsInTree',
  'tree.showCurrentScanMode',
  'tree.subTagClickUrl',
  'tree.sortTagsOnlyViewAlphabetically',
  'tree.sort',
  'tree.tagsOnly',
  'tree.tooltipFormat',
  'tree.trackFile',
  'tree.buttons.reveal',
  'tree.buttons.scanMode',
  'tree.buttons.viewStyle',
  'tree.buttons.groupByTag',
  'tree.buttons.groupBySubTag',
  'tree.buttons.filter',
  'tree.buttons.refresh',
  'tree.buttons.expand',
  'tree.buttons.export',
  'regex.regex',
  'regex.regexCaseSensitive',
  'regex.subTagRegex',
  'regex.enableMultiLine',
] as const;

/** Clippings' own settings, which todo-tree never had. */
export const NEW_KEYS = ['filtering.builtInExcludes', 'server.path', 'server.logLevel', 'trace.server'] as const;

/** todo-tree settings with no Clippings equivalent; `general.debug` maps to a log level. */
export const DROPPED_KEYS = [
  'ripgrep.ripgrep',
  'ripgrep.ripgrepArgs',
  'ripgrep.ripgrepMaxBuffer',
  'ripgrep.usePatternFile',
  'filtering.passGlobsToRipgrep',
  'tree.showInExplorer',
  'tree.showScanOpenFilesOrWorkspaceButton',
  'tree.showTagsFromOpenFilesOnly',
  'general.debug',
] as const;

export const TODO_TREE_KEYS: readonly string[] = [...CARRIED_KEYS, ...DROPPED_KEYS];
export const CLIPPINGS_KEYS: readonly string[] = [...CARRIED_KEYS, ...NEW_KEYS];

const carried = new Set<string>(CARRIED_KEYS);

/** The Clippings setting for a todo-tree key and value, or why there is none. */
export function mapSetting(key: string, value: unknown): { key: string; value: unknown } | { skip: string } {
  if (carried.has(key)) return { key, value };
  if (key === 'general.debug') {
    return value === true ? { key: 'server.logLevel', value: 'debug' } : { skip: 'general.debug is off' };
  }
  return { skip: `todo-tree.${key} has no Clippings equivalent` };
}
```

Create `extension/src/importer/plan.ts`:

```ts
// Whether to offer the todo-tree import and what it writes (spec 7.3). Pure.

import { CLIPPINGS_KEYS, mapSetting, TODO_TREE_KEYS } from './keys';

export interface Inspected {
  globalValue?: unknown;
  workspaceValue?: unknown;
  workspaceFolderValue?: unknown;
}

export type Scope = 'global' | 'workspace';

export interface Write {
  key: string;
  value: unknown;
  scope: Scope;
}

export interface ImportPlan {
  writes: Write[];
  /** Log lines for values the import leaves behind. */
  skipped: string[];
}

function explicit(i: Inspected | undefined): boolean {
  return i?.globalValue !== undefined || i?.workspaceValue !== undefined;
}

/**
 * Offer the import when some todo-tree key has a global or workspace value,
 * no Clippings key has one, and the user has not chosen Never.
 */
export function shouldOffer(
  inspect: (section: 'todo-tree' | 'clippings', key: string) => Inspected | undefined,
  decision: unknown,
): boolean {
  return (
    decision !== 'never' &&
    TODO_TREE_KEYS.some((k) => explicit(inspect('todo-tree', k))) &&
    !CLIPPINGS_KEYS.some((k) => explicit(inspect('clippings', k)))
  );
}

export function importPlan(inspect: (key: string) => Inspected | undefined): ImportPlan {
  const plan: ImportPlan = { writes: [], skipped: [] };
  for (const key of TODO_TREE_KEYS) {
    const i = inspect(key);
    if (!i) continue;
    if (i.workspaceFolderValue !== undefined) {
      plan.skipped.push(`todo-tree.${key}: skipped the workspace folder value, which todo-tree ignored`);
    }
    for (const [scope, value] of [
      ['global', i.globalValue],
      ['workspace', i.workspaceValue],
    ] as const) {
      if (value === undefined) continue;
      const mapped = mapSetting(key, value);
      if ('skip' in mapped) plan.skipped.push(`${mapped.skip}; skipped its ${scope} value`);
      else plan.writes.push({ ...mapped, scope });
    }
  }
  return plan;
}
```

Create `extension/src/importer/run.ts`:

```ts
// The todo-tree settings import (spec 7.3): the activation offer and the
// Import Settings from Todo Tree command.

import * as vscode from 'vscode';
import type { Prompts } from '../ui/prompts';
import { importPlan, shouldOffer, type Inspected } from './plan';

export const IMPORT_OFFER_KEY = 'importOffer';

function inspect(section: string, key: string): Inspected | undefined {
  return vscode.workspace.getConfiguration(section).inspect(key);
}

export class TodoTreeImporter {
  constructor(
    private readonly context: vscode.ExtensionContext,
    private readonly prompts: Prompts,
    private readonly log: vscode.LogOutputChannel,
  ) {}

  /** On activation: offers the import once the conditions of spec 7.3 hold. */
  async offer(): Promise<void> {
    if (!shouldOffer(inspect, this.context.globalState.get(IMPORT_OFFER_KEY))) return;
    const choice = await this.prompts.message(
      'info',
      'Clippings found Todo Tree settings. Import them into Clippings?',
      'Import',
      'Not Now',
      'Never',
    );
    if (choice === 'Import') await this.run();
    if (choice === 'Never') await this.context.globalState.update(IMPORT_OFFER_KEY, 'never');
  }

  /** Copies each carried todo-tree value to the same scope under `clippings.*`. */
  async run(): Promise<number> {
    const plan = importPlan((key) => inspect('todo-tree', key));
    for (const line of plan.skipped) this.log.info(`Import: ${line}`);
    let written = 0;
    for (const w of plan.writes) {
      const target = w.scope === 'global' ? vscode.ConfigurationTarget.Global : vscode.ConfigurationTarget.Workspace;
      try {
        await vscode.workspace.getConfiguration('clippings').update(w.key, w.value, target);
        written++;
      } catch (err) {
        this.log.warn(`Import: could not write clippings.${w.key}: ${String(err)}`);
      }
    }
    const noun = written === 1 ? 'setting' : 'settings';
    void this.prompts.message('info', `Clippings: imported ${written} ${noun} from Todo Tree.`);
    return written;
  }
}
```

Replace `extension/src/testApi.ts` with:

```ts
// What `activate` returns. The `test` hooks let the integration tests
// observe state VS Code offers no API to read (spec 12.5).

import type * as vscode from 'vscode';
import type { ApplySource } from './decorations/manager';
import type { DecorationsParams, StatusParams } from './protocol';
import type { PersistedViewState } from './state/viewState';
import type { StatusBarView } from './status/presentation';
import type { TestItem } from './tree/testItems';
import type { Prompts } from './ui/prompts';

export interface ServerHooks {
  readonly running: boolean;
  readonly pid: number | undefined;
  readonly onStatus: vscode.Event<StatusParams>;
  readonly onRunning: vscode.Event<void>;
  /** Fires when repeated crashes stop the automatic restarts (spec 7.4). */
  readonly onGaveUp: vscode.Event<string>;
  /** Shortens the start timeout (`START_TIMEOUT_MS`) so tests need not wait for it. */
  setStartTimeout(ms: number): void;
  status(): StatusParams | undefined;
}

export interface TreeHooks {
  readonly view: vscode.TreeView<string>;
  /** Fires whenever the provider asks VS Code to refetch. */
  readonly onDidChange: vscode.Event<unknown>;
  /** The provider's children of `parent` (top level when omitted), as tree items. */
  items(parent?: string): Promise<TestItem[]>;
  /** The current tree item epoch (spec 7.5). */
  readonly epoch: number;
  /** The line the last todo click flashed. */
  lastFlash(): { uri: string; line: number } | undefined;
}

export interface DecorationHooks {
  /** Fires after decorations are applied to a document's visible editors. */
  readonly onApplied: vscode.Event<{ uri: string; source: ApplySource }>;
  /** The decorations last applied to a document. */
  entry(uri: string): DecorationsParams | undefined;
  readonly generation: number | undefined;
  readonly styleKeys: string[];
  /** The options a key's decoration type was created with. */
  options(key: string): vscode.DecorationRenderOptions | undefined;
}

export interface TestHooks {
  /** Answers prompts and records messages. */
  readonly prompts: Prompts;
  /** Runs the activation-time import offer again. */
  offerImport(): Promise<void>;
  viewState(): PersistedViewState;
  /** The context keys last set (spec 7.6). */
  contextKeys(): Readonly<Record<string, boolean | string>>;
  /** What the status bar item shows. */
  statusBar(): StatusBarView | undefined;
  readonly decorations: DecorationHooks;
  readonly server: ServerHooks;
  readonly tree: TreeHooks;
}

export interface ClippingsApi {
  readonly version: string;
  readonly test: TestHooks;
}
```

Replace `extension/src/extension.ts` with:

```ts
// Composition root: builds each part of the extension and wires them to the
// server connection.

import * as vscode from 'vscode';
import { registerExpandCommands, resetExpansion } from './commands/expand';
import { registerFilterCommands } from './commands/filters';
import { registerGoToCommands } from './commands/goTo';
import { registerNavigationCommands } from './commands/navigation';
import { registerScanCommands } from './commands/scan';
import { needsRestart, registerServerCommands } from './commands/server';
import { registerSettingCommands } from './commands/settings';
import { registerStatusBarCommand } from './commands/statusBar';
import { registerViewCommands } from './commands/view';
import { affectsServer, readConfiguration } from './config/read';
import { ConfigurationSync, replacesTree } from './config/sync';
import { ContextKeys } from './context/apply';
import { contextValues } from './context/keys';
import { SettingWriter } from './config/writes';
import { DecorationManager } from './decorations/manager';
import { registerExport } from './export/documents';
import { IconFiles } from './icons/files';
import { IconResolver } from './icons/resolver';
import { invalidIcons } from './icons/svg';
import { TodoTreeImporter } from './importer/run';
import type { StatusParams } from './protocol';
import { ServerConnection } from './server/connection';
import { ViewStateStore } from './state/viewState';
import { StatusController } from './status/controller';
import { OnceNotice } from './status/presentation';
import type { ClippingsApi } from './testApi';
import { Expansion } from './tree/expansion';
import { NodeCache } from './tree/nodeCache';
import { LineFlash } from './tree/open';
import { TreeProvider } from './tree/provider';
import { Revealer } from './tree/reveal';
import { Prompts } from './ui/prompts';
import { testItem } from './tree/testItems';

let connection: ServerConnection | undefined;

export function activate(context: vscode.ExtensionContext): ClippingsApi {
  const log = vscode.window.createOutputChannel('Clippings', { log: true });
  const store = new ViewStateStore(context.workspaceState);
  const prompts = new Prompts();
  const activeUri = () => vscode.window.activeTextEditor?.document.uri.toString() ?? null;
  const server = new ServerConnection({
    context,
    log,
    settings: () => sync.current,
    activeUri,
    error: (message, ...actions) => prompts.message('error', message, ...actions),
  });
  connection = server;
  const sync = new ConfigurationSync(
    () => readConfiguration(store.snapshot()),
    (settings) => server.configure(settings),
  );
  let lastStatus: StatusParams | undefined;
  const contextKeys = new ContextKeys();
  const updateContext = () => void contextKeys.apply(contextValues(sync.current, lastStatus));
  sync.onPush(updateContext);
  updateContext();

  const icons = new IconResolver(
    new IconFiles(vscode.Uri.joinPath(context.globalStorageUri, 'icons').fsPath),
    vscode.Uri.joinPath(context.extensionUri, 'resources', 'todo-default.svg'),
  );
  const decorations = new DecorationManager(icons);
  const cache = new NodeCache();
  const expansion = new Expansion(store);
  const provider = new TreeProvider(server, cache, {
    icons,
    itemId: (id) => expansion.itemId(id),
    expanded: (node) => expansion.expanded(node),
  });
  const treeView = vscode.window.createTreeView('clippings-view', { treeDataProvider: provider });
  const revealer = new Revealer({
    find: (uri, line) => server.find(uri, line),
    cache,
    view: treeView,
    settings: () => sync.current,
  });
  const flash = new LineFlash();
  const writer = new SettingWriter(prompts);
  const statusController = new StatusController(treeView, prompts);
  const iconWarnings = new OnceNotice();
  const checkIcons = () => {
    const bad = invalidIcons(sync.current);
    const notice = iconWarnings.next(bad.length > 0 ? [`Invalid icons: ${bad.join(', ')}`] : []);
    if (notice) void prompts.message('warning', notice);
  };
  sync.onPush(checkIcons);
  checkIcons();

  context.subscriptions.push(
    log,
    server,
    provider,
    treeView,
    ...registerServerCommands(server, log),
    revealer,
    flash,
    ...registerExpandCommands({ store, sync, expansion, provider }),
    ...registerNavigationCommands(revealer, flash),
    ...registerViewCommands({ store, sync, expansion, provider, prompts }),
    ...registerFilterCommands({ store, sync, cache, prompts }),
    ...registerSettingCommands(writer, prompts),
    ...registerGoToCommands(server),
    ...registerExport(server),
    statusController,
    registerStatusBarCommand({ sync, writer, prompts, view: treeView }),
    ...registerScanCommands(server),
    server.onStatus((s) => {
      lastStatus = s;
      updateContext();
      statusController.update(s);
    }),
    decorations,
    server.onNewInstance(() => {
      provider.reset();
      decorations.reset();
    }),
    server.onStyles((p) => decorations.onStyles(p)),
    server.onDecorations((p) => decorations.onDecorations(p)),
    vscode.window.onDidChangeVisibleTextEditors((editors) => decorations.onVisibleEditors(editors)),
    vscode.workspace.onDidCloseTextDocument((d) => decorations.onDocumentClosed(d)),
    server.onTreeChanged((p) => {
      if (p.refresh.includes(null)) expansion.onRootRefresh();
      provider.refresh(p.refresh);
    }),
    treeView.onDidExpandElement((e) => expansion.set(e.element, true)),
    treeView.onDidCollapseElement((e) => expansion.set(e.element, false)),
    vscode.workspace.onDidChangeConfiguration((e) => {
      if (needsRestart(e)) {
        sync.push();
        void server.restart();
        return;
      }
      if (!affectsServer(e)) return;
      const push = sync.push();
      if (e.affectsConfiguration('clippings.tree.expanded')) {
        resetExpansion({ expansion, provider }, replacesTree(push));
      }
    }),
    vscode.window.onDidChangeActiveTextEditor((editor) => {
      server.activeEditor(activeUri());
      revealer.onActiveEditor(editor);
    }),
  );
  const importer = new TodoTreeImporter(context, prompts, log);
  context.subscriptions.push(
    vscode.commands.registerCommand('clippings.importTodoTreeSettings', () => importer.run()),
  );
  void server.start();
  void importer.offer();

  const manifest = context.extension.packageJSON as { version: string };
  return {
    version: manifest.version,
    test: {
      prompts,
      offerImport: () => importer.offer(),
      viewState: () => store.snapshot(),
      contextKeys: () => contextKeys.values,
      statusBar: () => statusController.shown,
      decorations: {
        onApplied: decorations.onApplied,
        entry: (uri) => decorations.entry(uri),
        get generation() {
          return decorations.currentGeneration;
        },
        get styleKeys() {
          return decorations.styleKeys;
        },
        options: (key) => decorations.options(key),
      },
      server: {
        get running() {
          return server.running;
        },
        get pid() {
          return server.pid;
        },
        onStatus: server.onStatus,
        onRunning: server.onRunning,
        onGaveUp: server.onGaveUp,
        setStartTimeout: (ms) => (server.startTimeoutMs = ms),
        status: () => lastStatus,
      },
      tree: {
        view: treeView,
        onDidChange: provider.onDidChangeTreeData,
        items: async (parent) => {
          const ids = await provider.getChildren(parent);
          return ids.map((id) => testItem(id, provider.getTreeItem(id)));
        },
        get epoch() {
          return expansion.currentEpoch;
        },
        lastFlash: () => flash.last,
      },
    },
  };
}

export async function deactivate(): Promise<void> {
  await connection?.stop();
  connection = undefined;
}
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `pnpm -C extension dev && pnpm -C extension typecheck && pnpm -C extension build`
Expected: no errors.

Run: `pnpm -C extension test:unit`
Expected: `79 passing`.

Run: `pnpm -C extension test:integration`
Expected: `65 passing`. A VS Code test window opens, runs the suite and closes.

- [ ] **Step 5: Commit**

```bash
git add -A
git commit -F - <<'EOF'
feat(extension): offer and run the todo-tree settings import

Co-Authored-By: <your model attribution>
EOF
```

### Task 18: Every command and the host-time budget

Two suites that check the whole extension. Every one of the 37 declared and three registration-only commands is registered and runs once without error (spec 7.6 and 12.5). A fixture file with 1,000 todos is rewritten on disk while its node is expanded, and the provider's synchronous work per delta is checked against the 30 ms target of spec 13 (ruling 26). `TreePerf` instruments the provider.

**Files:**
- Create: `extension/src/tree/perf.ts`
- Modify: `extension/src/tree/provider.ts`, `extension/src/testApi.ts`, `extension/src/extension.ts`
- Test: `extension/src/test/integration/commands.test.ts`, `extension/src/test/integration/perf.test.ts`

**Interfaces:**
- Consumes: `protocol.ts` (Task 2): `ViewNode`.
- Produces, in `tree/perf.ts`: `class TreePerf` with `busyMs: number`, `items: number`, `lastChangeAt: number`, `lastItemAt: number`, `reset(): void`, `time<T>(work: () => T): T`, `itemBuilt(): void`, `whenItems(count: number): Promise<void>`.
- Produces, in `tree/provider.ts`: `TreeProvider` gains `readonly perf: TreePerf`.
- Produces, in `testApi.ts`: `TreeHooks` gains `readonly perf: TreePerf`, `node(id: string): ViewNode | undefined`.

- [ ] **Step 1: Write the failing tests**

Create `extension/src/test/integration/commands.test.ts`:

```ts
import * as assert from 'node:assert/strict';
import * as vscode from 'vscode';
import type { ClippingsApi } from '../../testApi';
import { DEFAULT_TREE } from './fixture';
import { getApi, itemAt, setSetting, treeBecomes, whenIdle, workspacePath } from './helpers';

const REGISTRATION_ONLY = ['clippings.openUrl', 'clippings.stopScan', 'clippings.onStatusBarClicked'];

describe('every command', function () {
  this.timeout(60_000);
  let api: ClippingsApi;
  let declared: string[];

  before(async () => {
    api = await getApi();
    await whenIdle(api);
    await treeBecomes(api, DEFAULT_TREE);
    const ext = vscode.extensions.getExtension('clippings-dev.clippings');
    const manifest = ext?.packageJSON as { contributes: { commands: { command: string }[] } };
    declared = manifest.contributes.commands.map((c) => c.command);
  });

  after(async () => {
    for (const key of ['tree.scanMode', 'tree.showCountsInTree', 'tree.showBadges', 'tree.disableCompactFolders']) {
      await setSetting(key, undefined, vscode.ConfigurationTarget.Workspace);
    }
    await vscode.commands.executeCommand('workbench.action.closeAllEditors');
    await vscode.commands.executeCommand('clippings.resetCache');
    await whenIdle(api);
    await treeBecomes(api, DEFAULT_TREE);
  });

  it('registers the 37 declared and 3 registration-only commands', async () => {
    assert.equal(declared.length, 37);
    const registered = new Set(await vscode.commands.getCommands(true));
    for (const id of [...declared, ...REGISTRATION_ONLY]) assert.ok(registered.has(id), id);
  });

  it('runs each command at least once without error', async () => {
    const ran = new Set<string>();
    const run = async (id: string, ...args: unknown[]) => {
      await vscode.commands.executeCommand(id, ...args);
      ran.add(id);
    };
    const src = (await itemAt(api, 'workspace', 'src')).id;
    const lib = (await itemAt(api, 'workspace', 'lib')).id;
    const plan = (await itemAt(api, 'workspace', 'docs', 'plan.md')).id;
    const editor = await vscode.window.showTextDocument(vscode.Uri.file(workspacePath('src', 'app.ts')));

    for (const id of ['showFlatView', 'showTagsOnlyView', 'showTreeView', 'groupByTag', 'ungroupByTag']) {
      await run(`clippings.${id}`);
    }
    for (const id of ['groupBySubTag', 'ungroupBySubTag', 'expand', 'collapse', 'refresh', 'stopScan']) {
      await run(`clippings.${id}`);
    }
    api.test.prompts.script(undefined);
    await run('clippings.filter');
    await run('clippings.filterClear');
    for (const id of ['scanOpenFilesOnly', 'scanCurrentFileOnly', 'scanWorkspaceOnly', 'scanWorkspaceAndOpenFiles']) {
      await run(`clippings.${id}`);
    }
    api.test.prompts.script(undefined, undefined);
    await run('clippings.addTag');
    await run('clippings.removeTag');
    await run('clippings.exportTree');
    await run('clippings.showOnlyThisFolder', src);
    await run('clippings.showOnlyThisFolderAndSubfolders', src);
    await run('clippings.excludeThisFolder', lib);
    await run('clippings.excludeThisFile', plan);
    api.test.prompts.script('OK', undefined);
    await run('clippings.switchScope');
    await run('clippings.removeFilter');
    await run('clippings.resetAllFilters');
    for (const id of ['toggleItemCounts', 'toggleBadges', 'toggleCompactFolders']) {
      await run(`clippings.${id}`);
      await run(`clippings.${id}`);
    }
    await vscode.window.showTextDocument(editor.document);
    await run('clippings.goToNext');
    await run('clippings.goToPrevious');
    await run('clippings.revealInFile');
    await run('clippings.revealInFile', editor.document.uri.toString(), { line: 3, character: 2 });
    await run('clippings.reveal');
    await run('clippings.openUrl');
    await run('clippings.onStatusBarClicked');
    await run('clippings.importTodoTreeSettings');
    await run('clippings.resetCache');
    await run('clippings.showLog');
    await run('clippings.restartServer');
    await whenIdle(api);

    const missing = [...declared, ...REGISTRATION_ONLY].filter((id) => !ran.has(id));
    assert.deepEqual(missing, []);
  });
});
```

Create `extension/src/test/integration/perf.test.ts`:

```ts
import * as assert from 'node:assert/strict';
import { mkdirSync, rmSync, writeFileSync } from 'node:fs';
import * as vscode from 'vscode';
import type { ClippingsApi } from '../../testApi';
import { DEFAULT_TREE } from './fixture';
import { getApi, itemAt, treeBecomes, waitFor, whenIdle, workspacePath } from './helpers';

const COUNT = 1000;
const TARGET_MS = 30;

function manyTodos(version: number): string {
  return Array.from({ length: COUNT }, (_, i) => `// TODO item ${i} version ${version}\n`).join('');
}

function median(values: number[]): number {
  const sorted = [...values].sort((a, b) => a - b);
  return sorted[Math.floor(sorted.length / 2)] ?? NaN;
}

describe('extension host performance (spec 13)', function () {
  this.timeout(120_000);
  let api: ClippingsApi;
  const path = workspacePath('perf', 'many.ts');

  before(async () => {
    api = await getApi();
    await whenIdle(api);
    mkdirSync(workspacePath('perf'), { recursive: true });
    writeFileSync(path, manyTodos(0));
    await waitFor(
      'the perf file in the tree',
      async () => (await api.test.tree.items()).length > 0 && (await itemAt(api, 'workspace', 'perf', 'many.ts').catch(() => undefined)),
      [api.test.tree.onDidChange],
    );
    await vscode.commands.executeCommand('clippings-view.focus');
  });

  after(async () => {
    rmSync(workspacePath('perf'), { recursive: true, force: true });
    await treeBecomes(api, DEFAULT_TREE);
  });

  it(`applies a delta that refreshes ${COUNT} visible nodes in under ${TARGET_MS} ms of host time`, async () => {
    const perf = api.test.tree.perf;
    const file = await itemAt(api, 'workspace', 'perf', 'many.ts');
    perf.reset();
    await api.test.tree.view.reveal(file.id, { expand: true, focus: false, select: false });
    await perf.whenItems(COUNT);

    const busy: number[] = [];
    const wall: number[] = [];
    for (let version = 1; version <= 7; version++) {
      perf.reset();
      writeFileSync(path, manyTodos(version));
      await perf.whenItems(COUNT);
      busy.push(perf.busyMs);
      wall.push(perf.lastItemAt - perf.lastChangeAt);
    }
    const children = await api.test.tree.items(file.id);
    assert.equal(children.length, COUNT);
    assert.equal(children[0]?.label, 'TODO item 0 version 7');

    // The JSON-RPC layer parses the children response on the host too.
    const nodes = children.map((c) => api.test.tree.node(c.id));
    assert.ok(nodes.every((n) => n !== undefined));
    const parse: number[] = [];
    let payload = '';
    for (let i = 0; i < 7; i++) {
      // A fresh string each time, as each response is.
      payload = JSON.stringify({ jsonrpc: '2.0', id: i, result: { nodes } });
      const start = performance.now();
      JSON.parse(payload);
      parse.push(performance.now() - start);
    }

    const host = median(busy) + median(parse);
    console.log(
      `perf: ${COUNT} nodes, provider ${median(busy).toFixed(2)} ms, JSON parse ${median(parse).toFixed(2)} ms ` +
        `(${(payload.length / 1024).toFixed(0)} KiB), host total ${host.toFixed(2)} ms, ` +
        `end-to-end incl. server round trip ${median(wall).toFixed(2)} ms; provider runs ${busy.map((b) => b.toFixed(1)).join(', ')}`,
    );
    assert.ok(host < TARGET_MS, `host time ${host.toFixed(2)} ms is over ${TARGET_MS} ms`);
  });
});
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `pnpm -C extension typecheck`
Expected: FAIL. `error TS2339` for `perf` and `node` on `TreeHooks`.

- [ ] **Step 3: Write the implementation**

Create `extension/src/tree/perf.ts`:

```ts
// Extension host time spent applying tree updates (spec 13): the provider's
// own synchronous work per refresh, children response and tree item.

export class TreePerf {
  /** Milliseconds spent in the provider's synchronous code since the last reset. */
  busyMs = 0;
  /** Tree items built since the last reset. */
  items = 0;
  /** When the last `clippings/treeChanged` arrived, as `performance.now()`. */
  lastChangeAt = 0;
  /** When the last tree item was built. */
  lastItemAt = 0;
  private waiters: { count: number; resolve: () => void }[] = [];

  reset(): void {
    this.busyMs = 0;
    this.items = 0;
  }

  /** Runs `work`, adding its duration to `busyMs`. */
  time<T>(work: () => T): T {
    const start = performance.now();
    try {
      return work();
    } finally {
      this.busyMs += performance.now() - start;
    }
  }

  itemBuilt(): void {
    this.items++;
    this.lastItemAt = performance.now();
    if (this.waiters.length === 0) return;
    const ready = this.waiters.filter((w) => this.items >= w.count);
    this.waiters = this.waiters.filter((w) => this.items < w.count);
    for (const w of ready) w.resolve();
  }

  /** Resolves once `count` items have been built since the last reset. */
  whenItems(count: number): Promise<void> {
    if (this.items >= count) return Promise.resolve();
    return new Promise((resolve) => this.waiters.push({ count, resolve }));
  }
}
```

Replace `extension/src/tree/provider.ts` with:

```ts
// The tree data provider over node ID strings (spec 7.5). Children come from
// `clippings/children`; `clippings/treeChanged` names the parents to refetch.

import * as vscode from 'vscode';
import type { ViewNode } from '../protocol';
import { treeItem, type ItemContext } from './items';
import type { NodeCache } from './nodeCache';
import { TreePerf } from './perf';

export interface ChildrenSource {
  children(parent: string | null): Promise<ViewNode[]>;
}

export class TreeProvider implements vscode.TreeDataProvider<string>, vscode.Disposable {
  private readonly changed = new vscode.EventEmitter<string | string[] | undefined>();
  readonly onDidChangeTreeData = this.changed.event;
  readonly perf = new TreePerf();

  constructor(
    private readonly source: ChildrenSource,
    private readonly cache: NodeCache,
    private readonly ctx: ItemContext,
  ) {}

  async getChildren(element?: string): Promise<string[]> {
    const parent = element ?? null;
    const nodes = await this.source.children(parent);
    return this.perf.time(() => {
      this.cache.record(parent, nodes);
      return nodes.map((n) => n.id);
    });
  }

  getTreeItem(element: string): vscode.TreeItem {
    const item = this.perf.time(() => {
      const node = this.cache.get(element);
      if (node) return treeItem(node, this.ctx);
      const bare = new vscode.TreeItem('');
      bare.id = this.ctx.itemId(element);
      return bare;
    });
    this.perf.itemBuilt();
    return item;
  }

  getParent(element: string): string | undefined {
    return this.cache.parent(element) ?? undefined;
  }

  /** Applies `clippings/treeChanged`: `null` is the root; unknown IDs are ignored. */
  refresh(parents: readonly (string | null)[]): void {
    this.perf.lastChangeAt = performance.now();
    this.perf.time(() => {
      if (parents.includes(null)) return this.changed.fire(undefined);
      const known = parents.filter((p): p is string => p !== null && this.cache.has(p));
      if (known.length > 0) this.changed.fire(known);
    });
  }

  /** Refetches the whole tree. */
  refreshAll(): void {
    this.changed.fire(undefined);
  }

  /** A new server instance: forget every node and refetch. */
  reset(): void {
    this.cache.clear();
    this.changed.fire(undefined);
  }

  dispose(): void {
    this.changed.dispose();
  }
}
```

Replace `extension/src/testApi.ts` with:

```ts
// What `activate` returns. The `test` hooks let the integration tests
// observe state VS Code offers no API to read (spec 12.5).

import type * as vscode from 'vscode';
import type { ApplySource } from './decorations/manager';
import type { DecorationsParams, StatusParams, ViewNode } from './protocol';
import type { PersistedViewState } from './state/viewState';
import type { StatusBarView } from './status/presentation';
import type { TreePerf } from './tree/perf';
import type { TestItem } from './tree/testItems';
import type { Prompts } from './ui/prompts';

export interface ServerHooks {
  readonly running: boolean;
  readonly pid: number | undefined;
  readonly onStatus: vscode.Event<StatusParams>;
  readonly onRunning: vscode.Event<void>;
  /** Fires when repeated crashes stop the automatic restarts (spec 7.4). */
  readonly onGaveUp: vscode.Event<string>;
  /** Shortens the start timeout (`START_TIMEOUT_MS`) so tests need not wait for it. */
  setStartTimeout(ms: number): void;
  status(): StatusParams | undefined;
}

export interface TreeHooks {
  readonly view: vscode.TreeView<string>;
  /** Fires whenever the provider asks VS Code to refetch. */
  readonly onDidChange: vscode.Event<unknown>;
  /** The provider's children of `parent` (top level when omitted), as tree items. */
  items(parent?: string): Promise<TestItem[]>;
  /** The current tree item epoch (spec 7.5). */
  readonly epoch: number;
  /** The line the last todo click flashed. */
  lastFlash(): { uri: string; line: number } | undefined;
  /** Extension host time spent applying tree updates (spec 13). */
  readonly perf: TreePerf;
  /** The last node received for an ID. */
  node(id: string): ViewNode | undefined;
}

export interface DecorationHooks {
  /** Fires after decorations are applied to a document's visible editors. */
  readonly onApplied: vscode.Event<{ uri: string; source: ApplySource }>;
  /** The decorations last applied to a document. */
  entry(uri: string): DecorationsParams | undefined;
  readonly generation: number | undefined;
  readonly styleKeys: string[];
  /** The options a key's decoration type was created with. */
  options(key: string): vscode.DecorationRenderOptions | undefined;
}

export interface TestHooks {
  /** Answers prompts and records messages. */
  readonly prompts: Prompts;
  /** Runs the activation-time import offer again. */
  offerImport(): Promise<void>;
  viewState(): PersistedViewState;
  /** The context keys last set (spec 7.6). */
  contextKeys(): Readonly<Record<string, boolean | string>>;
  /** What the status bar item shows. */
  statusBar(): StatusBarView | undefined;
  readonly decorations: DecorationHooks;
  readonly server: ServerHooks;
  readonly tree: TreeHooks;
}

export interface ClippingsApi {
  readonly version: string;
  readonly test: TestHooks;
}
```

Replace `extension/src/extension.ts` with:

```ts
// Composition root: builds each part of the extension and wires them to the
// server connection.

import * as vscode from 'vscode';
import { registerExpandCommands, resetExpansion } from './commands/expand';
import { registerFilterCommands } from './commands/filters';
import { registerGoToCommands } from './commands/goTo';
import { registerNavigationCommands } from './commands/navigation';
import { registerScanCommands } from './commands/scan';
import { needsRestart, registerServerCommands } from './commands/server';
import { registerSettingCommands } from './commands/settings';
import { registerStatusBarCommand } from './commands/statusBar';
import { registerViewCommands } from './commands/view';
import { affectsServer, readConfiguration } from './config/read';
import { ConfigurationSync, replacesTree } from './config/sync';
import { ContextKeys } from './context/apply';
import { contextValues } from './context/keys';
import { SettingWriter } from './config/writes';
import { DecorationManager } from './decorations/manager';
import { registerExport } from './export/documents';
import { IconFiles } from './icons/files';
import { IconResolver } from './icons/resolver';
import { invalidIcons } from './icons/svg';
import { TodoTreeImporter } from './importer/run';
import type { StatusParams } from './protocol';
import { ServerConnection } from './server/connection';
import { ViewStateStore } from './state/viewState';
import { StatusController } from './status/controller';
import { OnceNotice } from './status/presentation';
import type { ClippingsApi } from './testApi';
import { Expansion } from './tree/expansion';
import { NodeCache } from './tree/nodeCache';
import { LineFlash } from './tree/open';
import { TreeProvider } from './tree/provider';
import { Revealer } from './tree/reveal';
import { Prompts } from './ui/prompts';
import { testItem } from './tree/testItems';

let connection: ServerConnection | undefined;

export function activate(context: vscode.ExtensionContext): ClippingsApi {
  const log = vscode.window.createOutputChannel('Clippings', { log: true });
  const store = new ViewStateStore(context.workspaceState);
  const prompts = new Prompts();
  const activeUri = () => vscode.window.activeTextEditor?.document.uri.toString() ?? null;
  const server = new ServerConnection({
    context,
    log,
    settings: () => sync.current,
    activeUri,
    error: (message, ...actions) => prompts.message('error', message, ...actions),
  });
  connection = server;
  const sync = new ConfigurationSync(
    () => readConfiguration(store.snapshot()),
    (settings) => server.configure(settings),
  );
  let lastStatus: StatusParams | undefined;
  const contextKeys = new ContextKeys();
  const updateContext = () => void contextKeys.apply(contextValues(sync.current, lastStatus));
  sync.onPush(updateContext);
  updateContext();

  const icons = new IconResolver(
    new IconFiles(vscode.Uri.joinPath(context.globalStorageUri, 'icons').fsPath),
    vscode.Uri.joinPath(context.extensionUri, 'resources', 'todo-default.svg'),
  );
  const decorations = new DecorationManager(icons);
  const cache = new NodeCache();
  const expansion = new Expansion(store);
  const provider = new TreeProvider(server, cache, {
    icons,
    itemId: (id) => expansion.itemId(id),
    expanded: (node) => expansion.expanded(node),
  });
  const treeView = vscode.window.createTreeView('clippings-view', { treeDataProvider: provider });
  const revealer = new Revealer({
    find: (uri, line) => server.find(uri, line),
    cache,
    view: treeView,
    settings: () => sync.current,
  });
  const flash = new LineFlash();
  const writer = new SettingWriter(prompts);
  const statusController = new StatusController(treeView, prompts);
  const iconWarnings = new OnceNotice();
  const checkIcons = () => {
    const bad = invalidIcons(sync.current);
    const notice = iconWarnings.next(bad.length > 0 ? [`Invalid icons: ${bad.join(', ')}`] : []);
    if (notice) void prompts.message('warning', notice);
  };
  sync.onPush(checkIcons);
  checkIcons();

  context.subscriptions.push(
    log,
    server,
    provider,
    treeView,
    ...registerServerCommands(server, log),
    revealer,
    flash,
    ...registerExpandCommands({ store, sync, expansion, provider }),
    ...registerNavigationCommands(revealer, flash),
    ...registerViewCommands({ store, sync, expansion, provider, prompts }),
    ...registerFilterCommands({ store, sync, cache, prompts }),
    ...registerSettingCommands(writer, prompts),
    ...registerGoToCommands(server),
    ...registerExport(server),
    statusController,
    registerStatusBarCommand({ sync, writer, prompts, view: treeView }),
    ...registerScanCommands(server),
    server.onStatus((s) => {
      lastStatus = s;
      updateContext();
      statusController.update(s);
    }),
    decorations,
    server.onNewInstance(() => {
      provider.reset();
      decorations.reset();
    }),
    server.onStyles((p) => decorations.onStyles(p)),
    server.onDecorations((p) => decorations.onDecorations(p)),
    vscode.window.onDidChangeVisibleTextEditors((editors) => decorations.onVisibleEditors(editors)),
    vscode.workspace.onDidCloseTextDocument((d) => decorations.onDocumentClosed(d)),
    server.onTreeChanged((p) => {
      if (p.refresh.includes(null)) expansion.onRootRefresh();
      provider.refresh(p.refresh);
    }),
    treeView.onDidExpandElement((e) => expansion.set(e.element, true)),
    treeView.onDidCollapseElement((e) => expansion.set(e.element, false)),
    vscode.workspace.onDidChangeConfiguration((e) => {
      if (needsRestart(e)) {
        sync.push();
        void server.restart();
        return;
      }
      if (!affectsServer(e)) return;
      const push = sync.push();
      if (e.affectsConfiguration('clippings.tree.expanded')) {
        resetExpansion({ expansion, provider }, replacesTree(push));
      }
    }),
    vscode.window.onDidChangeActiveTextEditor((editor) => {
      server.activeEditor(activeUri());
      revealer.onActiveEditor(editor);
    }),
  );
  const importer = new TodoTreeImporter(context, prompts, log);
  context.subscriptions.push(
    vscode.commands.registerCommand('clippings.importTodoTreeSettings', () => importer.run()),
  );
  void server.start();
  void importer.offer();

  const manifest = context.extension.packageJSON as { version: string };
  return {
    version: manifest.version,
    test: {
      prompts,
      offerImport: () => importer.offer(),
      viewState: () => store.snapshot(),
      contextKeys: () => contextKeys.values,
      statusBar: () => statusController.shown,
      decorations: {
        onApplied: decorations.onApplied,
        entry: (uri) => decorations.entry(uri),
        get generation() {
          return decorations.currentGeneration;
        },
        get styleKeys() {
          return decorations.styleKeys;
        },
        options: (key) => decorations.options(key),
      },
      server: {
        get running() {
          return server.running;
        },
        get pid() {
          return server.pid;
        },
        onStatus: server.onStatus,
        onRunning: server.onRunning,
        onGaveUp: server.onGaveUp,
        setStartTimeout: (ms) => (server.startTimeoutMs = ms),
        status: () => lastStatus,
      },
      tree: {
        view: treeView,
        onDidChange: provider.onDidChangeTreeData,
        items: async (parent) => {
          const ids = await provider.getChildren(parent);
          return ids.map((id) => testItem(id, provider.getTreeItem(id)));
        },
        get epoch() {
          return expansion.currentEpoch;
        },
        lastFlash: () => flash.last,
        perf: provider.perf,
        node: (id) => cache.get(id),
      },
    },
  };
}

export async function deactivate(): Promise<void> {
  await connection?.stop();
  connection = undefined;
}
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `pnpm -C extension dev && pnpm -C extension typecheck && pnpm -C extension build`
Expected: no errors.

Run: `pnpm -C extension test:unit`
Expected: `79 passing`.

Run: `pnpm -C extension test:integration`
Expected: `68 passing`. A VS Code test window opens, runs the suite and closes.

- [ ] **Step 5: Commit**

```bash
git add -A
git commit -F - <<'EOF'
test(extension): run every command and measure the host time of a 1,000-node delta

Co-Authored-By: <your model attribution>
EOF
```

### Task 19: Run Extension launch configuration

Spec 8.3's local development loop: `.vscode/launch.json` runs the extension against the debug server, with `CLIPPINGS_SERVER_PATH` pointing at `target/debug/clippings` and `RUST_BACKTRACE=1` (ruling 1), after a pre-launch task that runs `pnpm dev && pnpm build`. A unit test pins both files.

**Files:**
- Create: `.vscode/launch.json`, `.vscode/tasks.json`
- Test: `extension/src/test/unit/devLaunch.test.ts`

**Interfaces:**
- Produces: the launch configuration `Run Extension` and the task `clippings: build server and extension` in the repository's `.vscode/`.

- [ ] **Step 1: Write the failing tests**

Create `extension/src/test/unit/devLaunch.test.ts`:

```ts
import * as assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';

// Local development (spec 8.3): F5 builds the debug server and points the
// extension at it.
const root = resolve(__dirname, '../../../..');
const read = (path: string) => JSON.parse(readFileSync(resolve(root, path), 'utf8')) as unknown;

interface Launch {
  configurations: {
    name: string;
    type: string;
    args: string[];
    env: Record<string, string>;
    windows: { env: Record<string, string> };
    preLaunchTask: string;
  }[];
}
interface Tasks {
  tasks: { label: string; command: string; options: { cwd: string } }[];
}

describe('local development', () => {
  it('runs the extension against the debug server with backtraces', () => {
    const launch = read('.vscode/launch.json') as Launch;
    const run = launch.configurations.find((c) => c.name === 'Run Extension');
    assert.ok(run);
    assert.equal(run.type, 'extensionHost');
    assert.deepEqual(run.args, ['--extensionDevelopmentPath=${workspaceFolder}/extension']);
    assert.equal(run.env['CLIPPINGS_SERVER_PATH'], '${workspaceFolder}/target/debug/clippings');
    assert.equal(run.env['RUST_BACKTRACE'], '1');
    assert.match(run.windows.env['CLIPPINGS_SERVER_PATH'] ?? '', /clippings\.exe$/);

    const tasks = read('.vscode/tasks.json') as Tasks;
    const build = tasks.tasks.find((t) => t.label === run.preLaunchTask);
    assert.ok(build, 'the pre-launch task exists');
    assert.equal(build.command, 'pnpm dev && pnpm build');
  });

  it('builds the debug server with pnpm dev', () => {
    const manifest = read('extension/package.json') as { scripts: Record<string, string> };
    assert.equal(manifest.scripts['dev'], 'cargo build --manifest-path ../Cargo.toml -p clippings');
  });
});
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `pnpm -C extension build && pnpm -C extension test:unit`
Expected: FAIL. `80 passing`, `1 failing`: `runs the extension against the debug server with backtraces` fails with `ENOENT` for `.vscode/launch.json`.

- [ ] **Step 3: Write the implementation**

Create `.vscode/launch.json`:

```json
{
  "version": "0.2.0",
  "configurations": [
    {
      "name": "Run Extension",
      "type": "extensionHost",
      "request": "launch",
      "args": ["--extensionDevelopmentPath=${workspaceFolder}/extension"],
      "env": {
        "CLIPPINGS_SERVER_PATH": "${workspaceFolder}/target/debug/clippings",
        "RUST_BACKTRACE": "1"
      },
      "windows": {
        "env": {
          "CLIPPINGS_SERVER_PATH": "${workspaceFolder}\\target\\debug\\clippings.exe",
          "RUST_BACKTRACE": "1"
        }
      },
      "outFiles": ["${workspaceFolder}/extension/dist/**/*.js"],
      "preLaunchTask": "clippings: build server and extension"
    }
  ]
}
```

Create `.vscode/tasks.json`:

```json
{
  "version": "2.0.0",
  "tasks": [
    {
      "label": "clippings: build server and extension",
      "type": "shell",
      "command": "pnpm dev && pnpm build",
      "options": { "cwd": "${workspaceFolder}/extension" },
      "group": "build",
      "problemMatcher": []
    }
  ]
}
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `pnpm -C extension dev && pnpm -C extension typecheck && pnpm -C extension build`
Expected: no errors.

Run: `pnpm -C extension test:unit`
Expected: `81 passing`.

Run: `pnpm -C extension test:integration`
Expected: `68 passing`. A VS Code test window opens, runs the suite and closes.

- [ ] **Step 5: Commit**

```bash
git add -A
git commit -F - <<'EOF'
chore: add the Run Extension launch configuration for local development

Co-Authored-By: <your model attribution>
EOF
```

### Task 20: Start on the defaults when a setting has the wrong type

VS Code passes on whatever `settings.json` holds, so a hand-edited value can have the wrong type. The server exited during `initialize` when it could not read the settings, leaving the extension without a server; it now starts on the defaults and warns, as it already does for an unreadable `clippings/configure` (ruling 24, Review Focus 1). The client builds its document selector and the track file check from `config/schemes.ts`, which falls back to the default schemes for such a value. The task also sorts two imports in `extension.ts`.

**Files:**
- Create: `extension/src/config/schemes.ts`
- Modify: `crates/clippings-core/src/server/main_loop.rs`, `extension/src/server/connection.ts`, `extension/src/tree/reveal.ts`, `extension/src/extension.ts`
- Test: `crates/clippings-core/tests/server.rs` (modified, diff), `extension/src/test/integration/server.test.ts` (modified), `extension/src/test/unit/schemes.test.ts`

**Interfaces:**
- Produces: when the settings in `initializationOptions` cannot be read, `clippings lsp` answers `initialize`, starts on the default settings, and puts `Invalid configuration, using the defaults: <serde error>` in the first `clippings/status` warnings, until a readable `clippings/configure` clears it.
- Produces, in `config/schemes.ts`: `DEFAULT_SCHEMES` (const); `function schemeList(value: unknown): readonly string[]`; `function documentSelector(value: unknown): { scheme: string }[]`.

- [ ] **Step 1: Write the failing tests**

Apply this diff to `crates/clippings-core/tests/server.rs` with `git apply`:

```diff
diff --git a/crates/clippings-core/tests/server.rs b/crates/clippings-core/tests/server.rs
index 12f059d..2726f2f 100644
--- a/crates/clippings-core/tests/server.rs
+++ b/crates/clippings-core/tests/server.rs
@@ -372,6 +372,25 @@ fn invalid_regex_reports_an_error_and_keeps_the_tree() {
     c.shutdown();
 }
 
+#[test]
+fn a_setting_of_the_wrong_type_starts_on_the_defaults_with_a_warning() {
+    let (_t, root) = workspace();
+    let mut c = Client::start(&root, json!({ "general": { "schemes": "file" } }), true);
+    let status = c.expect("clippings/status", |s| {
+        s["warnings"].as_array().is_some_and(|w| !w.is_empty())
+    });
+    let warning = status["warnings"][0].as_str().unwrap();
+    assert!(
+        warning.starts_with("Invalid configuration, using the defaults: invalid type"),
+        "{warning}"
+    );
+    let name = root.file_name().unwrap().to_string_lossy().to_string();
+    assert!(c.settled_top().contains(&name), "the tree is served");
+    c.notify("clippings/configure", settings());
+    c.expect("clippings/status", |s| s["warnings"] == json!([]));
+    c.shutdown();
+}
+
 #[test]
 fn protocol_version_mismatch_is_rejected() {
     let (_t, root) = workspace();
```

Replace `extension/src/test/integration/server.test.ts` with:

```ts
import * as assert from 'node:assert/strict';
import { chmodSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import * as vscode from 'vscode';
import { START_TIMEOUT_MS } from '../../server/connection';
import type { ClippingsApi } from '../../testApi';
import { DEFAULT_TREE } from './fixture';
import { getApi, setSetting, treeBecomes, waitFor, whenIdle, withTimeout } from './helpers';

describe('server lifecycle', () => {
  let api: ClippingsApi;

  before(async () => {
    api = await getApi();
    await whenIdle(api);
  });

  it('starts the locally built server and receives its status', () => {
    const status = api.test.server.status();
    assert.ok(status);
    assert.match(status.instance, /^[0-9a-f]+-[0-9a-f]+$/);
    assert.equal(status.error, null);
    assert.ok(api.test.server.pid);
  });

  it('restarts on demand with a new server instance', async () => {
    const before = api.test.server.status()?.instance;
    await vscode.commands.executeCommand('clippings.restartServer');
    const s = api.test.server;
    await waitFor('a new instance', () => s.running && s.status()?.instance !== before, [s.onStatus, s.onRunning]);
    await whenIdle(api);
  });

  it('restarts a crashed server', async () => {
    const s = api.test.server;
    const instance = s.status()?.instance;
    const pid = s.pid;
    assert.ok(pid);
    process.kill(pid, 'SIGKILL');
    await waitFor(
      'a restarted server',
      () => s.running && s.pid !== pid && s.status()?.instance !== instance,
      [s.onStatus, s.onRunning],
    );
    await whenIdle(api);
  });

  it('restarts when a setting fixed at start changes', async () => {
    const s = api.test.server;
    const instance = s.status()?.instance;
    await vscode.workspace
      .getConfiguration('clippings')
      .update('server.logLevel', 'debug', vscode.ConfigurationTarget.Global);
    try {
      await waitFor('a restart', () => s.running && s.status()?.instance !== instance, [s.onStatus, s.onRunning]);
    } finally {
      await vscode.workspace
        .getConfiguration('clippings')
        .update('server.logLevel', undefined, vscode.ConfigurationTarget.Global);
    }
    await whenIdle(api);
  });

  it('stops restarting after five crashes in three minutes and restarts on demand', async function () {
    this.timeout(60_000);
    const s = api.test.server;
    await vscode.commands.executeCommand('clippings.restartServer');
    await whenIdle(api);
    let notice: string | undefined;
    const subscription = s.onGaveUp((message) => (notice = message));
    try {
      for (let crash = 1; crash <= 5; crash++) {
        const pid = s.pid;
        assert.ok(pid, `a running server before crash ${crash}`);
        process.kill(pid, 'SIGKILL');
        if (crash < 5) {
          await waitFor(`a restart after crash ${crash}`, () => s.running && s.pid !== pid, [s.onStatus, s.onRunning]);
        }
      }
      const message = await waitFor('the crash notice', () => notice, [s.onGaveUp]);
      assert.match(message, /crashed 5 times in the last 3 minutes/);
    } finally {
      subscription.dispose();
    }
    await vscode.commands.executeCommand('clippings.restartServer');
    await whenIdle(api);
    assert.ok(s.running);
  });

  it('counts a server that exits while starting as a crash and restarts on demand', async function () {
    if (process.platform === 'win32') this.skip();
    this.timeout(60_000);
    const s = api.test.server;
    // Passes the probe, then exits as soon as `initialize` arrives.
    const dir = mkdtempSync(join(tmpdir(), 'clippings-fake-'));
    const fake = join(dir, 'clippings');
    writeFileSync(
      fake,
      [
        '#!/bin/sh',
        'if [ "$1" = probe ]; then',
        '  echo \'{"version":"0.0.0","target":"fake","protocolVersion":1}\'',
        '  exit 0',
        'fi',
        'head -c 1 > /dev/null',
        'exit 1',
        '',
      ].join('\n'),
    );
    chmodSync(fake, 0o755);
    const real = process.env['CLIPPINGS_SERVER_PATH'];
    let notice: string | undefined;
    const subscription = s.onGaveUp((message) => (notice = message));
    try {
      process.env['CLIPPINGS_SERVER_PATH'] = fake;
      const restart = vscode.commands.executeCommand('clippings.restartServer');
      await withTimeout('Restart Server with a server that exits', restart);
      const message = await waitFor('the crash notice', () => notice, [s.onGaveUp]);
      assert.match(message, /crashed 5 times in the last 3 minutes/);
      assert.equal(s.running, false);
      const shown = api.test.prompts.shown.at(-1);
      assert.deepEqual(shown, { kind: 'error', message, actions: ['Restart', 'Show Log'] });
    } finally {
      process.env['CLIPPINGS_SERVER_PATH'] = real;
      subscription.dispose();
      rmSync(dir, { recursive: true, force: true });
    }
    const restart = vscode.commands.executeCommand('clippings.restartServer');
    await withTimeout('Restart Server with the real server', restart);
    await whenIdle(api);
    assert.ok(s.running);
    await treeBecomes(api, DEFAULT_TREE);
  });

  it('counts a server that does not finish starting in time as a crash and kills it', async function () {
    if (process.platform === 'win32') this.skip();
    this.timeout(60_000);
    const s = api.test.server;
    // Passes the probe, then reads `initialize` and never answers.
    const dir = mkdtempSync(join(tmpdir(), 'clippings-fake-'));
    const fake = join(dir, 'clippings');
    const pids = join(dir, 'pids');
    writeFileSync(
      fake,
      [
        '#!/bin/sh',
        'if [ "$1" = probe ]; then',
        '  echo \'{"version":"0.0.0","target":"fake","protocolVersion":1}\'',
        '  exit 0',
        'fi',
        `echo $$ >> '${pids}'`,
        // Keeps stdout open on fd 3 while discarding the requests.
        'exec cat 3>&1 > /dev/null',
        '',
      ].join('\n'),
    );
    chmodSync(fake, 0o755);
    const real = process.env['CLIPPINGS_SERVER_PATH'];
    let notice: string | undefined;
    const subscription = s.onGaveUp((message) => (notice = message));
    try {
      process.env['CLIPPINGS_SERVER_PATH'] = fake;
      s.setStartTimeout(500);
      const restart = vscode.commands.executeCommand('clippings.restartServer');
      await withTimeout('Restart Server with a server that hangs', restart);
      const message = await waitFor('the crash notice', () => notice, [s.onGaveUp]);
      assert.match(message, /crashed 5 times in the last 3 minutes/);
      assert.equal(s.running, false);
      const started = readFileSync(pids, 'utf8').trim().split('\n').map(Number);
      assert.equal(started.length, 5, 'five attempts');
      for (const pid of started) assert.throws(() => process.kill(pid, 0), /ESRCH/, `server process ${pid} is gone`);
      const shown = api.test.prompts.shown.at(-1);
      assert.deepEqual(shown, { kind: 'error', message, actions: ['Restart', 'Show Log'] });
    } finally {
      process.env['CLIPPINGS_SERVER_PATH'] = real;
      s.setStartTimeout(START_TIMEOUT_MS);
      subscription.dispose();
      rmSync(dir, { recursive: true, force: true });
    }
    const restart = vscode.commands.executeCommand('clippings.restartServer');
    await withTimeout('Restart Server with the real server', restart);
    await whenIdle(api);
    assert.ok(s.running);
    await treeBecomes(api, DEFAULT_TREE);
  });

  it('starts on the defaults with a warning when a setting has the wrong type', async () => {
    const s = api.test.server;
    const instance = s.status()?.instance;
    await setSetting('general.schemes', 'file');
    try {
      const status = await waitFor(
        'a new server with a warning',
        () => {
          const current = s.status();
          return current && current.instance !== instance && current.warnings.length > 0 ? current : undefined;
        },
        [s.onStatus],
      );
      assert.match(status.warnings[0] ?? '', /^Invalid configuration, using the defaults: invalid type/);
      const warned = api.test.prompts.shown.some(
        (p) => p.kind === 'warning' && p.message.includes('Invalid configuration'),
      );
      assert.ok(warned);
    } finally {
      await setSetting('general.schemes', undefined);
    }
    await waitFor('a server without warnings', () => s.running && s.status()?.warnings.length === 0, [
      s.onStatus,
      s.onRunning,
    ]);
    await whenIdle(api);
  });

  it('shows the log', async () => {
    await vscode.commands.executeCommand('clippings.showLog');
  });
});
```

Create `extension/src/test/unit/schemes.test.ts`:

```ts
import * as assert from 'node:assert/strict';
import { DEFAULT_SCHEMES, documentSelector, schemeList } from '../../config/schemes';

describe('general.schemes', () => {
  it('selects one filter per configured scheme', () => {
    assert.deepEqual(documentSelector(['file', 'vscode-notebook-cell']), [
      { scheme: 'file' },
      { scheme: 'vscode-notebook-cell' },
    ]);
    assert.deepEqual(documentSelector([]), []);
  });

  it('falls back to the defaults for a value of the wrong type', () => {
    for (const value of ['file', undefined, null, 3, { file: true }, ['file', 3]]) {
      assert.deepEqual(schemeList(value), DEFAULT_SCHEMES, JSON.stringify(value));
    }
    assert.deepEqual(DEFAULT_SCHEMES, ['file', 'ssh', 'untitled', 'vscode-notebook-cell']);
  });
});
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p clippings-core --test server a_setting_of_the_wrong_type`
Expected: FAIL. The test panics with `response: Disconnected`: the server exits instead of answering `initialize`.

Run: `pnpm -C extension typecheck`
Expected: FAIL with `error TS2307: Cannot find module '../../config/schemes'`.

- [ ] **Step 3: Write the implementation**

Create `extension/src/config/schemes.ts`:

```ts
// `general.schemes` as the client uses it (spec 6.1, 7.4, 7.5). Pure. A
// settings file can hold a value of the wrong type, which VS Code passes on;
// the server then starts on its defaults, and so does the client.

/** todo-tree's default, as in the manifest and the server's settings. */
export const DEFAULT_SCHEMES: readonly string[] = ['file', 'ssh', 'untitled', 'vscode-notebook-cell'];

/** The configured schemes, or the defaults when the value is not a list of strings. */
export function schemeList(value: unknown): readonly string[] {
  return Array.isArray(value) && value.every((s) => typeof s === 'string') ? value : DEFAULT_SCHEMES;
}

/** The language client's document selector: one filter per scheme. */
export function documentSelector(value: unknown): { scheme: string }[] {
  return schemeList(value).map((scheme) => ({ scheme }));
}
```

Replace `crates/clippings-core/src/server/main_loop.rs` with:

```rust
//! The server's event loop over an `lsp-server` connection.

use super::{Env, Server};
use crate::fs::{Fs, NativeFs};
use crate::protocol::InitializeParams;
use crate::settings::Settings;
use crate::PROTOCOL_VERSION;
use crossbeam_channel::{after, never, select};
use lsp_server::{Connection, ErrorCode, Message, Response};
use serde_json::{json, Value};
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::sync::Arc;
use std::time::Instant;

fn capabilities() -> serde_json::Value {
    json!({
        "capabilities": {
            "textDocumentSync": { "openClose": true, "change": 2 },
            "workspace": { "workspaceFolders": { "supported": true, "changeNotifications": true } }
        },
        "serverInfo": { "name": "clippings", "version": env!("CARGO_PKG_VERSION") }
    })
}

/// Replaces initial settings the server cannot read with an empty object, so
/// the server starts on the defaults, and returns why. A hand-edited settings
/// file can hold a value of the wrong type, which VS Code passes on.
fn take_unreadable_settings(raw: &mut Value) -> Option<String> {
    let settings = raw.get_mut("initializationOptions")?.get_mut("settings")?;
    let error = serde_json::from_value::<Settings>(settings.clone()).err()?;
    *settings = json!({});
    Some(error.to_string())
}

/// Runs the handshake and the event loop until `exit`.
pub fn run(connection: Connection, fs: Arc<dyn Fs>, env: Env) -> Result<(), String> {
    let (id, mut raw) = connection.initialize_start().map_err(|e| e.to_string())?;
    let unreadable = take_unreadable_settings(&mut raw);
    let params: InitializeParams = serde_json::from_value(raw).map_err(|e| e.to_string())?;
    let version = params
        .initialization_options
        .as_ref()
        .map(|o| o.protocol_version);
    if version != Some(PROTOCOL_VERSION) {
        let message =
            format!("protocol version mismatch: client {version:?}, server {PROTOCOL_VERSION}");
        let _ = connection
            .sender
            .send(Response::new_err(id, ErrorCode::InvalidRequest as i32, message.clone()).into());
        return Err(message);
    }
    connection
        .initialize_finish(id, capabilities())
        .map_err(|e| e.to_string())?;
    tracing::info!(
        "clippings {} serving {} workspace folder(s), protocol version {PROTOCOL_VERSION}",
        env!("CARGO_PKG_VERSION"),
        params.workspace_folders.as_ref().map_or(0, Vec::len),
    );
    let mut server = Server::new(connection.sender.clone(), fs, env, &params);
    if let Some(e) = unreadable {
        tracing::warn!("bad configuration: {e}");
        server.config_warning = Some(format!("Invalid configuration, using the defaults: {e}"));
    }
    server.start(Instant::now());
    let work = server.work_rx.clone();
    loop {
        let timeout = match server.next_deadline() {
            Some(d) => after(d.saturating_duration_since(Instant::now())),
            None => never(),
        };
        select! {
            recv(connection.receiver) -> msg => {
                let Ok(msg) = msg else { break };
                match msg {
                    Message::Request(req) => {
                        if connection.handle_shutdown(&req).map_err(|e| e.to_string())? {
                            break;
                        }
                        let id = req.id.clone();
                        let resp = catch_unwind(AssertUnwindSafe(|| server.handle_request(req)))
                            .unwrap_or_else(|_| Response::new_err(id, ErrorCode::InternalError as i32, "request panicked".into()));
                        let _ = connection.sender.send(resp.into());
                    }
                    Message::Notification(n) => {
                        if n.method == "exit" {
                            break;
                        }
                        if catch_unwind(AssertUnwindSafe(|| server.handle_notification(n, Instant::now()))).is_err() {
                            server.recover(Instant::now());
                        }
                    }
                    Message::Response(_) => {}
                }
            }
            recv(work) -> w => {
                if let Ok(w) = w {
                    if catch_unwind(AssertUnwindSafe(|| server.handle_work(w, Instant::now()))).is_err() {
                        server.recover(Instant::now());
                    }
                }
            }
            recv(timeout) -> _ => {
                if catch_unwind(AssertUnwindSafe(|| server.tick(Instant::now()))).is_err() {
                    server.recover(Instant::now());
                }
            }
        }
    }
    Ok(())
}

/// `clippings lsp`: the server over stdin and stdout.
pub fn run_stdio() -> Result<(), String> {
    let (connection, io) = Connection::stdio();
    let env: Env = Arc::new(|n| std::env::var(n).ok());
    let result = run(connection, Arc::new(NativeFs), env);
    io.join().map_err(|e| e.to_string())?;
    result
}
```

Replace `extension/src/server/connection.ts` with:

```ts
// The server's lifecycle (spec 7.4): resolve the binary, spawn `clippings
// lsp` through the language client, relay its notifications, and restart on
// demand or after a crash. Everything else talks to the server through this
// class.

import { spawn, type ChildProcess } from 'node:child_process';
import * as vscode from 'vscode';
import { LanguageClient, State, type LanguageClientOptions, type ServerOptions } from 'vscode-languageclient/node';
import { documentSelector } from '../config/schemes';
import {
  PROTOCOL_VERSION,
  type DecorationsParams,
  type Direction,
  type ExportResult,
  type Position,
  type Range,
  type Settings,
  type StatusParams,
  type StylesParams,
  type TreeChangedParams,
  type ViewNode,
} from '../protocol';
import { CrashHistory, GIVE_UP_MESSAGE } from './crashHistory';
import { CrashPolicy } from './crashPolicy';
import { locateServer } from './locate';
import * as m from './messages';
import { ResolutionError } from './resolve';

/** How long a server may take to finish starting before it counts as a crash. */
export const START_TIMEOUT_MS = 10_000;
/** How long to wait for a killed server process to exit, per signal. */
const KILL_WAIT_MS = 2000;

export interface ConnectionHost {
  readonly context: vscode.ExtensionContext;
  readonly log: vscode.LogOutputChannel;
  /** The configuration object to send, read fresh on each call. */
  settings(): Settings;
  /** The active editor's document URI, for `clippings/activeEditor`. */
  activeUri(): string | null;
  /** Shows an error with action buttons and resolves with the chosen one. */
  error(message: string, ...actions: string[]): PromiseLike<string | undefined>;
}

export class ServerConnection implements vscode.Disposable {
  private client: LanguageClient | undefined;
  /**
   * The current client's server process. Clippings spawns it itself: the
   * language client forgets its process once the connection closes, and a
   * server that closed its output can still be alive and must be killed.
   */
  private process: ChildProcess | undefined;
  private starting: Promise<void> | undefined;
  /** Crashes and start failures across every client of this connection. */
  private readonly crashes = new CrashHistory();
  /** `START_TIMEOUT_MS`; tests shorten it. */
  startTimeoutMs = START_TIMEOUT_MS;
  private readonly emitters = {
    gaveUp: new vscode.EventEmitter<string>(),
    status: new vscode.EventEmitter<StatusParams>(),
    treeChanged: new vscode.EventEmitter<TreeChangedParams>(),
    styles: new vscode.EventEmitter<StylesParams>(),
    decorations: new vscode.EventEmitter<DecorationsParams>(),
    running: new vscode.EventEmitter<void>(),
    instance: new vscode.EventEmitter<string>(),
  };
  private instance: string | undefined;
  readonly onStatus = this.emitters.status.event;
  readonly onTreeChanged = this.emitters.treeChanged.event;
  readonly onStyles = this.emitters.styles.event;
  readonly onDecorations = this.emitters.decorations.event;
  /** Fires with the notice when repeated crashes stop the automatic restarts. */
  readonly onGaveUp = this.emitters.gaveUp.event;
  /** Fires each time a server, new or restarted, is ready for requests. */
  readonly onRunning = this.emitters.running.event;
  /**
   * Fires before the first status of a new server instance, so listeners
   * discard their node, decoration and style state (spec 6.2).
   */
  readonly onNewInstance = this.emitters.instance.event;

  private readonly trust: vscode.Disposable;

  constructor(private readonly host: ConnectionHost) {
    // A workspace `server.path` counts only once the workspace is trusted.
    this.trust = vscode.workspace.onDidGrantWorkspaceTrust(() => {
      if (vscode.workspace.getConfiguration('clippings').inspect('server.path')?.workspaceValue) void this.restart();
    });
  }

  get running(): boolean {
    return this.client?.state === State.Running;
  }

  /** The server process ID, for tests. */
  get pid(): number | undefined {
    return this.client ? this.process?.pid : undefined;
  }

  start(): Promise<void> {
    this.starting ??= this.doStart().finally(() => (this.starting = undefined));
    return this.starting;
  }

  /** Restarts on demand: forgets past crashes, then starts a fresh client. */
  async restart(): Promise<void> {
    this.crashes.clear();
    await this.starting;
    await this.stop();
    await this.start();
  }

  async stop(): Promise<void> {
    const client = this.client;
    const child = this.process;
    this.client = undefined;
    this.process = undefined;
    if (client) {
      try {
        await client.dispose(2000);
      } catch (err) {
        this.host.log.warn(`Stopping the server failed: ${String(err)}`);
      }
    }
    await killProcess(child);
  }

  /**
   * Starts a client, and starts a fresh one after each start failure until
   * the crash limit. Settles once a server runs, resolution fails, or the
   * limit is reached, so `restart` never waits on a server that died.
   */
  private async doStart(): Promise<void> {
    while ((await this.startOnce()) === 'failed') {
      if (this.crashes.record(Date.now()) === 'give up') {
        void this.showCrash(GIVE_UP_MESSAGE);
        return;
      }
    }
  }

  private async startOnce(): Promise<'started' | 'failed' | 'error'> {
    const { log, context } = this.host;
    let serverPath: string;
    try {
      serverPath = (await locateServer(context, (msg) => log.info(msg))).candidate.path;
    } catch (err) {
      const message = err instanceof Error ? err.message : String(err);
      log.error(message);
      void this.showError(message, err instanceof ResolutionError);
      return 'error';
    }
    const settings = this.host.settings();
    const env = {
      ...process.env,
      RUST_BACKTRACE: '1',
      CLIPPINGS_LOG: vscode.workspace.getConfiguration('clippings').get<string>('server.logLevel', 'info'),
    };
    // Called for the first start and for each restart after a crash.
    let child: ChildProcess | undefined;
    const serverOptions: ServerOptions = () => {
      child = spawn(serverPath, ['lsp'], { env, windowsHide: true });
      if (this.client === client) this.process = child;
      return Promise.resolve(child);
    };
    // Set while `startOnce` waits for this client; a start failure after that
    // comes from the client restarting a crashed server.
    let waiting: ((outcome: 'failed') => void) | undefined;
    const failed = new Promise<'failed'>((resolve) => (waiting = resolve));
    // Each start, first or after a crash, must reach the running state in time.
    let watchdog: NodeJS.Timeout | undefined;
    let abandoned = false;
    const startFailed = (reason: string) => {
      if (abandoned) return;
      abandoned = true;
      clearTimeout(watchdog);
      log.error(reason);
      if (waiting) waiting('failed');
      else void this.afterStartFailure(client, child);
    };
    const policy = new CrashPolicy(this.crashes, {
      startFailed: () => startFailed('The server exited while starting.'),
      gaveUp: (message) => void this.showCrash(message),
    });
    const clientOptions: LanguageClientOptions = {
      // `vscode-notebook-cell` in the list covers notebook cells (spec 6.1).
      documentSelector: documentSelector(settings.general.schemes),
      initializationOptions: () => ({ protocolVersion: PROTOCOL_VERSION, settings: this.host.settings() }),
      outputChannel: log,
      errorHandler: policy,
    };
    const client = new LanguageClient('clippings', 'Clippings', serverOptions, clientOptions);
    client.onNotification(m.Status, (p) => {
      if (p.instance !== this.instance) {
        this.instance = p.instance;
        this.emitters.instance.fire(p.instance);
      }
      this.emitters.status.fire(p);
    });
    client.onNotification(m.TreeChanged, (p) => this.emitters.treeChanged.fire(p));
    client.onNotification(m.Styles, (p) => this.emitters.styles.fire(p));
    client.onNotification(m.Decorations, (p) => this.emitters.decorations.fire(p));
    client.onDidChangeState((e) => {
      clearTimeout(watchdog);
      if (e.newState === State.Starting) {
        const seconds = this.startTimeoutMs / 1000;
        watchdog = setTimeout(
          () => startFailed(`The server did not finish starting within ${seconds} s.`),
          this.startTimeoutMs,
        );
      }
      policy.setRunning(e.newState === State.Running);
      if (e.newState === State.Running && this.client === client) this.onClientRunning(client);
    });
    this.client = client;
    // When the server exits or hangs during `initialize`, `start()` may never
    // settle; a start failure settles the race instead.
    const started = client.start().then(() => 'started' as const);
    started.catch(() => undefined);
    try {
      const outcome = await Promise.race([started, failed]);
      if (outcome === 'failed') await this.discard(client, child);
      return outcome;
    } catch (err) {
      // `initialize` failed or the connection closed under it: a start failure.
      abandoned = true;
      clearTimeout(watchdog);
      log.error(`The server failed to start: ${String(err)}`);
      await this.discard(client, child);
      return 'failed';
    } finally {
      waiting = undefined;
    }
  }

  /** A crashed server that the client restarted, then failed to start. */
  private async afterStartFailure(client: LanguageClient, child: ChildProcess | undefined): Promise<void> {
    await this.discard(client, child);
    if (this.crashes.record(Date.now()) === 'give up') void this.showCrash(GIVE_UP_MESSAGE);
    else void this.start();
  }

  /** Forgets a client whose server failed; it cannot be started again. */
  private async discard(client: LanguageClient, child: ChildProcess | undefined): Promise<void> {
    if (this.client === client) {
      this.client = undefined;
      this.process = undefined;
    }
    await killProcess(child);
    try {
      await client.dispose(2000);
    } catch {
      // A client that never ran cannot be stopped; there is nothing to stop.
    }
  }

  private onClientRunning(client: LanguageClient): void {
    // Settings may have changed between `initialize` and now.
    void client.sendNotification(m.Configure, this.host.settings());
    void client.sendNotification(m.ActiveEditor, { uri: this.host.activeUri() });
    this.emitters.running.fire();
  }

  private async showError(message: string, offerSetting: boolean): Promise<void> {
    const actions = offerSetting ? ['Show Log', 'Open Setting'] : ['Show Log'];
    const choice = await this.host.error(message, ...actions);
    if (choice === 'Show Log') this.host.log.show(true);
    if (choice === 'Open Setting') {
      await vscode.commands.executeCommand('workbench.action.openSettings', 'clippings.server.path');
    }
  }

  private async showCrash(message: string): Promise<void> {
    this.host.log.error(message);
    this.emitters.gaveUp.fire(message);
    const choice = await this.host.error(message, 'Restart', 'Show Log');
    if (choice === 'Restart') await this.restart();
    if (choice === 'Show Log') this.host.log.show(true);
  }

  // ---- client to server ----

  configure(settings: Settings): void {
    if (this.running) void this.client?.sendNotification(m.Configure, settings);
  }

  activeEditor(uri: string | null): void {
    if (this.running) void this.client?.sendNotification(m.ActiveEditor, { uri });
  }

  rescan(): void {
    if (this.running) void this.client?.sendNotification(m.Rescan, {});
  }

  stopScan(): void {
    if (this.running) void this.client?.sendNotification(m.StopScan, {});
  }

  async children(parent: string | null): Promise<ViewNode[]> {
    if (!this.running || !this.client) return [];
    return (await this.client.sendRequest(m.Children, { parent })).nodes;
  }

  async find(uri: string, line: number | null): Promise<ViewNode[][]> {
    if (!this.running || !this.client) return [];
    return (await this.client.sendRequest(m.Find, { uri, line })).paths;
  }

  async navigate(uri: string, positions: Position[], direction: Direction): Promise<Range[] | null> {
    if (!this.running || !this.client) return null;
    return (await this.client.sendRequest(m.Navigate, { uri, positions, direction })).ranges;
  }

  async export(): Promise<ExportResult | undefined> {
    if (!this.running || !this.client) return undefined;
    return this.client.sendRequest(m.Export, {});
  }

  dispose(): void {
    this.trust.dispose();
    void this.stop();
    for (const e of Object.values(this.emitters)) e.dispose();
  }
}

/** Ends a server process that did not exit by itself: SIGTERM, then SIGKILL. */
async function killProcess(child: ChildProcess | undefined): Promise<void> {
  if (!child) return;
  const exited = () => child.exitCode !== null || child.signalCode !== null;
  for (const signal of ['SIGTERM', 'SIGKILL'] as const) {
    if (exited()) return;
    const exit = new Promise<void>((resolve) => child.once('exit', () => resolve()));
    child.kill(signal);
    let timer: NodeJS.Timeout | undefined;
    await Promise.race([exit, new Promise<void>((resolve) => (timer = setTimeout(resolve, KILL_WAIT_MS)))]);
    clearTimeout(timer);
  }
}
```

Replace `extension/src/tree/reveal.ts` with:

```ts
// Reveal Current File In Tree and track file (spec 7.5).

import * as vscode from 'vscode';
import { schemeList } from '../config/schemes';
import type { Settings, ViewNode } from '../protocol';
import type { NodeCache } from './nodeCache';

export interface RevealDeps {
  find(uri: string, line: number | null): Promise<ViewNode[][]>;
  cache: NodeCache;
  view: vscode.TreeView<string>;
  settings(): Settings;
}

export const TRACK_DELAY_MS = 500;

export class Revealer implements vscode.Disposable {
  private timer: NodeJS.Timeout | undefined;

  constructor(private readonly deps: RevealDeps) {}

  /** Reveals the document's file node; false when the tree has none. */
  async reveal(uri: vscode.Uri, focus: boolean): Promise<boolean> {
    const paths = await this.deps.find(uri.toString(), null);
    const first = paths[0];
    const target = first?.at(-1);
    if (!target) return false;
    for (const path of paths) this.deps.cache.recordPath(path);
    await this.deps.view.reveal(target.id, { select: true, focus, expand: false });
    return true;
  }

  /** The `clippings.reveal` command: the active editor's file, if the view is visible. */
  async revealActive(): Promise<void> {
    const editor = vscode.window.activeTextEditor;
    if (editor && this.deps.view.visible) await this.reveal(editor.document.uri, false);
  }

  /** Track file: reveal the new active editor's file after a pause. */
  onActiveEditor(editor: vscode.TextEditor | undefined): void {
    clearTimeout(this.timer);
    if (!editor) return;
    const uri = editor.document.uri;
    this.timer = setTimeout(() => {
      const { tree, general } = this.deps.settings();
      if (tree.autoRefresh && tree.trackFile && schemeList(general.schemes).includes(uri.scheme) && this.deps.view.visible) {
        void this.reveal(uri, false);
      }
    }, TRACK_DELAY_MS);
  }

  dispose(): void {
    clearTimeout(this.timer);
  }
}
```

Replace `extension/src/extension.ts` with:

```ts
// Composition root: builds each part of the extension and wires them to the
// server connection.

import * as vscode from 'vscode';
import { registerExpandCommands, resetExpansion } from './commands/expand';
import { registerFilterCommands } from './commands/filters';
import { registerGoToCommands } from './commands/goTo';
import { registerNavigationCommands } from './commands/navigation';
import { registerScanCommands } from './commands/scan';
import { needsRestart, registerServerCommands } from './commands/server';
import { registerSettingCommands } from './commands/settings';
import { registerStatusBarCommand } from './commands/statusBar';
import { registerViewCommands } from './commands/view';
import { affectsServer, readConfiguration } from './config/read';
import { ConfigurationSync, replacesTree } from './config/sync';
import { SettingWriter } from './config/writes';
import { ContextKeys } from './context/apply';
import { contextValues } from './context/keys';
import { DecorationManager } from './decorations/manager';
import { registerExport } from './export/documents';
import { IconFiles } from './icons/files';
import { IconResolver } from './icons/resolver';
import { invalidIcons } from './icons/svg';
import { TodoTreeImporter } from './importer/run';
import type { StatusParams } from './protocol';
import { ServerConnection } from './server/connection';
import { ViewStateStore } from './state/viewState';
import { StatusController } from './status/controller';
import { OnceNotice } from './status/presentation';
import type { ClippingsApi } from './testApi';
import { Expansion } from './tree/expansion';
import { NodeCache } from './tree/nodeCache';
import { LineFlash } from './tree/open';
import { TreeProvider } from './tree/provider';
import { Revealer } from './tree/reveal';
import { testItem } from './tree/testItems';
import { Prompts } from './ui/prompts';

let connection: ServerConnection | undefined;

export function activate(context: vscode.ExtensionContext): ClippingsApi {
  const log = vscode.window.createOutputChannel('Clippings', { log: true });
  const store = new ViewStateStore(context.workspaceState);
  const prompts = new Prompts();
  const activeUri = () => vscode.window.activeTextEditor?.document.uri.toString() ?? null;
  const server = new ServerConnection({
    context,
    log,
    settings: () => sync.current,
    activeUri,
    error: (message, ...actions) => prompts.message('error', message, ...actions),
  });
  connection = server;
  const sync = new ConfigurationSync(
    () => readConfiguration(store.snapshot()),
    (settings) => server.configure(settings),
  );
  let lastStatus: StatusParams | undefined;
  const contextKeys = new ContextKeys();
  const updateContext = () => void contextKeys.apply(contextValues(sync.current, lastStatus));
  sync.onPush(updateContext);
  updateContext();

  const icons = new IconResolver(
    new IconFiles(vscode.Uri.joinPath(context.globalStorageUri, 'icons').fsPath),
    vscode.Uri.joinPath(context.extensionUri, 'resources', 'todo-default.svg'),
  );
  const decorations = new DecorationManager(icons);
  const cache = new NodeCache();
  const expansion = new Expansion(store);
  const provider = new TreeProvider(server, cache, {
    icons,
    itemId: (id) => expansion.itemId(id),
    expanded: (node) => expansion.expanded(node),
  });
  const treeView = vscode.window.createTreeView('clippings-view', { treeDataProvider: provider });
  const revealer = new Revealer({
    find: (uri, line) => server.find(uri, line),
    cache,
    view: treeView,
    settings: () => sync.current,
  });
  const flash = new LineFlash();
  const writer = new SettingWriter(prompts);
  const statusController = new StatusController(treeView, prompts);
  const iconWarnings = new OnceNotice();
  const checkIcons = () => {
    const bad = invalidIcons(sync.current);
    const notice = iconWarnings.next(bad.length > 0 ? [`Invalid icons: ${bad.join(', ')}`] : []);
    if (notice) void prompts.message('warning', notice);
  };
  sync.onPush(checkIcons);
  checkIcons();

  context.subscriptions.push(
    log,
    server,
    provider,
    treeView,
    ...registerServerCommands(server, log),
    revealer,
    flash,
    ...registerExpandCommands({ store, sync, expansion, provider }),
    ...registerNavigationCommands(revealer, flash),
    ...registerViewCommands({ store, sync, expansion, provider, prompts }),
    ...registerFilterCommands({ store, sync, cache, prompts }),
    ...registerSettingCommands(writer, prompts),
    ...registerGoToCommands(server),
    ...registerExport(server),
    statusController,
    registerStatusBarCommand({ sync, writer, prompts, view: treeView }),
    ...registerScanCommands(server),
    server.onStatus((s) => {
      lastStatus = s;
      updateContext();
      statusController.update(s);
    }),
    decorations,
    server.onNewInstance(() => {
      provider.reset();
      decorations.reset();
    }),
    server.onStyles((p) => decorations.onStyles(p)),
    server.onDecorations((p) => decorations.onDecorations(p)),
    vscode.window.onDidChangeVisibleTextEditors((editors) => decorations.onVisibleEditors(editors)),
    vscode.workspace.onDidCloseTextDocument((d) => decorations.onDocumentClosed(d)),
    server.onTreeChanged((p) => {
      if (p.refresh.includes(null)) expansion.onRootRefresh();
      provider.refresh(p.refresh);
    }),
    treeView.onDidExpandElement((e) => expansion.set(e.element, true)),
    treeView.onDidCollapseElement((e) => expansion.set(e.element, false)),
    vscode.workspace.onDidChangeConfiguration((e) => {
      if (needsRestart(e)) {
        sync.push();
        void server.restart();
        return;
      }
      if (!affectsServer(e)) return;
      const push = sync.push();
      if (e.affectsConfiguration('clippings.tree.expanded')) {
        resetExpansion({ expansion, provider }, replacesTree(push));
      }
    }),
    vscode.window.onDidChangeActiveTextEditor((editor) => {
      server.activeEditor(activeUri());
      revealer.onActiveEditor(editor);
    }),
  );
  const importer = new TodoTreeImporter(context, prompts, log);
  context.subscriptions.push(
    vscode.commands.registerCommand('clippings.importTodoTreeSettings', () => importer.run()),
  );
  void server.start();
  void importer.offer();

  const manifest = context.extension.packageJSON as { version: string };
  return {
    version: manifest.version,
    test: {
      prompts,
      offerImport: () => importer.offer(),
      viewState: () => store.snapshot(),
      contextKeys: () => contextKeys.values,
      statusBar: () => statusController.shown,
      decorations: {
        onApplied: decorations.onApplied,
        entry: (uri) => decorations.entry(uri),
        get generation() {
          return decorations.currentGeneration;
        },
        get styleKeys() {
          return decorations.styleKeys;
        },
        options: (key) => decorations.options(key),
      },
      server: {
        get running() {
          return server.running;
        },
        get pid() {
          return server.pid;
        },
        onStatus: server.onStatus,
        onRunning: server.onRunning,
        onGaveUp: server.onGaveUp,
        setStartTimeout: (ms) => (server.startTimeoutMs = ms),
        status: () => lastStatus,
      },
      tree: {
        view: treeView,
        onDidChange: provider.onDidChangeTreeData,
        items: async (parent) => {
          const ids = await provider.getChildren(parent);
          return ids.map((id) => testItem(id, provider.getTreeItem(id)));
        },
        get epoch() {
          return expansion.currentEpoch;
        },
        lastFlash: () => flash.last,
        perf: provider.perf,
        node: (id) => cache.get(id),
      },
    },
  };
}

export async function deactivate(): Promise<void> {
  await connection?.stop();
  connection = undefined;
}
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test -p clippings-core --test server`
Expected: 12 tests pass.
Then run: `cargo fmt --all && cargo clippy --all-targets -- -D warnings && INSTA_UPDATE=no cargo test --all`
Expected: no diffs, no warnings, and every test passes (202 passed, 2 ignored).

Run: `pnpm -C extension dev && pnpm -C extension typecheck && pnpm -C extension build`
Expected: no errors.

Run: `pnpm -C extension test:unit`
Expected: `83 passing`.

Run: `pnpm -C extension test:integration`
Expected: `69 passing`. A VS Code test window opens, runs the suite and closes.

- [ ] **Step 5: Commit**

```bash
git add -A
git commit -F - <<'EOF'
fix: start on the defaults when a setting has the wrong type

Co-Authored-By: <your model attribution>
EOF
```

### Task 21: Record this plan's rulings in the spec

The spec is the binding authority, so the rulings at the top of this plan go into it wherever they change or clarify behaviour. Ruling 3 needs no edit, because spec 7.6 already says the status bar cycle writes where the value lives.

**Files:**
- Modify: `docs/superpowers/specs/2026-09-23-clippings-design.md` (diff)

**Interfaces:**
- None.

- [ ] **Step 1: Edit the spec**

The edits, section by section:

1. **Section 6.1**: the `$/cancelRequest` line is removed, because the client sends none (ruling 29).
2. **Section 6.2**, styles and decorations semantics: a server's first `status` precedes its first `styles` reset (ruling 19).
3. **Section 6.3**: the client sends whole groups, client-only keys included (ruling 17); an object the server cannot read means the defaults at `initialize` and the previous object on `clippings/configure`, with a warning either way (ruling 24).
4. **Section 7.1**: every command's category is `Clippings` and the Marketplace category is `Other` (ruling 22); the container icon is Clippings' own drawing (ruling 4).
5. **Section 7.2**: `general.revealBehaviour` keeps three values (ruling 21); the settings UI groups for the new settings (ruling 15).
6. **Section 7.3**: the offer and completion texts, and skipped values logged (ruling 16).
7. **Section 7.4**: a workspace `server.path` in an untrusted workspace, and the restart on granting trust (ruling 23); `CLIPPINGS_LOG` in the spawn environment (ruling 18); the crash notice replaces the language client's own, a server that exits before it is running or does not finish starting within 10 seconds counts as a crash and its process is killed, and Restart Server forgets past crashes (ruling 28); a new Malformed settings paragraph (ruling 24).
8. **Section 7.5**: when the epoch bumps, and that it survives `resetCache` (rulings 8 and 9); `clippings.reveal` only while the view is visible (ruling 6); track file cancels a pending reveal (ruling 7).
9. **Section 7.6**: menu clauses test `view == clippings-view`, and the Show Tree View clause is parenthesised (ruling 20); a Folder and file filters paragraph on paths, escaping and Reset All Filters (rulings 2, 10 and 11).
10. **Section 7.7**: the scanning and interrupted texts (ruling 25); Clippings' own icon artwork and why (ruling 4); which icon names are checked (ruling 5).
11. **Section 8.3**: the launch configuration uses `CLIPPINGS_SERVER_PATH` and a pre-launch build (ruling 1).
12. **Section 10.1**: an unreadable configuration keeps the previous one, or the defaults at startup (ruling 24).
13. **Section 10.3**: `CLIPPINGS_LOG`, its values and default, and the startup line (ruling 18).
14. **Section 11.2**: own icon artwork under new behaviour; Reset All Filters, the Show Tree View guard and track file cancelling under fixes (rulings 2, 4, 7 and 20).
15. **Section 12.5**: how integration runs are isolated, where the server comes from, test hooks, the prompts module and the VS Code version (rulings 12, 13, 14 and 27).
16. **Section 13**: what extension host time counts, and the prototype's measurement (ruling 26).

Apply this diff to `docs/superpowers/specs/2026-09-23-clippings-design.md` with `git apply`. It is the exact text of the edits above:

```diff
diff --git a/docs/superpowers/specs/2026-09-23-clippings-design.md b/docs/superpowers/specs/2026-09-23-clippings-design.md
index fbc5a68..8c4b8a9 100644
--- a/docs/superpowers/specs/2026-09-23-clippings-design.md
+++ b/docs/superpowers/specs/2026-09-23-clippings-design.md
@@ -338,7 +338,6 @@ Transport: JSON-RPC over stdio. The server uses the `lsp-server` crate. The clie
 - Text document sync: open, close and incremental change, for the schemes in `general.schemes`.
 - `workspace/didChangeWorkspaceFolders`: roots change, full rescan.
 - `client/registerCapability` and `client/unregisterCapability` for `workspace/didChangeWatchedFiles`, and the resulting notifications.
-- `$/cancelRequest` for the custom requests.
 
 Full scans are not LSP requests and do not use LSP progress. They report through `clippings/status` and are cancelled with `clippings/stopScan`.
 
@@ -363,6 +362,7 @@ Full scans are not LSP requests and do not use LSP progress. They report through
 
 - `generation` increases only when `reset` is true. A non-reset `styles` message adds keys to the current generation.
 - The server sends `styles` for new keys before the `decorations` message that uses them.
+- A server's first `status` precedes its first `styles` reset, so a client that discards its style generation on seeing a new instance keeps that reset.
 - A `decorations` message fully replaces the previous one for that document. Keys absent from `ranges` are cleared.
 - The client drops a `decorations` message whose generation is not its current generation, or whose version is older than the document's current version.
 - After a reset the server resends decorations for every open document.
@@ -376,7 +376,9 @@ All `clippings.*` settings are declared with window scope, except `server.path`,
 - the true keys of `files.exclude` and `search.exclude`, and `explorer.compactFolders`;
 - the view state and temporary globs from workspace storage.
 
-The object has `general`, `highlights`, `filtering`, `tree` and `regex` as nested objects mirroring the settings groups, plus `viewState { flat, tagsOnly, expanded, groupedByTag, groupedBySubTag, filter, includeGlobs, excludeGlobs }`, `filesExclude`, `searchExclude` and `explorerCompactFolders`. Unknown fields are ignored.
+The object has `general`, `highlights`, `filtering`, `tree` and `regex` as nested objects mirroring the settings groups, plus `viewState { flat, tagsOnly, expanded, groupedByTag, groupedBySubTag, filter, includeGlobs, excludeGlobs }`, `filesExclude`, `searchExclude` and `explorerCompactFolders`. Unknown fields are ignored. The client sends each group as VS Code resolves it, including client-only keys such as `tree.buttons`, `filtering.scopes` and `general.statusBarClickBehaviour`, which the server ignores.
+
+VS Code passes on whatever a settings file holds, including a value of the wrong type. The server then cannot read the object: at `initialize` it starts on the defaults, and on `clippings/configure` it keeps the previous object. Either way `clippings/status` carries a warning until a readable object arrives.
 
 On each `clippings/configure` the server diffs against the previous object. Every row whose fields changed applies:
 
@@ -393,14 +395,14 @@ On each `clippings/configure` the server diffs against the previous object. Ever
 
 ### 7.1 Manifest
 
-- `name` `clippings`, display name `Clippings`, category `Clippings`.
+- `name` `clippings`, display name `Clippings`. Every command's category is `Clippings`. The Marketplace category is `Other`, because Marketplace categories come from a fixed list.
 - `publisher` is the maintainer's Marketplace publisher ID. Until one exists the value is `clippings-dev` and CI publishing jobs are skipped.
 - `engines.vscode`: the minimum version required by the pinned `vscode-languageclient` 10.x release.
 - `extensionKind: ["workspace"]`, so the platform VSIX installs where the files are and the server runs there.
 - `activationEvents: ["onStartupFinished"]`.
 - `capabilities.untrustedWorkspaces`: supported `limited`, with `clippings.server.path` as a restricted setting.
 - `capabilities.virtualWorkspaces`: false.
-- An activity bar view container `clippings` with one view `clippings-view`, whose `when` clause is `!clippings-is-empty`.
+- An activity bar view container `clippings` with one view `clippings-view`, whose `when` clause is `!clippings-is-empty`. The container icon is Clippings' own drawing.
 
 ### 7.2 Settings
 
@@ -415,7 +417,7 @@ Carried over unchanged in name, type and default:
 - `tree.buttons.*`: `reveal`, `scanMode`, `viewStyle`, `groupByTag`, `groupBySubTag`, `filter`, `refresh`, `expand`, `export`.
 - `regex.*`: `regex`, `regexCaseSensitive`, `subTagRegex`, `enableMultiLine`.
 
-That is 61 carried settings.
+That is 61 carried settings. `general.revealBehaviour` keeps v0.0.224's three values, `start of line`, `start of todo` and `end of todo`; the `leave focus in tree` value added in v0.0.225 is not carried.
 
 New:
 
@@ -435,29 +437,35 @@ Dropped:
 | `tree.showInExplorer`, `tree.showScanOpenFilesOrWorkspaceButton`, `tree.showTagsFromOpenFilesOnly` | deprecated and unread in todo-tree |
 | `general.debug` | replaced by `server.logLevel`; the importer maps `true` to `debug` |
 
+In the settings UI, `filtering.builtInExcludes` sits in the Filtering group, and `server.path`, `server.logLevel` and `trace.server` form a Server group in the place of todo-tree's dropped Ripgrep group.
+
 ### 7.3 Importing todo-tree settings
 
 On activation, if any `todo-tree.*` key has an explicit global or workspace value, no `clippings.*` key has one, and the user has not chosen Never, a notification offers to import. Choices are Import, Not Now and Never. The command `clippings.importTodoTreeSettings` runs the import on demand.
 
 The import copies each carried key's global and workspace values to the same scopes under `clippings.*`, maps `general.debug` as described above, and skips the other dropped keys. Workspace-folder values are skipped and logged: todo-tree read its settings at window level, so VS Code already ignored them. Keybindings are not imported.
 
+The offer reads `Clippings found Todo Tree settings. Import them into Clippings?`. A finished import reports `Clippings: imported N settings from Todo Tree.`, and each skipped value is logged in the Clippings output channel.
+
 ### 7.4 Server lifecycle
 
 **Resolution order**, stopping at the first candidate that passes the probe:
 
 1. the `CLIPPINGS_SERVER_PATH` environment variable;
-2. `clippings.server.path`, prompting Allow or Deny once per resolved path when it comes from workspace settings;
+2. `clippings.server.path`, prompting Allow or Deny once per resolved path when it comes from workspace settings. In an untrusted workspace the workspace value is ignored without a prompt and the user's own value applies. Granting trust restarts the server when a workspace value exists;
 3. the bundled `bin/clippings` next to `bin/platform.ok`;
 4. `clippings` on PATH, in development builds only.
 
 **Probe**: run `<candidate> probe`, which prints `{ version, target, protocolVersion }` as JSON and exits 0, with a 15 second timeout. The bundled binary is made executable first if needed. A candidate fails if it does not start, times out, exits non-zero, or reports a different protocol version. If every candidate fails, one error lists each candidate and its failure, with buttons to open the output channel and the setting. This is tinymist's aggregation pattern with CodeLLDB's marker file (survey §5 item 2).
 
-**Spawn**: `clippings lsp` through the language client, with `RUST_BACKTRACE=1` and the log level in the environment. stdout carries JSON-RPC only and stderr goes to the Clippings output channel. The server exits when stdin closes.
+**Spawn**: `clippings lsp` through the language client, with `RUST_BACKTRACE=1` and `CLIPPINGS_LOG` set to `server.logLevel` in the environment. stdout carries JSON-RPC only and stderr goes to the Clippings output channel. The server exits when stdin closes.
 
-**Crashes**: a custom error handler restarts the server after each crash until five crashes occur within three minutes. It then stops and shows a notification with Restart and Show Log. The client discards its caches when it sees a new server instance (section 6.2).
+**Crashes**: a custom error handler restarts the server after each crash until five crashes occur within three minutes. It then stops and shows a notification with Restart and Show Log, in place of the language client's own. A server that fails to start counts as a crash: when it exits before it is running, or does not finish starting within 10 seconds, the client kills its process, discards that language client and starts a fresh one, until the same limit, so a server that always fails at startup stops after five attempts. `clippings.restartServer` forgets past crashes. The client discards its caches when it sees a new server instance (section 6.2).
 
 **Restarts**: `clippings.restartServer` restarts on demand. A change to `general.schemes`, `server.path`, `server.logLevel` or `trace.server` restarts the client, because the document selector and spawn environment are fixed at start.
 
+**Malformed settings**: a setting of the wrong type does not stop the server from starting (section 6.3). Where the client reads `general.schemes` itself, for the document selector and track file, a value that is not a list of strings counts as the default schemes.
+
 One server serves all workspace folders in a window.
 
 ### 7.5 Tree view
@@ -468,14 +476,15 @@ One server serves all workspace folders in a window.
 - On `clippings/treeChanged` the client fires the change event for each listed parent, with `undefined` for `null`. VS Code then refetches those parents' children. IDs the client has never loaded are ignored.
 - **Expansion is client-side.** `collapsibleState` comes from the client's expansion map, keyed by node ID, and falls back to the node's `defaultExpanded`. Expand and collapse events update the map in workspace storage and are not sent to the server.
 - **Expand Tree and Collapse Tree** set the persisted `expanded` view state, clear the expansion map, and bump the epoch, then refresh the root. New TreeItem IDs make VS Code read `collapsibleState` afresh; VS Code otherwise keeps its own expansion state for known IDs. The epoch also bumps on `clippings.resetCache` and when `tree.expanded` changes, and never on ordinary deltas, so selection and focus survive edits.
-- **Reveal and track file** call `clippings/find`, cache the returned nodes and their parents, and call `TreeView.reveal` on the first path's last node.
-- **Track file** runs 500 ms after the active editor changes, when `tree.autoRefresh` and `tree.trackFile` are true, the document's scheme is in `general.schemes`, and the view is visible. It reveals without taking focus.
+- **When the epoch bumps.** A change that makes the server re-render the whole tree, which it reports as a root refresh, is a change of view mode, grouping, `expanded` or scan mode. For such a change the epoch bumps at that root refresh, so VS Code reads the new defaults under new IDs; bumping earlier would cache the old defaults under the new IDs. For any other change the epoch bumps at once and the client refreshes the root. The epoch survives `resetCache` and only grows, so tree item IDs never repeat.
+- **Reveal and track file** call `clippings/find`, cache the returned nodes and their parents, and call `TreeView.reveal` on the first path's last node. `clippings.reveal` acts only while the view is visible, from the command palette too, as in todo-tree.
+- **Track file** runs 500 ms after the active editor changes, when `tree.autoRefresh` and `tree.trackFile` are true, the document's scheme is in `general.schemes`, and the view is visible. It reveals without taking focus. Another editor change within the 500 ms cancels the pending reveal.
 - Todo clicks open the document at the node's position and flash the line for 150 ms using one reused decoration type.
 - When `needsScan` is set, the view's message is `Click the refresh button to scan...`. It clears when a scan starts.
 
 ### 7.6 Commands, menus and context keys
 
-Every todo-tree command exists under the `clippings.` prefix with the same suffix, title, icon, menu placement and when-clause, with `todo-tree-*` context keys renamed `clippings-*`. That covers the 34 declared commands and the three registration-only commands `openUrl`, `stopScan` and `onStatusBarClicked` (inventory §7, §4.8).
+Every todo-tree command exists under the `clippings.` prefix with the same suffix, title, icon, menu placement and when-clause, with `todo-tree-*` context keys renamed `clippings-*`. Menu when-clauses test `view == clippings-view` where todo-tree matched `view =~ /todo-tree/`. The Show Tree View context menu entry's clause is parenthesised, `view == clippings-view && (clippings-flat == true || clippings-tags-only == true)`, so its second condition stays inside the view guard. That covers the 34 declared commands and the three registration-only commands `openUrl`, `stopScan` and `onStatusBarClicked` (inventory §7, §4.8).
 
 New commands: `clippings.importTodoTreeSettings`, `clippings.restartServer`, `clippings.showLog`.
 
@@ -503,6 +512,8 @@ New commands: `clippings.importTodoTreeSettings`, `clippings.restartServer`, `cl
 
 View-state commands update workspace storage and send `clippings/configure`. `clippings.resetCache` clears all persisted view state, filters and the expansion map, bumps the epoch, then sends `clippings/configure`.
 
+**Folder and file filters** take the filesystem path from the node's own key (`w:`, `d:` or `f:`), found through the parent recorded for the node, because keys can contain `/`. A file node of a document without a file path has nothing to filter. The path becomes a glob with its metacharacters escaped as `globset::escape` does, and with forward slashes on Windows. Reset All Filters clears the text filter as well as the temporary globs.
+
 Export opens a read-only virtual document under the `clippings-export` scheme with the server's content, named by the formatted export path.
 
 ### 7.7 Decorations, status bar and icons
@@ -510,9 +521,11 @@ Export opens a read-only virtual document under the `clippings-export` scheme wi
 - The `styles` handler creates each `TextEditorDecorationType` synchronously before it returns, so a following `decorations` message always finds its types. Gutter icons use deterministic file paths, and a missing icon file is written synchronously the first time its name and colour appear.
 - Decorations are applied to every visible editor of the document, following the rules in section 6.2.
 - The client keeps the last applied decorations per document URI and version. When an editor becomes visible, the cached entry is applied immediately if its version equals the document's current version. The entry is dropped when the document closes.
-- The status bar item sits on the left at priority 0 and shows the server's text. While a full scan runs it shows scanning and clicking stops the scan. After a stop it shows interrupted and clicking refreshes. Otherwise a click follows `general.statusBarClickBehaviour`: `reveal` focuses the view, `toggle highlights` flips `highlights.enabled`, and `cycle` steps total, tags, top three, current file.
+- The status bar item sits on the left at priority 0 and shows the server's text. While a full scan runs it shows `Clippings: Scanning...` and clicking stops the scan. After a stop it shows `Clippings: Scanning interrupted.` and clicking refreshes. Otherwise a click follows `general.statusBarClickBehaviour`: `reveal` focuses the view, `toggle highlights` flips `highlights.enabled`, and `cycle` steps total, tags, top three, current file.
 - The view's badge and title follow `clippings/status`. Configuration warnings are shown once per distinct set.
 - Icons: codicons become theme icons. Octicons, todo-tree's own icons and the check-circle icon are rendered to SVG once per name and colour under global storage and cached in memory.
+- The `todo-tree` and `todo-tree-filled` icons and the default gutter icon are Clippings' own drawings, and the check-circle is the octicon `check-circle-fill`. todo-tree's icon files are licensed CC BY-ND 3.0, not MIT, so none is copied.
+- Icon names that are neither octicons, `todo-tree`, `todo-tree-filled` nor `$(name)` codicons are reported once as a warning. Codicon names are not checked, because the extension has no codicon list.
 
 ### 7.8 Persisted state
 
@@ -552,7 +565,7 @@ Plus one universal VSIX without a binary or `platform.ok`, which works only when
 
 ### 8.3 Local development
 
-`pnpm dev` builds the debug server. The extension launch configuration sets `clippings.server.path` to that binary and `RUST_BACKTRACE=1`, so F5 runs the extension against a fresh build.
+`pnpm dev` builds the debug server. The Run Extension launch configuration in `.vscode/launch.json` runs `pnpm dev && pnpm build` first, then sets the `CLIPPINGS_SERVER_PATH` environment variable to that binary and `RUST_BACKTRACE=1`, so F5 runs the extension against a fresh build. It uses the environment variable rather than `clippings.server.path` because a launch configuration cannot set a setting, and the variable is resolution candidate 1, so no Allow or Deny prompt appears.
 
 ## 9. Multi-root and remote
 
@@ -569,6 +582,7 @@ Plus one universal VSIX without a binary or `platform.ok`, which works only when
 - An unreadable file is logged at debug level and skipped. The scan continues.
 - A regex that fails under both engines puts the server in an error state. The last good index and view stay in place, `clippings/status` carries the error, and the client shows one warning with an Open Settings action.
 - If the watcher reports an overflow or error, the server runs a full rescan and warns once, unless `tree.autoRefresh` is false (section 5.9).
+- A configuration the server cannot read keeps the previous one, or the defaults at startup, with a warning in `clippings/status` (section 6.3).
 
 ### 10.2 Client
 
@@ -579,7 +593,7 @@ Plus one universal VSIX without a binary or `platform.ok`, which works only when
 
 ### 10.3 Logging
 
-Server logs go to stderr at `server.logLevel` and appear in the Clippings output channel. `trace.server` traces JSON-RPC messages in the same channel.
+Server logs go to stderr and appear in the Clippings output channel. The level comes from the `CLIPPINGS_LOG` environment variable, which the client sets from `server.logLevel`: `error`, `warn`, `info`, `debug`, `trace` or `off`, and `info` when unset or unreadable. The server logs one info line at startup with its version, folder count and protocol version. `trace.server` traces JSON-RPC messages in the same channel.
 
 ## 11. Parity policy
 
@@ -605,6 +619,7 @@ New behaviour:
 
 - **File watching.** Changes on disk update the tree through VS Code's file watcher, in every mode that walks roots, including saves in `workspace only` mode. todo-tree 0.0.224 had no watcher and relied on git or periodic polling.
 - **Built-in never-index list** (section 5.3).
+- **Own icon artwork** for the view container, the two todo icons and the default gutter icon (section 7.7).
 
 Fixes:
 
@@ -641,6 +656,9 @@ Fixes:
 - Export reflects the visible tree, includes extra lines, and never merges two todos on one line or two same-named files into one key.
 - No global `RegExp.prototype.exec` replacement.
 - Glob patterns match dot-prefixed path segments.
+- Reset All Filters also clears the text filter, which todo-tree left in place so it returned on the next refresh.
+- The Show Tree View context menu entry stays inside its view guard.
+- Track file cancels a pending reveal when the editor changes again, instead of queueing reveals.
 
 ### 11.3 Dropped
 
@@ -672,6 +690,8 @@ Rust tests spawn `clippings lsp` against the fixture workspace and assert on mes
 
 `@vscode/test-electron` over the fixture workspace: tree labels in each view mode, every command, expand and collapse, decorations through a test hook, status bar text, context keys, settings import. This is a smoke suite, not an exhaustive one.
 
+Each run copies `tests/fixtures/workspace/` to a temporary directory and starts VS Code with a fresh user data directory holding quiet settings, so tests never touch the repository or the user's profile. Workspace trust is disabled, so untrusted behaviour is covered by unit tests. The server comes from `CLIPPINGS_SERVER_PATH`, which defaults to the debug build. Tests read state through test hooks that `activate` returns, and answer prompts through the one prompts module every command uses. The VS Code version is `stable` unless `CLIPPINGS_TEST_VSCODE` names another.
+
 ## 13. Performance targets and benchmarks
 
 `clippings bench` measures the core, and an extension test hook measures extension host time. Every run records the machine, the file count of each scan set, and ripgrep's time on the same set with equivalent flags and `--max-columns=1000` as the reference. Results are recorded in `docs/benchmarks/` for each milestone.
@@ -695,6 +715,8 @@ Scan sets:
 | Timer wakeups when idle with git and periodic refresh off | zero |
 | Server resident memory, tilliX wide | under 150 MB |
 
+Extension host time counts the provider's own synchronous work, handling `clippings/treeChanged`, recording children and building tree items, plus parsing the children response. VS Code's own tree conversion is not visible to the extension and is excluded. The prototype measured about 1.4 ms for 1,000 nodes.
+
 The view rebuild target reflects a prototype measurement of 12.5 ms on the same machine model. Sharing todo data between the index and the view, which would lower it further, is plan 4's optimisation.
 
 ## 14. Milestones
```

- [ ] **Step 2: Check it reads cleanly**

Re-read sections 6.1, 6.2, 6.3, 7, 8.3, 10, 11.2, 12.5 and 13 in full. Check that no sentence contradicts an edit, for example an older mention of `clippings.server.path` in the launch configuration, and that every edit states a behaviour, not a history.

- [ ] **Step 3: Commit**

```bash
git add -A
git commit -F - <<'EOF'
docs(spec): record the extension plan's rulings

Co-Authored-By: <your model attribution>
EOF
```
