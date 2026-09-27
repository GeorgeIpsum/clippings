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
