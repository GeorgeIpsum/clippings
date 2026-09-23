> Research artifact generated 2026-09-23. Maps every `contributes.*` entry of Gruntfuggly.todo-tree onto the `clippings.*` prefix per `docs/superpowers/specs/2026-09-23-clippings-design.md` §7.1, 7.2, 7.6 and §11, using the behavioural inventory `docs/research/todo-tree-parity-inventory.md` §4.8, §7 and §8.1.

## Provenance

- **Baseline**: todo-tree v0.0.224 (commit `a6f60e0`), the version the spec and inventory pin to. Fetched directly from `https://raw.githubusercontent.com/Gruntfuggly/todo-tree/a6f60e0/package.json` and `.../package.nls.json` (HTTP 200, both), saved to the session scratchpad and used as the source of truth below.
- **Installed copy for cross-check**: `~/.vscode/extensions/gruntfuggly.todo-tree-0.0.226/` (`package.json`, `package.nls.json`, `resources/`, `CHANGELOG.md`, `dist/extension.js`, `License.txt`).
- **Diff, 0.0.224 → 0.0.226** (`python3 -m json.tool` on both, then `diff -u`): the **only** functional difference in `package.json` is `general.revealBehaviour` gaining a fourth enum value `"leave focus in tree"` with `markdownEnumDescriptions.4` = "Leave the focus in the tree view". `CHANGELOG.md` attributes this to **v0.0.225** ("Add option to leave the focus in the tree when selecting a TODO"); v0.0.226 only adds "Fix bug when converting transparent colours" (a `colours.js` runtime fix with no manifest impact). `package.nls.json` differs by exactly the one added enum-description key. No other command, menu, view, or setting differs between 0.0.224 and 0.0.226.
- Because the spec pins to 0.0.224, **`clippings.general.revealBehaviour` below uses the 3-value enum** (`start of line` / `start of todo` / `end of todo`), not the 4-value 0.0.226 enum. See Notes.
- `crates/clippings-core/src/config.rs:12-38` — `DEFAULT_BUILT_IN_EXCLUDES: [&str; 25]` — supplies the default for the new `filtering.builtInExcludes` setting; it matches spec §5.3's list exactly, 25 entries, same order.

Full assembled `contributes` JSON is saved at `docs/research/clippings-contributes.json` and validated with `python3 -m json.tool` (exit 0, see §6).

---

## 1. Commands

Todo-tree declares 34 commands in `contributes.commands` plus 3 registration-only commands (`todo-tree.openUrl`, `todo-tree.stopScan`, `todo-tree.onStatusBarClicked`) that extension.js registers with `vscode.commands.registerCommand` but never declares — confirmed by diffing package.json's `contributes.commands` (34 entries, counted with `python3` against the fetched 0.0.224 file) against inventory §7/§0#14. **This matches the spec's "34 declared commands and three registration-only" exactly.**

All todo-tree commands use `category: "%todo-tree.command.category%"` → resolved **"Todo Tree"**. All command icons in `contributes.commands` are **codicon strings** (`$(name)`) — VS Code renders codicons via the active color theme's icon-foreground token, so there is no separate light/dark asset; light and dark resolve to the same glyph automatically. No command in `contributes.commands` uses a `resources/` file path or a `{light, dark}` icon object.

Only two commands declare `enablement`: `expand` and `collapse`, both `todo-tree-collapsible`.

| todo-tree id | clippings id | Title (resolved) | Category (todo-tree / clippings) | Icon | Enablement (todo-tree / clippings) |
|---|---|---|---|---|---|
| `todo-tree.showFlatView` | `clippings.showFlatView` | Show Flat View | Todo Tree / Clippings | `$(list-unordered)` | — |
| `todo-tree.showTagsOnlyView` | `clippings.showTagsOnlyView` | Show Tags Only View | Todo Tree / Clippings | `$(list-flat)` | — |
| `todo-tree.showTreeView` | `clippings.showTreeView` | Show Tree View | Todo Tree / Clippings | `$(list-tree)` | — |
| `todo-tree.refresh` | `clippings.refresh` | Refresh | Todo Tree / Clippings | `$(refresh)` | — |
| `todo-tree.expand` | `clippings.expand` | Expand Tree | Todo Tree / Clippings | `$(expand-all)` | `todo-tree-collapsible` / `clippings-collapsible` |
| `todo-tree.collapse` | `clippings.collapse` | Collapse Tree | Todo Tree / Clippings | `$(collapse-all)` | `todo-tree-collapsible` / `clippings-collapsible` |
| `todo-tree.filter` | `clippings.filter` | Filter Tree | Todo Tree / Clippings | `$(filter)` | — |
| `todo-tree.filterClear` | `clippings.filterClear` | Clear Tree Filter | Todo Tree / Clippings | `$(clear-all)` | — |
| `todo-tree.groupByTag` | `clippings.groupByTag` | Group by Tag | Todo Tree / Clippings | `$(tag)` | — |
| `todo-tree.ungroupByTag` | `clippings.ungroupByTag` | Ungroup by Tag | Todo Tree / Clippings | `$(unfold)` | — |
| `todo-tree.groupBySubTag` | `clippings.groupBySubTag` | Group by Sub Tag | Todo Tree / Clippings | `$(chrome-restore)` | — |
| `todo-tree.ungroupBySubTag` | `clippings.ungroupBySubTag` | Ungroup by Sub Tag | Todo Tree / Clippings | `$(chrome-maximize)` | — |
| `todo-tree.scanOpenFilesOnly` | `clippings.scanOpenFilesOnly` | Scan Open Files Only | Todo Tree / Clippings | `$(files)` | — |
| `todo-tree.scanCurrentFileOnly` | `clippings.scanCurrentFileOnly` | Scan Current File Only | Todo Tree / Clippings | `$(symbol-file)` | — |
| `todo-tree.scanWorkspaceAndOpenFiles` | `clippings.scanWorkspaceAndOpenFiles` | Scan Workspace And Open Files | Todo Tree / Clippings | `$(folder-active)` | — |
| `todo-tree.scanWorkspaceOnly` | `clippings.scanWorkspaceOnly` | Scan Workspace Only | Todo Tree / Clippings | `$(folder)` | — |
| `todo-tree.addTag` | `clippings.addTag` | Add Tag | Todo Tree / Clippings | none | — |
| `todo-tree.removeTag` | `clippings.removeTag` | Remove Tag | Todo Tree / Clippings | none | — |
| `todo-tree.exportTree` | `clippings.exportTree` | Export Tree | Todo Tree / Clippings | `$(save)` | — |
| `todo-tree.showOnlyThisFolder` | `clippings.showOnlyThisFolder` | Only Show This Folder | Todo Tree / Clippings | `$(filter)` | — |
| `todo-tree.showOnlyThisFolderAndSubfolders` | `clippings.showOnlyThisFolderAndSubfolders` | Only Show This Folder And Subfolders | Todo Tree / Clippings | `$(filter)` | — |
| `todo-tree.switchScope` | `clippings.switchScope` | Switch Scope | Todo Tree / Clippings | `$(filter)` | — |
| `todo-tree.excludeThisFolder` | `clippings.excludeThisFolder` | Hide This Folder | Todo Tree / Clippings | `$(filter)` | — |
| `todo-tree.excludeThisFile` | `clippings.excludeThisFile` | Hide This File | Todo Tree / Clippings | `$(filter)` | — |
| `todo-tree.removeFilter` | `clippings.removeFilter` | Remove Filter | Todo Tree / Clippings | `$(filter)` | — |
| `todo-tree.resetAllFilters` | `clippings.resetAllFilters` | Reset All Filters | Todo Tree / Clippings | `$(clear-all)` | — |
| `todo-tree.reveal` | `clippings.reveal` | Reveal Current File In Tree | Todo Tree / Clippings | `$(location)` | — |
| `todo-tree.resetCache` | `clippings.resetCache` | Reset Cache | Todo Tree / Clippings | none | — |
| `todo-tree.toggleItemCounts` | `clippings.toggleItemCounts` | Toggle Item Counts | Todo Tree / Clippings | none | — |
| `todo-tree.toggleBadges` | `clippings.toggleBadges` | Toggle Badges | Todo Tree / Clippings | none | — |
| `todo-tree.toggleCompactFolders` | `clippings.toggleCompactFolders` | Toggle Compact Folders | Todo Tree / Clippings | none | — |
| `todo-tree.goToNext` | `clippings.goToNext` | Go To Next | Todo Tree / Clippings | none | — |
| `todo-tree.goToPrevious` | `clippings.goToPrevious` | Go To Previous | Todo Tree / Clippings | none | — |
| `todo-tree.revealInFile` | `clippings.revealInFile` | Reveal In File | Todo Tree / Clippings | none | — |

**Registration-only** (not in `contributes.commands`, so no title/category/icon/palette entry in either extension; registered only via `vscode.commands.registerCommand` in code, per inventory §0#14 and §7):

| todo-tree id | clippings id | Invoked by |
|---|---|---|
| `todo-tree.openUrl` | `clippings.openUrl` | sub-tag tree node click (`command: todo-tree.openUrl` set on the TreeItem, not a declared command) |
| `todo-tree.stopScan` | `clippings.stopScan` | status bar item's command while a scan is running |
| `todo-tree.onStatusBarClicked` | `clippings.onStatusBarClicked` | status bar item's command outside a scan (§9 behaviour) |

**New commands** (spec §7.6, §7.3, §7.4, §10.3), declared with category `Clippings`, no icon (title bar/palette-only, not on the tree view title):

| clippings id | Title | Category | Notes |
|---|---|---|---|
| `clippings.importTodoTreeSettings` | Import Settings from Todo Tree | Clippings | Spec §7.3: runs the settings import on demand |
| `clippings.restartServer` | Restart Server | Clippings | Spec §7.4: restarts the server process on demand |
| `clippings.showLog` | Show Log | Clippings | Spec §7.4/§10.3: opens the Clippings output channel (also the "Show Log" button on the 5-crash notification) |

**Totals**: 34 ported + 3 registration-only + 3 new = **37 declared commands, 3 registration-only** in clippings, matching the spec's count for the ported baseline plus the 3 new commands it lists.

---

## 2. Menus

Todo-tree declares three menu contribution points only — `view/title`, `view/item/context`, `commandPalette` — confirmed by enumerating `contributes.menus`'s keys in the fetched 0.0.224 and installed 0.0.226 `package.json`. **There is no `editor/context` menu and no other menu id.** Totals: `view/title` 18 entries, `view/item/context` 26 entries, `commandPalette` 4 entries = **48 menu entries**.

Rewrite rules applied: `todo-tree-<key>` → `clippings-<key>` for every context key; `view =~ /todo-tree/` → `view == clippings-view` (see Notes — the source uses a regex match against the view id string, not equality; clippings has exactly one view so equality against `clippings-view` is the faithful equivalent, but this is a judgment call since it changes the operator, not just the operand); `viewItem == folder` / `viewItem == file` left unchanged (todo-tree sets `contextValue` to `folder`/`file` on `TreeItem`s, inventory §4.4, and clippings does the same per spec §5.12 rendering rules). `group` and `order` (the `@N` suffix) are copied verbatim.

### 2.1 `view/title` (18 entries, all `group: navigation@N`)

| clippings command | when (rewritten) | group |
|---|---|---|
| `clippings.exportTree` | `view == clippings-view && clippings-show-export-button == true` | `navigation@1` |
| `clippings.reveal` | `view == clippings-view && clippings-tags-only == false && clippings-show-reveal-button == true` | `navigation@2` |
| `clippings.scanOpenFilesOnly` | `view == clippings-view && clippings-scan-mode == 'workspace only' && clippings-show-scan-mode-button == true` | `navigation@3` |
| `clippings.scanCurrentFileOnly` | `view == clippings-view && clippings-scan-mode == 'open files' && clippings-show-scan-mode-button == true` | `navigation@3` |
| `clippings.scanWorkspaceAndOpenFiles` | `view == clippings-view && clippings-scan-mode == 'current file' && clippings-show-scan-mode-button == true` | `navigation@3` |
| `clippings.scanWorkspaceOnly` | `view == clippings-view && clippings-scan-mode == 'workspace' && clippings-show-scan-mode-button == true` | `navigation@3` |
| `clippings.showFlatView` | `view == clippings-view && clippings-flat == false && clippings-tags-only == false && clippings-show-view-style-button == true` | `navigation@4` |
| `clippings.showTagsOnlyView` | `view == clippings-view && clippings-flat == true && clippings-tags-only == false && clippings-show-view-style-button == true` | `navigation@4` |
| `clippings.showTreeView` | `view == clippings-view && clippings-flat == false && clippings-tags-only == true && clippings-show-view-style-button == true` | `navigation@4` |
| `clippings.groupByTag` | `view == clippings-view && clippings-grouped-by-tag == false && clippings-show-group-by-tag-button == true` | `navigation@5` |
| `clippings.ungroupByTag` | `view == clippings-view && clippings-grouped-by-tag == true && clippings-show-group-by-tag-button == true` | `navigation@5` |
| `clippings.groupBySubTag` | `view == clippings-view && clippings-grouped-by-sub-tag == false && clippings-show-group-by-sub-tag-button == true && clippings-has-sub-tags == true` | `navigation@6` |
| `clippings.ungroupBySubTag` | `view == clippings-view && clippings-grouped-by-sub-tag == true && clippings-show-group-by-sub-tag-button == true && clippings-has-sub-tags == true` | `navigation@6` |
| `clippings.filter` | `view == clippings-view && clippings-filtered == false && clippings-show-filter-button == true` | `navigation@7` |
| `clippings.filterClear` | `view == clippings-view && clippings-filtered == true && clippings-show-filter-button == true` | `navigation@7` |
| `clippings.refresh` | `view == clippings-view && clippings-show-refresh-button == true` | `navigation@8` |
| `clippings.expand` | `view == clippings-view && clippings-expanded == false && clippings-show-expand-button == true` | `navigation@9` |
| `clippings.collapse` | `view == clippings-view && clippings-expanded == true && clippings-show-expand-button == true` | `navigation@9` |

### 2.2 `view/item/context` (26 entries)

| clippings command | when (rewritten) | group |
|---|---|---|
| `clippings.filter` | `view == clippings-view && clippings-filtered == false` | `1-filters@1` |
| `clippings.filterClear` | `view == clippings-view && clippings-global-filter-active` | `1-filters@2` |
| `clippings.excludeThisFolder` | `view == clippings-view && viewItem == folder` | `1-filters@3` |
| `clippings.excludeThisFile` | `view == clippings-view && viewItem == file` | `1-filters@4` |
| `clippings.showOnlyThisFolder` | `view == clippings-view && viewItem == folder` | `1-filters@5` |
| `clippings.showOnlyThisFolderAndSubfolders` | `view == clippings-view && viewItem == folder` | `1-filters@6` |
| `clippings.removeFilter` | `view == clippings-view && clippings-folder-filter-active` | `1-filters@7` |
| `clippings.resetAllFilters` | `view == clippings-view && clippings-folder-filter-active` | `1-filters@8` |
| `clippings.toggleItemCounts` | `view == clippings-view` | `2-toggles` |
| `clippings.toggleBadges` | `view == clippings-view` | `2-toggles` |
| `clippings.toggleCompactFolders` | `view == clippings-view && clippings-can-toggle-compact-folders == true` | `2-toggles` |
| `clippings.scanOpenFilesOnly` | `view == clippings-view && clippings-scan-mode != 'open files'` | `3-view` |
| `clippings.scanCurrentFileOnly` | `view == clippings-view && clippings-scan-mode != 'current file'` | `3-view` |
| `clippings.scanWorkspaceAndOpenFiles` | `view == clippings-view && clippings-scan-mode != 'workspace'` | `3-view` |
| `clippings.scanWorkspaceOnly` | `view == clippings-view && clippings-scan-mode != 'workspace only'` | `3-view` |
| `clippings.expand` | `view == clippings-view && clippings-expanded == false` | `4-tree@1` |
| `clippings.collapse` | `view == clippings-view && clippings-expanded == true` | `4-tree@1` |
| `clippings.showFlatView` | `view == clippings-view && clippings-flat == false` | `4-tree@2` |
| `clippings.showTagsOnlyView` | `view == clippings-view && clippings-tags-only == false` | `4-tree@3` |
| `clippings.showTreeView` | `view == clippings-view && clippings-flat == true \|\| clippings-tags-only == true` | `4-tree@4` |
| `clippings.groupByTag` | `view == clippings-view && clippings-grouped-by-tag == false` | `4-tree@5` |
| `clippings.ungroupByTag` | `view == clippings-view && clippings-grouped-by-tag == true` | `4-tree@5` |
| `clippings.groupBySubTag` | `view == clippings-view && clippings-grouped-by-sub-tag == false && clippings-has-sub-tags == true` | `4-tree@6` |
| `clippings.ungroupBySubTag` | `view == clippings-view && clippings-grouped-by-sub-tag == true && clippings-has-sub-tags == true` | `4-tree@6` |
| `clippings.exportTree` | `view == clippings-view` | `5-misc1` |
| `clippings.reveal` | `view == clippings-view` | `6-misc2` |

Note the precedence bug carried verbatim from source: `clippings.showTreeView`'s when-clause (`4-tree@4`) has no parenthesisation — `view == clippings-view && clippings-flat == true || clippings-tags-only == true` binds as `(view == clippings-view && clippings-flat == true) || (clippings-tags-only == true)`, so the item can appear on a context menu for a different view if `tags-only` happens to be true. This is copied byte-for-byte from todo-tree's own `when` clause (`view =~ /todo-tree/ && todo-tree-flat == true || todo-tree-tags-only == true`), which has the identical bug (inventory doesn't call this out explicitly, but it is visible directly in the source). Kept as-is per the "documented behaviour is reproduced, undocumented bugs are fixed" split (spec §11) — this is a manifest-level `when`-clause quirk, not covered by any Rust core module, so whether to fix it is a manifest/extension-layer decision, flagged in Notes.

### 2.3 `commandPalette` (4 entries, all `when: "false"` — hides these from the palette)

| clippings command | when |
|---|---|
| `clippings.showOnlyThisFolder` | `false` |
| `clippings.showOnlyThisFolderAndSubfolders` | `false` |
| `clippings.excludeThisFolder` | `false` |
| `clippings.excludeThisFile` | `false` |

Every other declared command (all 30 remaining ported commands, plus the 3 new ones, which have no `commandPalette` override) is palette-visible by VS Code's default rule (a declared command with a title is palette-visible unless overridden).

---

## 3. Keybindings

Todo-tree contributes **no keybindings** — `contributes.keybindings` is absent from both the fetched 0.0.224 `package.json` and the installed 0.0.226 copy (confirmed: `grep -c '"keybindings"' package.json` → 0; also confirmed by inventory §1.1 "Keybindings / viewsWelcome / colors | none contributed"). Clippings therefore contributes none either. Spec §7 does not add any.

---

## 4. viewsContainers and views

Todo-tree (`contributes.viewsContainers.activitybar[0]` / `contributes.views["todo-tree-container"][0]`):

```json
{
  "id": "todo-tree-container",
  "title": "%todo-tree.activitybar.title%",   // resolved: "TODOs"
  "icon": "resources/todo-tree-container.svg"
}
```
```json
{
  "id": "todo-tree-view",
  "name": "%todo-tree.container.name%",        // resolved: "TODOs"
  "when": "!todo-tree-is-empty"
}
```

Clippings, per spec §7.1 ("An activity bar view container `clippings` with one view `clippings-view`, whose `when` clause is `!clippings-is-empty`"):

```json
{
  "id": "clippings",
  "title": "TODOs",
  "icon": "resources/clippings-container.svg"
}
```
```json
{
  "id": "clippings-view",
  "name": "TODOs",
  "when": "!clippings-is-empty"
}
```

The resolved title/name text "TODOs" is generic (it never said "Todo Tree"), so it is carried unchanged per spec §7.2's "say Clippings where todo-tree said Todo Tree" rule — there is nothing to rewrite.

**Activity bar icon**: todo-tree uses `resources/todo-tree-container.svg`, a 24×24 outline SVG (clipboard-with-checkmark motif, `stroke="black"`, no fill — themes recolor it via the standard activity-bar icon mask). Full source is reproduced in §7 below. For clippings this asset is copied to `resources/clippings-container.svg` (see §7 for licensing).

---

## 5. Settings

Todo-tree declares **70 settings** across 7 configuration groups (General 13, Highlights 7, Filtering 9, Tree 24, Buttons 9, Regex 6, Ripgrep 2 — counted directly from the fetched 0.0.224 `package.json`, matches inventory §8.1's "70 declared settings (3 deprecated)"). Spec §7.2 carries 61 of them unchanged, drops 9, and adds 4 new ones. **61 + 4 = 65 settings in clippings**, confirmed by rebuilding the configuration groups programmatically from the source JSON and counting (`61 carried`, `4 new`, `65 total` — script output).

**Scope** (spec §6.3): every `clippings.*` setting is declared `"scope": "window"` **except** `clippings.server.path`, which is `"scope": "machine-overridable"` and is additionally listed in the top-level `capabilities.untrustedWorkspaces.restrictedConfigurations` array (outside `contributes`, so not shown in the settings JSON fragments below — see §6's manifest note). Todo-tree itself declares no explicit `"scope"` on any setting except `todo-tree.regex.regex` (`"scope": "language-overridable"`, which inventory §0#19 confirms is **never honoured** — every reader calls `getConfiguration('todo-tree.regex')` with no resource/language argument). VS Code's implicit default scope when none is declared is `window`, so todo-tree's *declared* settings were already effectively window-scoped in practice; clippings makes this explicit everywhere and drops the misleading `language-overridable` scope from `regex.regex` per spec's Non-goals ("Language-overridable `regex.regex`. todo-tree declares it but never honours it").

### 5.1 Carried settings (61) — full schema, grouped as in the assembled manifest

Key renaming is mechanical: `todo-tree.<group>.<name>` → `clippings.<group>.<name>`. Three carried settings' markdown text needed editing beyond the key rename (documented individually below); every other carried setting's schema (type/default/enum/enumDescriptions/items/minimum/etc.) and resolved description text is **copied verbatim** from todo-tree v0.0.224.

#### General (order 1) — 12 carried (`general.debug` dropped, see §5.3)

```json
{
  "clippings.general.automaticGitRefreshInterval": { "default": 0, "markdownDescription": "Polling interval (in seconds) for automatically refreshing the tree when your repository is updated. Set to '0' to disable.", "type": "integer", "scope": "window" },
  "clippings.general.periodicRefreshInterval": { "default": 0, "markdownDescription": "Periodic refresh interval (in minutes) for automatically refreshing the tree. Set to '0' to disable.", "type": "integer", "scope": "window" },
  "clippings.general.revealBehaviour": {
    "default": "start of todo",
    "enum": ["start of line", "start of todo", "end of todo"],
    "markdownDescription": "Sets where the cursor is positioned when revealing a todo.",
    "markdownEnumDescriptions": [
      "Moves the cursor to the start of the line",
      "Moves the cursor to the beginning of the todo",
      "Moves the cursor to the end of the todo"
    ],
    "type": "string", "scope": "window"
  },
  "clippings.general.exportPath": { "default": "~/todo-tree-%Y%m%d-%H%M.txt", "markdownDescription": "Path to use when exporting the tree. Environment variables will be expanded, e.g `${HOME}` and the path is passed through strftime (see <https://github.com/samsonjs/strftime>). Set the extension to `.json` to export as a JSON record.", "type": "string", "scope": "window" },
  "clippings.general.rootFolder": { "default": "", "markdownDescription": "Folder in which to run the search (defaults to the workspace folder).", "type": "string", "scope": "window" },
  "clippings.general.schemes": { "default": ["file", "ssh", "untitled", "vscode-notebook-cell"], "items": { "type": "string" }, "markdownDescription": "Editor schemes to find TODOs in. To find TODOs in settings files, for instance, add `vscode-userdata` or for output windows, add `output`.", "type": "array", "scope": "window" },
  "clippings.general.statusBar": {
    "default": "none",
    "enum": ["none", "total", "tags", "top three", "current file"],
    "markdownDescription": "What to show in the status bar - nothing, total count, counts per tag, top three counts per tag or count of tags in the current file.",
    "markdownEnumDescriptions": [
      "Only show the scanning status in the status bar",
      "Show the total count of tags in the status bar",
      "Show a breakdown of the count of each tag in the status bar",
      "Show the count of the top three tags in the status bar",
      "Show the count of tags in the current file in the status bar"
    ],
    "type": "string", "scope": "window"
  },
  "clippings.general.showIconsInsteadOfTagsInStatusBar": { "default": false, "markdownDescription": "Show icons instead of tags in the status bar", "type": "boolean", "scope": "window" },
  "clippings.general.statusBarClickBehaviour": {
    "default": "reveal",
    "enum": ["cycle", "reveal", "toggle highlights"],
    "markdownDescription": "What to do when the status bar is clicked.",
    "markdownEnumDescriptions": [
      "Toggle between showing total count and the top three tag counts",
      "Reveal the tree view",
      "Toggle highlighting"
    ],
    "type": "string", "scope": "window"
  },
  "clippings.general.tagGroups": { "default": {}, "markdownDescription": "Allows similar tags to be grouped under the same type, e.g. `{ \"FIX\": [\"FIXME\",\"FIXIT\"] }`. *Note: All tags must also be in the `clippings.general.tags` tag list. If a tag group is defined, custom highlights apply to the group, not the tags within the group.*", "type": "object", "additionalProperties": { "type": "array", "items": { "type": "string" } }, "scope": "window" },
  "clippings.general.tags": { "default": ["BUG", "HACK", "FIXME", "TODO", "XXX", "[ ]", "[x]"], "items": { "type": "string" }, "markdownDescription": "List of tags. *Note, if one tag starts with another tag, the longer tag should be specified first to prevent the shorter tag being matched.*", "type": "array", "scope": "window" },
  "clippings.general.showActivityBarBadge": { "default": false, "markdownDescription": "Show a badge in the activity bar indicating the total number of TODOs", "type": "boolean", "scope": "window" }
}
```
*Edited*: `tagGroups`'s description referenced `` `todo-tree.general.tags` `` — rewritten to `` `clippings.general.tags` `` (an in-manifest cross-reference, not a mention of todo-tree's own behaviour).

*Kept as-is despite being misleading*: `general.tags`'s note ("the longer tag should be specified first") describes the README's advice, which inventory §3.1 shows the **code doesn't actually implement** (`getTagRegex` sorts reverse-lexicographically, not by configured order or length) — but spec §11.2 lists "The tag alternation is ordered longest first, as the README promises" as a clippings **fix**, so for clippings this description becomes *true* rather than aspirational. No text change needed, flagged for awareness only.

#### Highlights (order 2) — 7 carried, all unchanged except two cross-references

```json
{
  "clippings.highlights.customHighlight": {
    "default": { "BUG": {"icon":"bug"}, "HACK": {"icon":"tools"}, "FIXME": {"icon":"flame"}, "XXX": {"icon":"x"}, "[ ]": {"icon":"issue-draft"}, "[x]": {"icon":"issue-closed"} },
    "markdownDescription": "Custom configuration for highlighting, [Read more...](https://github.com/Gruntfuggly/todo-tree#highlighting).",
    "type": "object",
    "additionalProperties": {
      "type": "object",
      "properties": {
        "foreground": {"type":"string","format":"color-hex"}, "background": {"type":"string","format":"color-hex"},
        "opacity": {"type":"number"}, "fontWeight": {"type":"string"}, "fontStyle": {"type":"string"},
        "textDecoration": {"type":"string"}, "borderRadius": {"type":"string"}, "icon": {"type":"string"},
        "iconColour": {"type":"string","format":"color-hex"}, "gutterIcon": {"type":"boolean"},
        "rulerColour": {"type":"string","format":"color-hex"}, "rulerOpacity": {"type":"number"},
        "rulerLane": {"type":"string","enum":["none","left","center","right","full"]},
        "type": {"type":"string","enum":["tag","text","tag-and-comment","tag-and-subTag","text-and-comment","line","whole-line","none"]},
        "hideFromTree": {"type":"boolean"}, "hideFromStatusBar": {"type":"boolean"}, "hideFromActivityBar": {"type":"boolean"}
      }
    },
    "scope": "window"
  },
  "clippings.highlights.defaultHighlight": {
    "default": {},
    "markdownDescription": "Default configuration for highlighting. [Read more...](https://github.com/Gruntfuggly/todo-tree#highlighting).",
    "type": "object",
    "properties": { "...": "identical 17-key property set to customHighlight's additionalProperties, listed above" },
    "scope": "window"
  },
  "clippings.highlights.enabled": { "default": true, "markdownDescription": "Set to false to disable highlighting.", "type": "boolean", "scope": "window" },
  "clippings.highlights.highlightDelay": { "default": 500, "markdownDescription": "Delay before highlighting tags within files (milliseconds).", "type": "integer", "scope": "window" },
  "clippings.highlights.useColourScheme": { "default": false, "markdownDescription": "Use a colour scheme to colour the tags. This scheme is applied to the tags in the order of tags. The colours can be modified using `clippings.highlights.foregroundColourScheme` and `clippings.highlights.backgroundColourScheme`. The colour scheme overrides colours in the default highlight, but not the custom highlight.", "type": "boolean", "scope": "window" },
  "clippings.highlights.foregroundColourScheme": { "default": ["white","black","black","white","white","white","black"], "items": {"type":"string"}, "markdownDescription": "A list of colours which is applied to tag highlights in the same order as the tags. Repeats if necessary and is overridden by `clippings.highlights.customHighlight`.", "type": "array", "scope": "window" },
  "clippings.highlights.backgroundColourScheme": { "default": ["red","orange","yellow","green","blue","indigo","violet"], "items": {"type":"string"}, "markdownDescription": "A list of colours which is applied to tag highlights in the same order as the tags. Repeats if necessary and is overridden by `clippings.highlights.customHighlight`.", "type": "array", "scope": "window" }
}
```
*Edited*: `useColourScheme`, `foregroundColourScheme`, `backgroundColourScheme` each had a `` `todo-tree.highlights.*` `` cross-reference rewritten to `` `clippings.highlights.*` ``.

*Left unchanged (flagged in Notes)*: `customHighlight` and `defaultHighlight`'s "[Read more...]" links still point at `https://github.com/Gruntfuggly/todo-tree#highlighting` — todo-tree's own upstream README. Clippings has no README section describing highlighting yet, so redirecting the link would be a decision, not a mechanical rewrite. The **18th** highlight attribute sub-key from inventory §8.4, `iconColor` (US spelling, checked before `iconColour` at `attributes.js:58`), is **not** in the declared JSON schema in either version — it is read by code but undeclared, so it is intentionally absent here too, carried as the same undeclared-but-honoured quirk.

#### Filtering (order 3) — 8 carried + 1 new (`filtering.passGlobsToRipgrep` dropped, see §5.3; `filtering.builtInExcludes` new, see §5.2)

```json
{
  "clippings.filtering.excludedWorkspaces": { "default": [], "items": {"type":"string"}, "markdownDescription": "An array of workspace names to exclude as roots in the tree (wildcards can be used).", "type": "array", "scope": "window" },
  "clippings.filtering.excludeGlobs": { "default": ["**/node_modules/*/**"], "items": {"type":"string"}, "markdownDescription": "Globs for use in limiting search results by exclusion (applied after **includeGlobs**), e.g. `[\"**/*.txt\"]` to ignore all .txt files.", "type": "array", "scope": "window" },
  "clippings.filtering.ignoreGitSubmodules": { "default": false, "markdownDescription": "If true, any subfolders containing a .git file will be ignored when searching.", "type": "boolean", "scope": "window" },
  "clippings.filtering.includedWorkspaces": { "default": [], "items": {"type":"string"}, "markdownDescription": "An array of workspace names to include as roots in the tree (wildcards can be used). An empty array includes all workspace folders.", "type": "array", "scope": "window" },
  "clippings.filtering.includeGlobs": { "default": [], "items": {"type":"string"}, "markdownDescription": "Globs for use in limiting search results by inclusion, e.g. `[\"**/unit-tests/*.js\"]` to only show .js files in unit-tests subfolders.", "type": "array", "scope": "window" },
  "clippings.filtering.includeHiddenFiles": { "default": false, "markdownDescription": "Include hidden files (starting with a .).", "type": "boolean", "scope": "window" },
  "clippings.filtering.scopes": { "default": [], "markdownDescription": "Scopes (sets of globs) that can be switched between", "type": "array", "scope": "window" },
  "clippings.filtering.useBuiltInExcludes": {
    "default": "none",
    "enum": ["none", "file excludes", "search excludes", "file and search excludes"],
    "markdownDescription": "Add VSCode's `files.exclude` and/or `search.exclude` list to the ignored paths.",
    "markdownEnumDescriptions": [
      "Don't used any built in excludes",
      "Use the Files:Exclude setting",
      "Use the Search:Exclude setting",
      "Use the Files:Exclude and the Search:Exclude setting"
    ],
    "type": "string", "scope": "window"
  }
}
```
Note: `filtering.scopes` declares **no `items` schema** in todo-tree (array of untyped items) even though inventory §8.1 describes its actual shape as `{name, includeGlobs, excludeGlobs}` objects — this is copied verbatim (an undocumented-shape quirk of the source, not something to "fix" at the manifest level). `useBuiltInExcludes`'s enum value `"file excludes"` is carried unchanged even though inventory §0#15 shows todo-tree's own `config.js:155` tests for the singular `"file exclude"` and so this value is **dead in todo-tree** — spec §11.2 lists `"useBuiltInExcludes: 'file excludes' works"` as a clippings **fix**, so the enum string itself needs no change, only the matching logic (out of scope for this manifest doc).

#### Tree (order 4) — 21 carried (`showInExplorer`, `showScanOpenFilesOrWorkspaceButton`, `showTagsFromOpenFilesOnly` dropped, see §5.3)

```json
{
  "clippings.tree.autoRefresh": { "default": true, "markdownDescription": "Refresh the tree when files are opened or saved.", "type": "boolean", "scope": "window" },
  "clippings.tree.disableCompactFolders": { "default": false, "markdownDescription": "Prevent the tree from showing compact folders.", "type": "boolean", "scope": "window" },
  "clippings.tree.expanded": { "default": false, "markdownDescription": "When opening new workspaces, show the tree initially fully expanded.", "type": "boolean", "scope": "window" },
  "clippings.tree.filterCaseSensitive": { "default": false, "markdownDescription": "Set to true if you want the view filtering to be case sensitive.", "type": "boolean", "scope": "window" },
  "clippings.tree.flat": { "default": false, "markdownDescription": "When opening new workspaces, show the tree initially as flat list of files.", "type": "boolean", "scope": "window" },
  "clippings.tree.groupedByTag": { "default": false, "markdownDescription": "When opening new workspaces, show the tree initially grouped by tag.", "type": "boolean", "scope": "window" },
  "clippings.tree.groupedBySubTag": { "default": false, "markdownDescription": "When opening new workspaces, show the tree initially grouped by sub tag.", "type": "boolean", "scope": "window" },
  "clippings.tree.hideIconsWhenGroupedByTag": { "default": false, "markdownDescription": "Save some space by hiding the item icons when grouped by tag.", "type": "boolean", "scope": "window" },
  "clippings.tree.hideTreeWhenEmpty": { "default": false, "markdownDescription": "Hide the view if it is empty.", "type": "boolean", "scope": "window" },
  "clippings.tree.labelFormat": { "default": "${tag} ${after}", "markdownDescription": "Format for tree items.", "type": "string", "scope": "window" },
  "clippings.tree.scanAtStartup": { "default": true, "markdownDescription": "Normally the tree is built as soon as the window is opened. If you have a large code base and want to manually start the scan, set this to false.", "type": "boolean", "scope": "window" },
  "clippings.tree.scanMode": {
    "default": "workspace",
    "enum": ["workspace", "open files", "current file", "workspace only"],
    "markdownDescription": "Set this to change which files are scanned.",
    "markdownEnumDescriptions": [
      "Scan the whole workspace (or workspaces) and open file",
      "Scan open files only",
      "Scan the current file only",
      "Scan the workspace but don't refresh files open in the editor"
    ],
    "type": "string", "scope": "window"
  },
  "clippings.tree.showBadges": { "default": true, "markdownDescription": "Show badges and SCM state in the tree view.", "type": "boolean", "scope": "window" },
  "clippings.tree.showCountsInTree": { "default": false, "markdownDescription": "Show counts of TODOs in the tree.", "type": "boolean", "scope": "window" },
  "clippings.tree.showCurrentScanMode": { "default": true, "markdownDescription": "Show the current scan mode at the top of the tree view", "type": "boolean", "scope": "window" },
  "clippings.tree.subTagClickUrl": { "default": "", "markdownDescription": "The URL to open when clicking on a sub tag in the tree. Can include placeholders as defined in `clippings.tree.labelFormat`.", "type": "string", "scope": "window" },
  "clippings.tree.sortTagsOnlyViewAlphabetically": { "default": false, "markdownDescription": "Sort items in the tags only view alphabetically instead of by file and line number.", "type": "boolean", "scope": "window" },
  "clippings.tree.sort": { "default": true, "markdownDescription": "The walker searches using multiple threads to improve performance. The tree is sorted when it is populated so that it stays stable. If you want to use the walker's own (unspecified, non-deterministic) order, set this to false.", "type": "boolean", "scope": "window" },
  "clippings.tree.tagsOnly": { "default": false, "markdownDescription": "When opening new workspaces, show only tag elements in tree.", "type": "boolean", "scope": "window" },
  "clippings.tree.tooltipFormat": { "default": "${filepath}, line ${line}", "markdownDescription": "Tree item tooltip format.", "type": "string", "scope": "window" },
  "clippings.tree.trackFile": { "default": true, "markdownDescription": "Track the current file in the tree view.", "type": "boolean", "scope": "window" }
}
```
*Edited (editorial, not mechanical)*: `subTagClickUrl`'s cross-reference rewritten `todo-tree.tree.labelFormat` → `clippings.tree.labelFormat`. `tree.sort`'s description **rewritten**: source text is `"ripgrep searches using multiple threads to improve performance. The tree is sorted when it is populated so that it stays stable. If you want to use ripgrep's own sort arguments, set this to false."` — clippings has no ripgrep dependency (spec §7.2 drops the whole `ripgrep.*` group; §11.3 "there is no ripgrep binary"), but the underlying justification still holds: spec §5.3 says the walker uses "the `ignore` crate's parallel walker with one worker per available core", so results still arrive out of a stable order without `tree.sort`. Reworded to name "the walker" instead of "ripgrep" and drop the now-meaningless "ripgrep's own sort arguments" clause. **This is the one setting description in the carried set that needed a substantive rewrite, not just a key-prefix fix — flagged in Notes for confirmation.**

#### Buttons (order 5) — 9 carried, all unchanged

```json
{
  "clippings.tree.buttons.reveal": { "default": true, "markdownDescription": "Show a button in the tree view title bar to reveal the current item (only when track file is not enabled).", "type": "boolean", "scope": "window" },
  "clippings.tree.buttons.scanMode": { "default": false, "markdownDescription": "Show a button in the tree view title bar to change the Scan Mode setting.", "type": "boolean", "scope": "window" },
  "clippings.tree.buttons.viewStyle": { "default": true, "markdownDescription": "Show a button in the tree view title bar to change the view style (tree, flat or tags only).", "type": "boolean", "scope": "window" },
  "clippings.tree.buttons.groupByTag": { "default": true, "markdownDescription": "Show a button in the tree view title bar to enable grouping items by tag.", "type": "boolean", "scope": "window" },
  "clippings.tree.buttons.groupBySubTag": { "default": false, "markdownDescription": "Show a button in the tree view title bar to enable grouping items by sub tag.", "type": "boolean", "scope": "window" },
  "clippings.tree.buttons.filter": { "default": true, "markdownDescription": "Show a button in the tree view title bar allowing the tree to be filtered by entering some text.", "type": "boolean", "scope": "window" },
  "clippings.tree.buttons.refresh": { "default": true, "markdownDescription": "Show a refresh button in the tree view title bar.", "type": "boolean", "scope": "window" },
  "clippings.tree.buttons.expand": { "default": true, "markdownDescription": "Show a button in the tree view title bar to expand or collapse the whole tree.", "type": "boolean", "scope": "window" },
  "clippings.tree.buttons.export": { "default": false, "markdownDescription": "Show a button in the tree view title bar to create a file showing the tree content.", "type": "boolean", "scope": "window" }
}
```

#### Regex (order 6) — 4 carried (`ripgrep.ripgrep`, `ripgrep.ripgrepArgs` dropped from this group, see §5.3)

```json
{
  "clippings.regex.regex": { "default": "(//|#|<!--|;|/\\*|^|^[ \\t]*(-|\\d+.))\\s*($TAGS)", "markdownDescription": "Regular expression for matching TODOs. Note: **($TAGS)** will be replaced by the expanded tag list. For some of the extension features to work, **($TAGS)** should be present in the regex, however, the basic functionality should still work if you need to explicitly expand the tag list.", "type": "string", "minLength": 1, "scope": "window" },
  "clippings.regex.regexCaseSensitive": { "default": true, "markdownDescription": "Use a case sensitive regular expression.", "type": "boolean", "scope": "window" },
  "clippings.regex.subTagRegex": { "default": "", "markdownDescription": "Regular expression for processing the text to the right of the tag, e.g. for extracting a sub tag, or removing unwanted characters.", "type": "string", "scope": "window" },
  "clippings.regex.enableMultiLine": { "default": false, "markdownDescription": "Force the regex to match over multiple lines. Allows use of `[\\s\\S]` to match anything including newlines.", "type": "boolean", "scope": "window" }
}
```
`regex.regex` dropped its `"scope": "language-overridable"` (never honoured by todo-tree, and explicitly a Non-goal in spec §1) and gained explicit `"scope": "window"`; `minLength: 1` is carried unchanged.

### 5.2 New settings (4) — spec §7.2

```json
{
  "clippings.filtering.builtInExcludes": {
    "default": [".git",".hg",".svn","node_modules",".pnpm-store",".yarn/cache",".claude",".next",".nuxt",".output",".turbo",".cache",".parcel-cache",".svelte-kit",".angular",".vercel",".sst",".terraform","target","__pycache__",".venv","venv",".gradle",".idea",".vs"],
    "items": { "type": "string" },
    "markdownDescription": "Directory names Clippings never indexes below each scan root, in addition to `clippings.filtering.excludeGlobs` and `.gitignore`. An entry without a `/` matches a directory name at any depth below the root; an entry with a `/` matches a trailing run of path components. Never matched against the scan root itself or its ancestors.",
    "type": "array",
    "scope": "window"
  },
  "clippings.server.path": {
    "default": "",
    "markdownDescription": "Path to a Clippings server binary to use instead of the bundled one. `~` and `${workspaceFolder}` are expanded. When set at workspace scope, the client prompts Allow or Deny once per resolved path before using it.",
    "type": "string",
    "scope": "machine-overridable"
  },
  "clippings.server.logLevel": {
    "default": "info",
    "enum": ["error", "warn", "info", "debug", "trace"],
    "markdownDescription": "Log level for the Clippings server, shown in the Clippings output channel.",
    "type": "string",
    "scope": "window"
  },
  "clippings.trace.server": {
    "default": "off",
    "enum": ["off", "messages", "verbose"],
    "markdownDescription": "Traces the communication between VS Code and the Clippings language server.",
    "type": "string",
    "scope": "window"
  }
}
```

- `filtering.builtInExcludes`'s default is the exact 25-entry list from `crates/clippings-core/src/config.rs:12-38` (`DEFAULT_BUILT_IN_EXCLUDES`), which is byte-identical to spec §5.3's list, in the same order. A unit test at `config.rs` asserts `c.built_in_excludes.len() == 25`.
- `server.path` is additionally declared `restricted` via the top-level `capabilities.untrustedWorkspaces.restrictedConfigurations` array per spec §7.1 (`"capabilities.untrustedWorkspaces": supported "limited", with clippings.server.path as a restricted setting`) — this is a manifest field outside `contributes`, not shown in §6's JSON block, but listed here for completeness.
- `server.logLevel` and `trace.server` descriptions are original (no todo-tree equivalent); `trace.server`'s description follows `vscode-languageclient`'s conventional wording for this exact setting name/shape, per spec §7.2 ("the language client's standard setting").

### 5.3 Dropped settings (9) — spec §7.2, with reasons

| todo-tree key | Type / default (source) | Reason (spec §7.2) |
|---|---|---|
| `ripgrep.ripgrep` | string, `""` | no ripgrep binary |
| `ripgrep.ripgrepArgs` | string, `"--max-columns=1000 --no-config "` | no ripgrep binary |
| `ripgrep.ripgrepMaxBuffer` | integer (KB), `200` | no ripgrep binary |
| `ripgrep.usePatternFile` | boolean, `true` | no ripgrep binary |
| `filtering.passGlobsToRipgrep` | boolean, `true` | globs always apply in the walker |
| `tree.showInExplorer` | boolean, `false`, `deprecationMessage` set | deprecated and unread in todo-tree (only drives the `<189` one-time migration notice, inventory §1.9) |
| `tree.showScanOpenFilesOrWorkspaceButton` | boolean, `false`, `deprecationMessage` set | deprecated and unread in todo-tree |
| `tree.showTagsFromOpenFilesOnly` | boolean, `false`, `deprecationMessage` set | deprecated and unread in todo-tree |
| `general.debug` | boolean, `false` | replaced by `server.logLevel`; the importer (`clippings.importTodoTreeSettings`) maps a `true` value to `logLevel: "debug"` per spec §7.2 |

`70 declared − 9 dropped = 61 carried`. `61 carried + 4 new = 65 total clippings settings`, confirming the spec's stated carried count of 61.

---

## 6. Assembled `contributes` JSON

Assembled from the tables above (commands, menus, viewsContainers, views, configuration — no keybindings section since none exist). Saved verbatim at `docs/research/clippings-contributes.json` and validated:

```
$ python3 -m json.tool docs/research/clippings-contributes.json > /dev/null && echo OK
OK
```

```json
{
  "contributes": {
    "viewsContainers": {
      "activitybar": [
        { "id": "clippings", "title": "TODOs", "icon": "resources/clippings-container.svg" }
      ]
    },
    "views": {
      "clippings": [
        { "id": "clippings-view", "name": "TODOs", "when": "!clippings-is-empty" }
      ]
    },
    "menus": {
      "view/title": [
        { "command": "clippings.exportTree", "when": "view == clippings-view && clippings-show-export-button == true", "group": "navigation@1" },
        { "command": "clippings.reveal", "when": "view == clippings-view && clippings-tags-only == false && clippings-show-reveal-button == true", "group": "navigation@2" },
        { "command": "clippings.scanOpenFilesOnly", "when": "view == clippings-view && clippings-scan-mode == 'workspace only' && clippings-show-scan-mode-button == true", "group": "navigation@3" },
        { "command": "clippings.scanCurrentFileOnly", "when": "view == clippings-view && clippings-scan-mode == 'open files' && clippings-show-scan-mode-button == true", "group": "navigation@3" },
        { "command": "clippings.scanWorkspaceAndOpenFiles", "when": "view == clippings-view && clippings-scan-mode == 'current file' && clippings-show-scan-mode-button == true", "group": "navigation@3" },
        { "command": "clippings.scanWorkspaceOnly", "when": "view == clippings-view && clippings-scan-mode == 'workspace' && clippings-show-scan-mode-button == true", "group": "navigation@3" },
        { "command": "clippings.showFlatView", "when": "view == clippings-view && clippings-flat == false && clippings-tags-only == false && clippings-show-view-style-button == true", "group": "navigation@4" },
        { "command": "clippings.showTagsOnlyView", "when": "view == clippings-view && clippings-flat == true && clippings-tags-only == false && clippings-show-view-style-button == true", "group": "navigation@4" },
        { "command": "clippings.showTreeView", "when": "view == clippings-view && clippings-flat == false && clippings-tags-only == true && clippings-show-view-style-button == true", "group": "navigation@4" },
        { "command": "clippings.groupByTag", "when": "view == clippings-view && clippings-grouped-by-tag == false && clippings-show-group-by-tag-button == true", "group": "navigation@5" },
        { "command": "clippings.ungroupByTag", "when": "view == clippings-view && clippings-grouped-by-tag == true && clippings-show-group-by-tag-button == true", "group": "navigation@5" },
        { "command": "clippings.groupBySubTag", "when": "view == clippings-view && clippings-grouped-by-sub-tag == false && clippings-show-group-by-sub-tag-button == true && clippings-has-sub-tags == true", "group": "navigation@6" },
        { "command": "clippings.ungroupBySubTag", "when": "view == clippings-view && clippings-grouped-by-sub-tag == true && clippings-show-group-by-sub-tag-button == true && clippings-has-sub-tags == true", "group": "navigation@6" },
        { "command": "clippings.filter", "when": "view == clippings-view && clippings-filtered == false && clippings-show-filter-button == true", "group": "navigation@7" },
        { "command": "clippings.filterClear", "when": "view == clippings-view && clippings-filtered == true && clippings-show-filter-button == true", "group": "navigation@7" },
        { "command": "clippings.refresh", "when": "view == clippings-view && clippings-show-refresh-button == true", "group": "navigation@8" },
        { "command": "clippings.expand", "when": "view == clippings-view && clippings-expanded == false && clippings-show-expand-button == true", "group": "navigation@9" },
        { "command": "clippings.collapse", "when": "view == clippings-view && clippings-expanded == true && clippings-show-expand-button == true", "group": "navigation@9" }
      ],
      "view/item/context": [
        { "command": "clippings.filter", "when": "view == clippings-view && clippings-filtered == false", "group": "1-filters@1" },
        { "command": "clippings.filterClear", "when": "view == clippings-view && clippings-global-filter-active", "group": "1-filters@2" },
        { "command": "clippings.excludeThisFolder", "when": "view == clippings-view && viewItem == folder", "group": "1-filters@3" },
        { "command": "clippings.excludeThisFile", "when": "view == clippings-view && viewItem == file", "group": "1-filters@4" },
        { "command": "clippings.showOnlyThisFolder", "when": "view == clippings-view && viewItem == folder", "group": "1-filters@5" },
        { "command": "clippings.showOnlyThisFolderAndSubfolders", "when": "view == clippings-view && viewItem == folder", "group": "1-filters@6" },
        { "command": "clippings.removeFilter", "when": "view == clippings-view && clippings-folder-filter-active", "group": "1-filters@7" },
        { "command": "clippings.resetAllFilters", "when": "view == clippings-view && clippings-folder-filter-active", "group": "1-filters@8" },
        { "command": "clippings.toggleItemCounts", "when": "view == clippings-view", "group": "2-toggles" },
        { "command": "clippings.toggleBadges", "when": "view == clippings-view", "group": "2-toggles" },
        { "command": "clippings.toggleCompactFolders", "when": "view == clippings-view && clippings-can-toggle-compact-folders == true", "group": "2-toggles" },
        { "command": "clippings.scanOpenFilesOnly", "when": "view == clippings-view && clippings-scan-mode != 'open files'", "group": "3-view" },
        { "command": "clippings.scanCurrentFileOnly", "when": "view == clippings-view && clippings-scan-mode != 'current file'", "group": "3-view" },
        { "command": "clippings.scanWorkspaceAndOpenFiles", "when": "view == clippings-view && clippings-scan-mode != 'workspace'", "group": "3-view" },
        { "command": "clippings.scanWorkspaceOnly", "when": "view == clippings-view && clippings-scan-mode != 'workspace only'", "group": "3-view" },
        { "command": "clippings.expand", "when": "view == clippings-view && clippings-expanded == false", "group": "4-tree@1" },
        { "command": "clippings.collapse", "when": "view == clippings-view && clippings-expanded == true", "group": "4-tree@1" },
        { "command": "clippings.showFlatView", "when": "view == clippings-view && clippings-flat == false", "group": "4-tree@2" },
        { "command": "clippings.showTagsOnlyView", "when": "view == clippings-view && clippings-tags-only == false", "group": "4-tree@3" },
        { "command": "clippings.showTreeView", "when": "view == clippings-view && clippings-flat == true || clippings-tags-only == true", "group": "4-tree@4" },
        { "command": "clippings.groupByTag", "when": "view == clippings-view && clippings-grouped-by-tag == false", "group": "4-tree@5" },
        { "command": "clippings.ungroupByTag", "when": "view == clippings-view && clippings-grouped-by-tag == true", "group": "4-tree@5" },
        { "command": "clippings.groupBySubTag", "when": "view == clippings-view && clippings-grouped-by-sub-tag == false && clippings-has-sub-tags == true", "group": "4-tree@6" },
        { "command": "clippings.ungroupBySubTag", "when": "view == clippings-view && clippings-grouped-by-sub-tag == true && clippings-has-sub-tags == true", "group": "4-tree@6" },
        { "command": "clippings.exportTree", "when": "view == clippings-view", "group": "5-misc1" },
        { "command": "clippings.reveal", "when": "view == clippings-view", "group": "6-misc2" }
      ],
      "commandPalette": [
        { "command": "clippings.showOnlyThisFolder", "when": "false" },
        { "command": "clippings.showOnlyThisFolderAndSubfolders", "when": "false" },
        { "command": "clippings.excludeThisFolder", "when": "false" },
        { "command": "clippings.excludeThisFile", "when": "false" }
      ]
    },
    "commands": [
      { "command": "clippings.showFlatView", "title": "Show Flat View", "category": "Clippings", "icon": "$(list-unordered)" },
      { "command": "clippings.showTagsOnlyView", "title": "Show Tags Only View", "category": "Clippings", "icon": "$(list-flat)" },
      { "command": "clippings.showTreeView", "title": "Show Tree View", "category": "Clippings", "icon": "$(list-tree)" },
      { "command": "clippings.refresh", "title": "Refresh", "category": "Clippings", "icon": "$(refresh)" },
      { "command": "clippings.expand", "title": "Expand Tree", "category": "Clippings", "icon": "$(expand-all)", "enablement": "clippings-collapsible" },
      { "command": "clippings.collapse", "title": "Collapse Tree", "category": "Clippings", "icon": "$(collapse-all)", "enablement": "clippings-collapsible" },
      { "command": "clippings.filter", "title": "Filter Tree", "category": "Clippings", "icon": "$(filter)" },
      { "command": "clippings.filterClear", "title": "Clear Tree Filter", "category": "Clippings", "icon": "$(clear-all)" },
      { "command": "clippings.groupByTag", "title": "Group by Tag", "category": "Clippings", "icon": "$(tag)" },
      { "command": "clippings.ungroupByTag", "title": "Ungroup by Tag", "category": "Clippings", "icon": "$(unfold)" },
      { "command": "clippings.groupBySubTag", "title": "Group by Sub Tag", "category": "Clippings", "icon": "$(chrome-restore)" },
      { "command": "clippings.ungroupBySubTag", "title": "Ungroup by Sub Tag", "category": "Clippings", "icon": "$(chrome-maximize)" },
      { "command": "clippings.scanOpenFilesOnly", "title": "Scan Open Files Only", "category": "Clippings", "icon": "$(files)" },
      { "command": "clippings.scanCurrentFileOnly", "title": "Scan Current File Only", "category": "Clippings", "icon": "$(symbol-file)" },
      { "command": "clippings.scanWorkspaceAndOpenFiles", "title": "Scan Workspace And Open Files", "category": "Clippings", "icon": "$(folder-active)" },
      { "command": "clippings.scanWorkspaceOnly", "title": "Scan Workspace Only", "category": "Clippings", "icon": "$(folder)" },
      { "command": "clippings.addTag", "title": "Add Tag", "category": "Clippings" },
      { "command": "clippings.removeTag", "title": "Remove Tag", "category": "Clippings" },
      { "command": "clippings.exportTree", "title": "Export Tree", "category": "Clippings", "icon": "$(save)" },
      { "command": "clippings.showOnlyThisFolder", "title": "Only Show This Folder", "category": "Clippings", "icon": "$(filter)" },
      { "command": "clippings.showOnlyThisFolderAndSubfolders", "title": "Only Show This Folder And Subfolders", "category": "Clippings", "icon": "$(filter)" },
      { "command": "clippings.switchScope", "title": "Switch Scope", "category": "Clippings", "icon": "$(filter)" },
      { "command": "clippings.excludeThisFolder", "title": "Hide This Folder", "category": "Clippings", "icon": "$(filter)" },
      { "command": "clippings.excludeThisFile", "title": "Hide This File", "category": "Clippings", "icon": "$(filter)" },
      { "command": "clippings.removeFilter", "title": "Remove Filter", "category": "Clippings", "icon": "$(filter)" },
      { "command": "clippings.resetAllFilters", "title": "Reset All Filters", "category": "Clippings", "icon": "$(clear-all)" },
      { "command": "clippings.reveal", "title": "Reveal Current File In Tree", "category": "Clippings", "icon": "$(location)" },
      { "command": "clippings.resetCache", "title": "Reset Cache", "category": "Clippings" },
      { "command": "clippings.toggleItemCounts", "title": "Toggle Item Counts", "category": "Clippings" },
      { "command": "clippings.toggleBadges", "title": "Toggle Badges", "category": "Clippings" },
      { "command": "clippings.toggleCompactFolders", "title": "Toggle Compact Folders", "category": "Clippings" },
      { "command": "clippings.goToNext", "title": "Go To Next", "category": "Clippings" },
      { "command": "clippings.goToPrevious", "title": "Go To Previous", "category": "Clippings" },
      { "command": "clippings.revealInFile", "title": "Reveal In File", "category": "Clippings" },
      { "command": "clippings.importTodoTreeSettings", "title": "Import Settings from Todo Tree", "category": "Clippings" },
      { "command": "clippings.restartServer", "title": "Restart Server", "category": "Clippings" },
      { "command": "clippings.showLog", "title": "Show Log", "category": "Clippings" }
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
  }
}
```

This block is identical to `docs/research/clippings-contributes.json` (validated above), reproduced in full here per "ready-to-use `contributes` JSON block". Group placement for the new settings (`builtInExcludes` in the existing "Filtering" group; `server.path`/`server.logLevel`/`trace.server` in a new "Server" group, order 7, taking the numeric slot vacated by the dropped "Ripgrep" group) is an editorial choice made for this doc, not dictated by the spec — flagged in Notes.

---

## 7. Resources

`resources/` files that `contributes` (menus/views) or the runtime icon code (`icons.js`) reference, with licence:

| File | Referenced by | Licence | Copy for clippings? |
|---|---|---|---|
| `resources/todo-tree-container.svg` | `contributes.viewsContainers.activitybar[0].icon` (declarative) | MIT (top-level `License.txt`, Copyright (c) Nigel Scott) | Yes — copy to `resources/clippings-container.svg`, attribute per MIT (retain/accompany with the license text) |
| `resources/todo-tree.png` | top-level `package.json` `"icon"` field (Marketplace listing icon; **not** inside `contributes`) | MIT (same) | Yes if clippings wants a Marketplace icon derived from todo-tree's; otherwise clippings needs its own |
| `resources/icons/light/todo-green.svg` | `icons.js:17` (`lightIconPath`), runtime fallback gutter icon, **not declared in package.json** | **CC BY-ND 3.0** (ModernUIIcons.com / Austin Andrews (@templarian) — see `resources/icons/license.txt`), **not MIT** | Caution — see Notes; the file can likely be copied verbatim (unmodified) but the "No Derived Works" term is in tension with any recoloring |
| `resources/icons/dark/todo-green.svg` | `icons.js:16` (`darkIconPath`), same fallback role | same CC BY-ND 3.0 licence, same file/folder | same caution |
| `resources/icons/light/*.png` (11 files: `clear-filter`, `collapse`, `expand`, `filter`, `flat`, `notag`, `refresh`, `reveal`, `scan-open-files`, `scan-workspace`, `tag`, `tags`, `tree`) + `resources/icons/light/refresh.svg` | **not referenced** anywhere in `dist/extension.js` (grepped; zero hits) — dead assets from a pre-codicon UI generation, still shipped in the VSIX | same CC BY-ND 3.0 licence (all under `resources/icons/`, covered by the one `license.txt` in that folder) | No — unused by current (0.0.224+) behaviour, no need to carry over, and they're the least clear licence-wise |
| `resources/button-icons/*.png` (16 files) | **not referenced** anywhere in `dist/extension.js` (grepped; zero hits) — likely an even older pre-codicon command-icon set | No separate licence file in this folder; presumed covered by the top-level MIT licence, unconfirmed | No — unused, and provenance is less certain than the top-level assets |
| `resources/screenshot.png` | README only, not `contributes`, not runtime code | MIT (presumed, own screenshot) | No — not part of the manifest surface |

**Icon generation not backed by any `resources/` file**: the `todo-tree` / `todo-tree-filled` icon names (inventory §0#17's validation bug) and octicon-backed icons (via the `@primer/octicons` npm dependency, MIT-licensed, not a `resources/` asset) are generated as inline SVG strings at runtime in `icons.js` and cached under `context.globalStorageUri`, never shipped as static files. Per spec §7.7 this generation moves to the extension's icon-rendering code ("Octicons, todo-tree's own icons and the check-circle icon are rendered to SVG once per name and colour under global storage"), so clippings needs the SVG **path data**, not `resources/` files, for `todo-tree`/`todo-tree-filled`/the default check-circle icon — these are reproduced faithfully in `icons.js`'s source (fetched from `a6f60e0/src/icons.js`) and are part of the extension's own MIT-licensed code, not third-party assets.

---

## Notes (ambiguities and inconsistencies needing a decision)

1. **`view =~ /todo-tree/` → `view == clippings-view` changes the operator, not just the operand.** The task's rewrite rule as stated ("`view == todo-tree-view` becomes `view == clippings-view`") assumes an equality check, but todo-tree's actual `package.json` uses a regex match `view =~ /todo-tree/` on every menu `when` clause (confirmed directly in both the fetched 0.0.224 and installed 0.0.226 files). Since clippings has exactly one view (`clippings-view`, spec §7.1), I rewrote every occurrence to equality against that single id. This is faithful in effect (today) but is a genuine format change, not a search-and-replace — confirm this is the intended approach, especially if a second view is ever added to the container (the regex form would then need to become `view =~ /^clippings/` or similar rather than a single equality).
2. **`tree.sort`'s description needed a substantive rewrite, not just a key-prefix fix.** Source text explicitly names ripgrep's multi-threaded search and "ripgrep's own sort arguments" — both meaningless for clippings, which has no ripgrep dependency. I reworded it to reference "the walker" per spec §5.3's parallel `ignore`-crate walker, but this is an editorial judgment call on wording, not a verbatim carry. Please review the exact phrasing in §5.1 (Tree group).
3. **Two `customHighlight`/`defaultHighlight` descriptions still link to todo-tree's own upstream README** (`https://github.com/Gruntfuggly/todo-tree#highlighting`). Left unchanged because clippings has no equivalent docs page yet; decide whether to point at a future clippings README section, drop the link, or keep pointing upstream (the highlighting *behaviour* is carried with parity per spec §11.1, so linking to todo-tree's explanation of that behaviour is arguably still accurate).
4. **New-settings group placement is my own organizational choice, not spec-mandated.** I put `filtering.builtInExcludes` in the existing "Filtering" group and created a new "Server" group (order 7, taking the numeric slot vacated by the dropped "Ripgrep" group) for `server.path`, `server.logLevel`, and `trace.server`. The spec's §7.2 table doesn't assign settings-UI group titles/order for the new settings — confirm this grouping and ordering is acceptable, or specify a different one.
5. **`resources/icons/` (both `todo-green.svg` variants, and the 11+1 unused light-theme icon files) are licensed CC BY-ND 3.0 from ModernUIIcons.com / Austin Andrews, not MIT**, despite the task prompt's framing ("todo-tree is MIT, so they can be copied with attribution"). The folder's own `license.txt` says attribution is *not required* ("No Attribution and No Derived Works... * The license is for attribution, but this is not required") but derivative works are nominally restricted. The only file of this set actually used by current (0.0.224+) behaviour is `todo-green.svg` (light+dark), used unmodified as a fallback icon path (not recolored/derived by code) — so a verbatim, unmodified copy is likely fine, but this should get a real legal read before shipping, since it is not covered by todo-tree's MIT grant. The 14 other files under `resources/icons/` and the 16 files under `resources/button-icons/` are confirmed dead code (zero references in `dist/extension.js`) and are recommended **not** to be carried over regardless of licence.
6. **`view/item/context`'s `showTreeView` entry (`4-tree@4`) carries an operator-precedence quirk from source verbatim**: `view == clippings-view && clippings-flat == true || clippings-tags-only == true` evaluates as `(view == clippings-view && clippings-flat == true) || (clippings-tags-only == true)` with no `view` guard on the second disjunct. This is copied byte-for-byte from todo-tree's own (equally unguarded) clause. Spec §11 splits behaviour into "documented behaviour is reproduced, undocumented bugs are fixed" — this is a manifest-level bug inventory doesn't explicitly catalog; decide whether it counts as one to fix or one to reproduce.
7. **`filtering.scopes` has no `items` schema in todo-tree** (bare `"type": "array"`) despite holding structured `{name, includeGlobs, excludeGlobs}` objects per inventory §8.1/§2.10. Carried as-is (undocumented shape, not a behavioural bug in the Rust core's sense), but flagging in case the settings UI would benefit from a real `items` schema in clippings even though todo-tree never had one.
8. **`general.revealBehaviour`'s enum is pinned to 0.0.224's 3 values**, deliberately excluding 0.0.225's `"leave focus in tree"` addition (confirmed absent from the fetched 0.0.224 source, present in installed 0.0.226). If the actual target parity version should track the installed 0.0.226 instead of the spec's pinned 0.0.224, this enum (and its `markdownEnumDescriptions`) would need a 4th value — flagging since the spec text says "commit a6f60e0 (v0.0.224)" explicitly but the environment's installed copy is 0.0.226.
9. **Command `category` text**: todo-tree resolves `%todo-tree.command.category%` to the literal string `"Todo Tree"`, which appears **nowhere else** in any carried setting's markdown text (verified by grep on the nls file) — so the "say Clippings where todo-tree said Todo Tree" substitution in practice affects only this one string across all 37 commands, plus (per spec §7.6) the status-bar cycle's four info messages, which are extension-code strings outside `contributes` and thus outside this manifest doc's scope.
