# Clippings Server Implementation Plan (Plan 2 of 4)

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Turn the core into the language server the extension talks to: settings, highlight styles, the tree view model with IDs and deltas, decorations, status, navigation and export, the protocol types, open-document sync, the scheduler with file watching and timers, and the `clippings lsp` and `clippings watch` commands.

**Architecture:** Pure computation lives in `clippings-core` modules that take `Settings` and index data and return values: `styles`, `view` (place, shape, render, delta, export), `decorations`, `status`, `navigate`. The `server` module owns one scheduler (`Server`) that turns client messages, worker results and timers into outgoing JSON-RPC notifications, over an `lsp-server` connection. Full walks and git polling run on worker threads; everything else runs on the scheduler thread under `catch_unwind`. File watching uses VS Code's watcher through dynamic registration, with a `notify` fallback.

**Tech Stack:** Rust 2021, minimum 1.85. New crates: lsp-server 0.10, crossbeam-channel 0.5.17, notify 8.2, chrono 0.4.45 (clock and std features only), serde_json with `preserve_order`. Existing: everything from plan 1.

**Spec:** `docs/superpowers/specs/2026-09-23-clippings-design.md`. This plan implements sections 5.7, 5.10 to 5.15, 6, 10.1 and 12.4, and the extension-facing half of 13. The last task records this plan's rulings in the spec.

**Builds on:** plan 1, merged on `main` at `cc06bad`. Its modules (`config`, `globs`, `roots`, `ignore_rules`, `admission`, `pattern`, `position`, `comments`, `extract`, `model`, `scanner`, `walker`, `index`, `report`) are used as they are, with the small additions in Task 1.

## Global Constraints

- `cargo fmt --all` must leave no diff, and `cargo clippy --all-targets -- -D warnings` must pass before every commit. Tests run with `INSTA_UPDATE=no` unless a task says otherwise.
- Commit messages: the subject line from the task, a blank line, then your model attribution trailer (`Co-Authored-By: ...`). Use the heredoc form shown in each task so the blank line is kept.
- `PROTOCOL_VERSION` stays `1`. The server rejects any other version at `initialize`.
- Positions on the wire are 0-based line and 0-based UTF-16 column.
- Timing constants: view rebuilds coalesce for 50 ms, file events for 50 ms, a buffer is rescanned for the tree 150 ms after its last edit, and decorations follow `highlights.highlightDelay` (default 500 ms).
- The server echoes the exact URI string it received in `didOpen` in every per-document message. URIs it produces itself use `uri::file_uri`, which matches VS Code's encoding.
- Node IDs are strings: a node's ID is its parent's ID, `/`, and its own key (spec 5.12), except that a todo without a file parent uses `t:<uri>:<line>:<column>`.
- Settings defaults equal todo-tree v0.0.224 (inventory §8.1).
- Performance targets checked in Task 16, on the maintainer's machine: decorations for a 50,000-line buffer under 20 ms, and a view rebuild plus diff with 10,000 todos under 15 ms.

## Review Focus

These inputs are implied by the spec but easy to miss. Each has a test in the task that owns the code.

1. **Edits that mix multi-byte characters with UTF-16 ranges** must apply exactly, or buffers drift from the editor. Test `incremental_edits_in_utf16` in Task 12.
2. **Paths with spaces, brackets and non-ASCII characters** must become URIs exactly as VS Code writes them and round-trip back to paths. Tests `encodes_like_vscode` and `round_trips` in Task 2.
3. **An invalid regex typed into settings mid-session** must report an error without clearing the tree. Test `invalid_regex_reports_an_error_and_keeps_the_tree` in Task 14.
4. **A deleted folder arrives as one event**, and every todo beneath it must disappear. Test `file_events_update_the_tree` in Task 14.
5. **A client without dynamic watcher registration**, such as another editor, must still see disk changes. Test `without_dynamic_registration_notify_watches_the_disk` in Task 14.

## Rulings made while prototyping this plan

Each was taken where the spec was silent or unworkable. Task 17 writes them into the spec.

1. Todos without a file parent, in the tags-only view, use `t:<uri>:<line>:<column>`, because `t:<line>:<column>` would collide across files.
2. `Todo` gains `textEnd`, the end of the line the match starts on, so `revealBehaviour: "end of todo"` has a real target.
3. Buffer scans, view builds and decoration computation run on the scheduler thread under `catch_unwind`. Only full walks and git polling use worker threads. They are fast enough, and panic handling is unchanged.
4. Icon-name validation happens in the extension, which owns the octicon set. The server reports colour and placeholder warnings.
5. The auto-contrast foreground applies to hex and `rgb()` backgrounds. Named colours such as `red` get none, as in todo-tree, because mapping 147 names to RGB values adds a table for little gain.
6. The view rebuild target is 15 ms at 10,000 todos, not 5 ms. The prototype measured 12.5 ms on an M4 Pro. Sharing todo data between the index and the view would get lower, and is left to plan 4's benchmark work.
7. With grouping by both tag and sub-tag, tag grouping wins and no sub-tag level or pseudo-folder is created, as in todo-tree.
8. Status bar tooltips read `Clippings total`, `Clippings tags counts`, `Clippings tags counts in current file` and `Clippings top three tag counts`, and the badge tooltip is `N todos`. Status bar spacing matches todo-tree exactly, including its double spaces.

---

### Task 1: Dependencies, line-end positions and pattern flags

Small additions to plan 1's modules that later tasks rely on, and the golden snapshots re-recorded for the new `textEnd` field.

**Files:**
- Modify: `Cargo.toml`, `crates/clippings-core/Cargo.toml`, `crates/clippings/Cargo.toml`
- Modify: `crates/clippings-core/src/model.rs`, `crates/clippings-core/src/scanner.rs`, `crates/clippings-core/src/position.rs`, `crates/clippings-core/src/pattern.rs`, `crates/clippings-core/src/index.rs` (test helper), `crates/clippings-core/src/walker.rs` (test)
- Modify (re-recorded): `crates/clippings-core/tests/snapshots/golden__default_scan.snap`, `crates/clippings-core/tests/snapshots/golden__sub_tag_multi_line_scan.snap`

**Interfaces:**
- Produces: `Todo.text_end: Position` (serde `textEnd`), the end of the line on which the match starts.
- Produces: `position::Range { start: Position, end: Position }` (serde, `Ord`), `LineIndex::offset(&self, Position) -> usize` (UTF-16 position to byte offset, clamped to the line), `LineIndex::line_end(&self, line: usize) -> Position`, `LineIndex::line_count(&self) -> usize`.
- Produces: `ScanPattern.flags: String` (`(?m)` plus `i`/`s`), and `pattern::flag_prefix` becomes `pub`.

- [ ] **Step 1: Add the dependencies**

In the root `Cargo.toml` `[workspace.dependencies]`, add these lines and change `serde_json`:

```toml
crossbeam-channel = "0.5.17"
lsp-server = "0.10"
notify = "8.2"
chrono = { version = "0.4.45", default-features = false, features = ["clock", "std"] }
serde_json = { version = "1", features = ["preserve_order"] }
```

In `crates/clippings-core/Cargo.toml` `[dependencies]`, add:

```toml
chrono.workspace = true
crossbeam-channel.workspace = true
lsp-server.workspace = true
notify.workspace = true
```

In `crates/clippings/Cargo.toml` `[dev-dependencies]`, add:

```toml
lsp-server.workspace = true
```

Run: `cargo build`. Expected: dependencies download and the workspace builds.

- [ ] **Step 2: Write the failing tests**

Add this test to the tests module in `crates/clippings-core/src/position.rs`:

```rust
    #[test]
    fn offset_inverts_position() {
        let t = "ééé // TODO\n😀 x".as_bytes();
        let li = LineIndex::new(t);
        for off in [0, 2, 4, 7, 11, 14, 16, 20] {
            if std::str::from_utf8(&t[..off]).is_ok() {
                assert_eq!(li.offset(li.position(off)), off, "offset {off}");
            }
        }
        assert_eq!(li.line_end(0), Position { line: 0, character: 11 });
        assert_eq!(li.offset(Position { line: 0, character: 99 }), 14, "clamped to line end");
    }
```

Add this test to the tests module in `crates/clippings-core/src/walker.rs` (a plan 1 deferred item):

```rust
    #[test]
    fn a_cancelled_walk_reports_it_and_scans_nothing() {
        let t = tempfile::tempdir().unwrap();
        std::fs::write(t.path().join("a.ts"), "// TODO\n").unwrap();
        let cfg = CoreConfig::default();
        let p = crate::pattern::build(&cfg).unwrap();
        let out = walk_and_scan(&cfg, &[t.path().to_path_buf()], &p, Arc::new(NativeFs), &AtomicBool::new(true)).unwrap();
        assert!(out.cancelled);
        assert!(out.files.is_empty() && out.seen.is_empty());
    }
```

Run: `cargo test -p clippings-core position walker`
Expected: compile error, `offset` and `line_end` not found. (The walker test passes once it compiles; it pins existing behaviour.)

- [ ] **Step 3: Add `Range`, `offset`, `line_end` and `line_count`**

In `crates/clippings-core/src/position.rs`, add above `/// Line start offsets of a UTF-8 text.`:

```rust
/// A half-open range of positions.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct Range {
    pub start: Position,
    pub end: Position,
}
```

and add these methods inside `impl<'a> LineIndex<'a>`, before `pub fn position`:

```rust
    /// Number of lines (a text ending in a newline has an empty last line).
    pub fn line_count(&self) -> usize {
        self.starts.len()
    }

    /// Byte offset of a position, clamped to its line. The inverse of `position`.
    pub fn offset(&self, pos: Position) -> usize {
        let line = (pos.line as usize).min(self.starts.len() - 1);
        let (start, end) = self.line_range(line);
        let text = String::from_utf8_lossy(&self.text[start..end]);
        let mut units = 0usize;
        for (i, ch) in text.char_indices() {
            if units >= pos.character as usize {
                return start + i;
            }
            units += ch.len_utf16();
        }
        end
    }

    /// Position of the end of a line's content.
    pub fn line_end(&self, line: usize) -> Position {
        let line = line.min(self.starts.len() - 1);
        self.position(self.line_range(line).1)
    }
```

- [ ] **Step 4: Add `textEnd` to `Todo`**

In `crates/clippings-core/src/model.rs`, add after the `end` field of `Todo`:

```rust
    /// End of the line on which the match starts: where "reveal at end of todo" lands.
    pub text_end: Position,
```

In `crates/clippings-core/src/scanner.rs`, in the `out.push(Todo { ... })` of `todos_from_lines`, add after `end: shift(li.position(e)),`:

```rust
            text_end: shift(li.position(first_end.max(s))),
```

In the `todo` helper of the tests module in `crates/clippings-core/src/index.rs`, add `text_end: p,` after `end: p,`.

- [ ] **Step 5: Expose the pattern flags**

In `crates/clippings-core/src/pattern.rs`: make `fn flag_prefix` `pub`, add this field at the end of `ScanPattern`:

```rust
    /// Inline flags for the source: `(?m)` plus `i` and `s` as configured.
    pub flags: String,
```

and set it in `build`'s returned struct: `flags: flag_prefix(cfg),`.

- [ ] **Step 6: Re-record and review the golden snapshots**

Run: `INSTA_UPDATE=always cargo test -p clippings-core --test golden`, then `git diff --stat crates/clippings-core/tests/snapshots`.

Expected: both snapshots change only by added `textEnd` objects. Check that no line was removed:

```bash
git diff crates/clippings-core/tests/snapshots | grep -c '^-  '   # must print 0
```

Then run the whole suite: `INSTA_UPDATE=no cargo test --all`. Expected: all pass.
Then run: `cargo fmt --all && cargo clippy --all-targets -- -D warnings`
Expected: no diffs, no warnings.

- [ ] **Step 7: Commit**

```bash
git add -A
git commit -F - <<'EOF'
feat(core): line-end positions, ranges and pattern flags for the server

Co-Authored-By: <your model attribution>
EOF
```

### Task 2: File URIs

Implements spec section 5.7's URI rules: `file:` URIs byte-identical to VS Code's `URI.file(path).toString()`, and the reverse for `file:` and `vscode-notebook-cell:` URIs.

**Files:**
- Create: `crates/clippings-core/src/uri.rs`
- Modify: `crates/clippings-core/src/lib.rs` (add `pub mod uri;`, keeping the declarations sorted)

**Interfaces:**
- Produces: `file_uri(&Path) -> String`, `percent_decode(&str) -> String`, `uri_path(&str) -> Option<String>`, `scheme(&str) -> Option<String>` (lower-cased), `uri_to_path(&str) -> Option<PathBuf>` (file and notebook-cell schemes only), `uri_basename(&str) -> String`.

- [ ] **Step 1: Write the failing tests**

Add `pub mod uri;` to `crates/clippings-core/src/lib.rs`. Create `crates/clippings-core/src/uri.rs` containing only this test module:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encodes_like_vscode() {
        assert_eq!(
            file_uri(Path::new("/Users/g1n/a b/c.ts")),
            "file:///Users/g1n/a%20b/c.ts"
        );
        assert_eq!(
            file_uri(Path::new("/r/app/[slug]/page.tsx")),
            "file:///r/app/%5Bslug%5D/page.tsx"
        );
        assert_eq!(file_uri(Path::new("/r/é.ts")), "file:///r/%C3%A9.ts");
        assert_eq!(
            file_uri(Path::new("C:\\Users\\x\\a.ts")),
            "file:///c%3A/Users/x/a.ts"
        );
    }

    #[test]
    fn decodes_file_uris() {
        assert_eq!(
            uri_to_path("file:///r/app/%5Bslug%5D/page.tsx"),
            Some(PathBuf::from("/r/app/[slug]/page.tsx"))
        );
        assert_eq!(
            uri_to_path("file:///c%3A/Users/x/a.ts"),
            Some(PathBuf::from("C:\\Users\\x\\a.ts"))
        );
        assert_eq!(
            uri_to_path("vscode-notebook-cell:/r/n.ipynb#W0sZmlsZQ%3D%3D"),
            Some(PathBuf::from("/r/n.ipynb"))
        );
        assert_eq!(uri_to_path("untitled:Untitled-1"), None);
    }

    #[test]
    fn round_trips() {
        for p in ["/a/b c/d.rs", "/x/%/y", "/q/[a]/(b)/c.ts"] {
            assert_eq!(uri_to_path(&file_uri(Path::new(p))), Some(PathBuf::from(p)));
        }
    }

    #[test]
    fn basenames() {
        assert_eq!(uri_basename("untitled:Untitled-1"), "Untitled-1");
        assert_eq!(uri_basename("file:///r/a%20b.ts"), "a b.ts");
    }
}
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p clippings-core uri`
Expected: compile error, `file_uri` and `uri_to_path` not found.

- [ ] **Step 3: Write the implementation**

Put this above the test module in `crates/clippings-core/src/uri.rs`:

```rust
//! File URIs in the exact form VS Code produces (`URI.file(path).toString()`),
//! and the reverse (spec section 5.7).

use std::path::{Path, PathBuf};

fn unreserved(b: u8) -> bool {
    b.is_ascii_alphanumeric() || matches!(b, b'-' | b'.' | b'_' | b'~')
}

fn encode_segment(out: &mut String, s: &str) {
    for &b in s.as_bytes() {
        if unreserved(b) {
            out.push(b as char);
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
}

/// `file:` URI for an absolute path, encoded as VS Code encodes it: every
/// byte outside `A-Za-z0-9-._~` is percent-encoded except `/`, and a Windows
/// drive letter is lower-cased with its colon encoded as `%3A`.
pub fn file_uri(path: &Path) -> String {
    let s = path.to_string_lossy().replace('\\', "/");
    let mut rest = s.as_str();
    let mut out = String::from("file://");
    let bytes = rest.as_bytes();
    if bytes.len() >= 2 && bytes[0].is_ascii_alphabetic() && bytes[1] == b':' {
        out.push('/');
        out.push((bytes[0] as char).to_ascii_lowercase());
        out.push_str("%3A");
        rest = &rest[2..];
    }
    let mut first = true;
    for segment in rest.split('/') {
        if !first {
            out.push('/');
        }
        first = false;
        encode_segment(&mut out, segment);
    }
    out
}

fn hex(b: u8) -> Option<u8> {
    (b as char).to_digit(16).map(|d| d as u8)
}

/// Percent-decodes a string, leaving malformed escapes as they are.
pub fn percent_decode(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%' && i + 2 < b.len() {
            if let (Some(h), Some(l)) = (hex(b[i + 1]), hex(b[i + 2])) {
                out.push(h * 16 + l);
                i += 3;
                continue;
            }
        }
        out.push(b[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// The path part of a URI: everything after `scheme://authority`, decoded,
/// without query or fragment.
pub fn uri_path(uri: &str) -> Option<String> {
    let (_, rest) = uri.split_once(':')?;
    let rest = rest
        .strip_prefix("//")
        .map(|r| r.find('/').map_or("", |i| &r[i..]))
        .unwrap_or(rest);
    let rest = rest.split(['?', '#']).next().unwrap_or("");
    Some(percent_decode(rest))
}

/// The URI's scheme, lower-cased.
pub fn scheme(uri: &str) -> Option<String> {
    uri.split_once(':').map(|(s, _)| s.to_ascii_lowercase())
}

/// Filesystem path for a `file:` URI, or for a `vscode-notebook-cell:` URI
/// whose path is the notebook's file path. Other schemes have no path.
pub fn uri_to_path(uri: &str) -> Option<PathBuf> {
    let s = scheme(uri)?;
    if s != "file" && s != "vscode-notebook-cell" {
        return None;
    }
    let p = uri_path(uri)?;
    let b = p.as_bytes();
    // `/c:/x` is a Windows drive path.
    if b.len() >= 3 && b[0] == b'/' && b[1].is_ascii_alphabetic() && b[2] == b':' {
        let drive = (b[1] as char).to_ascii_uppercase();
        return Some(PathBuf::from(format!(
            "{drive}:{}",
            p[3..].replace('/', "\\")
        )));
    }
    Some(PathBuf::from(p))
}

/// Last path segment of a URI, decoded.
pub fn uri_basename(uri: &str) -> String {
    let p = uri_path(uri).unwrap_or_default();
    p.rsplit('/').next().unwrap_or("").to_string()
}
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test -p clippings-core uri`
Expected: 4 tests pass.
Then run: `cargo fmt --all && cargo clippy --all-targets -- -D warnings`
Expected: no diffs, no warnings.

- [ ] **Step 5: Commit**

```bash
git add -A
git commit -F - <<'EOF'
feat(core): VS Code-compatible file URIs

Co-Authored-By: <your model attribution>
EOF
```

### Task 3: Settings

The full configuration object the extension sends (spec section 6.3): nested like the `clippings.*` settings with todo-tree's defaults, plus the client-side extras (`viewState`, `filesExclude`, `searchExclude`, `explorerCompactFolders`). `changes_from` classifies a configuration change into the actions of spec 6.3's table.

**Files:**
- Create: `crates/clippings-core/src/settings.rs`
- Modify: `crates/clippings-core/src/lib.rs` (add `pub mod settings;`, keeping the declarations sorted)

**Interfaces:**
- Consumes: `config::{CoreConfig, ScanMode, UseBuiltInExcludes, DEFAULT_*}`.
- Produces: `Settings { general, highlights, filtering, tree, regex, view_state, files_exclude, search_exclude, explorer_compact_folders }` (serde camelCase, defaults, unknown fields ignored), `Attributes` (the 18 highlight attributes, `type` as `kind`), `RevealBehaviour`, `StatusBarMode`, `ViewState`, `ViewOptions`, `Changes { rescan, styles, view, status, timers, view_mode }`.
- Produces: `Settings::tags()`, `view() -> ViewOptions` (view state over settings), `core() -> CoreConfig`, `custom(&str) -> Option<&Attributes>`, `group_of(&str)`, `key_of(&str) -> &str`, `changes_from(&Settings) -> Changes`.

- [ ] **Step 1: Write the failing tests**

Add `pub mod settings;` to `crates/clippings-core/src/lib.rs`. Create `crates/clippings-core/src/settings.rs` containing only this test module:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_match_todo_tree() {
        let s = Settings::default();
        assert_eq!(s.general.tags.len(), 7);
        assert_eq!(
            s.general.schemes,
            vec!["file", "ssh", "untitled", "vscode-notebook-cell"]
        );
        assert_eq!(s.highlights.highlight_delay, 500);
        assert!(s.highlights.enabled);
        assert_eq!(s.custom("BUG").unwrap().icon.as_deref(), Some("bug"));
        assert_eq!(s.tree.label_format, "${tag} ${after}");
        assert_eq!(s.tree.tooltip_format, "${filepath}, line ${line}");
        assert!(
            s.tree.show_badges && s.tree.sort && s.tree.show_current_scan_mode && s.tree.track_file
        );
        assert_eq!(s.general.export_path, "~/todo-tree-%Y%m%d-%H%M.txt");
        assert_eq!(s.core(), CoreConfig::default());
    }

    #[test]
    fn deserializes_the_client_shape() {
        let s: Settings = serde_json::from_str(
            r#"{"general":{"tags":["TODO","FIXME"],"statusBar":"top three","revealBehaviour":"end of todo"},
                "highlights":{"customHighlight":{"TODO":{"type":"text","iconColour":"red","rulerLane":"left"}}},
                "tree":{"scanMode":"workspace only","showCountsInTree":true},
                "viewState":{"flat":true,"filter":"fix"},
                "filesExclude":["**/.git"],"explorerCompactFolders":true,
                "server":{"logLevel":"debug"}}"#,
        )
        .unwrap();
        assert_eq!(s.general.status_bar, StatusBarMode::TopThree);
        assert_eq!(s.general.reveal_behaviour, RevealBehaviour::EndOfTodo);
        let todo = s.custom("TODO").unwrap();
        assert_eq!(todo.kind.as_deref(), Some("text"));
        assert_eq!(todo.icon_colour.as_deref(), Some("red"));
        assert_eq!(todo.ruler_lane, Some(Value::String("left".into())));
        assert!(s.view().flat);
        assert!(!s.view().tags_only);
        assert_eq!(s.core().scan_mode, ScanMode::WorkspaceOnly);
        assert!(s.explorer_compact_folders);
    }

    #[test]
    fn view_state_overrides_settings() {
        let mut s = Settings::default();
        s.tree.flat = true;
        assert!(s.view().flat);
        s.view_state.flat = Some(false);
        assert!(!s.view().flat);
    }

    #[test]
    fn group_and_key() {
        let mut s = Settings::default();
        s.general
            .tag_groups
            .insert("FIX".into(), vec!["FIXME".into(), "BUG".into()]);
        assert_eq!(s.key_of("BUG"), "FIX");
        assert_eq!(s.key_of("TODO"), "TODO");
    }

    #[test]
    fn change_classification() {
        let base = Settings::default();
        let mut s = base.clone();
        s.regex.regex = r"($TAGS)".into();
        let c = s.changes_from(&base);
        assert!(c.rescan && c.styles && c.view && c.status);

        let mut s = base.clone();
        s.general.tag_groups.insert("G".into(), vec!["TODO".into()]);
        let c = s.changes_from(&base);
        assert!(!c.rescan && c.styles && c.view);

        let mut s = base.clone();
        s.highlights.default_highlight.foreground = Some("red".into());
        let c = s.changes_from(&base);
        assert!(!c.rescan && c.styles);

        let mut s = base.clone();
        s.general.status_bar = StatusBarMode::Total;
        let c = s.changes_from(&base);
        assert!(!c.rescan && !c.styles && c.status);

        let mut s = base.clone();
        s.general.periodic_refresh_interval = 5;
        let c = s.changes_from(&base);
        assert!(c.timers && !c.rescan && !c.view);

        let mut s = base.clone();
        s.view_state.flat = Some(true);
        let c = s.changes_from(&base);
        assert!(c.view && c.view_mode && !c.rescan);

        let mut off = base.clone();
        off.tree.auto_refresh = false;
        assert!(
            base.changes_from(&off).rescan,
            "autoRefresh false -> true rescans"
        );

        assert!(!base.changes_from(&base).any());
    }
}
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p clippings-core settings`
Expected: compile error, `Settings` not found.

- [ ] **Step 3: Write the implementation**

Put this above the test module in `crates/clippings-core/src/settings.rs`:

```rust
//! The full resolved configuration the extension sends in `initialize` and
//! `clippings/configure` (spec section 6.3). The nesting mirrors the
//! `clippings.*` settings; serde uses camelCase and fills defaults, which
//! equal todo-tree v0.0.224's. Unknown fields are ignored.

use crate::config::{
    CoreConfig, ScanMode, UseBuiltInExcludes, DEFAULT_BUILT_IN_EXCLUDES, DEFAULT_REGEX,
    DEFAULT_TAGS,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum RevealBehaviour {
    #[serde(rename = "start of line")]
    StartOfLine,
    #[default]
    #[serde(rename = "start of todo")]
    StartOfTodo,
    #[serde(rename = "end of todo")]
    EndOfTodo,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum StatusBarMode {
    #[default]
    #[serde(rename = "none")]
    None,
    #[serde(rename = "total")]
    Total,
    #[serde(rename = "tags")]
    Tags,
    #[serde(rename = "top three")]
    TopThree,
    #[serde(rename = "current file")]
    CurrentFile,
}

/// Per-tag highlight attributes (`customHighlight.<key>` and `defaultHighlight`).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Attributes {
    #[serde(rename = "type")]
    pub kind: Option<String>,
    pub foreground: Option<String>,
    pub background: Option<String>,
    pub opacity: Option<f64>,
    pub ruler_colour: Option<String>,
    pub ruler_opacity: Option<f64>,
    /// A lane number or one of `none`, `left`, `center`, `right`, `full`.
    pub ruler_lane: Option<Value>,
    pub border_radius: Option<String>,
    pub font_style: Option<String>,
    pub font_weight: Option<String>,
    pub text_decoration: Option<String>,
    pub gutter_icon: Option<bool>,
    pub icon: Option<String>,
    pub icon_colour: Option<String>,
    /// US spelling, checked before `iconColour` as in todo-tree.
    pub icon_color: Option<String>,
    pub hide_from_tree: Option<bool>,
    pub hide_from_status_bar: Option<bool>,
    pub hide_from_activity_bar: Option<bool>,
}

fn icon(name: &str) -> Attributes {
    Attributes {
        icon: Some(name.to_string()),
        ..Default::default()
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct General {
    pub automatic_git_refresh_interval: u64,
    pub periodic_refresh_interval: u64,
    pub reveal_behaviour: RevealBehaviour,
    pub export_path: String,
    pub root_folder: String,
    pub schemes: Vec<String>,
    pub status_bar: StatusBarMode,
    pub show_icons_instead_of_tags_in_status_bar: bool,
    pub tag_groups: BTreeMap<String, Vec<String>>,
    pub tags: Vec<String>,
    pub show_activity_bar_badge: bool,
}

impl Default for General {
    fn default() -> Self {
        Self {
            automatic_git_refresh_interval: 0,
            periodic_refresh_interval: 0,
            reveal_behaviour: RevealBehaviour::StartOfTodo,
            export_path: "~/todo-tree-%Y%m%d-%H%M.txt".to_string(),
            root_folder: String::new(),
            schemes: ["file", "ssh", "untitled", "vscode-notebook-cell"]
                .map(String::from)
                .to_vec(),
            status_bar: StatusBarMode::None,
            show_icons_instead_of_tags_in_status_bar: false,
            tag_groups: BTreeMap::new(),
            tags: DEFAULT_TAGS.map(String::from).to_vec(),
            show_activity_bar_badge: false,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Highlights {
    pub custom_highlight: BTreeMap<String, Attributes>,
    pub default_highlight: Attributes,
    pub enabled: bool,
    pub highlight_delay: u64,
    pub use_colour_scheme: bool,
    pub foreground_colour_scheme: Vec<String>,
    pub background_colour_scheme: Vec<String>,
}

impl Default for Highlights {
    fn default() -> Self {
        let custom = [
            ("BUG", "bug"),
            ("HACK", "tools"),
            ("FIXME", "flame"),
            ("XXX", "x"),
            ("[ ]", "issue-draft"),
            ("[x]", "issue-closed"),
        ];
        Self {
            custom_highlight: custom
                .iter()
                .map(|(t, i)| (t.to_string(), icon(i)))
                .collect(),
            default_highlight: Attributes::default(),
            enabled: true,
            highlight_delay: 500,
            use_colour_scheme: false,
            foreground_colour_scheme: [
                "white", "black", "black", "white", "white", "white", "black",
            ]
            .map(String::from)
            .to_vec(),
            background_colour_scheme: [
                "red", "orange", "yellow", "green", "blue", "indigo", "violet",
            ]
            .map(String::from)
            .to_vec(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Filtering {
    pub excluded_workspaces: Vec<String>,
    pub exclude_globs: Vec<String>,
    pub ignore_git_submodules: bool,
    pub included_workspaces: Vec<String>,
    pub include_globs: Vec<String>,
    pub include_hidden_files: bool,
    pub use_built_in_excludes: UseBuiltInExcludes,
    pub built_in_excludes: Vec<String>,
}

impl Default for Filtering {
    fn default() -> Self {
        Self {
            excluded_workspaces: Vec::new(),
            exclude_globs: vec!["**/node_modules/*/**".to_string()],
            ignore_git_submodules: false,
            included_workspaces: Vec::new(),
            include_globs: Vec::new(),
            include_hidden_files: false,
            use_built_in_excludes: UseBuiltInExcludes::None,
            built_in_excludes: DEFAULT_BUILT_IN_EXCLUDES.map(String::from).to_vec(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Tree {
    pub auto_refresh: bool,
    pub disable_compact_folders: bool,
    pub expanded: bool,
    pub filter_case_sensitive: bool,
    pub flat: bool,
    pub grouped_by_tag: bool,
    pub grouped_by_sub_tag: bool,
    pub hide_icons_when_grouped_by_tag: bool,
    pub hide_tree_when_empty: bool,
    pub label_format: String,
    pub scan_at_startup: bool,
    pub scan_mode: ScanMode,
    pub show_badges: bool,
    pub show_counts_in_tree: bool,
    pub show_current_scan_mode: bool,
    pub sub_tag_click_url: String,
    pub sort_tags_only_view_alphabetically: bool,
    pub sort: bool,
    pub tags_only: bool,
    pub tooltip_format: String,
    pub track_file: bool,
}

impl Default for Tree {
    fn default() -> Self {
        Self {
            auto_refresh: true,
            disable_compact_folders: false,
            expanded: false,
            filter_case_sensitive: false,
            flat: false,
            grouped_by_tag: false,
            grouped_by_sub_tag: false,
            hide_icons_when_grouped_by_tag: false,
            hide_tree_when_empty: false,
            label_format: "${tag} ${after}".to_string(),
            scan_at_startup: true,
            scan_mode: ScanMode::Workspace,
            show_badges: true,
            show_counts_in_tree: false,
            show_current_scan_mode: true,
            sub_tag_click_url: String::new(),
            sort_tags_only_view_alphabetically: false,
            sort: true,
            tags_only: false,
            tooltip_format: "${filepath}, line ${line}".to_string(),
            track_file: true,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct RegexSettings {
    pub regex: String,
    pub regex_case_sensitive: bool,
    pub sub_tag_regex: String,
    pub enable_multi_line: bool,
}

impl Default for RegexSettings {
    fn default() -> Self {
        Self {
            regex: DEFAULT_REGEX.to_string(),
            regex_case_sensitive: true,
            sub_tag_regex: String::new(),
            enable_multi_line: false,
        }
    }
}

/// View state the user set by clicking view buttons, kept in the client's
/// workspace storage. A set value overrides the matching `tree.*` setting.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct ViewState {
    pub flat: Option<bool>,
    pub tags_only: Option<bool>,
    pub expanded: Option<bool>,
    pub grouped_by_tag: Option<bool>,
    pub grouped_by_sub_tag: Option<bool>,
    /// The tree filter text; empty means no filter.
    pub filter: String,
    /// Temporary include globs from the folder context menu and scopes.
    pub include_globs: Vec<String>,
    /// Temporary exclude globs from the folder and file context menus and scopes.
    pub exclude_globs: Vec<String>,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Settings {
    pub general: General,
    pub highlights: Highlights,
    pub filtering: Filtering,
    pub tree: Tree,
    pub regex: RegexSettings,
    pub view_state: ViewState,
    /// Keys of `files.exclude` whose value is exactly `true`.
    pub files_exclude: Vec<String>,
    /// Keys of `search.exclude` whose value is exactly `true`.
    pub search_exclude: Vec<String>,
    /// `explorer.compactFolders`.
    pub explorer_compact_folders: bool,
}

/// The effective view options after applying the view state over the settings.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ViewOptions {
    pub flat: bool,
    pub tags_only: bool,
    pub expanded: bool,
    pub grouped_by_tag: bool,
    pub grouped_by_sub_tag: bool,
}

/// What a configuration change requires (spec section 6.3). Every flag whose
/// fields changed is set; callers apply all of them.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Changes {
    pub rescan: bool,
    pub styles: bool,
    pub view: bool,
    pub status: bool,
    pub timers: bool,
    /// The view mode or grouping changed, so the whole tree is replaced.
    pub view_mode: bool,
}

impl Changes {
    pub fn any(&self) -> bool {
        self.rescan || self.styles || self.view || self.status || self.timers
    }
}

impl Settings {
    /// Tags to search for: `general.tags`, or `["TODO"]` when empty.
    pub fn tags(&self) -> Vec<String> {
        if self.general.tags.is_empty() {
            vec!["TODO".to_string()]
        } else {
            self.general.tags.clone()
        }
    }

    pub fn view(&self) -> ViewOptions {
        let v = &self.view_state;
        let t = &self.tree;
        ViewOptions {
            flat: v.flat.unwrap_or(t.flat),
            tags_only: v.tags_only.unwrap_or(t.tags_only),
            expanded: v.expanded.unwrap_or(t.expanded),
            grouped_by_tag: v.grouped_by_tag.unwrap_or(t.grouped_by_tag),
            grouped_by_sub_tag: v.grouped_by_sub_tag.unwrap_or(t.grouped_by_sub_tag),
        }
    }

    /// The scanner's configuration.
    pub fn core(&self) -> CoreConfig {
        CoreConfig {
            tags: self.general.tags.clone(),
            regex: self.regex.regex.clone(),
            regex_case_sensitive: self.regex.regex_case_sensitive,
            sub_tag_regex: self.regex.sub_tag_regex.clone(),
            enable_multi_line: self.regex.enable_multi_line,
            tag_groups: self.general.tag_groups.clone(),
            include_globs: self.filtering.include_globs.clone(),
            exclude_globs: self.filtering.exclude_globs.clone(),
            temp_include_globs: self.view_state.include_globs.clone(),
            temp_exclude_globs: self.view_state.exclude_globs.clone(),
            files_exclude: self.files_exclude.clone(),
            search_exclude: self.search_exclude.clone(),
            use_built_in_excludes: self.filtering.use_built_in_excludes,
            built_in_excludes: self.filtering.built_in_excludes.clone(),
            include_hidden_files: self.filtering.include_hidden_files,
            ignore_git_submodules: self.filtering.ignore_git_submodules,
            root_folder: self.general.root_folder.clone(),
            included_workspaces: self.filtering.included_workspaces.clone(),
            excluded_workspaces: self.filtering.excluded_workspaces.clone(),
            scan_mode: self.tree.scan_mode,
            respect_ignore_files: true,
        }
    }

    /// Attributes for a decoration or tree key: an exact `customHighlight`
    /// entry, if any.
    pub fn custom(&self, key: &str) -> Option<&Attributes> {
        self.highlights.custom_highlight.get(key)
    }

    /// Group name for a tag, if `tagGroups` maps it.
    pub fn group_of(&self, tag: &str) -> Option<&str> {
        self.general
            .tag_groups
            .iter()
            .find(|(_, tags)| tags.iter().any(|t| t == tag))
            .map(|(g, _)| g.as_str())
    }

    /// The decoration and tree key for a tag: its group, else the tag.
    pub fn key_of<'a>(&'a self, tag: &'a str) -> &'a str {
        self.group_of(tag).unwrap_or(tag)
    }

    /// What changed between `old` and `self` (spec section 6.3).
    pub fn changes_from(&self, old: &Settings) -> Changes {
        let mut c = Changes::default();
        let old_core = old.core();
        let new_core = self.core();
        let rescan_fields_changed = {
            let mut a = old_core.clone();
            let mut b = new_core.clone();
            // Tag groups apply at view build, never rescan.
            a.tag_groups.clear();
            b.tag_groups.clear();
            a != b
        };
        c.rescan = rescan_fields_changed || (!old.tree.auto_refresh && self.tree.auto_refresh);
        let hide = |s: &Settings, f: fn(&Attributes) -> Option<bool>| {
            s.highlights
                .custom_highlight
                .iter()
                .map(|(k, a)| (k.clone(), f(a)))
                .collect::<Vec<_>>()
        };
        c.styles = old.general.tags != self.general.tags
            || old.general.tag_groups != self.general.tag_groups
            || old.regex != self.regex
            || old_core.include_globs != new_core.include_globs
            || old_core.exclude_globs != new_core.exclude_globs
            || old_core.temp_include_globs != new_core.temp_include_globs
            || old_core.temp_exclude_globs != new_core.temp_exclude_globs
            || old_core.vscode_excludes() != new_core.vscode_excludes()
            || old.highlights != self.highlights
            || old.general.schemes != self.general.schemes;
        c.view = old.general.tag_groups != self.general.tag_groups
            || old.view_state != self.view_state
            || old.tree != self.tree
            || old.general.reveal_behaviour != self.general.reveal_behaviour
            || old.general.status_bar != self.general.status_bar
            || old.general.tags != self.general.tags
            || old.explorer_compact_folders != self.explorer_compact_folders
            || old.highlights.custom_highlight != self.highlights.custom_highlight
            || old.highlights.default_highlight != self.highlights.default_highlight
            || old.highlights.use_colour_scheme != self.highlights.use_colour_scheme
            || old.highlights.background_colour_scheme != self.highlights.background_colour_scheme
            || c.rescan;
        c.status = old.general.status_bar != self.general.status_bar
            || old.general.show_icons_instead_of_tags_in_status_bar
                != self.general.show_icons_instead_of_tags_in_status_bar
            || old.general.show_activity_bar_badge != self.general.show_activity_bar_badge
            || hide(old, |a| a.hide_from_status_bar) != hide(self, |a| a.hide_from_status_bar)
            || old.highlights.default_highlight.hide_from_status_bar
                != self.highlights.default_highlight.hide_from_status_bar
            || c.view;
        c.timers = old.general.automatic_git_refresh_interval
            != self.general.automatic_git_refresh_interval
            || old.general.periodic_refresh_interval != self.general.periodic_refresh_interval;
        c.view_mode = old.view() != self.view() || old.tree.scan_mode != self.tree.scan_mode;
        c
    }
}
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test -p clippings-core settings`
Expected: 5 tests pass.
Then run: `cargo fmt --all && cargo clippy --all-targets -- -D warnings`
Expected: no diffs, no warnings.

- [ ] **Step 5: Commit**

```bash
git add -A
git commit -F - <<'EOF'
feat(core): full settings object and change classification

Co-Authored-By: <your model attribution>
EOF
```

### Task 4: Colours

Colour helpers ported from todo-tree's `utils.js`, with the colour-name lists copied from todo-tree so validity checks agree exactly.

**Files:**
- Create: `crates/clippings-core/src/colour_names.rs`, `crates/clippings-core/src/colours.rs`
- Modify: `crates/clippings-core/src/lib.rs` (add `pub mod colour_names;` and `pub mod colours;`)

**Interfaces:**
- Produces: `colour_names::{NAMED_COLOURS: [&str; 147], THEME_COLOURS: [&str; N]}`.
- Produces: `colours::{is_hex, is_rgb, is_named, is_theme, is_valid}(&str) -> bool`, `js_number(f64) -> String`, `hex_to_rgba(&str, opacity: f64) -> String`, `set_rgb_alpha(&str, f64) -> String`, `apply_opacity(&str, opacity: f64) -> String`, `rgb_components(&str) -> Option<(u32, u32, u32)>`, `complementary(&str) -> Option<&'static str>`.

- [ ] **Step 1: Create the colour-name tables**

`crates/clippings-core/src/colour_names.rs`, copied from todo-tree's `src/colourNames.js` and `src/themeColourNames.js`:

```rust
//! CSS named colours and VS Code theme colour ids, copied from todo-tree v0.0.224
//! (`src/colourNames.js` and `src/themeColourNames.js`, MIT licence).

pub const NAMED_COLOURS: [&str; 147] = [
    "aliceblue",
    "antiquewhite",
    "aqua",
    "aquamarine",
    "azure",
    "beige",
    "bisque",
    "black",
    "blanchedalmond",
    "blue",
    "blueviolet",
    "brown",
    "burlywood",
    "cadetblue",
    "chartreuse",
    "chocolate",
    "coral",
    "cornflowerblue",
    "cornsilk",
    "crimson",
    "cyan",
    "darkblue",
    "darkcyan",
    "darkgoldenrod",
    "darkgray",
    "darkgreen",
    "darkgrey",
    "darkkhaki",
    "darkmagenta",
    "darkolivegreen",
    "darkorange",
    "darkorchid",
    "darkred",
    "darksalmon",
    "darkseagreen",
    "darkslateblue",
    "darkslategray",
    "darkslategrey",
    "darkturquoise",
    "darkviolet",
    "deeppink",
    "deepskyblue",
    "dimgray",
    "dimgrey",
    "dodgerblue",
    "firebrick",
    "floralwhite",
    "forestgreen",
    "fuchsia",
    "gainsboro",
    "ghostwhite",
    "gold",
    "goldenrod",
    "gray",
    "grey",
    "green",
    "greenyellow",
    "honeydew",
    "hotpink",
    "indianred",
    "indigo",
    "ivory",
    "khaki",
    "lavender",
    "lavenderblush",
    "lawngreen",
    "lemonchiffon",
    "lightblue",
    "lightcoral",
    "lightcyan",
    "lightgoldenrodyellow",
    "lightgray",
    "lightgreen",
    "lightgrey",
    "lightpink",
    "lightsalmon",
    "lightseagreen",
    "lightskyblue",
    "lightslategray",
    "lightslategrey",
    "lightsteelblue",
    "lightyellow",
    "lime",
    "limegreen",
    "linen",
    "magenta",
    "maroon",
    "mediumaquamarine",
    "mediumblue",
    "mediumorchid",
    "mediumpurple",
    "mediumseagreen",
    "mediumslateblue",
    "mediumspringgreen",
    "mediumturquoise",
    "mediumvioletred",
    "midnightblue",
    "mintcream",
    "mistyrose",
    "moccasin",
    "navajowhite",
    "navy",
    "oldlace",
    "olive",
    "olivedrab",
    "orange",
    "orangered",
    "orchid",
    "palegoldenrod",
    "palegreen",
    "paleturquoise",
    "palevioletred",
    "papayawhip",
    "peachpuff",
    "peru",
    "pink",
    "plum",
    "powderblue",
    "purple",
    "red",
    "rosybrown",
    "royalblue",
    "saddlebrown",
    "salmon",
    "sandybrown",
    "seagreen",
    "seashell",
    "sienna",
    "silver",
    "skyblue",
    "slateblue",
    "slategray",
    "slategrey",
    "snow",
    "springgreen",
    "steelblue",
    "tan",
    "teal",
    "thistle",
    "tomato",
    "turquoise",
    "violet",
    "wheat",
    "white",
    "whitesmoke",
    "yellow",
    "yellowgreen",
];

pub const THEME_COLOURS: [&str; 388] = [
    "activityBar.activeBorder",
    "activityBar.background",
    "activityBar.border",
    "activityBar.dropBorder",
    "activityBar.foreground",
    "activityBar.inactiveForeground",
    "activityBarBadge.background",
    "activityBarBadge.foreground",
    "badge.background",
    "badge.foreground",
    "breadcrumb.activeSelectionForeground",
    "breadcrumb.background",
    "breadcrumb.focusForeground",
    "breadcrumb.foreground",
    "breadcrumbPicker.background",
    "button.background",
    "button.foreground",
    "button.hoverBackground",
    "button.secondaryBackground",
    "button.secondaryForeground",
    "button.secondaryHoverBackground",
    "checkbox.background",
    "checkbox.border",
    "checkbox.foreground",
    "debugConsole.errorForeground",
    "debugConsole.infoForeground",
    "debugConsole.sourceForeground",
    "debugConsole.warningForeground",
    "debugConsoleInputIcon.foreground",
    "debugExceptionWidget.background",
    "debugExceptionWidget.border",
    "debugIcon.breakpointCurrentStackframeForeground",
    "debugIcon.breakpointDisabledForeground",
    "debugIcon.breakpointForeground",
    "debugIcon.breakpointStackframeForeground",
    "debugIcon.breakpointUnverifiedForeground",
    "debugIcon.continueForeground",
    "debugIcon.disconnectForeground",
    "debugIcon.pauseForeground",
    "debugIcon.restartForeground",
    "debugIcon.startForeground",
    "debugIcon.stepBackForeground",
    "debugIcon.stepIntoForeground",
    "debugIcon.stepOutForeground",
    "debugIcon.stepOverForeground",
    "debugIcon.stopForeground",
    "debugTokenExpression.boolean",
    "debugTokenExpression.error",
    "debugTokenExpression.name",
    "debugTokenExpression.number",
    "debugTokenExpression.string",
    "debugTokenExpression.value",
    "debugToolBar.background",
    "debugView.exceptionLabelBackground",
    "debugView.exceptionLabelForeground",
    "debugView.stateLabelBackground",
    "debugView.stateLabelForeground",
    "debugView.valueChangedHighlight",
    "descriptionForeground",
    "diffEditor.diagonalFill",
    "diffEditor.insertedTextBackground",
    "diffEditor.removedTextBackground",
    "dropdown.background",
    "dropdown.border",
    "dropdown.foreground",
    "editor.background",
    "editor.findMatchBackground",
    "editor.findMatchHighlightBackground",
    "editor.findRangeHighlightBackground",
    "editor.focusedStackFrameHighlightBackground",
    "editor.foldBackground",
    "editor.foreground",
    "editor.hoverHighlightBackground",
    "editor.inactiveSelectionBackground",
    "editor.lineHighlightBorder",
    "editor.onTypeRenameBackground",
    "editor.rangeHighlightBackground",
    "editor.selectionBackground",
    "editor.selectionHighlightBackground",
    "editor.snippetFinalTabstopHighlightBorder",
    "editor.snippetTabstopHighlightBackground",
    "editor.stackFrameHighlightBackground",
    "editor.symbolHighlightBackground",
    "editor.wordHighlightBackground",
    "editor.wordHighlightStrongBackground",
    "editorActiveLineNumber.foreground",
    "editorBracketMatch.background",
    "editorBracketMatch.border",
    "editorCodeLens.foreground",
    "editorCursor.foreground",
    "editorError.foreground",
    "editorGroup.border",
    "editorGroup.dropBackground",
    "editorGroupHeader.noTabsBackground",
    "editorGroupHeader.tabsBackground",
    "editorGutter.addedBackground",
    "editorGutter.background",
    "editorGutter.commentRangeForeground",
    "editorGutter.deletedBackground",
    "editorGutter.foldingControlForeground",
    "editorGutter.modifiedBackground",
    "editorHint.foreground",
    "editorHoverWidget.background",
    "editorHoverWidget.border",
    "editorHoverWidget.foreground",
    "editorHoverWidget.statusBarBackground",
    "editorIndentGuide.activeBackground",
    "editorIndentGuide.background",
    "editorInfo.foreground",
    "editorLightBulb.foreground",
    "editorLightBulbAutoFix.foreground",
    "editorLineNumber.activeForeground",
    "editorLineNumber.foreground",
    "editorLink.activeForeground",
    "editorMarkerNavigation.background",
    "editorMarkerNavigationError.background",
    "editorMarkerNavigationInfo.background",
    "editorMarkerNavigationWarning.background",
    "editorOverviewRuler.addedForeground",
    "editorOverviewRuler.border",
    "editorOverviewRuler.bracketMatchForeground",
    "editorOverviewRuler.commonContentForeground",
    "editorOverviewRuler.currentContentForeground",
    "editorOverviewRuler.deletedForeground",
    "editorOverviewRuler.errorForeground",
    "editorOverviewRuler.findMatchForeground",
    "editorOverviewRuler.incomingContentForeground",
    "editorOverviewRuler.infoForeground",
    "editorOverviewRuler.modifiedForeground",
    "editorOverviewRuler.rangeHighlightForeground",
    "editorOverviewRuler.selectionHighlightForeground",
    "editorOverviewRuler.warningForeground",
    "editorOverviewRuler.wordHighlightForeground",
    "editorOverviewRuler.wordHighlightStrongForeground",
    "editorPane.background",
    "editorRuler.foreground",
    "editorSuggestWidget.background",
    "editorSuggestWidget.border",
    "editorSuggestWidget.foreground",
    "editorSuggestWidget.highlightForeground",
    "editorSuggestWidget.selectedBackground",
    "editorUnnecessaryCode.opacity",
    "editorWarning.foreground",
    "editorWhitespace.foreground",
    "editorWidget.background",
    "editorWidget.border",
    "editorWidget.foreground",
    "errorForeground",
    "extensionBadge.remoteBackground",
    "extensionBadge.remoteForeground",
    "extensionButton.prominentBackground",
    "extensionButton.prominentForeground",
    "extensionButton.prominentHoverBackground",
    "focusBorder",
    "foreground",
    "gitDecoration.addedResourceForeground",
    "gitDecoration.conflictingResourceForeground",
    "gitDecoration.deletedResourceForeground",
    "gitDecoration.ignoredResourceForeground",
    "gitDecoration.modifiedResourceForeground",
    "gitDecoration.submoduleResourceForeground",
    "gitDecoration.untrackedResourceForeground",
    "icon.foreground",
    "imagePreview.border",
    "input.background",
    "input.foreground",
    "input.placeholderForeground",
    "inputOption.activeBackground",
    "inputOption.activeBorder",
    "inputOption.activeForeground",
    "inputValidation.errorBackground",
    "inputValidation.errorBorder",
    "inputValidation.infoBackground",
    "inputValidation.infoBorder",
    "inputValidation.warningBackground",
    "inputValidation.warningBorder",
    "list.activeSelectionBackground",
    "list.activeSelectionForeground",
    "list.deemphasizedForeground",
    "list.dropBackground",
    "list.errorForeground",
    "list.filterMatchBackground",
    "list.focusBackground",
    "list.highlightForeground",
    "list.hoverBackground",
    "list.inactiveSelectionBackground",
    "list.invalidItemForeground",
    "list.warningForeground",
    "listFilterWidget.background",
    "listFilterWidget.noMatchesOutline",
    "listFilterWidget.outline",
    "menu.background",
    "menu.foreground",
    "menu.selectionBackground",
    "menu.selectionForeground",
    "menu.separatorBackground",
    "menubar.selectionBackground",
    "menubar.selectionForeground",
    "merge.commonContentBackground",
    "merge.commonHeaderBackground",
    "merge.currentContentBackground",
    "merge.currentHeaderBackground",
    "merge.incomingContentBackground",
    "merge.incomingHeaderBackground",
    "minimap.errorHighlight",
    "minimap.findMatchHighlight",
    "minimap.selectionHighlight",
    "minimap.warningHighlight",
    "minimapGutter.addedBackground",
    "minimapGutter.deletedBackground",
    "minimapGutter.modifiedBackground",
    "minimapSlider.activeBackground",
    "minimapSlider.background",
    "minimapSlider.hoverBackground",
    "notebook.cellBorderColor",
    "notebook.cellHoverBackground",
    "notebook.cellInsertionIndicator",
    "notebook.cellStatusBarItemHoverBackground",
    "notebook.cellToolbarSeparator",
    "notebook.focusedCellBackground",
    "notebook.focusedCellBorder",
    "notebook.focusedEditorBorder",
    "notebook.outputContainerBackgroundColor",
    "notebook.symbolHighlightBackground",
    "notebookScrollbarSlider.activeBackground",
    "notebookScrollbarSlider.background",
    "notebookScrollbarSlider.hoverBackground",
    "notebookStatusErrorIcon.foreground",
    "notebookStatusRunningIcon.foreground",
    "notebookStatusSuccessIcon.foreground",
    "notificationCenterHeader.background",
    "notificationLink.foreground",
    "notifications.background",
    "notifications.border",
    "notifications.foreground",
    "notificationsErrorIcon.foreground",
    "notificationsInfoIcon.foreground",
    "notificationsWarningIcon.foreground",
    "panel.background",
    "panel.border",
    "panel.dropBorder",
    "panelSection.border",
    "panelSection.dropBackground",
    "panelSectionHeader.background",
    "panelTitle.activeBorder",
    "panelTitle.activeForeground",
    "panelTitle.inactiveForeground",
    "peekView.border",
    "peekViewEditor.background",
    "peekViewEditor.matchHighlightBackground",
    "peekViewEditorGutter.background",
    "peekViewResult.background",
    "peekViewResult.fileForeground",
    "peekViewResult.lineForeground",
    "peekViewResult.matchHighlightBackground",
    "peekViewResult.selectionBackground",
    "peekViewResult.selectionForeground",
    "peekViewTitle.background",
    "peekViewTitleDescription.foreground",
    "peekViewTitleLabel.foreground",
    "pickerGroup.border",
    "pickerGroup.foreground",
    "problemsErrorIcon.foreground",
    "problemsInfoIcon.foreground",
    "problemsWarningIcon.foreground",
    "progressBar.background",
    "quickInput.background",
    "quickInput.foreground",
    "quickInputTitle.background",
    "scm.providerBorder",
    "scrollbar.shadow",
    "scrollbarSlider.activeBackground",
    "scrollbarSlider.background",
    "scrollbarSlider.hoverBackground",
    "searchEditor.findMatchBackground",
    "settings.checkboxBackground",
    "settings.checkboxBorder",
    "settings.checkboxForeground",
    "settings.dropdownBackground",
    "settings.dropdownBorder",
    "settings.dropdownForeground",
    "settings.dropdownListBorder",
    "settings.headerForeground",
    "settings.modifiedItemIndicator",
    "settings.numberInputBackground",
    "settings.numberInputForeground",
    "settings.textInputBackground",
    "settings.textInputForeground",
    "sideBar.background",
    "sideBar.border",
    "sideBar.dropBackground",
    "sideBarSectionHeader.background",
    "sideBarSectionHeader.border",
    "sideBarTitle.foreground",
    "statusBar.background",
    "statusBar.border",
    "statusBar.debuggingBackground",
    "statusBar.debuggingBorder",
    "statusBar.debuggingForeground",
    "statusBar.foreground",
    "statusBar.noFolderBackground",
    "statusBar.noFolderBorder",
    "statusBar.noFolderForeground",
    "statusBarItem.activeBackground",
    "statusBarItem.hoverBackground",
    "statusBarItem.prominentBackground",
    "statusBarItem.prominentForeground",
    "statusBarItem.prominentHoverBackground",
    "statusBarItem.remoteBackground",
    "statusBarItem.remoteForeground",
    "symbolIcon.arrayForeground",
    "symbolIcon.booleanForeground",
    "symbolIcon.classForeground",
    "symbolIcon.colorForeground",
    "symbolIcon.constantForeground",
    "symbolIcon.constructorForeground",
    "symbolIcon.enumeratorForeground",
    "symbolIcon.enumeratorMemberForeground",
    "symbolIcon.eventForeground",
    "symbolIcon.fieldForeground",
    "symbolIcon.fileForeground",
    "symbolIcon.folderForeground",
    "symbolIcon.functionForeground",
    "symbolIcon.interfaceForeground",
    "symbolIcon.keyForeground",
    "symbolIcon.keywordForeground",
    "symbolIcon.methodForeground",
    "symbolIcon.moduleForeground",
    "symbolIcon.namespaceForeground",
    "symbolIcon.nullForeground",
    "symbolIcon.numberForeground",
    "symbolIcon.objectForeground",
    "symbolIcon.operatorForeground",
    "symbolIcon.packageForeground",
    "symbolIcon.propertyForeground",
    "symbolIcon.referenceForeground",
    "symbolIcon.snippetForeground",
    "symbolIcon.stringForeground",
    "symbolIcon.structForeground",
    "symbolIcon.textForeground",
    "symbolIcon.typeParameterForeground",
    "symbolIcon.unitForeground",
    "symbolIcon.variableForeground",
    "tab.activeBackground",
    "tab.activeForeground",
    "tab.activeModifiedBorder",
    "tab.border",
    "tab.inactiveBackground",
    "tab.inactiveForeground",
    "tab.inactiveModifiedBorder",
    "tab.unfocusedActiveBackground",
    "tab.unfocusedActiveForeground",
    "tab.unfocusedActiveModifiedBorder",
    "tab.unfocusedInactiveBackground",
    "tab.unfocusedInactiveForeground",
    "tab.unfocusedInactiveModifiedBorder",
    "terminal.ansiBlack",
    "terminal.ansiBlue",
    "terminal.ansiBrightBlack",
    "terminal.ansiBrightBlue",
    "terminal.ansiBrightCyan",
    "terminal.ansiBrightGreen",
    "terminal.ansiBrightMagenta",
    "terminal.ansiBrightRed",
    "terminal.ansiBrightWhite",
    "terminal.ansiBrightYellow",
    "terminal.ansiCyan",
    "terminal.ansiGreen",
    "terminal.ansiMagenta",
    "terminal.ansiRed",
    "terminal.ansiWhite",
    "terminal.ansiYellow",
    "terminal.border",
    "terminal.foreground",
    "terminal.selectionBackground",
    "textBlockQuote.background",
    "textBlockQuote.border",
    "textCodeBlock.background",
    "textLink.activeForeground",
    "textLink.foreground",
    "textPreformat.foreground",
    "textSeparator.foreground",
    "titleBar.activeBackground",
    "titleBar.activeForeground",
    "titleBar.inactiveBackground",
    "titleBar.inactiveForeground",
    "tree.indentGuidesStroke",
    "widget.shadow",
];
```

- [ ] **Step 2: Write the failing tests**

Add both module declarations to `lib.rs`. Create `crates/clippings-core/src/colours.rs` containing only:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classification() {
        assert!(is_hex("#ff0000") && is_hex("f00") && is_hex("#ff000080") && is_hex("bad"));
        assert!(!is_hex("#ff000") && !is_hex("red") && !is_hex("#ggg"));
        assert!(is_rgb("rgb(1, 2, 3)") && is_rgb("RGBA(1,2,3,0.5)") && !is_rgb("rgb(1,2)"));
        assert!(is_named("Red") && !is_named("reddish"));
        assert!(is_theme("editor.foreground") && !is_theme("editor.nope"));
        assert!(is_valid("peachpuff") && !is_valid("") && !is_valid("nope"));
    }

    #[test]
    fn opacity_matches_todo_tree() {
        assert_eq!(apply_opacity("#ff0000", 100.0), "rgba(255,0,0,1)");
        assert_eq!(apply_opacity("#f00", 50.0), "rgba(255,0,0,0.5)");
        assert_eq!(apply_opacity("#f00", 0.25), "rgba(255,0,0,0.25)");
        assert_eq!(apply_opacity("#ff000080", 100.0), "rgba(255,0,0,0.5)");
        assert_eq!(apply_opacity("rgb(1,2,3)", 100.0), "rgb(1,2,3)");
        assert_eq!(apply_opacity("rgb(1,2,3)", 40.0), "rgba(1,2,3,0.4)");
        assert_eq!(apply_opacity("red", 40.0), "red");
    }

    #[test]
    fn contrast() {
        assert_eq!(complementary("#ffff00"), Some("#000000"));
        assert_eq!(complementary("rgba(0,0,128,0.5)"), Some("#ffffff"));
        assert_eq!(complementary("red"), None);
    }
}
```

- [ ] **Step 3: Run the tests to verify they fail**

Run: `cargo test -p clippings-core colours`
Expected: compile error, `is_hex` and `apply_opacity` not found.

- [ ] **Step 4: Write the implementation**

Put this above the tests in `crates/clippings-core/src/colours.rs`:

```rust
//! Colour helpers ported from todo-tree's `utils.js` and `highlights.js`.

use crate::colour_names::{NAMED_COLOURS, THEME_COLOURS};
use regex::Regex;
use std::sync::OnceLock;

fn rgb_regex() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r"(?i)^rgba?\((\d+),\s*(\d+),\s*(\d+)(?:,\s*(\d+(?:\.\d+)?))?\)$").unwrap()
    })
}

/// `#rgb`, `#rgba`, `#rrggbb` or `#rrggbbaa`, with or without the `#`, as todo-tree accepts.
pub fn is_hex(colour: &str) -> bool {
    let without = colour.strip_prefix('#').unwrap_or(colour);
    let first = without.split(' ').next().unwrap_or("");
    let hex: String = first.chars().filter(|c| c.is_ascii_hexdigit()).collect();
    hex.len() == without.len() && matches!(hex.len(), 3 | 4 | 6 | 8)
}

pub fn is_rgb(colour: &str) -> bool {
    rgb_regex().is_match(colour)
}

fn named_set() -> &'static std::collections::HashSet<&'static str> {
    static SET: OnceLock<std::collections::HashSet<&'static str>> = OnceLock::new();
    SET.get_or_init(|| NAMED_COLOURS.into_iter().collect())
}

fn theme_set() -> &'static std::collections::HashSet<&'static str> {
    static SET: OnceLock<std::collections::HashSet<&'static str>> = OnceLock::new();
    SET.get_or_init(|| THEME_COLOURS.into_iter().collect())
}

pub fn is_named(colour: &str) -> bool {
    named_set().contains(colour.to_lowercase().as_str())
}

pub fn is_theme(colour: &str) -> bool {
    theme_set().contains(colour)
}

pub fn is_valid(colour: &str) -> bool {
    !colour.is_empty() && (is_named(colour) || is_theme(colour) || is_hex(colour) || is_rgb(colour))
}

/// A number formatted as JavaScript's `String(number)` does for these values.
pub fn js_number(n: f64) -> String {
    if n.fract() == 0.0 && n.abs() < 1e15 {
        format!("{}", n as i64)
    } else {
        format!("{n}")
    }
}

fn component(digits: &str) -> u32 {
    let d = if digits.len() == 1 {
        format!("{digits}{digits}")
    } else {
        digits.to_string()
    };
    u32::from_str_radix(&d, 16).unwrap_or(0)
}

/// todo-tree's `hexToRgba`: `opacity` is a percentage; hex alpha overrides it.
pub fn hex_to_rgba(hex: &str, opacity: f64) -> String {
    let hex = hex.replacen('#', "", 1);
    let rgb_len = if hex.len() == 3 || hex.len() == 4 {
        3
    } else {
        6
    };
    let rgb = &hex[..rgb_len.min(hex.len())];
    let n = rgb.len() / 3;
    let (r, g, b) = (
        component(&rgb[..n]),
        component(&rgb[n..2 * n]),
        component(&rgb[2 * n..3 * n]),
    );
    let mut opacity = opacity;
    if hex.len() == 4 || hex.len() == 8 {
        let a = &hex[3 * hex.len() / 4..];
        opacity = (component(a) as f64 * 100.0 / 255.0).trunc();
    }
    format!("rgba({r},{g},{b},{})", js_number(opacity / 100.0))
}

/// todo-tree's `setRgbAlpha`.
pub fn set_rgb_alpha(rgb: &str, alpha: f64) -> String {
    match rgb_regex().captures(rgb) {
        Some(c) => format!("rgba({},{},{},{})", &c[1], &c[2], &c[3], js_number(alpha)),
        None => rgb.to_string(),
    }
}

/// todo-tree's `applyOpacity`: hex becomes `rgba(...)`; rgb gets an alpha
/// unless the opacity is exactly 100; anything else is unchanged.
pub fn apply_opacity(colour: &str, opacity: f64) -> String {
    if is_hex(colour) {
        hex_to_rgba(
            colour,
            if opacity < 1.0 {
                opacity * 100.0
            } else {
                opacity
            },
        )
    } else if is_rgb(colour) {
        if opacity != 100.0 {
            set_rgb_alpha(
                colour,
                if opacity > 1.0 {
                    opacity / 100.0
                } else {
                    opacity
                },
            )
        } else {
            colour.to_string()
        }
    } else {
        colour.to_string()
    }
}

/// Red, green and blue of a hex or `rgb(a)` colour.
pub fn rgb_components(colour: &str) -> Option<(u32, u32, u32)> {
    if is_hex(colour) {
        let hex = colour.trim_start_matches('#');
        let rgb = &hex[..if hex.len() == 3 || hex.len() == 4 {
            3
        } else {
            6
        }];
        let n = rgb.len() / 3;
        return Some((
            component(&rgb[..n]),
            component(&rgb[n..2 * n]),
            component(&rgb[2 * n..]),
        ));
    }
    let c = rgb_regex().captures(colour)?;
    Some((c[1].parse().ok()?, c[2].parse().ok()?, c[3].parse().ok()?))
}

/// Black or white, whichever contrasts with `colour` (luminance threshold 0.179).
pub fn complementary(colour: &str) -> Option<&'static str> {
    let (r, g, b) = rgb_components(colour)?;
    let lin = |v: u32| {
        let c = v as f64 / 255.0;
        if c <= 0.03928 {
            c / 12.92
        } else {
            ((c + 0.055) / 1.055).powf(2.4)
        }
    };
    let l = 0.2126 * lin(r) + 0.7152 * lin(g) + 0.0722 * lin(b);
    Some(if l > 0.179 { "#000000" } else { "#ffffff" })
}
```

- [ ] **Step 5: Run the tests to verify they pass**

Run: `cargo test -p clippings-core colours`
Expected: 3 tests pass.
Then run: `cargo fmt --all && cargo clippy --all-targets -- -D warnings`
Expected: no diffs, no warnings.

- [ ] **Step 6: Commit**

```bash
git add -A
git commit -F - <<'EOF'
feat(core): colour validation, opacity and contrast

Co-Authored-By: <your model attribution>
EOF
```

### Task 5: Label placeholders

todo-tree's `formatLabel` for `tree.labelFormat`, `tree.tooltipFormat` and `tree.subTagClickUrl`, with a hand-written placeholder scanner. A regex replacement per todo cost several milliseconds per 10,000 todos. Unknown placeholders stay in the output as written.

**Files:**
- Create: `crates/clippings-core/src/labels.rs`
- Modify: `crates/clippings-core/src/lib.rs` (add `pub mod labels;`, keeping the declarations sorted)

**Interfaces:**
- Produces: `LabelFields<'a> { line (0-based), column (1-based), tag, sub_tag, before, after, file_path }` (`Default`), `format(template: &str, &LabelFields) -> String`, `unexpected_placeholder(template: &str) -> Option<String>`.

- [ ] **Step 1: Write the failing tests**

Add `pub mod labels;` to `crates/clippings-core/src/lib.rs`. Create `crates/clippings-core/src/labels.rs` containing only this test module:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    fn fields() -> LabelFields<'static> {
        LabelFields {
            line: 4,
            column: 3,
            tag: "todo",
            sub_tag: "alice",
            before: "x();",
            after: "fix it",
            file_path: "/r/src/a.ts",
        }
    }

    #[test]
    fn substitutes_every_placeholder_case_insensitively() {
        let f = fields();
        assert_eq!(format("${tag} ${after}", &f), "todo fix it");
        assert_eq!(
            format(
                "${TAG:UpperCase}|${subtag:capitalize}|${Line}:${column}",
                &f
            ),
            "TODO|Alice|5:3"
        );
        assert_eq!(format("${filename} ${filepath}", &f), "a.ts /r/src/a.ts");
        assert_eq!(
            format(
                "${afterorbefore}",
                &LabelFields {
                    after: "",
                    ..fields()
                }
            ),
            "x();"
        );
        assert_eq!(format("${unknown} stays", &f), "${unknown} stays");
    }

    #[test]
    fn reports_unexpected_placeholders() {
        assert_eq!(unexpected_placeholder("${tag} ${after}"), None);
        assert_eq!(
            unexpected_placeholder("${tag} ${nope}"),
            Some("${nope}".to_string())
        );
    }
}
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p clippings-core labels`
Expected: compile error, `format` and `LabelFields` not found.

- [ ] **Step 3: Write the implementation**

Put this above the test module in `crates/clippings-core/src/labels.rs`:

```rust
//! Label, tooltip and URL placeholders, ported from todo-tree's `formatLabel`.

use regex::Regex;
use std::sync::OnceLock;

/// The values a template can refer to.
#[derive(Clone, Debug, Default)]
pub struct LabelFields<'a> {
    /// 0-based line; `${line}` prints it 1-based.
    pub line: u32,
    /// `${column}` as stored: 1-based UTF-16 column.
    pub column: u32,
    pub tag: &'a str,
    pub sub_tag: &'a str,
    pub before: &'a str,
    pub after: &'a str,
    pub file_path: &'a str,
}

fn capitalize(s: &str) -> String {
    let mut c = s.chars();
    match c.next() {
        Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
        None => String::new(),
    }
}

/// The value for a placeholder name, compared case-insensitively, or `None`
/// for names todo-tree does not know (those stay in the output as written).
fn value(name: &str, f: &LabelFields, tag: &str, sub: &str) -> Option<String> {
    let is = |n: &str| name.eq_ignore_ascii_case(n);
    Some(if is("line") {
        (f.line + 1).to_string()
    } else if is("column") {
        f.column.to_string()
    } else if is("tag") {
        tag.to_string()
    } else if is("tag:uppercase") {
        tag.to_uppercase()
    } else if is("tag:lowercase") {
        tag.to_lowercase()
    } else if is("tag:capitalize") {
        capitalize(tag)
    } else if is("subtag") {
        sub.to_string()
    } else if is("subtag:uppercase") {
        sub.to_uppercase()
    } else if is("subtag:lowercase") {
        sub.to_lowercase()
    } else if is("subtag:capitalize") {
        capitalize(sub)
    } else if is("before") {
        f.before.to_string()
    } else if is("after") {
        f.after.to_string()
    } else if is("afterorbefore") {
        (if f.after.is_empty() {
            f.before
        } else {
            f.after
        })
        .to_string()
    } else if is("filename") {
        f.file_path
            .rsplit(['/', '\\'])
            .next()
            .unwrap_or("")
            .to_string()
    } else if is("filepath") {
        f.file_path.to_string()
    } else {
        return None;
    })
}

pub fn format(template: &str, f: &LabelFields) -> String {
    let tag = f.tag.trim();
    let sub = f.sub_tag.trim();
    let mut out = String::with_capacity(template.len() + f.after.len());
    let mut rest = template;
    while let Some(start) = rest.find("${") {
        out.push_str(&rest[..start]);
        let after = &rest[start + 2..];
        match after.find('}') {
            Some(end) => match value(&after[..end], f, tag, sub) {
                Some(v) => {
                    out.push_str(&v);
                    rest = &after[end + 1..];
                }
                None => {
                    out.push_str("${");
                    rest = after;
                }
            },
            None => {
                out.push_str(&rest[start..]);
                rest = "";
            }
        }
    }
    out.push_str(rest);
    out
}

/// The first unrecognised `${...}` left after formatting, as todo-tree's
/// greedy `\$\{.*\}` reports it.
pub fn unexpected_placeholder(template: &str) -> Option<String> {
    static RE: OnceLock<Regex> = OnceLock::new();
    let re = RE.get_or_init(|| Regex::new(r"\$\{.*\}").unwrap());
    let formatted = format(template, &LabelFields::default());
    re.find(&formatted).map(|m| m.as_str().to_string())
}
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test -p clippings-core labels`
Expected: 2 tests pass.
Then run: `cargo fmt --all && cargo clippy --all-targets -- -D warnings`
Expected: no diffs, no warnings.

- [ ] **Step 5: Commit**

```bash
git add -A
git commit -F - <<'EOF'
feat(core): label and tooltip placeholders

Co-Authored-By: <your model attribution>
EOF
```

### Task 6: Highlight attributes, styles and icons

Ports todo-tree's `attributes.js`, `getDecoration` and `getIcon` (spec section 5.13), with the auto-contrast fix. A `Resolver` answers per-key attribute questions: an exact `customHighlight` entry, then `defaultHighlight`, then the built-in default, and the colour scheme by tag position. The server turns keys into `DecorationStyle`s, and the view turns them into `IconDescriptor`s.

**Files:**
- Create: `crates/clippings-core/src/styles.rs`
- Modify: `crates/clippings-core/src/lib.rs` (add `pub mod styles;`, keeping the declarations sorted)

**Interfaces:**
- Consumes: `colours::*`, `Settings`, `Attributes`.
- Produces: `Colour { Theme(String), Css(String) }`, `IconDescriptor { Codicon{name, colour}, Octicon{name, colour}, TodoTree{filled, colour}, Check{colour}, Default, Folder, File }` (serde tag `kind`), `DecorationStyle { color, background_color, overview_ruler_color, overview_ruler_lane, border_radius, font_style, font_weight, text_decoration, is_whole_line, gutter_icon }` (camelCase).
- Produces: `Resolver::new(&Settings)`, `foreground(&str) -> Option<String>`, `background(&str)`, `icon_colour(&str) -> String`, `icon_name(&str) -> Option<String>`, `kind(&str) -> Option<String>`, `flag(&str, fn(&Attributes) -> Option<bool>) -> bool`, `icon(&str) -> IconDescriptor`, `style(&str) -> DecorationStyle`; `colour_warnings(&Settings) -> Vec<String>`.

- [ ] **Step 1: Write the failing tests**

Add `pub mod styles;` to `crates/clippings-core/src/lib.rs`. Create `crates/clippings-core/src/styles.rs` containing only this test module:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::settings::Attributes;

    fn settings(f: impl FnOnce(&mut Settings)) -> Settings {
        let mut s = Settings::default();
        f(&mut s);
        s
    }

    #[test]
    fn default_style_inverts_editor_colours() {
        let s = Settings::default();
        let st = Resolver::new(&s).style("TODO");
        assert_eq!(st.color, Some(Colour::Theme("editor.background".into())));
        assert_eq!(
            st.background_color,
            Some(Colour::Theme("editor.foreground".into()))
        );
        assert_eq!(st.overview_ruler_lane, Some(4));
        assert_eq!(
            st.overview_ruler_color,
            Some(Colour::Theme("editor.foreground".into()))
        );
        assert_eq!(st.border_radius, "0.2em");
        assert!(!st.is_whole_line && st.gutter_icon.is_none());
    }

    #[test]
    fn custom_then_default_then_builtin() {
        let s = settings(|s| {
            s.highlights.default_highlight = Attributes {
                foreground: Some("red".into()),
                font_weight: Some("bold".into()),
                ..Default::default()
            };
            s.highlights.custom_highlight.insert(
                "TODO".into(),
                Attributes {
                    foreground: Some("blue".into()),
                    ..Default::default()
                },
            );
        });
        let r = Resolver::new(&s);
        assert_eq!(r.foreground("TODO").as_deref(), Some("blue"));
        assert_eq!(r.foreground("FIXME").as_deref(), Some("red"));
        assert_eq!(r.style("TODO").font_weight, "bold");
        assert_eq!(
            r.foreground("TODOS").as_deref(),
            Some("red"),
            "keys match exactly"
        );
    }

    #[test]
    fn colour_scheme_by_tag_position_and_not_for_groups() {
        let s = settings(|s| {
            s.highlights.use_colour_scheme = true;
            s.highlights.default_highlight.background = Some("pink".into());
        });
        let r = Resolver::new(&s);
        assert_eq!(r.background("BUG").as_deref(), Some("red"));
        assert_eq!(r.background("TODO").as_deref(), Some("green"));
        assert_eq!(r.foreground("TODO").as_deref(), Some("white"));
        assert_eq!(
            r.background("GROUP"),
            None,
            "defaultHighlight ignored for colours with a scheme"
        );
        assert_eq!(r.icon_colour("BUG"), "red");
    }

    #[test]
    fn background_opacity_and_auto_contrast() {
        let s = settings(|s| {
            s.highlights.custom_highlight.insert(
                "TODO".into(),
                Attributes {
                    background: Some("#ffff00".into()),
                    opacity: Some(50.0),
                    ..Default::default()
                },
            );
        });
        let st = Resolver::new(&s).style("TODO");
        assert_eq!(
            st.background_color,
            Some(Colour::Css("rgba(255,255,0,0.5)".into()))
        );
        assert_eq!(st.color, Some(Colour::Css("#000000".into())));
        assert_eq!(
            st.overview_ruler_color,
            Some(Colour::Css("rgba(255,255,0,0.5)".into()))
        );
    }

    #[test]
    fn theme_and_invalid_colours() {
        let s = settings(|s| {
            s.highlights.custom_highlight.insert(
                "A".into(),
                Attributes {
                    foreground: Some("errorForeground".into()),
                    background: Some("nonsense".into()),
                    ..Default::default()
                },
            );
        });
        let st = Resolver::new(&s).style("A");
        assert_eq!(st.color, Some(Colour::Theme("errorForeground".into())));
        assert_eq!(
            st.background_color,
            Some(Colour::Theme("editor.background".into()))
        );
        assert_eq!(
            colour_warnings(&s),
            vec!["Invalid colour settings: customHighlight.A.background (nonsense)"]
        );
    }

    #[test]
    fn ruler_lanes() {
        let lane = |v: Value| {
            let s = settings(|s| {
                s.highlights.custom_highlight.insert(
                    "T".into(),
                    Attributes {
                        ruler_lane: Some(v),
                        ..Default::default()
                    },
                );
            });
            Resolver::new(&s).style("T").overview_ruler_lane
        };
        assert_eq!(lane(Value::String("left".into())), Some(1));
        assert_eq!(lane(Value::String("FULL".into())), Some(7));
        assert_eq!(lane(Value::String("none".into())), None);
        assert_eq!(lane(Value::String("2".into())), Some(2));
        assert_eq!(lane(serde_json::json!(3)), Some(3));
    }

    #[test]
    fn icons() {
        let s = settings(|s| {
            let h = &mut s.highlights.custom_highlight;
            h.insert(
                "A".into(),
                Attributes {
                    icon: Some("$(bug)".into()),
                    icon_colour: Some("errorForeground".into()),
                    ..Default::default()
                },
            );
            h.insert(
                "B".into(),
                Attributes {
                    icon: Some("todo-tree-filled".into()),
                    icon_colour: Some("badge.background".into()),
                    ..Default::default()
                },
            );
            h.insert(
                "C".into(),
                Attributes {
                    background: Some("#123456".into()),
                    ..Default::default()
                },
            );
        });
        let r = Resolver::new(&s);
        assert_eq!(
            r.icon("A"),
            IconDescriptor::Codicon {
                name: "bug".into(),
                colour: Some("errorForeground".into())
            }
        );
        assert_eq!(
            r.icon("B"),
            IconDescriptor::TodoTree {
                filled: true,
                colour: "green".into()
            }
        );
        assert_eq!(
            r.icon("BUG"),
            IconDescriptor::Octicon {
                name: "bug".into(),
                colour: "green".into()
            }
        );
        assert_eq!(
            r.icon("C"),
            IconDescriptor::Check {
                colour: "#123456".into()
            }
        );
        let plain = Settings {
            highlights: crate::settings::Highlights {
                custom_highlight: Default::default(),
                ..Default::default()
            },
            ..Default::default()
        };
        assert_eq!(
            Resolver::new(&plain).icon("TODO"),
            IconDescriptor::Check {
                colour: "green".into()
            }
        );
    }
}
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p clippings-core styles`
Expected: compile error, `Resolver` not found.

- [ ] **Step 3: Write the implementation**

Put this above the test module in `crates/clippings-core/src/styles.rs`:

```rust
//! Highlight attribute resolution, decoration styles and icon descriptors
//! (spec section 5.13), ported from todo-tree's `attributes.js`,
//! `highlights.js` and `icons.js`.

use crate::colours::{apply_opacity, complementary, is_hex, is_named, is_rgb, is_theme, is_valid};
use crate::settings::{Attributes, Settings};
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// A colour for the client: a theme colour id or a CSS colour string.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Colour {
    Theme(String),
    Css(String),
}

/// What the client renders as an icon. SVG rendering and the octicon name
/// check happen on the client, which owns the octicon set; an unknown
/// octicon name falls back to `check` there.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", tag = "kind")]
pub enum IconDescriptor {
    /// A codicon; `colour` is a theme colour id when one was given.
    Codicon {
        name: String,
        colour: Option<String>,
    },
    /// An octicon rendered in `colour`.
    Octicon { name: String, colour: String },
    /// todo-tree's own icon, outline or filled.
    TodoTree { filled: bool, colour: String },
    /// The check-circle icon in `colour`.
    Check { colour: String },
    /// The bundled green default icon.
    Default,
    /// VS Code's folder icon from the file icon theme.
    Folder,
    /// VS Code's file icon from the file icon theme, for `resourceUri`.
    File,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DecorationStyle {
    pub color: Option<Colour>,
    pub background_color: Option<Colour>,
    pub overview_ruler_color: Option<Colour>,
    pub overview_ruler_lane: Option<u32>,
    pub border_radius: String,
    pub font_style: String,
    pub font_weight: String,
    pub text_decoration: String,
    pub is_whole_line: bool,
    pub gutter_icon: Option<IconDescriptor>,
}

/// Attribute lookup for one key (a tag, group, sub-tag or untagged match).
pub struct Resolver<'a> {
    settings: &'a Settings,
}

impl<'a> Resolver<'a> {
    pub fn new(settings: &'a Settings) -> Self {
        Self { settings }
    }

    /// todo-tree's `getAttribute`: an exact `customHighlight` entry, then
    /// `defaultHighlight` unless `ignore_default`.
    fn attr<T: Clone>(
        &self,
        key: &str,
        get: impl Fn(&Attributes) -> Option<T>,
        ignore_default: bool,
    ) -> Option<T> {
        if let Some(v) = self.settings.custom(key).and_then(&get) {
            return Some(v);
        }
        if ignore_default {
            None
        } else {
            get(&self.settings.highlights.default_highlight)
        }
    }

    fn scheme(&self, key: &str, colours: &[String]) -> Option<String> {
        let i = self.settings.tags().iter().position(|t| t == key)?;
        if colours.is_empty() {
            None
        } else {
            Some(colours[i % colours.len()].clone())
        }
    }

    pub fn foreground(&self, key: &str) -> Option<String> {
        let s = self.settings.highlights.use_colour_scheme;
        self.attr(key, |a| a.foreground.clone(), s).or_else(|| {
            s.then(|| self.scheme(key, &self.settings.highlights.foreground_colour_scheme))
                .flatten()
        })
    }

    pub fn background(&self, key: &str) -> Option<String> {
        let s = self.settings.highlights.use_colour_scheme;
        self.attr(key, |a| a.background.clone(), s).or_else(|| {
            s.then(|| self.scheme(key, &self.settings.highlights.background_colour_scheme))
                .flatten()
        })
    }

    pub fn icon_colour(&self, key: &str) -> String {
        let s = self.settings.highlights.use_colour_scheme;
        self.attr(key, |a| a.icon_color.clone(), false)
            .or_else(|| self.attr(key, |a| a.icon_colour.clone(), s))
            .or_else(|| {
                s.then(|| self.scheme(key, &self.settings.highlights.background_colour_scheme))
                    .flatten()
            })
            .or_else(|| self.attr(key, |a| a.foreground.clone(), s))
            .or_else(|| self.attr(key, |a| a.background.clone(), s))
            .unwrap_or_else(|| "green".to_string())
    }

    /// The `icon` attribute, with `defaultHighlight` applying.
    pub fn icon_name(&self, key: &str) -> Option<String> {
        self.attr(key, |a| a.icon.clone(), false)
    }

    pub fn kind(&self, key: &str) -> Option<String> {
        self.attr(key, |a| a.kind.clone(), false)
    }

    pub fn flag(&self, key: &str, get: fn(&Attributes) -> Option<bool>) -> bool {
        self.attr(key, get, false).unwrap_or(false)
    }

    pub fn icon(&self, key: &str) -> IconDescriptor {
        let colour = self.icon_colour(key);
        let not_theme = |c: &str| {
            if is_theme(c) {
                "green".to_string()
            } else {
                c.to_string()
            }
        };
        match self.attr(key, |a| a.icon.clone(), false) {
            Some(name) if name == "todo-tree" || name == "todo-tree-filled" => {
                IconDescriptor::TodoTree {
                    filled: name.ends_with("filled"),
                    colour: not_theme(&colour),
                }
            }
            Some(name) if name.trim().starts_with("$(") => {
                let t = name.trim();
                IconDescriptor::Codicon {
                    name: t[2..t.len().saturating_sub(1).max(2)].to_string(),
                    colour: is_theme(&colour).then_some(colour),
                }
            }
            Some(name) if !name.is_empty() => IconDescriptor::Octicon {
                name,
                colour: not_theme(&colour),
            },
            _ if is_hex(&colour) || is_rgb(&colour) || is_named(&colour) => {
                IconDescriptor::Check { colour }
            }
            _ => IconDescriptor::Default,
        }
    }

    fn lane(&self, key: &str) -> Option<u32> {
        match self.attr(key, |a| a.ruler_lane.clone(), false) {
            None => Some(4),
            Some(Value::Number(n)) => n.as_u64().map(|n| n as u32),
            Some(Value::String(s)) => match s.trim().parse::<u32>() {
                Ok(n) => Some(n),
                Err(_) => match s.to_lowercase().as_str() {
                    "left" => Some(1),
                    "center" => Some(2),
                    "right" => Some(4),
                    "full" => Some(7),
                    _ => None,
                },
            },
            Some(_) => None,
        }
    }

    fn colour(value: &str, fallback: &str) -> Colour {
        let lower = value.to_lowercase();
        if lower.contains("foreground") || lower.contains("background") {
            Colour::Theme(value.to_string())
        } else if !is_valid(value) {
            Colour::Theme(fallback.to_string())
        } else {
            Colour::Css(value.to_string())
        }
    }

    /// todo-tree's `getDecoration`, with the auto-contrast fix.
    pub fn style(&self, key: &str) -> DecorationStyle {
        let fg = self
            .foreground(key)
            .map(|c| Self::colour(&c, "editor.foreground"));
        let opacity = self.attr(key, |a| a.opacity, false).unwrap_or(100.0);
        let bg = self
            .background(key)
            .map(|c| match Self::colour(&c, "editor.background") {
                Colour::Css(css) => Colour::Css(apply_opacity(&css, opacity)),
                theme => theme,
            });
        let fg = fg.or_else(|| match &bg {
            Some(Colour::Css(css)) => complementary(css).map(|c| Colour::Css(c.to_string())),
            _ => None,
        });
        let (fg, bg) = match (fg, bg) {
            (None, None) => (
                Some(Colour::Theme("editor.background".into())),
                Some(Colour::Theme("editor.foreground".into())),
            ),
            other => other,
        };
        let lane = self.lane(key);
        let ruler = lane.map(|_| {
            let ruler_opacity = self.attr(key, |a| a.ruler_opacity, false).unwrap_or(100.0);
            match self.attr(key, |a| a.ruler_colour.clone(), false) {
                Some(c) if is_theme(&c) => Colour::Theme(c),
                Some(c) => Colour::Css(apply_opacity(&c, ruler_opacity)),
                None => match &bg {
                    Some(Colour::Css(c)) => Colour::Css(apply_opacity(c, ruler_opacity)),
                    Some(theme) => theme.clone(),
                    None => Colour::Theme("editor.foreground".into()),
                },
            }
        });
        let gutter = self.flag(key, |a| a.gutter_icon).then(|| self.icon(key));
        DecorationStyle {
            color: fg,
            background_color: bg,
            overview_ruler_color: ruler,
            overview_ruler_lane: lane,
            border_radius: self
                .attr(key, |a| a.border_radius.clone(), false)
                .unwrap_or_else(|| "0.2em".into()),
            font_style: self
                .attr(key, |a| a.font_style.clone(), false)
                .unwrap_or_else(|| "normal".into()),
            font_weight: self
                .attr(key, |a| a.font_weight.clone(), false)
                .unwrap_or_else(|| "normal".into()),
            text_decoration: self
                .attr(key, |a| a.text_decoration.clone(), false)
                .unwrap_or_default(),
            is_whole_line: self.kind(key).as_deref() == Some("whole-line"),
            gutter_icon: gutter,
        }
    }
}

/// Configuration warnings for invalid colours (todo-tree's `validateColours`).
pub fn colour_warnings(settings: &Settings) -> Vec<String> {
    let mut bad = Vec::new();
    let mut check = |prefix: &str, a: &Attributes| {
        for (name, v) in [
            ("foreground", &a.foreground),
            ("background", &a.background),
            ("iconColour", &a.icon_colour),
            ("rulerColour", &a.ruler_colour),
        ] {
            if let Some(v) = v {
                if !is_valid(v) {
                    bad.push(format!("{prefix}.{name} ({v})"));
                }
            }
        }
    };
    check("defaultHighlight", &settings.highlights.default_highlight);
    for (k, a) in &settings.highlights.custom_highlight {
        check(&format!("customHighlight.{k}"), a);
    }
    if bad.is_empty() {
        Vec::new()
    } else {
        vec![format!("Invalid colour settings: {}", bad.join(", "))]
    }
}
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test -p clippings-core styles`
Expected: 7 tests pass.
Then run: `cargo fmt --all && cargo clippy --all-targets -- -D warnings`
Expected: no diffs, no warnings.

- [ ] **Step 5: Commit**

```bash
git add -A
git commit -F - <<'EOF'
feat(core): highlight attribute resolution, styles and icons

Co-Authored-By: <your model attribution>
EOF
```

### Task 7: View model: placement, shaping and rendering

Spec section 5.12. `place` puts every effective todo into an arena with stable IDs, `shape` computes visibility, counts, order and compaction plus the status pseudo-nodes, and `render` produces the `ViewNode`s and answers `children`, `find` and per-key counts.

**Files:**
- Create: `crates/clippings-core/src/view/mod.rs`, `crates/clippings-core/src/view/place.rs`, `crates/clippings-core/src/view/shape.rs`, `crates/clippings-core/src/view/render.rs`, `crates/clippings-core/src/view/tests.rs`
- Modify: `crates/clippings-core/src/lib.rs` (add `pub mod view;`)

**Interfaces:**
- Consumes: `index::{EffectiveFile, SourcedTodo}`, `Settings`, `Resolver`, `labels::format`, `uri::{file_uri, uri_basename, uri_to_path}`, `roots::deepest_root`.
- Produces: `view::{Kind, TodoData, Node, Arena}` (`Arena::get_or_add(parent, key, make) -> usize` builds IDs), `place::{place, raw_label, todo_key}`, `shape::{shape, Shaped, filter_regex}`, `render::{View, ViewNode, NodeCommand}`.
- Produces: `View::build(&Settings, &[EffectiveFile], tree_roots: &[PathBuf]) -> View`, `View::children_of(Option<&str>) -> Vec<ViewNode>`, `View::find(uri, line: Option<u32>) -> Vec<Vec<ViewNode>>`, `View::counts(&Settings, hide: fn(&Attributes) -> Option<bool>, file: Option<&Path>) -> Vec<(String, usize)>`; fields `nodes`, `children`, `parents`, `has_sub_tags`, `is_empty`, `arena`, `shaped` are `pub` for the delta and export passes.
- `ViewNode { id, label, description, tooltip, icon, has_children, default_expanded, context_value, resource_uri, command }` is the wire shape of spec 6.2 (camelCase). `NodeCommand` is `Reveal { uri, position }` or `OpenUrl { url }` (serde tag `kind`).

- [ ] **Step 1: Write the failing tests**

Add `pub mod view;` to `lib.rs`. Create `crates/clippings-core/src/view/mod.rs` with exactly this content (Task 8 adds `delta` and `export`):

```rust
//! The tree view model (spec section 5.12): placement, filtering, sorting,
//! counts, compaction, rendering, deltas and export.
//!
//! A build runs in three passes. `place` puts every effective todo into an
//! arena of nodes with stable IDs. `shape` computes visibility, counts,
//! order and compaction. `render` turns the shaped arena into the
//! `ViewNode`s the client displays.

pub mod place;
pub mod render;
pub mod shape;
#[cfg(test)]
mod tests;

use crate::position::Position;
use std::collections::HashMap;
use std::path::PathBuf;

pub use render::{NodeCommand, ViewNode};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    /// A workspace folder.
    Root,
    /// A tag or tag-group level (`g:`).
    Tag,
    /// A sub-tag level or pseudo-folder (`s:`).
    SubTag,
    Folder,
    File,
    Todo,
    /// A continuation line of a multi-line todo.
    Extra,
    Status,
}

/// What a todo or extra-line node needs for labels, tooltips and commands.
#[derive(Clone, Debug, PartialEq)]
pub struct TodoData {
    /// The document URI to reveal: the buffer URI, or the file's `file:` URI.
    pub uri: String,
    pub start: Position,
    pub text_end: Position,
    /// The actual tag, before group mapping; empty when untagged.
    pub tag: String,
    pub sub_tag: Option<String>,
    pub before: String,
    pub after: String,
    /// For todo nodes: whether the match had continuation lines.
    pub multi_line: bool,
    /// For extra-line nodes: the line text.
    pub text: String,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Node {
    pub kind: Kind,
    /// The node's ID: its parent's ID, `/`, and its own key.
    pub id: String,
    /// The display name before decoration: folder or file name, tag, sub-tag.
    pub name: String,
    pub parent: Option<usize>,
    pub children: Vec<usize>,
    /// Filesystem path for roots, folders, files and todos.
    pub path: Option<PathBuf>,
    /// Document URI for file and todo nodes.
    pub uri: Option<String>,
    /// The decoration key (group or tag) for tag levels and todos.
    pub key: Option<String>,
    pub sub_tag: Option<String>,
    /// ` (dir)` suffix for files in the flat view and outside every root.
    pub path_label: Option<String>,
    pub todo: Option<TodoData>,
    /// Hidden by `hideFromTree`.
    pub hidden: bool,
    /// Status node text and icon.
    pub status: Option<(String, &'static str, Option<String>)>,
}

/// A built view: the arena plus the shaped order.
#[derive(Clone, Debug, Default)]
pub struct Arena {
    pub nodes: Vec<Node>,
    pub top: Vec<usize>,
    pub by_id: HashMap<String, usize>,
}

impl Arena {
    /// Returns the existing node with this parent and key, or adds one.
    pub fn get_or_add(
        &mut self,
        parent: Option<usize>,
        key: &str,
        make: impl FnOnce() -> Node,
    ) -> usize {
        let id = match parent {
            Some(p) => format!("{}/{}", self.nodes[p].id, key),
            None => key.to_string(),
        };
        if let Some(&i) = self.by_id.get(&id) {
            return i;
        }
        let mut node = make();
        node.id = id.clone();
        node.parent = parent;
        let i = self.nodes.len();
        self.nodes.push(node);
        match parent {
            Some(p) => self.nodes[p].children.push(i),
            None => self.top.push(i),
        }
        self.by_id.insert(id, i);
        i
    }
}

impl Node {
    pub fn new(kind: Kind, name: impl Into<String>) -> Self {
        Node {
            kind,
            id: String::new(),
            name: name.into(),
            parent: None,
            children: Vec::new(),
            path: None,
            uri: None,
            key: None,
            sub_tag: None,
            path_label: None,
            todo: None,
            hidden: false,
            status: None,
        }
    }

    pub fn is_container(&self) -> bool {
        !matches!(self.kind, Kind::Todo | Kind::Extra | Kind::Status)
    }

    pub fn is_folder_like(&self) -> bool {
        matches!(self.kind, Kind::Root | Kind::Folder | Kind::SubTag)
    }
}
```

Create `crates/clippings-core/src/view/tests.rs`:

```rust
use super::render::{NodeCommand, View};
use crate::index::{EffectiveFile, Source, SourcedTodo};
use crate::model::{ExtraLine, Todo};
use crate::position::Position;
use crate::settings::{Attributes, RevealBehaviour, Settings};
use crate::styles::IconDescriptor;
use std::path::PathBuf;

fn pos(line: u32, character: u32) -> Position {
    Position { line, character }
}

fn todo(line: u32, tag: &str, after: &str) -> Todo {
    Todo {
        start: pos(line, 0),
        end: pos(line, 2 + tag.len() as u32),
        text_end: pos(line, 3 + (tag.len() + after.len()) as u32),
        tag_start: Some(pos(line, 3)),
        tag_end: Some(pos(line, 3 + tag.len() as u32)),
        tag: tag.into(),
        sub_tag: None,
        before: String::new(),
        after: after.into(),
        extra_lines: vec![],
    }
}

struct Fixture {
    files: Vec<(PathBuf, Vec<Todo>)>,
}

impl Fixture {
    fn new() -> Self {
        Fixture { files: Vec::new() }
    }
    fn file(mut self, path: &str, todos: Vec<Todo>) -> Self {
        self.files.push((PathBuf::from(path), todos));
        self
    }
    fn build(&self, s: &Settings) -> View {
        let files: Vec<EffectiveFile> = self
            .files
            .iter()
            .map(|(p, t)| EffectiveFile {
                path: Some(p.as_path()),
                uri: None,
                source: Source::Disk,
                todos: t
                    .iter()
                    .map(|todo| SourcedTodo {
                        buffer_uri: None,
                        todo,
                    })
                    .collect(),
            })
            .collect();
        View::build(s, &files, &[PathBuf::from("/w")])
    }
}

fn quiet() -> Settings {
    let mut s = Settings::default();
    s.tree.show_current_scan_mode = false;
    s
}

fn labels(v: &View, parent: Option<&str>) -> Vec<String> {
    v.children_of(parent).into_iter().map(|n| n.label).collect()
}

fn ids(v: &View, parent: Option<&str>) -> Vec<String> {
    v.children_of(parent).into_iter().map(|n| n.id).collect()
}

fn sample() -> Fixture {
    Fixture::new()
        .file(
            "/w/src/b.ts",
            vec![todo(4, "FIXME", "later"), todo(1, "TODO", "first")],
        )
        .file("/w/src/a.ts", vec![todo(0, "BUG", "")])
        .file("/w/README.md", vec![todo(2, "TODO", "docs")])
}

#[test]
fn tree_view_places_root_folders_files_and_todos() {
    let v = sample().build(&quiet());
    assert_eq!(ids(&v, None), vec!["w:file:///w"]);
    assert_eq!(
        labels(&v, Some("w:file:///w")),
        vec!["src", "README.md"],
        "folders before files"
    );
    let src = "w:file:///w/d:/w/src";
    assert_eq!(labels(&v, Some(src)), vec!["a.ts", "b.ts"]);
    let b = format!("{src}/f:/w/src/b.ts");
    assert_eq!(
        ids(&v, Some(&b)),
        vec![format!("{b}/t:1:0"), format!("{b}/t:4:0")]
    );
    assert_eq!(labels(&v, Some(&b)), vec!["TODO first", "FIXME later"]);
    assert_eq!(
        labels(&v, Some(&format!("{src}/f:/w/src/a.ts"))),
        vec!["BUG "],
        "empty after keeps the format"
    );
    let root = &v.nodes["w:file:///w"];
    assert_eq!(root.context_value.as_deref(), Some("folder"));
    assert_eq!(root.resource_uri.as_deref(), Some("file:///w"));
    assert_eq!(v.nodes[&b].context_value.as_deref(), Some("file"));
    assert_eq!(v.nodes[&b].icon, Some(IconDescriptor::File));
    assert!(!v.is_empty);
}

#[test]
fn raw_label_when_format_is_empty() {
    let mut s = quiet();
    s.tree.label_format = String::new();
    let v = sample().build(&s);
    let a = "w:file:///w/d:/w/src/f:/w/src/a.ts";
    assert_eq!(labels(&v, Some(a)), vec!["BUG line 1"]);
}

#[test]
fn flat_view_uses_path_labels() {
    let mut s = quiet();
    s.view_state.flat = Some(true);
    let v = sample().build(&s);
    assert_eq!(
        labels(&v, Some("w:file:///w")),
        vec!["README.md", "a.ts (src)", "b.ts (src)"]
    );
}

#[test]
fn files_outside_every_root_are_top_level_with_absolute_dir() {
    let v = Fixture::new()
        .file("/elsewhere/x.ts", vec![todo(0, "TODO", "out")])
        .build(&quiet());
    assert_eq!(labels(&v, None), vec!["x.ts (/elsewhere)"]);
}

#[test]
fn grouping_by_tag_and_root_compaction_in_tags_only() {
    let mut s = quiet();
    s.view_state.tags_only = Some(true);
    s.view_state.grouped_by_tag = Some(true);
    let v = sample().build(&s);
    // TODO has two todos; BUG and FIXME have one each, so their tag nodes are
    // replaced by the todo. Order follows general.tags: BUG, HACK, FIXME, TODO.
    assert_eq!(labels(&v, None), vec!["BUG ", "FIXME later", "TODO"]);
    assert_eq!(labels(&v, Some("g:TODO")), vec!["TODO docs", "TODO first"]);
}

#[test]
fn tags_only_ungrouped_todo_ids_include_the_uri() {
    let mut s = quiet();
    s.view_state.tags_only = Some(true);
    let v = sample().build(&s);
    let top = ids(&v, None);
    assert!(
        top.contains(&"t:file:///w/src/b.ts:1:0".to_string()),
        "{top:?}"
    );
    assert_eq!(top.len(), 4);
}

#[test]
fn sub_tags_make_pseudo_folders_or_levels() {
    let mut t = todo(0, "TODO", "ship");
    t.sub_tag = Some("alice".into());
    let fx = Fixture::new().file("/w/a.ts", vec![t]);
    let v = fx.build(&quiet());
    let file = "w:file:///w/f:/w/a.ts";
    assert_eq!(labels(&v, Some(file)), vec!["alice"]);
    assert!(v.has_sub_tags);
    let mut s = quiet();
    s.view_state.grouped_by_sub_tag = Some(true);
    s.tree.sub_tag_click_url = "https://x/${subtag}".into();
    let v = fx.build(&s);
    let level = "w:file:///w/s:alice";
    assert_eq!(ids(&v, Some("w:file:///w")), vec![level]);
    assert_eq!(
        v.nodes[level].command,
        Some(NodeCommand::OpenUrl {
            url: "https://x/alice".into()
        })
    );
    assert_eq!(
        v.nodes[level].tooltip.as_deref(),
        Some("Click to open https://x/alice")
    );
    assert_eq!(v.nodes[level].context_value, None);
}

#[test]
fn filter_hides_non_matching_and_falls_back_to_literal() {
    let mut s = quiet();
    s.view_state.filter = "LATER".into();
    let v = sample().build(&s);
    let src = "w:file:///w/d:/w/src";
    assert_eq!(labels(&v, Some(src)), vec!["b.ts"]);
    s.view_state.filter = "(".into();
    let v = sample().build(&s);
    assert!(v.is_empty);
    let status = v.children_of(None);
    assert_eq!(
        status[0].description.as_deref(),
        Some("1 filter active, Nothing found")
    );
    assert_eq!(
        status[0].icon,
        Some(IconDescriptor::Codicon {
            name: "issues".into(),
            colour: None
        })
    );
}

#[test]
fn hide_from_tree_and_counts() {
    let mut s = quiet();
    s.tree.show_counts_in_tree = true;
    s.highlights.custom_highlight.insert(
        "FIXME".into(),
        Attributes {
            hide_from_tree: Some(true),
            ..Default::default()
        },
    );
    s.highlights.custom_highlight.insert(
        "BUG".into(),
        Attributes {
            hide_from_activity_bar: Some(true),
            ..Default::default()
        },
    );
    let v = sample().build(&s);
    let b = "w:file:///w/d:/w/src/f:/w/src/b.ts";
    assert_eq!(labels(&v, Some(b)), vec!["TODO first"]);
    assert_eq!(
        v.nodes["w:file:///w"].description.as_deref(),
        Some("2"),
        "BUG not counted, FIXME hidden"
    );
}

#[test]
fn compact_folders_join_single_folder_chains() {
    let mut s = quiet();
    s.explorer_compact_folders = true;
    let v = Fixture::new()
        .file("/w/a/b/c/x.ts", vec![todo(0, "TODO", "deep")])
        .build(&s);
    let kids = v.children_of(Some("w:file:///w"));
    assert_eq!(kids.len(), 1);
    assert_eq!(kids[0].label, "a/b/c");
    assert!(kids[0].id.ends_with("d:/w/a/b/c"));
    assert_eq!(labels(&v, Some(&kids[0].id)), vec!["x.ts"]);
}

#[test]
fn status_nodes_come_first() {
    let mut s = Settings::default();
    s.view_state.exclude_globs = vec!["**/gen/**".into()];
    let v = sample().build(&s);
    let top = v.children_of(None);
    assert_eq!(
        top[0].description.as_deref(),
        Some("Scan mode: workspace and open files")
    );
    assert_eq!(top[1].description.as_deref(), Some("1 filter active"));
    assert_eq!(
        top[1].tooltip.as_deref(),
        Some("Exclude: **/gen/**\n\nRight click for filter options")
    );
    assert_eq!(top[0].label, "");
}

#[test]
fn multi_line_todos_show_the_tag_and_extra_lines() {
    let mut t = todo(3, "TODO", "first");
    t.extra_lines = vec![
        ExtraLine {
            line: 4,
            text: "second".into(),
        },
        ExtraLine {
            line: 5,
            text: "third".into(),
        },
    ];
    let v = Fixture::new().file("/w/m.ts", vec![t]).build(&quiet());
    let file = "w:file:///w/f:/w/m.ts";
    let node = &v.children_of(Some(file))[0];
    assert_eq!(node.label, "TODO");
    assert!(node.has_children && node.default_expanded);
    assert_eq!(labels(&v, Some(&node.id)), vec!["second", "third"]);
}

#[test]
fn reveal_positions_follow_reveal_behaviour() {
    let fx = Fixture::new().file("/w/a.ts", vec![todo(2, "TODO", "go")]);
    let id = "w:file:///w/f:/w/a.ts/t:2:0";
    for (b, p) in [
        (RevealBehaviour::StartOfTodo, pos(2, 0)),
        (RevealBehaviour::StartOfLine, pos(2, 0)),
        (RevealBehaviour::EndOfTodo, pos(2, 9)),
    ] {
        let mut s = quiet();
        s.general.reveal_behaviour = b;
        let v = fx.build(&s);
        assert_eq!(
            v.nodes[id].command,
            Some(NodeCommand::Reveal {
                uri: "file:///w/a.ts".into(),
                position: p
            })
        );
    }
    let v = fx.build(&quiet());
    assert_eq!(v.nodes[id].tooltip.as_deref(), Some("/w/a.ts, line 3"));
}

#[test]
fn find_returns_paths_from_the_top() {
    let v = sample().build(&quiet());
    let paths = v.find("file:///w/src/b.ts", None);
    assert_eq!(paths.len(), 1);
    assert_eq!(
        paths[0]
            .iter()
            .map(|n| n.label.as_str())
            .collect::<Vec<_>>(),
        vec!["w", "src", "b.ts"]
    );
    let todos = v.find("file:///w/src/b.ts", Some(4));
    assert_eq!(todos[0].last().unwrap().label, "FIXME later");
    assert!(v.find("file:///w/nope.ts", None).is_empty());
}
```

Create empty placeholders so the module tree resolves: `crates/clippings-core/src/view/place.rs`, `crates/clippings-core/src/view/shape.rs` and `crates/clippings-core/src/view/render.rs`, each containing only a `//!` doc line.

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p clippings-core view::`
Expected: compile error, unresolved `super::render::View` and `place`.

- [ ] **Step 3: Write placement**

Replace `crates/clippings-core/src/view/place.rs`:

```rust
//! Placement (spec section 5.12): where each todo goes in the tree, flat
//! and tags-only views, and each node's ID.

use super::{Arena, Kind, Node, TodoData};
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
                                    &format!("d:{}", d.display()),
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
                        Some(p) => format!("f:{}", p.display()),
                        None => format!("f:{}", doc_uri.clone().unwrap_or_default()),
                    };
                    let file_idx = a.get_or_add(parent, &file_key, || {
                        let mut n = Node::new(
                            Kind::File,
                            file.path.map(name_of).unwrap_or_else(|| uri_basename(&uri)),
                        );
                        n.path = file.path.map(Path::to_path_buf);
                        n.uri = Some(disk_uri.clone().unwrap_or_else(|| uri.clone()));
                        let dir = file.path.and_then(|p| p.parent()).map(|d| match root {
                            Some(r) => d
                                .strip_prefix(r)
                                .unwrap_or(d)
                                .to_string_lossy()
                                .into_owned(),
                            None => d.to_string_lossy().into_owned(),
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

            let has_file_parent = parent.is_some_and(|p| {
                let k = a.nodes[p].kind;
                k == Kind::File
                    || (k == Kind::SubTag
                        && a.nodes[p]
                            .parent
                            .is_some_and(|g| a.nodes[g].kind == Kind::File))
            });
            let todo_key = if has_file_parent {
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

- [ ] **Step 4: Write shaping**

Replace `crates/clippings-core/src/view/shape.rs`:

```rust
//! Shaping (spec section 5.12): filter and visibility, counts, sort order,
//! folder and root compaction, and the status pseudo-nodes.

use super::{Arena, Kind, Node};
use crate::config::ScanMode;
use crate::settings::Settings;
use crate::styles::Resolver;
use regex::{Regex, RegexBuilder};
use std::cmp::Ordering;

/// The arena after shaping: which nodes show, in what order, with what counts.
#[derive(Clone, Debug, Default)]
pub struct Shaped {
    pub visible: Vec<bool>,
    /// Count of visible, counted todos at or below each node.
    pub counts: Vec<usize>,
    /// Displayed children per node, sorted, with compacted chains collapsed.
    pub children: Vec<Vec<usize>>,
    /// Displayed top-level nodes, status nodes first.
    pub top: Vec<usize>,
    /// Displayed parent per node.
    pub parent: Vec<Option<usize>>,
    /// Label override for the end of a compacted folder chain.
    pub compact_label: Vec<Option<String>>,
}

/// The filter as a regex: case-sensitive per `tree.filterCaseSensitive`,
/// literal text when it is not a valid regex, `None` when empty.
pub fn filter_regex(settings: &Settings) -> Option<Regex> {
    let text = settings.view_state.filter.as_str();
    if text.is_empty() {
        return None;
    }
    let ci = !settings.tree.filter_case_sensitive;
    RegexBuilder::new(text)
        .case_insensitive(ci)
        .build()
        .or_else(|_| {
            RegexBuilder::new(&regex::escape(text))
                .case_insensitive(ci)
                .build()
        })
        .ok()
}

/// Everything the comparator needs, computed once per node.
struct SortKey {
    folder_like: bool,
    tagged: bool,
    tag_index: i64,
    path: String,
    pos: (u32, u32),
}

fn sort_keys(a: &Arena, settings: &Settings, tags_only: bool) -> Vec<SortKey> {
    let order: std::collections::HashMap<String, i64> = settings
        .tags()
        .into_iter()
        .enumerate()
        .map(|(i, t)| (t, i as i64))
        .collect();
    a.nodes
        .iter()
        .map(|n| SortKey {
            folder_like: n.is_folder_like(),
            tagged: n.kind == Kind::Tag || (tags_only && n.kind == Kind::Todo),
            tag_index: n
                .key
                .as_ref()
                .and_then(|k| order.get(k).copied())
                .unwrap_or(-1),
            path: n
                .path
                .as_ref()
                .map(|p| p.to_string_lossy().into_owned())
                .unwrap_or_default(),
            pos: n
                .todo
                .as_ref()
                .map(|t| (t.start.line, t.start.character))
                .unwrap_or((0, 0)),
        })
        .collect()
}

fn compare(
    a: &Arena,
    keys: &[SortKey],
    settings: &Settings,
    x: usize,
    y: usize,
    tags_only: bool,
) -> Ordering {
    let (n, m) = (&keys[x], &keys[y]);
    let by_location = || {
        n.path
            .cmp(&m.path)
            .then(n.pos.cmp(&m.pos))
            .then(a.nodes[x].id.cmp(&a.nodes[y].id))
    };
    if !settings.tree.sort {
        return by_location();
    }
    let folders = m.folder_like.cmp(&n.folder_like);
    if folders != Ordering::Equal {
        return folders;
    }
    if tags_only && settings.tree.sort_tags_only_view_alphabetically {
        return a.nodes[x].name.cmp(&a.nodes[y].name).then_with(by_location);
    }
    if n.tagged && m.tagged && n.tag_index != m.tag_index {
        return n.tag_index.cmp(&m.tag_index);
    }
    by_location()
}

pub fn shape(a: &mut Arena, settings: &Settings) -> Shaped {
    let resolver = Resolver::new(settings);
    let filter = filter_regex(settings);
    let tags_only = settings.view().tags_only;
    let n = a.nodes.len();
    let keys = sort_keys(a, settings, tags_only);
    let mut visible = vec![false; n];
    let mut counts = vec![0usize; n];

    let mut hide_count: std::collections::HashMap<String, bool> = std::collections::HashMap::new();
    // Todos and extra lines: filter and hide flags.
    for i in 0..n {
        let node = &a.nodes[i];
        if node.kind != Kind::Todo {
            continue;
        }
        let matches = |text: &str| filter.as_ref().is_none_or(|re| re.is_match(text));
        let own = matches(&node.name);
        let extra = node.children.iter().any(|&c| matches(&a.nodes[c].name));
        let show = !node.hidden && (own || extra);
        visible[i] = show;
        for &c in &node.children {
            visible[c] = show;
        }
        let key = node.key.as_deref().unwrap_or("");
        let hidden_count = *hide_count
            .entry(key.to_string())
            .or_insert_with(|| resolver.flag(key, |x| x.hide_from_activity_bar));
        if show && !hidden_count {
            counts[i] = 1;
        }
    }
    // Containers: visible when any descendant is; counts sum bottom-up.
    fn settle(a: &Arena, i: usize, visible: &mut [bool], counts: &mut [usize]) {
        if !a.nodes[i].is_container() {
            return;
        }
        let mut any = false;
        let mut sum = 0;
        for &c in &a.nodes[i].children {
            settle(a, c, visible, counts);
            any |= visible[c];
            sum += counts[c];
        }
        visible[i] = any;
        counts[i] = sum;
    }
    for &t in &a.top.clone() {
        settle(a, t, &mut visible, &mut counts);
    }

    // Sorted, visible children.
    let order = |a: &Arena, list: &[usize]| {
        let mut v: Vec<usize> = list.iter().copied().filter(|&c| visible[c]).collect();
        v.sort_by(|&x, &y| compare(a, &keys, settings, x, y, tags_only));
        v
    };
    let mut children: Vec<Vec<usize>> = (0..n).map(|i| order(a, &a.nodes[i].children)).collect();
    let mut compact_label = vec![None; n];

    // Compact folders: a folder whose only child is a folder merges with it.
    if settings.explorer_compact_folders && !settings.tree.disable_compact_folders {
        for i in 0..n {
            children[i] = children[i]
                .iter()
                .map(|&c| {
                    if a.nodes[c].kind != Kind::Folder {
                        return c;
                    }
                    let mut end = c;
                    let mut label = a.nodes[c].name.clone();
                    while children[end].len() == 1 && a.nodes[children[end][0]].kind == Kind::Folder
                    {
                        end = children[end][0];
                        label = format!("{label}/{}", a.nodes[end].name);
                    }
                    // Ancestors come first in the arena, so the first label
                    // set is the whole chain's.
                    if end != c && compact_label[end].is_none() {
                        compact_label[end] = Some(label);
                    }
                    end
                })
                .collect();
        }
    }

    // Top level: root compaction of tag and sub-tag levels with one child.
    let mut top: Vec<usize> = order(a, &a.top)
        .into_iter()
        .map(|t| {
            let node = &a.nodes[t];
            if matches!(node.kind, Kind::Tag | Kind::SubTag) && children[t].len() == 1 {
                children[t][0]
            } else {
                t
            }
        })
        .collect();

    // Status pseudo-nodes.
    let vs = &settings.view_state;
    let mut total_filters = vs.include_globs.len() + vs.exclude_globs.len();
    let mut tooltip = String::new();
    if !vs.filter.is_empty() {
        tooltip.push_str(&format!("Tree Filter: \"{}\"\n", vs.filter));
        total_filters += 1;
    }
    for g in &vs.include_globs {
        tooltip.push_str(&format!("Include: {g}\n"));
    }
    for g in &vs.exclude_globs {
        tooltip.push_str(&format!("Exclude: {g}\n"));
    }
    let mut label = String::new();
    let mut icon = "filter";
    let mut status_tooltip = None;
    if total_filters > 0 {
        label = format!(
            "{total_filters} filter{} active",
            if total_filters == 1 { "" } else { "s" }
        );
        status_tooltip = Some(format!("{tooltip}\nRight click for filter options"));
    }
    if top.is_empty() {
        if !label.is_empty() {
            label.push_str(", ");
        }
        label.push_str("Nothing found");
        icon = "issues";
    }
    let mut status = Vec::new();
    if settings.tree.show_current_scan_mode {
        let mode = match settings.tree.scan_mode {
            ScanMode::Workspace => "workspace and open files",
            ScanMode::WorkspaceOnly => "workspace only",
            ScanMode::OpenFiles => "open files",
            ScanMode::CurrentFile => "current file",
        };
        status.push(a.get_or_add(None, "status:scan-mode", || {
            let mut n = Node::new(Kind::Status, "");
            n.status = Some((format!("Scan mode: {mode}"), "search", None));
            n
        }));
    }
    if !label.is_empty() {
        status.push(a.get_or_add(None, "status:filter", || {
            let mut n = Node::new(Kind::Status, "");
            n.status = Some((label.clone(), icon, status_tooltip.clone()));
            n
        }));
    }
    let added = a.nodes.len() - n;
    visible.extend(std::iter::repeat_n(true, added));
    counts.extend(std::iter::repeat_n(0, added));
    children.extend(std::iter::repeat_n(Vec::new(), added));
    compact_label.extend(std::iter::repeat_n(None, added));
    status.extend(top);
    top = status;

    // Displayed parents, walking down from the displayed top level.
    let mut parent = vec![None; a.nodes.len()];
    let mut stack: Vec<usize> = top.clone();
    while let Some(p) = stack.pop() {
        for &c in &children[p] {
            parent[c] = Some(p);
            stack.push(c);
        }
    }
    Shaped {
        visible,
        counts,
        children,
        top,
        parent,
        compact_label,
    }
}
```

- [ ] **Step 5: Write rendering**

Replace `crates/clippings-core/src/view/render.rs`:

```rust
//! Rendering (spec section 5.12): the `ViewNode`s the client displays, and
//! the built `View` that answers `children` and `find` requests.

use super::place::place;
use super::shape::{shape, Shaped};
use super::{Arena, Kind};
use crate::index::EffectiveFile;
use crate::labels::{self, LabelFields};
use crate::position::Position;
use crate::settings::{RevealBehaviour, Settings};
use crate::styles::{IconDescriptor, Resolver};
use crate::uri::file_uri;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", tag = "kind")]
pub enum NodeCommand {
    /// Open `uri` with the cursor at `position`.
    Reveal {
        uri: String,
        position: Position,
    },
    OpenUrl {
        url: String,
    },
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ViewNode {
    pub id: String,
    pub label: String,
    pub description: Option<String>,
    pub tooltip: Option<String>,
    pub icon: Option<IconDescriptor>,
    pub has_children: bool,
    pub default_expanded: bool,
    pub context_value: Option<String>,
    pub resource_uri: Option<String>,
    pub command: Option<NodeCommand>,
}

/// A built, shaped and rendered view.
#[derive(Clone, Debug, Default)]
pub struct View {
    pub arena: Arena,
    pub shaped: Shaped,
    /// Rendered nodes by ID, displayed nodes only.
    pub nodes: HashMap<String, ViewNode>,
    /// Displayed child IDs by parent ID; `None` is the top level.
    pub children: HashMap<Option<String>, Vec<String>>,
    /// Displayed parent ID by node ID; `None` for top-level nodes.
    pub parents: HashMap<String, Option<String>>,
    pub has_sub_tags: bool,
    pub is_empty: bool,
}

fn fields<'a>(t: &'a super::TodoData, path: &'a str) -> LabelFields<'a> {
    LabelFields {
        line: t.start.line,
        column: t.start.character + 1,
        tag: &t.tag,
        sub_tag: t.sub_tag.as_deref().unwrap_or(""),
        before: &t.before,
        after: &t.after,
        file_path: path,
    }
}

impl View {
    pub fn build(settings: &Settings, files: &[EffectiveFile], tree_roots: &[PathBuf]) -> View {
        let mut arena = place(settings, files, tree_roots);
        let shaped = shape(&mut arena, settings);
        let view = settings.view();
        let resolver = Resolver::new(settings);
        let grouped = view.grouped_by_tag || view.grouped_by_sub_tag;
        let mut nodes = HashMap::new();
        let mut children: HashMap<Option<String>, Vec<String>> = HashMap::new();
        let mut parents = HashMap::new();
        let mut icons: HashMap<String, IconDescriptor> = HashMap::new();
        let mut icon_of = |key: &str| {
            icons
                .entry(key.to_string())
                .or_insert_with(|| resolver.icon(key))
                .clone()
        };
        let mut has_sub_tags = false;
        let mut is_empty = true;

        let mut stack: Vec<usize> = shaped.top.clone();
        children.insert(
            None,
            shaped
                .top
                .iter()
                .map(|&i| arena.nodes[i].id.clone())
                .collect(),
        );
        while let Some(i) = stack.pop() {
            let n = &arena.nodes[i];
            let kids = &shaped.children[i];
            parents.insert(
                n.id.clone(),
                shaped.parent[i].map(|p| arena.nodes[p].id.clone()),
            );
            stack.extend(kids.iter().copied());
            if !kids.is_empty() {
                children.insert(
                    Some(n.id.clone()),
                    kids.iter().map(|&c| arena.nodes[c].id.clone()).collect(),
                );
            }
            has_sub_tags |= n.sub_tag.is_some();
            is_empty &= n.kind != Kind::Todo;
            let path_str = n.path.as_ref().map(|p| p.to_string_lossy().into_owned());
            let count = (settings.tree.show_counts_in_tree && n.is_container())
                .then(|| shaped.counts[i].to_string());
            let badge_uri = |n: &super::Node| {
                (settings.tree.show_badges
                    && matches!(n.kind, Kind::Root | Kind::Folder | Kind::File))
                .then(|| {
                    n.path
                        .as_ref()
                        .map(|p| file_uri(p))
                        .or_else(|| n.uri.clone())
                })
                .flatten()
            };
            let rendered = match n.kind {
                Kind::Status => {
                    let (text, icon, tooltip) = n.status.clone().unwrap_or_default();
                    ViewNode {
                        id: n.id.clone(),
                        label: String::new(),
                        description: Some(text),
                        tooltip,
                        icon: Some(IconDescriptor::Codicon {
                            name: icon.to_string(),
                            colour: None,
                        }),
                        has_children: false,
                        default_expanded: false,
                        context_value: None,
                        resource_uri: None,
                        command: None,
                    }
                }
                Kind::Todo | Kind::Extra => {
                    let t = n.todo.as_ref().expect("todo data");
                    let path = path_str.clone().unwrap_or_else(|| t.uri.clone());
                    let f = fields(t, &path);
                    let label = if n.kind == Kind::Extra
                        || t.multi_line
                        || settings.tree.label_format.is_empty()
                    {
                        n.name.clone()
                    } else {
                        labels::format(&settings.tree.label_format, &f)
                    };
                    let key = n.key.clone().unwrap_or_default();
                    let icon = (n.kind == Kind::Todo
                        && !(settings.tree.hide_icons_when_grouped_by_tag && grouped))
                        .then(|| icon_of(&key));
                    let position = match settings.general.reveal_behaviour {
                        RevealBehaviour::StartOfLine => Position {
                            line: t.start.line,
                            character: 0,
                        },
                        RevealBehaviour::StartOfTodo => t.start,
                        RevealBehaviour::EndOfTodo => t.text_end,
                    };
                    ViewNode {
                        id: n.id.clone(),
                        label,
                        description: None,
                        tooltip: Some(labels::format(&settings.tree.tooltip_format, &f)),
                        icon,
                        has_children: !kids.is_empty(),
                        default_expanded: t.multi_line,
                        context_value: None,
                        resource_uri: None,
                        command: Some(NodeCommand::Reveal {
                            uri: t.uri.clone(),
                            position,
                        }),
                    }
                }
                _ => {
                    let mut label = shaped.compact_label[i]
                        .clone()
                        .unwrap_or_else(|| n.name.clone());
                    if let Some(pl) = &n.path_label {
                        label = format!("{label} {pl}");
                    }
                    let click = (n.kind == Kind::SubTag
                        && !settings.tree.sub_tag_click_url.is_empty())
                    .then(|| {
                        let sub = n.sub_tag.clone().unwrap_or_default();
                        labels::format(
                            &settings.tree.sub_tag_click_url,
                            &LabelFields {
                                sub_tag: &sub,
                                ..Default::default()
                            },
                        )
                    });
                    let tooltip = match (&click, n.kind) {
                        (Some(url), _) => Some(format!("Click to open {url}")),
                        (None, Kind::Root | Kind::Folder | Kind::File) => {
                            path_str.clone().or_else(|| n.uri.clone())
                        }
                        _ => None,
                    };
                    let icon = match n.kind {
                        Kind::Root => IconDescriptor::Codicon {
                            name: "window".into(),
                            colour: None,
                        },
                        Kind::Tag => icon_of(n.key.as_deref().unwrap_or("")),
                        Kind::File => IconDescriptor::File,
                        _ => IconDescriptor::Folder,
                    };
                    ViewNode {
                        id: n.id.clone(),
                        label,
                        description: count,
                        tooltip,
                        icon: Some(icon),
                        has_children: !kids.is_empty(),
                        default_expanded: view.expanded,
                        context_value: match n.kind {
                            Kind::Root | Kind::Folder => Some("folder".into()),
                            Kind::File => Some("file".into()),
                            _ => None,
                        },
                        resource_uri: badge_uri(n),
                        command: click.map(|url| NodeCommand::OpenUrl { url }),
                    }
                }
            };
            nodes.insert(n.id.clone(), rendered);
        }
        View {
            arena,
            shaped,
            nodes,
            children,
            parents,
            has_sub_tags,
            is_empty,
        }
    }

    /// Displayed children of a node, or of the top level.
    pub fn children_of(&self, parent: Option<&str>) -> Vec<ViewNode> {
        self.children
            .get(&parent.map(str::to_string))
            .map(|ids| {
                ids.iter()
                    .filter_map(|id| self.nodes.get(id).cloned())
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Paths from the top level down to each displayed node for `uri`: file
    /// nodes when `line` is `None`, todos on that line otherwise.
    pub fn find(&self, uri: &str, line: Option<u32>) -> Vec<Vec<ViewNode>> {
        let path = crate::uri::uri_to_path(uri);
        let mut out = Vec::new();
        for (i, n) in self.arena.nodes.iter().enumerate() {
            if !self.nodes.contains_key(&n.id) {
                continue;
            }
            let hit = match line {
                None => {
                    n.kind == Kind::File
                        && (n.uri.as_deref() == Some(uri) || (path.is_some() && n.path == path))
                }
                Some(l) => {
                    n.kind == Kind::Todo
                        && (n.uri.as_deref() == Some(uri) || (path.is_some() && n.path == path))
                        && n.todo.as_ref().is_some_and(|t| t.start.line == l)
                }
            };
            if !hit {
                continue;
            }
            let mut chain = vec![i];
            let mut cur = i;
            while let Some(p) = self.shaped.parent[cur] {
                chain.push(p);
                cur = p;
            }
            chain.reverse();
            out.push(
                chain
                    .iter()
                    .filter_map(|&c| self.nodes.get(&self.arena.nodes[c].id).cloned())
                    .collect(),
            );
        }
        out.sort_by(|a: &Vec<ViewNode>, b| a.last().map(|n| &n.id).cmp(&b.last().map(|n| &n.id)));
        out
    }

    /// Visible todos per key, excluding keys with `hide` set.
    pub fn counts(
        &self,
        settings: &Settings,
        hide: fn(&crate::settings::Attributes) -> Option<bool>,
        file: Option<&std::path::Path>,
    ) -> Vec<(String, usize)> {
        let resolver = Resolver::new(settings);
        let mut counts: Vec<(String, usize)> = Vec::new();
        for (i, n) in self.arena.nodes.iter().enumerate() {
            if n.kind != Kind::Todo || !self.shaped.visible[i] {
                continue;
            }
            if let Some(f) = file {
                if n.path.as_deref() != Some(f) {
                    continue;
                }
            }
            let key = n.key.clone().unwrap_or_default();
            if resolver.flag(&key, hide) {
                continue;
            }
            match counts.iter_mut().find(|(k, _)| *k == key) {
                Some((_, c)) => *c += 1,
                None => counts.push((key, 1)),
            }
        }
        counts
    }
}
```

- [ ] **Step 6: Run the tests to verify they pass**

Run: `cargo test -p clippings-core view::`
Expected: 14 tests pass.
Then run: `cargo fmt --all && cargo clippy --all-targets -- -D warnings`
Expected: no diffs, no warnings.

- [ ] **Step 7: Commit**

```bash
git add -A
git commit -F - <<'EOF'
feat(core): tree view model with placement, shaping and rendering

Co-Authored-By: <your model attribution>
EOF
```

### Task 8: View deltas and export

Spec sections 5.12 (deltas) and 5.15 (export). `delta` compares two built views and names the parents the client must refresh. `export` produces the visible tree as JSON or in treeify's ASCII format. The treeify reference string in the test came from running the real `treeify` npm package that todo-tree uses.

**Files:**
- Create: `crates/clippings-core/src/view/delta.rs`, `crates/clippings-core/src/view/export.rs`
- Modify: `crates/clippings-core/src/view/mod.rs` (add `pub mod delta;` and `pub mod export;`), `crates/clippings-core/src/view/tests.rs`

**Interfaces:**
- Consumes: `View` and its `pub` fields from Task 7; `roots::expand_env`; `chrono`.
- Produces: `delta::delta(old: &View, new: &View, whole_tree: bool) -> Vec<Option<String>>` (`[None]` means refresh everything), `export::{export_value(&View, tags_only) -> serde_json::Value, treeify(&Value) -> String, export_path(template, env, home: Option<&str>, now: &DateTime<Local>) -> String, export_content(&View, tags_only, path) -> String}`.

- [ ] **Step 1: Write the failing tests**

Add `pub mod delta;` and `pub mod export;` to `crates/clippings-core/src/view/mod.rs` (keep the module list sorted). At the top of `crates/clippings-core/src/view/tests.rs` add:

```rust
use super::delta::delta;
use super::export::{export_path, export_value, treeify};
```

and append these tests:

```rust
#[test]
fn deltas_name_parents_to_refresh() {
    let s = quiet();
    let old = sample().build(&s);
    assert!(delta(&old, &old, false).is_empty());
    let changed = Fixture::new()
        .file(
            "/w/src/b.ts",
            vec![todo(4, "FIXME", "sooner"), todo(1, "TODO", "first")],
        )
        .file("/w/src/a.ts", vec![todo(0, "BUG", "")])
        .file("/w/README.md", vec![todo(2, "TODO", "docs")])
        .build(&s);
    assert_eq!(
        delta(&old, &changed, false),
        vec![Some("w:file:///w/d:/w/src/f:/w/src/b.ts".to_string())]
    );
    let removed = Fixture::new()
        .file(
            "/w/src/b.ts",
            vec![todo(4, "FIXME", "later"), todo(1, "TODO", "first")],
        )
        .file("/w/README.md", vec![todo(2, "TODO", "docs")])
        .build(&s);
    assert_eq!(
        delta(&old, &removed, false),
        vec![Some("w:file:///w/d:/w/src".to_string())]
    );
    assert_eq!(delta(&old, &changed, true), vec![None]);
}

#[test]
fn export_json_and_treeify() {
    let mut a = todo(0, "[ ]", "x");
    a.start = pos(0, 2);
    let mut b = todo(0, "[ ]", "y");
    b.start = pos(0, 8);
    let v = Fixture::new().file("/w/z.md", vec![a, b]).build(&quiet());
    let value = export_value(&v, false);
    assert_eq!(
        value,
        serde_json::json!({"w": {"z.md": {"line 1:3": "[ ] x", "line 1:9": "[ ] y"}}})
    );

    let o = serde_json::json!({"src":{"a.ts":{"line 1":"TODO one","line 3":{"TODO":{"second":{},"third":{}}}},"b.ts":{"line 2":"FIXME two"}},"z.md":{"line 1:3":"[ ] x","line 1:9":"[ ] y"}});
    // Reference output produced by the treeify npm package todo-tree uses.
    assert_eq!(treeify(&o), "├─ src\n│  ├─ a.ts\n│  │  ├─ line 1: TODO one\n│  │  └─ line 3\n│  │     └─ TODO\n│  │        ├─ second\n│  │        └─ third\n│  └─ b.ts\n│     └─ line 2: FIXME two\n└─ z.md\n   ├─ line 1:3: [ ] x\n   └─ line 1:9: [ ] y\n");
}

#[test]
fn export_path_expands_home_env_and_time() {
    use chrono::TimeZone;
    let now = chrono::Local
        .with_ymd_and_hms(2026, 9, 23, 14, 5, 0)
        .unwrap();
    let env = |n: &str| (n == "PROJ").then(|| "clip".to_string());
    assert_eq!(
        export_path("~/${PROJ}-%Y%m%d-%H%M.txt", &env, Some("/home/u"), &now),
        "/home/u/clip-20260923-1405.txt"
    );
}
```

Create `crates/clippings-core/src/view/delta.rs` and `crates/clippings-core/src/view/export.rs` each containing only a `//!` doc line.

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p clippings-core view::`
Expected: compile error, `delta` and `export_value` not found.

- [ ] **Step 3: Write the delta pass**

Replace `crates/clippings-core/src/view/delta.rs`:

```rust
//! Deltas (spec section 5.12): which displayed parents the client must
//! refresh after a rebuild.

use super::render::View;
use std::collections::BTreeSet;

/// Parent IDs to refresh, `None` meaning the top level. A view mode or
/// grouping change always refreshes the whole tree.
pub fn delta(old: &View, new: &View, whole_tree: bool) -> Vec<Option<String>> {
    if whole_tree {
        return vec![None];
    }
    let mut refresh: BTreeSet<Option<String>> = BTreeSet::new();
    for (id, node) in &new.nodes {
        if old.nodes.get(id) != Some(node) {
            refresh.insert(new.parents.get(id).cloned().flatten());
        }
    }
    let parents: BTreeSet<&Option<String>> =
        old.children.keys().chain(new.children.keys()).collect();
    for p in parents {
        if old.children.get(p) != new.children.get(p) {
            match p {
                None => {
                    refresh.insert(None);
                }
                Some(id) if new.nodes.contains_key(id) => {
                    refresh.insert(Some(id.clone()));
                }
                // A removed parent disappears through its own parent's list.
                Some(_) => {}
            }
        }
    }
    if refresh.contains(&None) {
        return vec![None];
    }
    refresh.into_iter().collect()
}
```

- [ ] **Step 4: Write export**

Replace `crates/clippings-core/src/view/export.rs`:

```rust
//! Export (spec section 5.15): the visible tree as JSON or as an ASCII tree
//! in treeify's format, and the export path.

use super::render::View;
use super::Kind;
use serde_json::{Map, Value};

fn node_value(view: &View, i: usize, tags_only: bool) -> (String, Value) {
    let n = &view.arena.nodes[i];
    let rendered = &view.nodes[&n.id];
    if n.kind == Kind::Todo {
        let t = n.todo.as_ref().expect("todo data");
        let siblings = view.shaped.parent[i].map_or(&view.shaped.top, |p| &view.shaped.children[p]);
        let shared = siblings.iter().filter(|&&s| {
            let o = &view.arena.nodes[s];
            o.kind == Kind::Todo
                && o.todo
                    .as_ref()
                    .is_some_and(|ot| ot.start.line == t.start.line && o.path == n.path)
        });
        let mut key = if shared.count() > 1 {
            format!("line {}:{}", t.start.line + 1, t.start.character + 1)
        } else {
            format!("line {}", t.start.line + 1)
        };
        if tags_only {
            let file = n
                .path
                .as_ref()
                .map(|p| p.to_string_lossy().into_owned())
                .unwrap_or_else(|| t.uri.clone());
            key = format!("{file} {key}");
        }
        let kids = &view.shaped.children[i];
        let value = if kids.is_empty() {
            Value::String(rendered.label.clone())
        } else {
            let mut extras = Map::new();
            for &c in kids {
                extras.insert(view.arena.nodes[c].name.clone(), Value::Object(Map::new()));
            }
            let mut m = Map::new();
            m.insert(rendered.label.clone(), Value::Object(extras));
            Value::Object(m)
        };
        return (key, value);
    }
    let mut m = Map::new();
    for &c in &view.shaped.children[i] {
        let (k, v) = node_value(view, c, tags_only);
        m.insert(k, v);
    }
    (rendered.label.clone(), Value::Object(m))
}

/// The visible tree, without status nodes, as a JSON object.
pub fn export_value(view: &View, tags_only: bool) -> Value {
    let mut m = Map::new();
    for &t in &view.shaped.top {
        if view.arena.nodes[t].kind == Kind::Status {
            continue;
        }
        let (k, v) = node_value(view, t, tags_only);
        m.insert(k, v);
    }
    Value::Object(m)
}

/// treeify's `asTree(value, true)`.
pub fn treeify(value: &Value) -> String {
    fn grow(key: &str, value: &Value, last: bool, ancestors_last: &[bool], out: &mut String) {
        let mut line = String::new();
        for &l in ancestors_last {
            line.push_str(if l { "   " } else { "│  " });
        }
        line.push_str(if last { "└─ " } else { "├─ " });
        line.push_str(key);
        match value {
            Value::Object(_) => {}
            Value::String(s) => {
                line.push_str(": ");
                line.push_str(s);
            }
            other => {
                line.push_str(": ");
                line.push_str(&other.to_string());
            }
        }
        out.push_str(&line);
        out.push('\n');
        if let Value::Object(m) = value {
            let mut next = ancestors_last.to_vec();
            next.push(last);
            let len = m.len();
            for (i, (k, v)) in m.iter().enumerate() {
                grow(k, v, i + 1 == len, &next, out);
            }
        }
    }
    let mut out = String::new();
    if let Value::Object(m) = value {
        let len = m.len();
        for (i, (k, v)) in m.iter().enumerate() {
            grow(k, v, i + 1 == len, &[], &mut out);
        }
    }
    out
}

/// `general.exportPath` with `~`, `${NAME}` and strftime placeholders expanded.
pub fn export_path(
    template: &str,
    env: &dyn Fn(&str) -> Option<String>,
    home: Option<&str>,
    now: &chrono::DateTime<chrono::Local>,
) -> String {
    let mut s = crate::roots::expand_env(template, env);
    if let (Some(rest), Some(h)) = (s.strip_prefix('~'), home) {
        s = format!("{h}{rest}");
    }
    let mut out = String::new();
    use std::fmt::Write;
    match write!(out, "{}", now.format(&s)) {
        Ok(()) => out,
        Err(_) => s,
    }
}

/// The export document: JSON when the path ends in `.json`, else treeify text.
pub fn export_content(view: &View, tags_only: bool, path: &str) -> String {
    let value = export_value(view, tags_only);
    if path.ends_with(".json") {
        serde_json::to_string_pretty(&value).unwrap_or_default()
    } else {
        treeify(&value)
    }
}
```

- [ ] **Step 5: Run the tests to verify they pass**

Run: `cargo test -p clippings-core view::`
Expected: 17 tests pass.
Then run: `cargo fmt --all && cargo clippy --all-targets -- -D warnings`
Expected: no diffs, no warnings.

- [ ] **Step 6: Commit**

```bash
git add -A
git commit -F - <<'EOF'
feat(core): view deltas and export

Co-Authored-By: <your model attribution>
EOF
```

### Task 9: Decorations

Decoration ranges per open document (spec section 5.13), keyed by group, tag, sub-tag or untagged match text, for every highlight `type`. `capture-groups:n,m` needs the main pattern's capture groups, so `CaptureRegex` compiles it with `regex::bytes` (or `fancy-regex` for look-around) on first use.

**Files:**
- Create: `crates/clippings-core/src/decorations.rs`
- Modify: `crates/clippings-core/src/lib.rs` (add `pub mod decorations;`, keeping the declarations sorted)

**Interfaces:**
- Consumes: `Todo`, `ScanPattern.{source, flags}`, `LineIndex::{offset, line_end, line_range, line_of, position}`, `Resolver::kind`, `Settings::{key_of, custom}`.
- Produces: `CaptureRegex::{new(&ScanPattern) -> Option<Self>, groups_at(&self, text, start) -> Option<Vec<Option<(usize, usize)>>>}`, `decorate(text: &[u8], todos: &[Todo], &Settings, &ScanPattern) -> BTreeMap<String, Vec<Range>>` (empty when highlights are disabled; ranges sorted and de-duplicated).

- [ ] **Step 1: Write the failing tests**

Add `pub mod decorations;` to `crates/clippings-core/src/lib.rs`. Create `crates/clippings-core/src/decorations.rs` containing only this test module:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::pattern::build;
    use crate::scanner::scan_text;
    use crate::settings::Attributes;

    fn run(
        text: &str,
        kind: Option<&str>,
        f: impl FnOnce(&mut Settings),
    ) -> BTreeMap<String, Vec<Range>> {
        let mut s = Settings::default();
        if let Some(k) = kind {
            s.highlights.default_highlight = Attributes {
                kind: Some(k.into()),
                ..Default::default()
            };
        }
        f(&mut s);
        let p = build(&s.core()).unwrap();
        let todos = scan_text(&p, text.as_bytes(), "a.ts");
        decorate(text.as_bytes(), &todos, &s, &p)
    }

    fn r(l1: u32, c1: u32, l2: u32, c2: u32) -> Range {
        Range {
            start: Position {
                line: l1,
                character: c1,
            },
            end: Position {
                line: l2,
                character: c2,
            },
        }
    }

    const TEXT: &str = "x();  // TODO fix this\n";

    #[test]
    fn tag_is_the_default() {
        assert_eq!(
            run(TEXT, None, |_| {}),
            BTreeMap::from([("TODO".to_string(), vec![r(0, 9, 0, 13)])])
        );
    }

    #[test]
    fn range_types() {
        assert_eq!(
            run(TEXT, Some("text"), |_| {})["TODO"],
            vec![r(0, 9, 0, 22)]
        );
        assert_eq!(
            run(TEXT, Some("tag-and-comment"), |_| {})["TODO"],
            vec![r(0, 6, 0, 13)]
        );
        assert_eq!(
            run(TEXT, Some("text-and-comment"), |_| {})["TODO"],
            vec![r(0, 6, 0, 22)]
        );
        assert_eq!(
            run(TEXT, Some("line"), |_| {})["TODO"],
            vec![r(0, 0, 0, 22)]
        );
        assert_eq!(
            run(TEXT, Some("whole-line"), |_| {})["TODO"],
            vec![r(0, 0, 0, 22)]
        );
        assert!(run(TEXT, Some("none"), |_| {}).is_empty());
        // Group 2 of the default regex is the nested list-marker group; the tag is group 3.
        assert_eq!(
            run(TEXT, Some("capture-groups:1,3"), |_| {})["TODO"],
            vec![r(0, 6, 0, 8), r(0, 9, 0, 13)]
        );
    }

    #[test]
    fn groups_key_by_group_name() {
        let d = run(TEXT, None, |s| {
            s.general
                .tag_groups
                .insert("WORK".into(), vec!["TODO".into()]);
        });
        assert!(d.contains_key("WORK") && !d.contains_key("TODO"));
    }

    #[test]
    fn sub_tags_get_their_own_key_when_configured() {
        let text = "// TODO(alice) ship\n";
        let d = run(text, Some("tag-and-subTag"), |s| {
            s.regex.sub_tag_regex = r"^\s*\((.*?)\)".into();
            s.highlights
                .custom_highlight
                .insert("alice".into(), Attributes::default());
        });
        assert_eq!(d["TODO"], vec![r(0, 3, 0, 7)]);
        assert_eq!(d["alice"], vec![r(0, 8, 0, 13)]);
    }

    #[test]
    fn disabled_highlights_clear_everything() {
        assert!(run(TEXT, None, |s| s.highlights.enabled = false).is_empty());
    }

    #[test]
    fn matches_without_tags_token_use_the_whole_match() {
        // Without `$TAGS` the whole match is the tag, keyed by its trimmed text.
        let d = run("  NOTE(bob) read\n", None, |s| {
            s.regex.regex = r"\s*NOTE\(\w+\)".into()
        });
        assert_eq!(d["NOTE(bob)"], vec![r(0, 0, 0, 11)]);
    }
}
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p clippings-core decorations`
Expected: compile error, `decorate` not found.

- [ ] **Step 3: Write the implementation**

Put this above the test module in `crates/clippings-core/src/decorations.rs`:

```rust
//! Decoration ranges per open document (spec section 5.13).

use crate::model::Todo;
use crate::pattern::ScanPattern;
use crate::position::{LineIndex, Position, Range};
use crate::settings::Settings;
use crate::styles::Resolver;
use std::collections::BTreeMap;

/// The main pattern with capture groups, for `capture-groups:n,m` highlights.
pub enum CaptureRegex {
    Std(regex::bytes::Regex),
    Fancy(fancy_regex::Regex),
}

impl CaptureRegex {
    pub fn new(p: &ScanPattern) -> Option<CaptureRegex> {
        let source = format!("{}{}", p.flags, p.source);
        match regex::bytes::Regex::new(&source) {
            Ok(re) => Some(CaptureRegex::Std(re)),
            Err(_) => fancy_regex::Regex::new(&source)
                .ok()
                .map(CaptureRegex::Fancy),
        }
    }

    /// Byte ranges of each group for the match that starts at `start`.
    pub fn groups_at(&self, text: &[u8], start: usize) -> Option<Vec<Option<(usize, usize)>>> {
        match self {
            CaptureRegex::Std(re) => {
                let c = re.captures_at(text, start)?;
                (c.get(0)?.start() == start).then(|| {
                    (0..c.len())
                        .map(|i| c.get(i).map(|m| (m.start(), m.end())))
                        .collect()
                })
            }
            CaptureRegex::Fancy(re) => {
                let s = std::str::from_utf8(text).ok()?;
                let c = re.captures_from_pos(s, start).ok()??;
                (c.get(0)?.start() == start).then(|| {
                    (0..c.len())
                        .map(|i| c.get(i).map(|m| (m.start(), m.end())))
                        .collect()
                })
            }
        }
    }
}

/// Decoration keys and ranges for one document. Keys absent from the result
/// have no ranges. Returns an empty map when highlights are disabled.
pub fn decorate(
    text: &[u8],
    todos: &[Todo],
    settings: &Settings,
    pattern: &ScanPattern,
) -> BTreeMap<String, Vec<Range>> {
    let mut out: BTreeMap<String, Vec<Range>> = BTreeMap::new();
    if !settings.highlights.enabled {
        return out;
    }
    let li = LineIndex::new(text);
    let resolver = Resolver::new(settings);
    let mut captures: Option<Option<CaptureRegex>> = None;
    for t in todos {
        let s = li.offset(t.start);
        let e = li.offset(t.end).max(s);
        let raw = String::from_utf8_lossy(&text[s..e]).into_owned();
        let (key, tag_range) = match (t.tag.is_empty(), t.tag_start, t.tag_end) {
            (false, Some(a), Some(b)) => (
                settings.key_of(&t.tag).to_string(),
                Range { start: a, end: b },
            ),
            _ => {
                let lead = raw.len() - raw.trim_start().len();
                let trail = raw.len() - raw.trim_end().len();
                let range = Range {
                    start: li.position(s + lead),
                    end: li.position(e - trail),
                };
                (raw.trim().to_string(), range)
            }
        };
        let end_line = if t.end.character == 0 && t.end.line > t.start.line {
            t.end.line - 1
        } else {
            t.end.line
        };
        let line_end = li.line_end(end_line as usize);
        let kind = resolver.kind(&key).unwrap_or_default();
        let mut add = |k: &str, r: Range| out.entry(k.to_string()).or_default().push(r);
        match kind.as_str() {
            "none" => {}
            "text" => add(
                &key,
                Range {
                    start: tag_range.start,
                    end: line_end,
                },
            ),
            "tag-and-comment" => add(
                &key,
                Range {
                    start: t.start,
                    end: tag_range.end,
                },
            ),
            "text-and-comment" => add(
                &key,
                Range {
                    start: t.start,
                    end: line_end,
                },
            ),
            "line" | "whole-line" => add(
                &key,
                Range {
                    start: Position {
                        line: tag_range.start.line,
                        character: 0,
                    },
                    end: line_end,
                },
            ),
            "tag-and-subTag" | "tag-and-subtag" => {
                add(&key, tag_range);
                if let Some(sub) = t
                    .sub_tag
                    .as_deref()
                    .filter(|s| settings.custom(s).is_some())
                {
                    let from = li.offset(tag_range.end);
                    let (_, eol) = li.line_range(li.line_of(from));
                    let hay = String::from_utf8_lossy(&text[from..eol.max(from)]).into_owned();
                    if let Some(i) = hay.find(sub) {
                        let a = from + i;
                        add(
                            sub,
                            Range {
                                start: li.position(a),
                                end: li.position(a + sub.len()),
                            },
                        );
                    }
                }
            }
            k if k.starts_with("capture-groups:") => {
                let re = captures.get_or_insert_with(|| CaptureRegex::new(pattern));
                if let Some(groups) = re.as_ref().and_then(|re| re.groups_at(text, s)) {
                    for n in k["capture-groups:".len()..]
                        .split(',')
                        .filter_map(|n| n.trim().parse::<usize>().ok())
                    {
                        if let Some(Some((a, b))) = groups.get(n) {
                            add(
                                &key,
                                Range {
                                    start: li.position(*a),
                                    end: li.position(*b),
                                },
                            );
                        }
                    }
                }
            }
            _ => add(&key, tag_range),
        }
    }
    for ranges in out.values_mut() {
        ranges.sort();
        ranges.dedup();
    }
    out
}
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test -p clippings-core decorations`
Expected: 6 tests pass.
Then run: `cargo fmt --all && cargo clippy --all-targets -- -D warnings`
Expected: no diffs, no warnings.

- [ ] **Step 5: Commit**

```bash
git add -A
git commit -F - <<'EOF'
feat(core): decoration ranges per highlight type

Co-Authored-By: <your model attribution>
EOF
```

### Task 10: Status bar, badge and view title

Spec section 5.14, ported from todo-tree's `updateInformation`, including its exact spacing. Counts come from the built view's visible todos.

**Files:**
- Create: `crates/clippings-core/src/status.rs`
- Modify: `crates/clippings-core/src/lib.rs` (add `pub mod status;`, keeping the declarations sorted)

**Interfaces:**
- Consumes: `View::counts`, `Resolver::icon_name`, `Settings`.
- Produces: `StatusBar { text, tooltip, visible }`, `Badge { value, tooltip }` (both serde camelCase), `Summary { status_bar, badge, view_title }`, `summarize(&View, &Settings, active_file: Option<&Path>) -> Summary`.

- [ ] **Step 1: Write the failing tests**

Add `pub mod status;` to `crates/clippings-core/src/lib.rs`. Create `crates/clippings-core/src/status.rs` containing only this test module:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::index::{EffectiveFile, Source, SourcedTodo};
    use crate::model::Todo;
    use crate::position::Position;
    use crate::settings::Attributes;
    use std::path::PathBuf;

    fn todo(line: u32, tag: &str) -> Todo {
        let p = Position { line, character: 0 };
        Todo {
            start: p,
            end: p,
            text_end: p,
            tag_start: None,
            tag_end: None,
            tag: tag.into(),
            sub_tag: None,
            before: String::new(),
            after: "x".into(),
            extra_lines: vec![],
        }
    }

    fn summary(f: impl FnOnce(&mut Settings), active: Option<&str>) -> Summary {
        let a = [todo(0, "TODO"), todo(1, "TODO"), todo(2, "FIXME")];
        let b = [todo(0, "BUG"), todo(1, "TODO")];
        let files = vec![
            EffectiveFile {
                path: Some(Path::new("/w/a.ts")),
                uri: None,
                source: Source::Disk,
                todos: a
                    .iter()
                    .map(|t| SourcedTodo {
                        buffer_uri: None,
                        todo: t,
                    })
                    .collect(),
            },
            EffectiveFile {
                path: Some(Path::new("/w/b.ts")),
                uri: None,
                source: Source::Disk,
                todos: b
                    .iter()
                    .map(|t| SourcedTodo {
                        buffer_uri: None,
                        todo: t,
                    })
                    .collect(),
            },
        ];
        let mut s = Settings::default();
        f(&mut s);
        let v = View::build(&s, &files, &[PathBuf::from("/w")]);
        summarize(&v, &s, active.map(Path::new))
    }

    // Spacing matches todo-tree exactly: each item ends in a space (two after
    // an icon) and items are joined with another space.
    #[test]
    fn total_and_tags_modes() {
        let t = summary(|s| s.general.status_bar = StatusBarMode::Total, None);
        assert_eq!(
            (t.status_bar.text.as_str(), t.status_bar.tooltip.as_str()),
            ("$(check) 5", "Clippings total")
        );
        let t = summary(|s| s.general.status_bar = StatusBarMode::Tags, None);
        assert_eq!(t.status_bar.text, "$(check) BUG: 1  FIXME: 1  TODO: 3");
        let t = summary(|s| s.general.status_bar = StatusBarMode::TopThree, None);
        assert_eq!(t.status_bar.text, "$(check) TODO: 3  BUG: 1  FIXME: 1");
        let t = summary(|s| s.general.status_bar = StatusBarMode::None, None);
        assert!(!t.status_bar.visible);
    }

    #[test]
    fn icons_and_current_file_and_scan_mode_suffix() {
        let t = summary(
            |s| {
                s.general.status_bar = StatusBarMode::Tags;
                s.general.show_icons_instead_of_tags_in_status_bar = true;
            },
            None,
        );
        assert_eq!(t.status_bar.text, "$(bug) 1   $(flame) 1   TODO: 3");
        let t = summary(
            |s| {
                s.general.status_bar = StatusBarMode::CurrentFile;
                s.tree.scan_mode = ScanMode::OpenFiles;
                s.tree.show_counts_in_tree = true;
            },
            Some("/w/b.ts"),
        );
        assert_eq!(
            t.status_bar.text,
            "$(check) BUG: 1  TODO: 1 (in open files)"
        );
        assert_eq!(t.view_title, "Tree (2)");
    }

    #[test]
    fn badge_and_hide_flags() {
        let t = summary(
            |s| {
                s.general.show_activity_bar_badge = true;
                s.general.status_bar = StatusBarMode::Total;
                s.highlights.custom_highlight.insert(
                    "TODO".into(),
                    Attributes {
                        hide_from_activity_bar: Some(true),
                        ..Default::default()
                    },
                );
            },
            None,
        );
        assert_eq!(t.badge.value, 2);
        assert_eq!(t.status_bar.text, "$(check) 2");
        let t = summary(
            |s| {
                s.general.status_bar = StatusBarMode::Tags;
                s.highlights.custom_highlight.insert(
                    "TODO".into(),
                    Attributes {
                        hide_from_status_bar: Some(true),
                        ..Default::default()
                    },
                );
            },
            None,
        );
        assert_eq!(t.status_bar.text, "$(check) BUG: 1  FIXME: 1");
    }
}
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p clippings-core status`
Expected: compile error, `summarize` not found.

- [ ] **Step 3: Write the implementation**

Put this above the test module in `crates/clippings-core/src/status.rs`:

```rust
//! Status bar text, activity badge and view title (spec section 5.14),
//! ported from todo-tree's `updateInformation`.

use crate::config::ScanMode;
use crate::settings::{Settings, StatusBarMode};
use crate::styles::Resolver;
use crate::view::render::View;
use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StatusBar {
    pub text: String,
    pub tooltip: String,
    pub visible: bool,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Badge {
    pub value: usize,
    pub tooltip: String,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Summary {
    pub status_bar: StatusBar,
    pub badge: Badge,
    pub view_title: String,
}

pub fn summarize(view: &View, settings: &Settings, active_file: Option<&Path>) -> Summary {
    let activity = view.counts(settings, |a| a.hide_from_activity_bar, None);
    let total: usize = activity.iter().map(|(_, c)| c).sum();
    let mode = settings.general.status_bar;
    let counts = match mode {
        StatusBarMode::CurrentFile => match active_file {
            Some(f) => view.counts(settings, |a| a.hide_from_status_bar, Some(f)),
            None => Vec::new(),
        },
        _ => view.counts(settings, |a| a.hide_from_status_bar, None),
    };
    let title_total = if mode == StatusBarMode::CurrentFile {
        counts.iter().map(|(_, c)| c).sum()
    } else {
        total
    };
    let v = settings.view();
    let mut title = if v.flat {
        "Flat"
    } else if v.tags_only {
        "Tags"
    } else {
        "Tree"
    }
    .to_string();
    if title_total > 0 && settings.tree.show_counts_in_tree {
        title = format!("{title} ({title_total})");
    }

    let resolver = Resolver::new(settings);
    let default_icon = settings.highlights.default_highlight.icon.clone();
    let show_icons = settings.general.show_icons_instead_of_tags_in_status_bar;
    let count_of = |tag: &str| counts.iter().find(|(k, _)| k == tag).map_or(0, |(_, c)| *c);
    let (mut text, tooltip, visible) = match mode {
        StatusBarMode::None => (String::new(), String::new(), false),
        StatusBarMode::Total => (
            format!("$(check) {total}"),
            "Clippings total".to_string(),
            true,
        ),
        _ => {
            let order: Vec<String> = if mode == StatusBarMode::TopThree {
                let mut c = counts.clone();
                c.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
                c.into_iter().take(3).map(|(k, _)| k).collect()
            } else {
                settings.tags()
            };
            let mut text = String::new();
            for tag in order {
                let n = count_of(&tag);
                if n == 0 {
                    continue;
                }
                if !text.is_empty() {
                    text.push(' ');
                }
                let icon = resolver.icon_name(&tag);
                match icon.filter(|i| show_icons && Some(i) != default_icon.as_ref()) {
                    Some(i) => {
                        let i = if i.trim().starts_with("$(") {
                            i
                        } else {
                            format!("$({i})")
                        };
                        text.push_str(&format!("{i} {n}  "));
                    }
                    None => text.push_str(&format!("{tag}: {n} ")),
                }
            }
            let mut text = if show_icons {
                text.trim().to_string()
            } else {
                format!("$(check) {}", text.trim())
            };
            if counts.is_empty() {
                text = "$(check) 0".to_string();
            }
            let tooltip = match mode {
                StatusBarMode::CurrentFile => "Clippings tags counts in current file",
                StatusBarMode::TopThree => "Clippings top three tag counts",
                _ => "Clippings tags counts",
            };
            (text, tooltip.to_string(), true)
        }
    };
    if visible {
        match settings.tree.scan_mode {
            ScanMode::OpenFiles => text.push_str(" (in open files)"),
            ScanMode::CurrentFile => text.push_str(" (in current file)"),
            _ => {}
        }
    }
    let badge_value = if settings.general.show_activity_bar_badge {
        total
    } else {
        0
    };
    Summary {
        status_bar: StatusBar {
            text,
            tooltip,
            visible,
        },
        badge: Badge {
            value: badge_value,
            tooltip: format!("{badge_value} todos"),
        },
        view_title: title,
    }
}
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test -p clippings-core status`
Expected: 3 tests pass.
Then run: `cargo fmt --all && cargo clippy --all-targets -- -D warnings`
Expected: no diffs, no warnings.

- [ ] **Step 5: Commit**

```bash
git add -A
git commit -F - <<'EOF'
feat(core): status bar text, badge and view title

Co-Authored-By: <your model attribution>
EOF
```

### Task 11: Go to next and previous

Spec section 5.15: search the open buffer from each cursor with the shared matcher. Neither direction wraps, and if any cursor has no match, no cursor moves.

**Files:**
- Create: `crates/clippings-core/src/navigate.rs`
- Modify: `crates/clippings-core/src/lib.rs` (add `pub mod navigate;`, keeping the declarations sorted)

**Interfaces:**
- Consumes: `ScanPattern.matcher`, `LineIndex`.
- Produces: `Direction { Next, Previous }` (serde camelCase), `navigate(text: &[u8], &ScanPattern, cursors: &[Position], Direction) -> Option<Vec<Range>>`.

- [ ] **Step 1: Write the failing tests**

Add `pub mod navigate;` to `crates/clippings-core/src/lib.rs`. Create `crates/clippings-core/src/navigate.rs` containing only this test module:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::CoreConfig;
    use crate::pattern::build;

    const TEXT: &[u8] = b"a // TODO one\nb\n// FIXME two\n";

    fn p(line: u32, character: u32) -> Position {
        Position { line, character }
    }

    #[test]
    fn next_skips_a_match_at_the_cursor() {
        let pat = build(&CoreConfig::default()).unwrap();
        let r = navigate(TEXT, &pat, &[p(0, 2)], Direction::Next).unwrap();
        assert_eq!(r[0].start, p(2, 0));
        let r = navigate(TEXT, &pat, &[p(0, 0)], Direction::Next).unwrap();
        assert_eq!(r[0].start, p(0, 2));
    }

    #[test]
    fn previous_and_no_wrap() {
        let pat = build(&CoreConfig::default()).unwrap();
        let r = navigate(TEXT, &pat, &[p(2, 0)], Direction::Previous).unwrap();
        assert_eq!(
            r[0],
            Range {
                start: p(0, 2),
                end: p(0, 9)
            }
        );
        assert!(navigate(TEXT, &pat, &[p(2, 5)], Direction::Next).is_none());
        assert!(
            navigate(TEXT, &pat, &[p(0, 0), p(2, 5)], Direction::Next).is_none(),
            "all or nothing"
        );
    }
}
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p clippings-core navigate`
Expected: compile error, `navigate` not found.

- [ ] **Step 3: Write the implementation**

Put this above the test module in `crates/clippings-core/src/navigate.rs`:

```rust
//! Go to next and previous todo in an open buffer (spec section 5.15).

use crate::pattern::ScanPattern;
use crate::position::{LineIndex, Position, Range};
use grep_matcher::Matcher;
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Direction {
    Next,
    Previous,
}

/// One target range per cursor, or `None` when any cursor has no match.
pub fn navigate(
    text: &[u8],
    pattern: &ScanPattern,
    cursors: &[Position],
    direction: Direction,
) -> Option<Vec<Range>> {
    let li = LineIndex::new(text);
    let mut matches: Vec<(usize, usize)> = Vec::new();
    let _ = pattern.matcher.find_iter(text, |m| {
        if m.start() != m.end() {
            matches.push((m.start(), m.end()));
        }
        true
    });
    cursors
        .iter()
        .map(|c| {
            let at = li.offset(*c);
            let hit = match direction {
                Direction::Next => matches.iter().find(|m| m.0 > at),
                Direction::Previous => matches.iter().rev().find(|m| m.0 < at),
            };
            hit.map(|&(s, e)| Range {
                start: li.position(s),
                end: li.position(e),
            })
        })
        .collect()
}
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test -p clippings-core navigate`
Expected: 2 tests pass.
Then run: `cargo fmt --all && cargo clippy --all-targets -- -D warnings`
Expected: no diffs, no warnings.

- [ ] **Step 5: Commit**

```bash
git add -A
git commit -F - <<'EOF'
feat(core): go to next and previous todo

Co-Authored-By: <your model attribution>
EOF
```

### Task 12: Protocol types and open-document sync

Spec section 6: the standard LSP payloads Clippings uses, defined here rather than taken from `lsp-types` so the wire shapes stay exact, and every custom `clippings/*` message. `documents` holds open buffers and applies incremental edits with UTF-16 ranges.

**Files:**
- Create: `crates/clippings-core/src/protocol.rs`, `crates/clippings-core/src/documents.rs`
- Modify: `crates/clippings-core/src/lib.rs` (add `pub mod documents;` and `pub mod protocol;`)

**Interfaces:**
- Produces: `protocol::method::*` constants; `WorkspaceFolder`, `InitializationOptions { protocol_version, settings }`, `InitializeParams { workspace_folders, initialization_options, capabilities }` with `dynamic_watchers()`, `DidOpenParams`, `DidChangeParams`, `ContentChange { range: Option<Range>, text }`, `DidCloseParams`, `FileEvent { uri, kind }` and `FILE_CREATED/FILE_CHANGED/FILE_DELETED`, `DidChangeWatchedFilesParams`, `DidChangeWorkspaceFoldersParams`, and the custom params and results `ActiveEditorParams`, `ChildrenParams/Result`, `FindParams/Result`, `NavigateParams/Result`, `ExportResult`, `TreeChangedParams { refresh }`, `StylesParams { generation, reset, styles }`, `DecorationsParams { uri, version, generation, ranges }`, `StatusParams`.
- Produces: `documents::{Document { uri, path, version, text, todos, admitted }, apply_changes(&mut String, &[ContentChange]), Documents { insert, get, get_mut, remove, iter, uris }}`.

- [ ] **Step 1: Write the failing tests**

Add both modules to `lib.rs`. Create `protocol.rs` containing only:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_initialize_params() {
        let p: InitializeParams = serde_json::from_str(
            r#"{"processId":1,"workspaceFolders":[{"uri":"file:///w","name":"w"}],
                "capabilities":{"workspace":{"didChangeWatchedFiles":{"dynamicRegistration":true}}},
                "initializationOptions":{"protocolVersion":1,"settings":{"general":{"tags":["TODO"]}}}}"#,
        )
        .unwrap();
        assert!(p.dynamic_watchers());
        let o = p.initialization_options.unwrap();
        assert_eq!(o.protocol_version, 1);
        assert_eq!(o.settings.general.tags, vec!["TODO"]);
    }

    #[test]
    fn status_is_camel_case() {
        let v = serde_json::to_value(StatusParams {
            needs_scan: true,
            view_title: "Tree".into(),
            ..Default::default()
        })
        .unwrap();
        assert_eq!(v["needsScan"], true);
        assert_eq!(v["viewTitle"], "Tree");
        assert_eq!(v["statusBar"]["visible"], false);
    }
}
```

and `documents.rs` containing only:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::position::{Position, Range};

    fn change(l1: u32, c1: u32, l2: u32, c2: u32, text: &str) -> ContentChange {
        ContentChange {
            range: Some(Range {
                start: Position {
                    line: l1,
                    character: c1,
                },
                end: Position {
                    line: l2,
                    character: c2,
                },
            }),
            text: text.into(),
        }
    }

    #[test]
    fn incremental_edits_in_utf16() {
        let mut t = "é // TODO a\nline two\n".to_string();
        apply_changes(&mut t, &[change(0, 10, 0, 11, "b")]);
        assert_eq!(t, "é // TODO b\nline two\n");
        apply_changes(
            &mut t,
            &[change(1, 0, 1, 4, "LINE"), change(0, 0, 0, 0, "x")],
        );
        assert_eq!(t, "xé // TODO b\nLINE two\n");
        apply_changes(&mut t, &[change(0, 12, 1, 0, " ")]);
        assert_eq!(t, "xé // TODO b LINE two\n");
        apply_changes(
            &mut t,
            &[ContentChange {
                range: None,
                text: "new".into(),
            }],
        );
        assert_eq!(t, "new");
    }
}
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p clippings-core protocol documents`
Expected: compile error, `InitializeParams` and `apply_changes` not found.

- [ ] **Step 3: Write the protocol types**

Put this above the tests in `crates/clippings-core/src/protocol.rs`:

```rust
//! Wire types (spec section 6): the standard LSP payloads Clippings uses and
//! the custom `clippings/*` messages. Mirrored in `extension/src/protocol.ts`.

use crate::position::{Position, Range};
use crate::settings::Settings;
use crate::status::{Badge, StatusBar};
use crate::styles::DecorationStyle;
use crate::view::ViewNode;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

pub mod method {
    pub const CONFIGURE: &str = "clippings/configure";
    pub const ACTIVE_EDITOR: &str = "clippings/activeEditor";
    pub const RESCAN: &str = "clippings/rescan";
    pub const STOP_SCAN: &str = "clippings/stopScan";
    pub const CHILDREN: &str = "clippings/children";
    pub const FIND: &str = "clippings/find";
    pub const NAVIGATE: &str = "clippings/navigate";
    pub const EXPORT: &str = "clippings/export";
    pub const TREE_CHANGED: &str = "clippings/treeChanged";
    pub const STYLES: &str = "clippings/styles";
    pub const DECORATIONS: &str = "clippings/decorations";
    pub const STATUS: &str = "clippings/status";
    pub const DID_OPEN: &str = "textDocument/didOpen";
    pub const DID_CHANGE: &str = "textDocument/didChange";
    pub const DID_CLOSE: &str = "textDocument/didClose";
    pub const DID_CHANGE_WATCHED_FILES: &str = "workspace/didChangeWatchedFiles";
    pub const DID_CHANGE_WORKSPACE_FOLDERS: &str = "workspace/didChangeWorkspaceFolders";
    pub const REGISTER_CAPABILITY: &str = "client/registerCapability";
    pub const UNREGISTER_CAPABILITY: &str = "client/unregisterCapability";
}

// ---- Standard LSP payloads ----

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkspaceFolder {
    pub uri: String,
    pub name: String,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InitializationOptions {
    pub protocol_version: u32,
    #[serde(default)]
    pub settings: Settings,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InitializeParams {
    #[serde(default)]
    pub workspace_folders: Option<Vec<WorkspaceFolder>>,
    #[serde(default)]
    pub initialization_options: Option<InitializationOptions>,
    #[serde(default)]
    pub capabilities: serde_json::Value,
}

impl InitializeParams {
    /// Whether the client can register file watchers dynamically.
    pub fn dynamic_watchers(&self) -> bool {
        self.capabilities
            .pointer("/workspace/didChangeWatchedFiles/dynamicRegistration")
            .and_then(|v| v.as_bool())
            .unwrap_or(false)
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TextDocumentItem {
    pub uri: String,
    #[serde(default)]
    pub language_id: String,
    pub version: i32,
    pub text: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DidOpenParams {
    pub text_document: TextDocumentItem,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct VersionedDocument {
    pub uri: String,
    pub version: i32,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DocumentId {
    pub uri: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ContentChange {
    /// Absent for a full-text replacement.
    #[serde(default)]
    pub range: Option<Range>,
    pub text: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DidChangeParams {
    pub text_document: VersionedDocument,
    pub content_changes: Vec<ContentChange>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DidCloseParams {
    pub text_document: DocumentId,
}

/// `1` created, `2` changed, `3` deleted.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FileEvent {
    pub uri: String,
    #[serde(rename = "type")]
    pub kind: u8,
}

pub const FILE_CREATED: u8 = 1;
pub const FILE_CHANGED: u8 = 2;
pub const FILE_DELETED: u8 = 3;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DidChangeWatchedFilesParams {
    pub changes: Vec<FileEvent>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct WorkspaceFoldersChange {
    pub added: Vec<WorkspaceFolder>,
    pub removed: Vec<WorkspaceFolder>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DidChangeWorkspaceFoldersParams {
    pub event: WorkspaceFoldersChange,
}

// ---- Custom messages ----

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ActiveEditorParams {
    pub uri: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ChildrenParams {
    pub parent: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ChildrenResult {
    pub nodes: Vec<ViewNode>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FindParams {
    pub uri: String,
    pub line: Option<u32>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FindResult {
    pub paths: Vec<Vec<ViewNode>>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct NavigateParams {
    pub uri: String,
    pub positions: Vec<Position>,
    pub direction: crate::navigate::Direction,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct NavigateResult {
    pub ranges: Option<Vec<Range>>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ExportResult {
    pub path: String,
    pub content: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TreeChangedParams {
    pub refresh: Vec<Option<String>>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct StylesParams {
    pub generation: u64,
    pub reset: bool,
    pub styles: BTreeMap<String, DecorationStyle>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct DecorationsParams {
    pub uri: String,
    pub version: i32,
    pub generation: u64,
    pub ranges: BTreeMap<String, Vec<Range>>,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StatusParams {
    pub instance: String,
    pub scanning: bool,
    pub interrupted: bool,
    pub needs_scan: bool,
    pub error: Option<String>,
    pub warnings: Vec<String>,
    pub status_bar: StatusBar,
    pub badge: Badge,
    pub view_title: String,
    pub has_sub_tags: bool,
    pub is_empty: bool,
}
```

- [ ] **Step 4: Write document sync**

Put this above the tests in `crates/clippings-core/src/documents.rs`:

```rust
//! Open documents and incremental text sync.

use crate::model::Todo;
use crate::position::LineIndex;
use crate::protocol::ContentChange;
use std::collections::BTreeMap;
use std::path::PathBuf;

#[derive(Clone, Debug)]
pub struct Document {
    pub uri: String,
    pub path: Option<PathBuf>,
    pub version: i32,
    pub text: String,
    /// The last scan of this buffer, used for decorations.
    pub todos: Vec<Todo>,
    /// Whether the document passes the open-buffer admission rules and schemes.
    pub admitted: bool,
}

/// Applies LSP content changes in order. Ranges use UTF-16 positions.
pub fn apply_changes(text: &mut String, changes: &[ContentChange]) {
    for c in changes {
        match c.range {
            None => *text = c.text.clone(),
            Some(r) => {
                let li = LineIndex::new(text.as_bytes());
                let a = li.offset(r.start).min(text.len());
                let b = li.offset(r.end).clamp(a, text.len());
                text.replace_range(a..b, &c.text);
            }
        }
    }
}

#[derive(Default)]
pub struct Documents {
    docs: BTreeMap<String, Document>,
}

impl Documents {
    pub fn insert(&mut self, doc: Document) {
        self.docs.insert(doc.uri.clone(), doc);
    }
    pub fn get(&self, uri: &str) -> Option<&Document> {
        self.docs.get(uri)
    }
    pub fn get_mut(&mut self, uri: &str) -> Option<&mut Document> {
        self.docs.get_mut(uri)
    }
    pub fn remove(&mut self, uri: &str) -> Option<Document> {
        self.docs.remove(uri)
    }
    pub fn iter(&self) -> impl Iterator<Item = &Document> {
        self.docs.values()
    }
    pub fn uris(&self) -> Vec<String> {
        self.docs.keys().cloned().collect()
    }
}
```

- [ ] **Step 5: Run the tests to verify they pass**

Run: `cargo test -p clippings-core protocol documents`
Expected: 3 tests pass.
Then run: `cargo fmt --all && cargo clippy --all-targets -- -D warnings`
Expected: no diffs, no warnings.

- [ ] **Step 6: Commit**

```bash
git add -A
git commit -F - <<'EOF'
feat(core): protocol types and incremental document sync

Co-Authored-By: <your model attribution>
EOF
```

### Task 13: File watching and git HEAD helpers

The watcher registration payload for VS Code (one `**/*` relative pattern per walked root), the `notify` fallback, and `git rev-parse HEAD` without a shell (spec sections 5.10 and 5.11).

**Files:**
- Create: `crates/clippings-core/src/server/mod.rs` (temporary, two lines), `crates/clippings-core/src/server/watch.rs`, `crates/clippings-core/src/server/git.rs`
- Modify: `crates/clippings-core/src/lib.rs` (add `pub mod server;`)

**Interfaces:**
- Produces: `server::watch::{REGISTRATION_ID, register_params(&[PathBuf]) -> Value, unregister_params() -> Value, to_file_events(&notify::Event) -> Vec<FileEvent>, NotifyWatcher::new(roots, on_events: impl Fn(Option<Vec<FileEvent>>) + Send + 'static) -> notify::Result<NotifyWatcher>}`; `on_events(None)` means the watcher failed.
- Produces: `server::git::head(&Path) -> Option<String>`.

- [ ] **Step 1: Write the failing tests**

Add `pub mod server;` to `lib.rs`. Create `crates/clippings-core/src/server/mod.rs` containing:

```rust
//! The language server.

pub mod git;
pub mod watch;
```

Create `watch.rs` containing only:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registration_uses_relative_patterns() {
        let v = register_params(&[PathBuf::from("/w/a b")]);
        assert_eq!(
            v["registrations"][0]["registerOptions"]["watchers"][0]["globPattern"]["baseUri"],
            "file:///w/a%20b"
        );
        assert_eq!(
            v["registrations"][0]["method"],
            "workspace/didChangeWatchedFiles"
        );
    }

    #[test]
    fn notify_reports_created_files() {
        let dir = tempfile::tempdir().unwrap();
        let root = dunce::canonicalize(dir.path()).unwrap();
        let (tx, rx) = std::sync::mpsc::channel();
        let _w = NotifyWatcher::new(std::slice::from_ref(&root), move |e| {
            let _ = tx.send(e);
        })
        .unwrap();
        std::thread::sleep(std::time::Duration::from_millis(200));
        std::fs::write(root.join("new.ts"), "// TODO\n").unwrap();
        let want = file_uri(&root.join("new.ts"));
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
        let mut seen = false;
        while std::time::Instant::now() < deadline && !seen {
            if let Ok(Some(events)) = rx.recv_timeout(std::time::Duration::from_millis(500)) {
                seen = events.iter().any(|e| e.uri == want);
            }
        }
        assert!(seen, "no event for {want}");
    }
}
```

and `git.rs` containing only:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn head_of_a_fresh_repo_and_a_plain_dir() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(head(dir.path()), None);
        let run = |args: &[&str]| {
            assert!(Command::new("git")
                .args(args)
                .current_dir(dir.path())
                .status()
                .unwrap()
                .success());
        };
        run(&["init", "-q"]);
        run(&[
            "-c",
            "user.name=t",
            "-c",
            "user.email=t@t",
            "commit",
            "-q",
            "--allow-empty",
            "-m",
            "x",
        ]);
        let h = head(dir.path()).unwrap();
        assert_eq!(h.len(), 40);
    }
}
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p clippings-core server::`
Expected: compile error, `register_params`, `NotifyWatcher` and `head` not found.

- [ ] **Step 3: Write the watcher helpers**

Put this above the tests in `watch.rs`:

```rust
//! File watching (spec section 5.10): client-side registration payloads and
//! the `notify` fallback used when the client cannot register watchers and
//! by `clippings watch`.

use crate::protocol::{FileEvent, FILE_CHANGED, FILE_CREATED, FILE_DELETED};
use crate::uri::file_uri;
use notify::event::{EventKind, ModifyKind};
use notify::{RecursiveMode, Watcher};
use serde_json::{json, Value};
use std::path::PathBuf;

pub const REGISTRATION_ID: &str = "clippings-watchers";

/// `client/registerCapability` params: one `**/*` watcher per walked root.
pub fn register_params(roots: &[PathBuf]) -> Value {
    let watchers: Vec<Value> = roots
        .iter()
        .map(|r| json!({ "globPattern": { "baseUri": file_uri(r), "pattern": "**/*" } }))
        .collect();
    json!({ "registrations": [{
        "id": REGISTRATION_ID,
        "method": "workspace/didChangeWatchedFiles",
        "registerOptions": { "watchers": watchers },
    }]})
}

pub fn unregister_params() -> Value {
    json!({ "unregisterations": [{ "id": REGISTRATION_ID, "method": "workspace/didChangeWatchedFiles" }] })
}

/// Converts a notify event into LSP file events.
pub fn to_file_events(event: &notify::Event) -> Vec<FileEvent> {
    let kind = |p: &PathBuf| match event.kind {
        EventKind::Create(_) => FILE_CREATED,
        EventKind::Remove(_) => FILE_DELETED,
        EventKind::Modify(ModifyKind::Name(_)) => {
            if p.exists() {
                FILE_CREATED
            } else {
                FILE_DELETED
            }
        }
        _ => FILE_CHANGED,
    };
    if matches!(event.kind, EventKind::Access(_)) {
        return Vec::new();
    }
    event
        .paths
        .iter()
        .map(|p| FileEvent {
            uri: file_uri(p),
            kind: kind(p),
        })
        .collect()
}

/// A recursive `notify` watcher over the given roots.
pub struct NotifyWatcher {
    _watcher: notify::RecommendedWatcher,
}

impl NotifyWatcher {
    /// `on_events` receives converted events, or `None` when the watcher
    /// reports an error such as an overflow.
    pub fn new(
        roots: &[PathBuf],
        on_events: impl Fn(Option<Vec<FileEvent>>) + Send + 'static,
    ) -> notify::Result<Self> {
        let mut watcher =
            notify::recommended_watcher(move |res: notify::Result<notify::Event>| match res {
                Ok(e) => {
                    let events = to_file_events(&e);
                    if !events.is_empty() {
                        on_events(Some(events));
                    }
                }
                Err(_) => on_events(None),
            })?;
        for r in roots {
            watcher.watch(r, RecursiveMode::Recursive)?;
        }
        Ok(Self { _watcher: watcher })
    }
}
```

- [ ] **Step 4: Write the git helper**

Put this above the tests in `git.rs`:

```rust
//! `git rev-parse HEAD` for automatic git refresh (spec section 5.11): run
//! without a shell, one call per workspace folder.

use std::path::Path;
use std::process::Command;

/// The folder's HEAD commit, or `None` when it is not a git work tree.
pub fn head(folder: &Path) -> Option<String> {
    let out = Command::new("git")
        .args(["rev-parse", "HEAD"])
        .current_dir(folder)
        .output()
        .ok()?;
    out.status
        .success()
        .then(|| String::from_utf8_lossy(&out.stdout).trim().to_string())
}
```

- [ ] **Step 5: Run the tests to verify they pass**

Run: `cargo test -p clippings-core server::`
Expected: 3 tests pass. The notify test waits for a real filesystem event, for up to 10 seconds.
Then run: `cargo fmt --all && cargo clippy --all-targets -- -D warnings`
Expected: no diffs, no warnings.

- [ ] **Step 6: Commit**

```bash
git add -A
git commit -F - <<'EOF'
feat(core): watcher registration, notify fallback and git HEAD

Co-Authored-By: <your model attribution>
EOF
```

### Task 14: The scheduler and server loop

Spec sections 5.8 to 5.11, 6 and 10.1. `Server` owns settings, roots, pattern, admission, index, open documents and the view, and handles every client message. It runs full walks and git polls on worker threads, and turns timers into view rebuilds, buffer rescans, decorations and file-event processing. `main_loop::run` does the `initialize` handshake with the protocol-version check, then selects over the connection, worker results and the next deadline. A panic in a handler answers the request with an error, or drops the index and rescans (`Server::recover`).

**Files:**
- Modify: `crates/clippings-core/src/server/mod.rs` (replace the temporary file)
- Create: `crates/clippings-core/src/server/main_loop.rs`, `crates/clippings-core/tests/server.rs`

**Interfaces:**
- Consumes: everything above.
- Produces: `server::{VIEW_DELAY, EVENT_DELAY, BUFFER_DELAY, Env, Work, Server}` with `Server::new(out: Sender<Message>, fs, env, &InitializeParams)`, `start(now)`, `handle_notification(Notification, now)`, `handle_request(Request) -> Response`, `handle_work(Work, now)`, `next_deadline() -> Option<Instant>`, `tick(now)`, `recover(now)`, and the public `work_rx`.
- Produces: `server::main_loop::{run(Connection, Arc<dyn Fs>, Env) -> Result<(), String>, run_stdio() -> Result<(), String>}`. Plan 3's extension depends on this behaviour: initialize result `capabilities.textDocumentSync = { openClose: true, change: 2 }`; the first `clippings/styles` is a reset with generation 0; `clippings/status` is sent only when it changes.

- [ ] **Step 1: Write the failing tests**

`crates/clippings-core/tests/server.rs`, which drives the real loop over an in-memory connection. The last three tests cover Review Focus items 3 to 5:

```rust
//! Protocol integration tests (spec section 12.4): the real server loop over
//! an in-memory connection.

use clippings_core::fs::NativeFs;
use clippings_core::server::main_loop::run;
use clippings_core::uri::file_uri;
use lsp_server::{Connection, Message, Notification, Request, RequestId};
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

const WAIT: Duration = Duration::from_secs(10);

struct Client {
    conn: Connection,
    thread: Option<JoinHandle<Result<(), String>>>,
    backlog: Vec<Message>,
    next_id: i32,
}

fn settings() -> Value {
    json!({ "highlights": { "highlightDelay": 0 }, "tree": { "showCurrentScanMode": false } })
}

impl Client {
    fn start(root: &Path, settings: Value, dynamic: bool) -> Client {
        Self::start_with(
            root,
            json!({ "protocolVersion": 1, "settings": settings }),
            dynamic,
        )
    }

    fn start_with(root: &Path, options: Value, dynamic: bool) -> Client {
        let (server, conn) = Connection::memory();
        let thread = std::thread::spawn(move || {
            run(
                server,
                Arc::new(NativeFs),
                Arc::new(|n| std::env::var(n).ok()),
            )
        });
        let mut c = Client {
            conn,
            thread: Some(thread),
            backlog: Vec::new(),
            next_id: 0,
        };
        let params = json!({
            "processId": null,
            "workspaceFolders": [{ "uri": file_uri(root), "name": "w" }],
            "capabilities": { "workspace": { "didChangeWatchedFiles": { "dynamicRegistration": dynamic } } },
            "initializationOptions": options,
        });
        let resp = c.request_raw("initialize", params);
        if resp.get("error").is_none() {
            c.notify("initialized", json!({}));
        }
        c
    }

    fn notify(&self, method: &str, params: Value) {
        self.conn
            .sender
            .send(Notification::new(method.to_string(), params).into())
            .unwrap();
    }

    /// Sends a request and returns the raw response (`result` or `error`).
    fn request_raw(&mut self, method: &str, params: Value) -> Value {
        self.next_id += 1;
        let id = RequestId::from(self.next_id);
        self.conn
            .sender
            .send(Request::new(id.clone(), method.to_string(), params).into())
            .unwrap();
        let deadline = Instant::now() + WAIT;
        loop {
            let msg = self
                .conn
                .receiver
                .recv_timeout(deadline - Instant::now())
                .expect("response");
            match msg {
                Message::Response(r) if r.id == id => {
                    return match r.response_result {
                        Ok(v) => json!({ "result": v }),
                        Err(e) => json!({ "error": e.message }),
                    };
                }
                other => self.backlog.push(other),
            }
        }
    }

    fn request(&mut self, method: &str, params: Value) -> Value {
        self.request_raw(method, params)["result"].clone()
    }

    /// Waits for a notification (or server request) with `method` matching `pred`.
    fn expect(&mut self, method: &str, pred: impl Fn(&Value) -> bool) -> Value {
        if let Some(i) = self.backlog.iter().position(|m| matches(m, method, &pred)) {
            return params(&self.backlog.remove(i));
        }
        let deadline = Instant::now() + WAIT;
        loop {
            let left = deadline.saturating_duration_since(Instant::now());
            let msg = self
                .conn
                .receiver
                .recv_timeout(left)
                .unwrap_or_else(|_| panic!("timed out waiting for {method}"));
            if matches(&msg, method, &pred) {
                return params(&msg);
            }
            self.backlog.push(msg);
        }
    }

    /// Waits until the tree has settled after a scan, then returns top-level labels.
    fn settled_top(&mut self) -> Vec<String> {
        self.expect("clippings/status", |s| s["scanning"] == false);
        std::thread::sleep(Duration::from_millis(200));
        labels(&self.request("clippings/children", json!({ "parent": null })))
    }

    /// Forgets every message received so far, so the next `expect` waits for a fresh one.
    fn drain(&mut self) {
        std::thread::sleep(Duration::from_millis(100));
        self.backlog.clear();
        while self.conn.receiver.try_recv().is_ok() {}
    }

    fn children(&mut self, parent: Option<&str>) -> Value {
        self.request("clippings/children", json!({ "parent": parent }))
    }

    fn shutdown(mut self) {
        let _ = self.request_raw("shutdown", Value::Null);
        self.notify("exit", Value::Null);
        let r = self.thread.take().unwrap().join().unwrap();
        assert!(r.is_ok(), "{r:?}");
    }
}

fn matches(m: &Message, method: &str, pred: &impl Fn(&Value) -> bool) -> bool {
    match m {
        Message::Notification(n) => n.method == method && pred(&n.params),
        Message::Request(r) => r.method == method && pred(&r.params),
        _ => false,
    }
}

fn params(m: &Message) -> Value {
    match m {
        Message::Notification(n) => n.params.clone(),
        Message::Request(r) => r.params.clone(),
        _ => Value::Null,
    }
}

fn labels(children: &Value) -> Vec<String> {
    children["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .map(|n| n["label"].as_str().unwrap().to_string())
        .collect()
}

fn workspace() -> (tempfile::TempDir, PathBuf) {
    let t = tempfile::tempdir().unwrap();
    let root = dunce::canonicalize(t.path()).unwrap();
    std::fs::create_dir_all(root.join("src")).unwrap();
    std::fs::write(root.join("src/a.ts"), "// TODO alpha\n").unwrap();
    std::fs::write(root.join("b.ts"), "// FIXME beta\n").unwrap();
    (t, root)
}

#[test]
fn initialize_scans_and_serves_the_tree() {
    let (_t, root) = workspace();
    let mut c = Client::start(&root, settings(), true);
    let reg = c.expect("client/registerCapability", |_| true);
    assert_eq!(
        reg["registrations"][0]["registerOptions"]["watchers"][0]["globPattern"]["baseUri"],
        file_uri(&root)
    );
    assert_eq!(
        c.settled_top(),
        vec![root.file_name().unwrap().to_string_lossy().to_string()]
    );
    let top = c.children(None);
    let root_id = top["nodes"][0]["id"].as_str().unwrap().to_string();
    assert_eq!(labels(&c.children(Some(&root_id))), vec!["src", "b.ts"]);
    let found = c.request(
        "clippings/find",
        json!({ "uri": file_uri(&root.join("b.ts")), "line": null }),
    );
    assert_eq!(found["paths"][0].as_array().unwrap().len(), 2);
    c.shutdown();
}

#[test]
fn open_buffers_get_styles_then_decorations_and_feed_the_tree() {
    let (_t, root) = workspace();
    let mut c = Client::start(&root, settings(), true);
    c.settled_top();
    let uri = file_uri(&root.join("b.ts"));
    c.notify("textDocument/didOpen", json!({ "textDocument": { "uri": uri, "languageId": "typescript", "version": 1, "text": "// FIXME beta\n// TODO gamma\n" } }));
    let styles = c.expect("clippings/styles", |s| s["reset"] == false);
    assert!(styles["styles"].get("FIXME").is_some() && styles["styles"].get("TODO").is_some());
    let deco = c.expect("clippings/decorations", |d| d["version"] == 1);
    assert_eq!(deco["uri"], uri);
    assert_eq!(
        deco["ranges"]["TODO"][0]["start"],
        json!({ "line": 1, "character": 3 })
    );
    c.expect("clippings/treeChanged", |_| true);

    c.notify("textDocument/didChange", json!({
        "textDocument": { "uri": uri, "version": 2 },
        "contentChanges": [{ "range": { "start": { "line": 1, "character": 3 }, "end": { "line": 1, "character": 7 } }, "text": "BUG" }]
    }));
    let deco = c.expect("clippings/decorations", |d| d["version"] == 2);
    assert!(deco["ranges"].get("TODO").is_none() && deco["ranges"].get("BUG").is_some());
    std::thread::sleep(Duration::from_millis(400));
    let root_id = c.children(None)["nodes"][0]["id"]
        .as_str()
        .unwrap()
        .to_string();
    let file_id = c.children(Some(&root_id))["nodes"][1]["id"]
        .as_str()
        .unwrap()
        .to_string();
    assert_eq!(
        labels(&c.children(Some(&file_id))),
        vec!["FIXME beta", "BUG gamma"],
        "by line"
    );

    let nav = c.request(
        "clippings/navigate",
        json!({ "uri": uri, "positions": [{ "line": 0, "character": 0 }], "direction": "next" }),
    );
    // The FIXME match starts at the cursor, so "next" is the todo on line 1.
    assert_eq!(
        nav["ranges"][0]["start"],
        json!({ "line": 1, "character": 0 })
    );

    c.notify(
        "textDocument/didClose",
        json!({ "textDocument": { "uri": uri } }),
    );
    std::thread::sleep(Duration::from_millis(300));
    assert_eq!(
        labels(&c.children(Some(&file_id))),
        vec!["FIXME beta"],
        "closing falls back to the disk content"
    );
    c.shutdown();
}

#[test]
fn file_events_update_the_tree() {
    let (_t, root) = workspace();
    let mut c = Client::start(&root, settings(), true);
    c.settled_top();
    c.drain();
    std::fs::create_dir_all(root.join("lib")).unwrap();
    std::fs::write(root.join("lib/new.ts"), "// HACK new\n").unwrap();
    c.notify(
        "workspace/didChangeWatchedFiles",
        json!({ "changes": [{ "uri": file_uri(&root.join("lib")), "type": 1 }] }),
    );
    c.expect("clippings/treeChanged", |_| true);
    let root_id = c.children(None)["nodes"][0]["id"]
        .as_str()
        .unwrap()
        .to_string();
    assert_eq!(
        labels(&c.children(Some(&root_id))),
        vec!["lib", "src", "b.ts"]
    );

    c.drain();
    std::fs::remove_dir_all(root.join("lib")).unwrap();
    c.notify(
        "workspace/didChangeWatchedFiles",
        json!({ "changes": [{ "uri": file_uri(&root.join("lib")), "type": 3 }] }),
    );
    c.expect("clippings/treeChanged", |_| true);
    assert_eq!(labels(&c.children(Some(&root_id))), vec!["src", "b.ts"]);
    c.shutdown();
}

#[test]
fn configuration_changes_rescan_or_rebuild() {
    let (_t, root) = workspace();
    let mut c = Client::start(&root, settings(), true);
    c.settled_top();
    c.drain();
    let mut s = settings();
    s["viewState"] = json!({ "flat": true });
    c.notify("clippings/configure", s.clone());
    let changed = c.expect("clippings/treeChanged", |_| true);
    assert_eq!(changed["refresh"], json!([null]));
    let root_id = c.children(None)["nodes"][0]["id"]
        .as_str()
        .unwrap()
        .to_string();
    // Flat view sorts files by path: /root/b.ts before /root/src/a.ts.
    assert_eq!(
        labels(&c.children(Some(&root_id))),
        vec!["b.ts", "a.ts (src)"]
    );

    s["general"] = json!({ "tags": ["TODO"] });
    c.drain();
    c.notify("clippings/configure", s);
    c.expect("clippings/status", |st| st["scanning"] == true);
    c.expect("clippings/status", |st| st["scanning"] == false);
    std::thread::sleep(Duration::from_millis(200));
    assert_eq!(labels(&c.children(Some(&root_id))), vec!["a.ts (src)"]);

    let exported = c.request("clippings/export", json!({}));
    assert!(exported["content"].as_str().unwrap().contains("a.ts (src)"));
    c.shutdown();
}

#[test]
fn invalid_regex_reports_an_error_and_keeps_the_tree() {
    let (_t, root) = workspace();
    let mut c = Client::start(&root, settings(), true);
    c.settled_top();
    let mut s = settings();
    s["regex"] = json!({ "regex": "(unclosed" });
    c.notify("clippings/configure", s);
    let st = c.expect("clippings/status", |st| st["error"].is_string());
    assert!(st["error"].as_str().unwrap().contains("regex"));
    c.shutdown();
}

#[test]
fn protocol_version_mismatch_is_rejected() {
    let (_t, root) = workspace();
    let c = Client::start_with(&root, json!({ "protocolVersion": 99 }), true);
    let mut c = c;
    let r = c.thread.take().unwrap().join().unwrap();
    assert!(r.unwrap_err().contains("protocol version mismatch"));
}

#[test]
fn without_dynamic_registration_notify_watches_the_disk() {
    let (_t, root) = workspace();
    let mut c = Client::start(&root, settings(), false);
    c.settled_top();
    std::thread::sleep(Duration::from_millis(300));
    std::fs::write(root.join("c.ts"), "// XXX from disk\n").unwrap();
    let deadline = Instant::now() + WAIT;
    loop {
        let root_id = c.children(None)["nodes"][0]["id"]
            .as_str()
            .unwrap()
            .to_string();
        if labels(&c.children(Some(&root_id))).contains(&"c.ts".to_string()) {
            break;
        }
        assert!(Instant::now() < deadline, "notify never reported c.ts");
        std::thread::sleep(Duration::from_millis(100));
    }
    c.shutdown();
}
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p clippings-core --test server`
Expected: compile error, unresolved `clippings_core::server::main_loop`.

- [ ] **Step 3: Write the server**

Replace `crates/clippings-core/src/server/mod.rs`:

```rust
//! The language server (spec sections 5.10, 5.11, 6): one scheduler owns
//! the index, open documents and view, and turns client messages, worker
//! results and timers into outgoing notifications.

pub mod git;
pub mod main_loop;
pub mod watch;

use crate::admission::Admission;
use crate::config::ScanMode;
use crate::decorations::decorate;
use crate::documents::{apply_changes, Document, Documents};
use crate::fs::Fs;
use crate::index::{BufferEntry, EffectiveContext, Index};
use crate::labels::unexpected_placeholder;
use crate::navigate::navigate;
use crate::pattern::{self, ScanPattern};
use crate::protocol::{self as p, method, FileEvent, InitializeParams, StatusParams};
use crate::roots::{resolve_roots, walked_roots, Roots};
use crate::scanner::{scan_file, scan_text};
use crate::settings::{Settings, StatusBarMode};
use crate::status::summarize;
use crate::styles::{colour_warnings, Resolver};
use crate::uri::{scheme, uri_to_path};
use crate::view::delta::delta;
use crate::view::export::{export_content, export_path};
use crate::view::render::View;
use crate::walker::{walk_and_scan, WalkOutcome};
use crossbeam_channel::{unbounded, Receiver, Sender};
use lsp_server::{Message, Notification, Request, RequestId, Response};
use serde::de::DeserializeOwned;
use serde::Serialize;
use std::collections::{BTreeMap, HashMap, HashSet};
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

/// View rebuilds coalesce for this long.
pub const VIEW_DELAY: Duration = Duration::from_millis(50);
/// File events coalesce for this long.
pub const EVENT_DELAY: Duration = Duration::from_millis(50);
/// A buffer is rescanned for the tree after this long without edits.
pub const BUFFER_DELAY: Duration = Duration::from_millis(150);

pub type Env = Arc<dyn Fn(&str) -> Option<String> + Send + Sync>;

/// Work finished off the scheduler thread.
pub enum Work {
    Walk {
        generation: u64,
        roots: Vec<PathBuf>,
        outcome: Result<WalkOutcome, String>,
    },
    Git {
        folder: PathBuf,
        head: Option<String>,
    },
    /// Events from the notify fallback; `None` means the watcher failed.
    Files(Option<Vec<FileEvent>>),
}

pub struct Server {
    out: Sender<Message>,
    fs: Arc<dyn Fs>,
    env: Env,
    settings: Settings,
    folders: Vec<PathBuf>,
    roots: Roots,
    walked: Vec<PathBuf>,
    pattern: Arc<ScanPattern>,
    error: Option<String>,
    admission: Arc<Admission>,
    index: Index,
    docs: Documents,
    view: View,
    active_uri: Option<String>,
    instance: String,
    scan_generation: u64,
    cancel: Arc<AtomicBool>,
    scanning: bool,
    interrupted: bool,
    needs_scan: bool,
    style_generation: u64,
    sent_styles: HashSet<String>,
    view_due: Option<Instant>,
    whole_tree: bool,
    buffer_due: BTreeMap<String, Instant>,
    decorations_due: BTreeMap<String, Instant>,
    events: Vec<FileEvent>,
    events_due: Option<Instant>,
    git_due: Option<Instant>,
    periodic_due: Option<Instant>,
    git_heads: HashMap<PathBuf, String>,
    git_in_flight: HashSet<PathBuf>,
    dynamic_watchers: bool,
    registered: bool,
    next_request: i32,
    notify: Option<watch::NotifyWatcher>,
    work_tx: Sender<Work>,
    pub work_rx: Receiver<Work>,
    last_status: Option<StatusParams>,
}

fn instance_id() -> String {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_nanos());
    format!("{:x}-{:x}", std::process::id(), nanos)
}

fn folder_paths(folders: &[p::WorkspaceFolder]) -> Vec<PathBuf> {
    folders
        .iter()
        .filter(|f| scheme(&f.uri).as_deref() == Some("file"))
        .filter_map(|f| uri_to_path(&f.uri))
        .collect()
}

impl Server {
    pub fn new(
        out: Sender<Message>,
        fs: Arc<dyn Fs>,
        env: Env,
        params: &InitializeParams,
    ) -> Server {
        let settings = params
            .initialization_options
            .clone()
            .unwrap_or_default()
            .settings;
        let folders = folder_paths(params.workspace_folders.as_deref().unwrap_or(&[]));
        let (work_tx, work_rx) = unbounded();
        let core = settings.core();
        let (pattern, error) = match pattern::build(&core) {
            Ok(pt) => (pt, None),
            Err(e) => (
                pattern::build(&crate::config::CoreConfig::default()).expect("default pattern"),
                Some(e.to_string()),
            ),
        };
        let mut s = Server {
            out,
            fs: fs.clone(),
            env,
            settings,
            folders,
            roots: Roots {
                tree_roots: vec![],
                scan_roots: vec![],
                from_root_folder: false,
            },
            walked: vec![],
            pattern: Arc::new(pattern),
            error,
            admission: Arc::new(Admission::new(&core, vec![], fs).expect("default admission")),
            index: Index::new(),
            docs: Documents::default(),
            view: View::default(),
            active_uri: None,
            instance: instance_id(),
            scan_generation: 0,
            cancel: Arc::new(AtomicBool::new(false)),
            scanning: false,
            interrupted: false,
            needs_scan: false,
            style_generation: 0,
            sent_styles: HashSet::new(),
            view_due: None,
            whole_tree: true,
            buffer_due: BTreeMap::new(),
            decorations_due: BTreeMap::new(),
            events: Vec::new(),
            events_due: None,
            git_due: None,
            periodic_due: None,
            git_heads: HashMap::new(),
            git_in_flight: HashSet::new(),
            dynamic_watchers: params.dynamic_watchers(),
            registered: false,
            next_request: 0,
            notify: None,
            work_tx,
            work_rx,
            last_status: None,
        };
        s.rebuild_scan_state();
        s
    }

    fn send_notification(&self, method: &str, params: impl Serialize) {
        let _ = self
            .out
            .send(Notification::new(method.to_string(), params).into());
    }

    fn send_request(&mut self, method: &str, params: impl Serialize) {
        self.next_request += 1;
        let id = RequestId::from(format!("clippings-{}", self.next_request));
        let _ = self
            .out
            .send(Request::new(id, method.to_string(), params).into());
    }

    /// Called once the client has sent `initialized`.
    pub fn start(&mut self, now: Instant) {
        self.update_watchers();
        self.send_notification(
            method::STYLES,
            p::StylesParams {
                generation: 0,
                reset: true,
                styles: BTreeMap::new(),
            },
        );
        if self.settings.tree.scan_at_startup {
            self.full_rescan();
        } else {
            self.needs_scan = true;
        }
        self.reset_timers(now);
        self.schedule_view(now, true);
        self.send_status();
    }

    fn rebuild_scan_state(&mut self) {
        let core = self.settings.core();
        match pattern::build(&core) {
            Ok(pt) => {
                self.pattern = Arc::new(pt);
                self.error = None;
            }
            Err(e) => self.error = Some(e.to_string()),
        }
        let env = self.env.clone();
        match resolve_roots(&self.folders, &core, &|n| env(n)) {
            Ok(r) => self.roots = r,
            Err(e) => self.error = Some(e.to_string()),
        }
        self.walked = walked_roots(&self.roots, core.scan_mode);
        match Admission::new(&core, self.walked.clone(), self.fs.clone()) {
            Ok(a) => self.admission = Arc::new(a),
            Err(e) => self.error = Some(e.to_string()),
        }
    }

    fn update_watchers(&mut self) {
        if self.dynamic_watchers {
            if self.registered {
                self.send_request(method::UNREGISTER_CAPABILITY, watch::unregister_params());
                self.registered = false;
            }
            if !self.walked.is_empty() {
                self.send_request(
                    method::REGISTER_CAPABILITY,
                    watch::register_params(&self.walked),
                );
                self.registered = true;
            }
        } else {
            let tx = self.work_tx.clone();
            self.notify = watch::NotifyWatcher::new(&self.walked, move |e| {
                let _ = tx.send(Work::Files(e));
            })
            .ok();
        }
    }

    fn full_rescan(&mut self) {
        self.cancel.store(true, Ordering::Relaxed);
        self.cancel = Arc::new(AtomicBool::new(false));
        self.scan_generation += 1;
        self.scanning = true;
        self.interrupted = false;
        self.needs_scan = false;
        let (generation, roots, core) = (
            self.scan_generation,
            self.walked.clone(),
            self.settings.core(),
        );
        let (pattern, fs, cancel, tx) = (
            self.pattern.clone(),
            self.fs.clone(),
            self.cancel.clone(),
            self.work_tx.clone(),
        );
        std::thread::spawn(move || {
            let outcome = catch_unwind(AssertUnwindSafe(|| {
                walk_and_scan(&core, &roots, &pattern, fs, &cancel)
            }))
            .map_err(|_| "scan panicked".to_string())
            .and_then(|r| r.map_err(|e| e.to_string()));
            let _ = tx.send(Work::Walk {
                generation,
                roots,
                outcome,
            });
        });
        self.send_status();
    }

    fn reset_timers(&mut self, now: Instant) {
        let g = self.settings.general.automatic_git_refresh_interval;
        let m = self.settings.general.periodic_refresh_interval;
        self.git_due = (g > 0).then(|| now + Duration::from_secs(g));
        self.periodic_due = (m > 0).then(|| now + Duration::from_secs(m * 60));
    }

    fn schedule_view(&mut self, now: Instant, whole_tree: bool) {
        self.whole_tree |= whole_tree;
        let due = now + VIEW_DELAY;
        self.view_due = Some(self.view_due.map_or(due, |d| d.min(due)));
    }

    fn buffer_admitted(&self, uri: &str, path: Option<&Path>) -> bool {
        let sch = scheme(uri).unwrap_or_default();
        self.settings.general.schemes.contains(&sch)
            && self.admission.admits_buffer(path, &self.roots.tree_roots)
    }

    fn scan_document(&self, doc: &Document) -> Vec<crate::model::Todo> {
        if !doc.admitted {
            return Vec::new();
        }
        let path = doc
            .path
            .as_ref()
            .map(|p| p.to_string_lossy().into_owned())
            .unwrap_or_else(|| doc.uri.clone());
        scan_text(&self.pattern, doc.text.as_bytes(), &path)
    }

    /// Rescans an open buffer and, when auto-refresh is on, feeds the index.
    fn rescan_buffer(&mut self, uri: &str, now: Instant) {
        let Some(doc) = self.docs.get(uri).cloned() else {
            return;
        };
        let todos = self.scan_document(&doc);
        if let Some(d) = self.docs.get_mut(uri) {
            d.todos = todos.clone();
        }
        if doc.admitted && self.settings.tree.auto_refresh {
            self.index.set_buffer(BufferEntry {
                uri: doc.uri.clone(),
                path: doc.path.clone(),
                version: doc.version,
                todos,
            });
            self.schedule_view(now, false);
        }
    }

    fn rescan_disk_file(&mut self, path: &Path) {
        match scan_file(self.fs.as_ref(), &self.pattern, path) {
            Ok(Some(todos)) => self.index.set_disk(path.to_path_buf(), todos),
            _ => self.index.remove_disk(path),
        }
    }

    fn rewalk_dir(&mut self, dir: &Path) {
        if !self.walked.iter().any(|r| dir.starts_with(r)) {
            return;
        }
        let mut files = Vec::new();
        let mut seen = Vec::new();
        for entry in ignore::WalkBuilder::new(dir)
            .standard_filters(false)
            .build()
            .flatten()
        {
            let path = entry.path();
            if !entry.file_type().is_some_and(|t| t.is_file()) || !self.admission.admits_disk(path)
            {
                continue;
            }
            if let Ok(Some(todos)) = scan_file(self.fs.as_ref(), &self.pattern, path) {
                seen.push(path.to_path_buf());
                if !todos.is_empty() {
                    files.push(crate::model::FileResult {
                        path: path.to_path_buf(),
                        todos,
                    });
                }
            }
        }
        self.index
            .apply_walk(&[dir.to_path_buf()], files, &seen, true);
    }

    fn process_events(&mut self, now: Instant) {
        let events = std::mem::take(&mut self.events);
        let mut full = false;
        for e in events {
            let Some(path) = uri_to_path(&e.uri) else {
                continue;
            };
            let name = path
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default();
            if matches!(name.as_str(), ".gitignore" | ".ignore" | ".rgignore") {
                self.admission.clear_ignore_cache();
                full = true;
                continue;
            }
            if e.kind == p::FILE_DELETED {
                self.index.remove_disk_prefix(&path);
            } else if self.fs.is_dir(&path) {
                self.rewalk_dir(&path);
            } else if self.admission.admits_disk(&path) {
                self.rescan_disk_file(&path);
            } else {
                self.index.remove_disk(&path);
            }
        }
        if full {
            self.full_rescan();
        }
        self.schedule_view(now, false);
    }

    fn rebuild_view(&mut self) {
        let active = self.active_uri.clone();
        let ctx = EffectiveContext {
            mode: self.settings.tree.scan_mode,
            walked_roots: &self.walked,
            active_uri: active.as_deref(),
        };
        let files = self.index.effective(&ctx);
        let built = catch_unwind(AssertUnwindSafe(|| {
            View::build(&self.settings, &files, &self.roots.tree_roots)
        }));
        let Ok(new) = built else {
            tracing::error!("view build panicked; keeping the previous view");
            return;
        };
        let refresh = delta(&self.view, &new, std::mem::take(&mut self.whole_tree));
        self.view = new;
        if !refresh.is_empty() {
            self.send_notification(method::TREE_CHANGED, p::TreeChangedParams { refresh });
        }
        self.send_status();
    }

    fn send_status(&mut self) {
        let active_path = self.active_uri.as_deref().and_then(uri_to_path);
        let summary = summarize(&self.view, &self.settings, active_path.as_deref());
        let mut warnings = colour_warnings(&self.settings);
        let bad: Vec<String> = [
            &self.settings.tree.label_format,
            &self.settings.tree.tooltip_format,
        ]
        .iter()
        .filter_map(|t| unexpected_placeholder(t))
        .collect();
        if !bad.is_empty() {
            warnings.push(format!("Unexpected placeholders ({})", bad.join(",")));
        }
        let status = StatusParams {
            instance: self.instance.clone(),
            scanning: self.scanning,
            interrupted: self.interrupted,
            needs_scan: self.needs_scan,
            error: self.error.clone(),
            warnings,
            status_bar: summary.status_bar,
            badge: summary.badge,
            view_title: summary.view_title,
            has_sub_tags: self.view.has_sub_tags,
            is_empty: self.view.is_empty,
        };
        if self.last_status.as_ref() != Some(&status) {
            self.send_notification(method::STATUS, &status);
            self.last_status = Some(status);
        }
    }

    fn send_decorations(&mut self, uri: &str) {
        let Some(doc) = self.docs.get(uri).cloned() else {
            return;
        };
        let todos = self.scan_document(&doc);
        if let Some(d) = self.docs.get_mut(uri) {
            d.todos = todos.clone();
        }
        let ranges = if doc.admitted {
            decorate(doc.text.as_bytes(), &todos, &self.settings, &self.pattern)
        } else {
            BTreeMap::new()
        };
        let resolver = Resolver::new(&self.settings);
        let new_styles: BTreeMap<String, _> = ranges
            .keys()
            .filter(|k| !self.sent_styles.contains(*k))
            .map(|k| (k.clone(), resolver.style(k)))
            .collect();
        if !new_styles.is_empty() {
            self.sent_styles.extend(new_styles.keys().cloned());
            self.send_notification(
                method::STYLES,
                p::StylesParams {
                    generation: self.style_generation,
                    reset: false,
                    styles: new_styles,
                },
            );
        }
        self.send_notification(
            method::DECORATIONS,
            p::DecorationsParams {
                uri: doc.uri,
                version: doc.version,
                generation: self.style_generation,
                ranges,
            },
        );
    }

    fn configure(&mut self, new: Settings, now: Instant) {
        let changes = new.changes_from(&self.settings);
        let old_walked = self.walked.clone();
        self.settings = new;
        if changes.rescan {
            self.rebuild_scan_state();
            if self.walked != old_walked {
                self.update_watchers();
            }
            for uri in self.docs.uris() {
                let admitted = self
                    .docs
                    .get(&uri)
                    .map(|d| self.buffer_admitted(&d.uri, d.path.as_deref()))
                    .unwrap_or(false);
                if let Some(d) = self.docs.get_mut(&uri) {
                    d.admitted = admitted;
                }
                self.rescan_buffer(&uri, now);
            }
            self.full_rescan();
        }
        if changes.styles {
            self.style_generation += 1;
            self.sent_styles.clear();
            self.send_notification(
                method::STYLES,
                p::StylesParams {
                    generation: self.style_generation,
                    reset: true,
                    styles: BTreeMap::new(),
                },
            );
            for uri in self.docs.uris() {
                self.decorations_due.insert(uri, now);
            }
        }
        if changes.view {
            self.schedule_view(now, changes.view_mode);
        }
        if changes.timers {
            self.reset_timers(now);
        }
        if changes.status {
            self.send_status();
        }
    }

    fn params<P: DeserializeOwned>(value: serde_json::Value) -> Option<P> {
        serde_json::from_value(value)
            .map_err(|e| tracing::warn!("bad params: {e}"))
            .ok()
    }

    pub fn handle_notification(&mut self, n: Notification, now: Instant) {
        match n.method.as_str() {
            method::DID_OPEN => {
                let Some(params) = Self::params::<p::DidOpenParams>(n.params) else {
                    return;
                };
                let item = params.text_document;
                let path = uri_to_path(&item.uri);
                let admitted = self.buffer_admitted(&item.uri, path.as_deref());
                let uri = item.uri.clone();
                self.docs.insert(Document {
                    uri: item.uri,
                    path,
                    version: item.version,
                    text: item.text,
                    todos: Vec::new(),
                    admitted,
                });
                self.rescan_buffer(&uri, now);
                self.decorations_due.insert(uri, now);
            }
            method::DID_CHANGE => {
                let Some(params) = Self::params::<p::DidChangeParams>(n.params) else {
                    return;
                };
                let uri = params.text_document.uri;
                let Some(doc) = self.docs.get_mut(&uri) else {
                    return;
                };
                apply_changes(&mut doc.text, &params.content_changes);
                doc.version = params.text_document.version;
                self.buffer_due.insert(uri.clone(), now + BUFFER_DELAY);
                self.decorations_due.insert(
                    uri,
                    now + Duration::from_millis(self.settings.highlights.highlight_delay),
                );
            }
            method::DID_CLOSE => {
                let Some(params) = Self::params::<p::DidCloseParams>(n.params) else {
                    return;
                };
                let uri = params.text_document.uri;
                self.docs.remove(&uri);
                self.buffer_due.remove(&uri);
                self.decorations_due.remove(&uri);
                if let Some(path) = self.index.remove_buffer(&uri) {
                    if self.admission.admits_disk(&path) {
                        self.rescan_disk_file(&path);
                    }
                }
                self.schedule_view(now, false);
            }
            method::DID_CHANGE_WATCHED_FILES => {
                let Some(params) = Self::params::<p::DidChangeWatchedFilesParams>(n.params) else {
                    return;
                };
                if self.settings.tree.auto_refresh {
                    self.events.extend(params.changes);
                    self.events_due.get_or_insert(now + EVENT_DELAY);
                }
            }
            method::DID_CHANGE_WORKSPACE_FOLDERS => {
                let Some(params) = Self::params::<p::DidChangeWorkspaceFoldersParams>(n.params)
                else {
                    return;
                };
                let removed = folder_paths(&params.event.removed);
                self.folders.retain(|f| !removed.contains(f));
                self.folders.extend(folder_paths(&params.event.added));
                self.rebuild_scan_state();
                self.update_watchers();
                self.full_rescan();
                self.schedule_view(now, true);
            }
            method::CONFIGURE => {
                if let Some(s) = Self::params::<Settings>(n.params) {
                    self.configure(s, now);
                }
            }
            method::ACTIVE_EDITOR => {
                let Some(params) = Self::params::<p::ActiveEditorParams>(n.params) else {
                    return;
                };
                self.active_uri = params.uri;
                if self.settings.tree.scan_mode == ScanMode::CurrentFile {
                    self.schedule_view(now, false);
                }
                if self.settings.general.status_bar == StatusBarMode::CurrentFile {
                    self.send_status();
                }
            }
            method::RESCAN => self.full_rescan(),
            method::STOP_SCAN => self.cancel.store(true, Ordering::Relaxed),
            _ => {}
        }
    }

    pub fn handle_request(&mut self, r: Request) -> Response {
        let id = r.id.clone();
        let ok = |v: serde_json::Value| Response::new_ok(id.clone(), v);
        let bad = |m: &str| {
            Response::new_err(
                id.clone(),
                lsp_server::ErrorCode::InvalidParams as i32,
                m.to_string(),
            )
        };
        match r.method.as_str() {
            method::CHILDREN => match Self::params::<p::ChildrenParams>(r.params) {
                Some(q) => ok(serde_json::to_value(p::ChildrenResult {
                    nodes: self.view.children_of(q.parent.as_deref()),
                })
                .unwrap()),
                None => bad("invalid params"),
            },
            method::FIND => match Self::params::<p::FindParams>(r.params) {
                Some(q) => ok(serde_json::to_value(p::FindResult {
                    paths: self.view.find(&q.uri, q.line),
                })
                .unwrap()),
                None => bad("invalid params"),
            },
            method::NAVIGATE => match Self::params::<p::NavigateParams>(r.params) {
                Some(q) => {
                    let ranges = self.docs.get(&q.uri).and_then(|d| {
                        navigate(d.text.as_bytes(), &self.pattern, &q.positions, q.direction)
                    });
                    ok(serde_json::to_value(p::NavigateResult { ranges }).unwrap())
                }
                None => bad("invalid params"),
            },
            method::EXPORT => {
                let env = self.env.clone();
                let home = env("HOME").or_else(|| env("USERPROFILE"));
                let path = export_path(
                    &self.settings.general.export_path,
                    &|n| env(n),
                    home.as_deref(),
                    &chrono::Local::now(),
                );
                let content = export_content(&self.view, self.settings.view().tags_only, &path);
                ok(serde_json::to_value(p::ExportResult { path, content }).unwrap())
            }
            _ => Response::new_err(
                id.clone(),
                lsp_server::ErrorCode::MethodNotFound as i32,
                format!("unknown method {}", r.method),
            ),
        }
    }

    pub fn handle_work(&mut self, w: Work, now: Instant) {
        match w {
            Work::Walk {
                generation,
                roots,
                outcome,
            } => {
                if generation != self.scan_generation {
                    return;
                }
                self.scanning = false;
                match outcome {
                    Ok(o) => {
                        self.interrupted = o.cancelled;
                        self.index
                            .apply_walk(&roots, o.files, &o.seen, !o.cancelled);
                    }
                    Err(e) => self.error = Some(e),
                }
                self.schedule_view(now, false);
                self.send_status();
            }
            Work::Git { folder, head } => {
                self.git_in_flight.remove(&folder);
                if let Some(h) = head {
                    let changed = self.git_heads.get(&folder).is_some_and(|old| *old != h);
                    self.git_heads.insert(folder, h);
                    if changed {
                        self.full_rescan();
                    }
                }
            }
            Work::Files(Some(events)) => {
                if self.settings.tree.auto_refresh {
                    self.events.extend(events);
                    self.events_due.get_or_insert(now + EVENT_DELAY);
                }
            }
            Work::Files(None) => {
                tracing::warn!("file watcher failed; rescanning");
                self.full_rescan();
            }
        }
    }

    /// The earliest pending timer.
    pub fn next_deadline(&self) -> Option<Instant> {
        [
            self.view_due,
            self.events_due,
            self.git_due,
            self.periodic_due,
        ]
        .into_iter()
        .flatten()
        .chain(self.buffer_due.values().copied())
        .chain(self.decorations_due.values().copied())
        .min()
    }

    /// Runs every timer due at `now`.
    pub fn tick(&mut self, now: Instant) {
        if self.events_due.is_some_and(|d| d <= now) {
            self.events_due = None;
            self.process_events(now);
        }
        let due: Vec<String> = self
            .buffer_due
            .iter()
            .filter(|(_, d)| **d <= now)
            .map(|(u, _)| u.clone())
            .collect();
        for uri in due {
            self.buffer_due.remove(&uri);
            self.rescan_buffer(&uri, now);
        }
        let due: Vec<String> = self
            .decorations_due
            .iter()
            .filter(|(_, d)| **d <= now)
            .map(|(u, _)| u.clone())
            .collect();
        for uri in due {
            self.decorations_due.remove(&uri);
            self.send_decorations(&uri);
        }
        if self.periodic_due.is_some_and(|d| d <= now) {
            self.full_rescan();
            self.periodic_due = Some(
                now + Duration::from_secs(
                    self.settings.general.periodic_refresh_interval.max(1) * 60,
                ),
            );
        }
        if self.git_due.is_some_and(|d| d <= now) {
            for folder in self.folders.clone() {
                if self.git_in_flight.insert(folder.clone()) {
                    let tx = self.work_tx.clone();
                    std::thread::spawn(move || {
                        let head = git::head(&folder);
                        let _ = tx.send(Work::Git { folder, head });
                    });
                }
            }
            self.git_due = Some(
                now + Duration::from_secs(
                    self.settings.general.automatic_git_refresh_interval.max(1),
                ),
            );
        }
        if self.view_due.is_some_and(|d| d <= now) {
            self.view_due = None;
            self.rebuild_view();
        }
    }

    /// After a panic in a handler: the index may be half-updated, so drop it
    /// and rebuild it with a full rescan.
    pub fn recover(&mut self, now: Instant) {
        self.index = Index::new();
        for uri in self.docs.uris() {
            self.rescan_buffer(&uri, now);
        }
        self.full_rescan();
        self.schedule_view(now, true);
    }
}
```

- [ ] **Step 4: Write the loop**

`crates/clippings-core/src/server/main_loop.rs`:

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

- [ ] **Step 5: Run the tests to verify they pass, repeatedly**

Run: `for i in 1 2 3 4 5; do cargo test -p clippings-core --test server 2>&1 | grep 'test result'; done`
Expected: `7 passed` five times. These tests wait on real timers and threads; any flake is a bug to fix, not a retry.

Then run: `INSTA_UPDATE=no cargo test --all`. Expected: all pass.
Then run: `cargo fmt --all && cargo clippy --all-targets -- -D warnings`
Expected: no diffs, no warnings.

- [ ] **Step 6: Commit**

```bash
git add -A
git commit -F - <<'EOF'
feat(core): scheduler and language server loop

Co-Authored-By: <your model attribution>
EOF
```

### Task 15: `clippings lsp` and `clippings watch`

The two new subcommands (spec section 4): `lsp` runs the server over stdio, and `watch` scans and then prints one JSON line per changed file using the same `notify` watcher. The tests spawn the real binary and speak JSON-RPC over pipes.

**Files:**
- Modify: `crates/clippings/src/main.rs`
- Create: `crates/clippings/src/watch.rs`, `crates/clippings/tests/lsp.rs`

**Interfaces:**
- Produces: the command lines `clippings lsp` and `clippings watch <ROOTS>... [--config FILE] [--hidden] [--no-ignore]`. `watch` prints `{"event":"ready","files":N,"todos":M}` first, then `{"event":"updated","path":REL,"todos":[...]}`, `{"event":"removed","path":REL}` or `{"event":"overflow"}` lines. `scan` keeps its behaviour and now shares `load_config` and `canonical` with `watch`.

- [ ] **Step 1: Write the failing tests**

`crates/clippings/tests/lsp.rs`:

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
    loop {
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
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p clippings --test lsp`
Expected: every test fails, because `lsp` and `watch` are not subcommands yet and clap exits with an error.

- [ ] **Step 3: Write `watch`**

`crates/clippings/src/watch.rs`:

```rust
//! `clippings watch`: scan, then print one JSON line per file whose todos
//! change, using the same `notify` watcher the server falls back to.

use anyhow::Result;
use clippings_core::admission::Admission;
use clippings_core::config::CoreConfig;
use clippings_core::fs::{Fs, NativeFs};
use clippings_core::globs::slash_path;
use clippings_core::pattern;
use clippings_core::protocol::FILE_DELETED;
use clippings_core::roots::deepest_root;
use clippings_core::scanner::scan_file;
use clippings_core::server::watch::NotifyWatcher;
use clippings_core::uri::uri_to_path;
use clippings_core::walker::walk_and_scan;
use serde_json::json;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;
use std::sync::{mpsc, Arc};

fn rel(path: &Path, roots: &[PathBuf]) -> String {
    deepest_root(path, roots)
        .and_then(|r| path.strip_prefix(r).ok())
        .map(slash_path)
        .unwrap_or_else(|| slash_path(path))
}

pub fn run(roots: Vec<PathBuf>, cfg: CoreConfig) -> Result<()> {
    let fs: Arc<dyn Fs> = Arc::new(NativeFs);
    let pattern = pattern::build(&cfg)?;
    let outcome = walk_and_scan(&cfg, &roots, &pattern, fs.clone(), &AtomicBool::new(false))?;
    let admission = Admission::new(&cfg, roots.clone(), fs.clone())?;
    let total: usize = outcome.files.iter().map(|f| f.todos.len()).sum();
    let mut out = std::io::stdout();
    writeln!(
        out,
        "{}",
        json!({ "event": "ready", "files": outcome.files.len(), "todos": total })
    )?;
    out.flush()?;

    let (tx, rx) = mpsc::channel();
    let _watcher = NotifyWatcher::new(&roots, move |e| {
        let _ = tx.send(e);
    })?;
    for events in rx {
        let Some(events) = events else {
            writeln!(out, "{}", json!({ "event": "overflow" }))?;
            continue;
        };
        for e in events {
            let Some(path) = uri_to_path(&e.uri) else {
                continue;
            };
            if e.kind != FILE_DELETED && fs.is_dir(&path) {
                continue;
            }
            let todos = if e.kind == FILE_DELETED || !admission.admits_disk(&path) {
                None
            } else {
                scan_file(fs.as_ref(), &pattern, &path).ok().flatten()
            };
            let line = match todos {
                Some(t) => json!({ "event": "updated", "path": rel(&path, &roots), "todos": t }),
                None => json!({ "event": "removed", "path": rel(&path, &roots) }),
            };
            writeln!(out, "{line}")?;
        }
        out.flush()?;
    }
    Ok(())
}
```

- [ ] **Step 4: Replace the command line**

Replace `crates/clippings/src/main.rs`:

```rust
use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use clippings_core::config::CoreConfig;
use clippings_core::fs::NativeFs;
use clippings_core::report::scan_report;
use std::path::PathBuf;
use std::sync::Arc;

mod watch;

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

fn main() -> Result<()> {
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
            if json {
                println!("{}", serde_json::to_string_pretty(&report)?);
            } else {
                for f in &report.files {
                    for t in &f.todos {
                        println!(
                            "{}:{}:{}: {} {}",
                            f.path,
                            t.start.line + 1,
                            t.start.character + 1,
                            t.tag,
                            t.after
                        );
                    }
                }
            }
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

- [ ] **Step 5: Run the tests to verify they pass**

Run: `INSTA_UPDATE=no cargo test --all`
Expected: all pass, including the 3 new binary tests.
Then run: `cargo fmt --all && cargo clippy --all-targets -- -D warnings`
Expected: no diffs, no warnings.

- [ ] **Step 6: Commit**

```bash
git add -A
git commit -F - <<'EOF'
feat: clippings lsp and clippings watch commands

Co-Authored-By: <your model attribution>
EOF
```

### Task 16: Performance checks and a real-repository smoke test

Spec section 13 for the server side. Two `#[ignore]` tests measure the budgets. They are too hardware-dependent for CI, so they run on demand in release mode. A smoke script runs the release server against tilliX, the benchmark repository.

**Files:**
- Create: `crates/clippings-core/tests/perf.rs`, `docs/benchmarks/2026-09-server.md`

- [ ] **Step 1: Write the performance tests**

`crates/clippings-core/tests/perf.rs`:

```rust
//! Performance checks for spec section 13. Run with
//! `cargo test --release -p clippings-core --test perf -- --ignored --nocapture`.

use clippings_core::decorations::decorate;
use clippings_core::index::{EffectiveFile, Source, SourcedTodo};
use clippings_core::pattern;
use clippings_core::scanner::scan_text;
use clippings_core::settings::Settings;
use clippings_core::view::delta::delta;
use clippings_core::view::render::View;
use std::path::PathBuf;
use std::time::Instant;

fn median(mut v: Vec<f64>) -> f64 {
    v.sort_by(|a, b| a.partial_cmp(b).unwrap());
    v[v.len() / 2]
}

#[test]
#[ignore]
fn view_rebuild_and_diff_with_10k_todos_under_15ms() {
    let s = Settings::default();
    let p = pattern::build(&s.core()).unwrap();
    // 500 files x 20 todos in 50 folders.
    let text: String = (0..20)
        .map(|i| format!("code();\n// TODO item {i}\n"))
        .collect();
    let todos = scan_text(&p, text.as_bytes(), "a.ts");
    let paths: Vec<PathBuf> = (0..500)
        .map(|i| PathBuf::from(format!("/w/d{}/f{i}.ts", i % 50)))
        .collect();
    let files: Vec<EffectiveFile> = paths
        .iter()
        .map(|path| EffectiveFile {
            path: Some(path.as_path()),
            uri: None,
            source: Source::Disk,
            todos: todos
                .iter()
                .map(|todo| SourcedTodo {
                    buffer_uri: None,
                    todo,
                })
                .collect(),
        })
        .collect();
    let roots = vec![PathBuf::from("/w")];
    let mut old = View::build(&s, &files, &roots);
    let mut times = Vec::new();
    for _ in 0..15 {
        let start = Instant::now();
        let new = View::build(&s, &files, &roots);
        let _ = delta(&old, &new, false);
        times.push(start.elapsed().as_secs_f64() * 1000.0);
        old = new;
    }
    let m = median(times);
    println!("view rebuild + diff, 10,000 todos: {m:.2} ms");
    assert!(m < 15.0, "{m:.2} ms");
}

#[test]
#[ignore]
fn decorations_for_a_50k_line_buffer_under_20ms() {
    let s = Settings::default();
    let p = pattern::build(&s.core()).unwrap();
    let text: String = (0..50_000)
        .map(|i| {
            if i % 100 == 0 {
                format!("// TODO {i}\n")
            } else {
                format!("let x{i} = {i};\n")
            }
        })
        .collect();
    let mut times = Vec::new();
    for _ in 0..15 {
        let start = Instant::now();
        let todos = scan_text(&p, text.as_bytes(), "a.ts");
        let d = decorate(text.as_bytes(), &todos, &s, &p);
        assert_eq!(d["TODO"].len(), 500);
        times.push(start.elapsed().as_secs_f64() * 1000.0);
    }
    let m = median(times);
    println!("scan + decorations, 50,000 lines: {m:.2} ms");
    assert!(m < 20.0, "{m:.2} ms");
}
```

- [ ] **Step 2: Run them in release mode**

Run: `cargo test --release -p clippings-core --test perf -- --ignored --nocapture`
Expected: both pass. The prototype measured 0.6 to 0.8 ms for scan plus decorations of 50,000 lines, and 12.5 ms for a rebuild plus diff with 10,000 todos, on an Apple M4 Pro.

- [ ] **Step 3: Smoke-test the server against tilliX**

On the maintainer's machine, where `~/tilli/tilliX` exists, run:

```bash
cargo build --release
python3 - <<'EOF'
import subprocess, json, time, os
B = "target/release/clippings"; root = os.path.expanduser("~/tilli/tilliX")
p = subprocess.Popen([B, "lsp"], stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.DEVNULL)
def send(m):
    b = json.dumps(m).encode(); p.stdin.write(b"Content-Length: %d\r\n\r\n" % len(b) + b); p.stdin.flush()
def recv():
    h = b""
    while not h.endswith(b"\r\n\r\n"): h += p.stdout.read(1)
    n = int([l for l in h.decode().split("\r\n") if l.lower().startswith("content-length")][0].split(":")[1])
    return json.loads(p.stdout.read(n))
t0 = time.perf_counter()
send({"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {"workspaceFolders": [{"uri": "file://" + root, "name": "tilliX"}], "capabilities": {"workspace": {"didChangeWatchedFiles": {"dynamicRegistration": True}}}, "initializationOptions": {"protocolVersion": 1, "settings": {"general": {"tags": ["BUG", "HACK", "FIXME", "TODO", "XXX", "[ ]", "QUESTION"], "statusBar": "total"}}}}})
recv(); send({"jsonrpc": "2.0", "method": "initialized", "params": {}})
while True:
    m = recv()
    if m.get("method") == "clippings/status" and m["params"]["scanning"] is False and m["params"]["statusBar"]["text"] != "$(check) 0":
        print("first full tree after %.0f ms: %s" % ((time.perf_counter() - t0) * 1000, m["params"]["statusBar"]["text"])); break
send({"jsonrpc": "2.0", "id": 3, "method": "shutdown"})
while recv().get("id") != 3: pass
send({"jsonrpc": "2.0", "method": "exit"}); p.wait(); print("exit code", p.returncode)
EOF
```

Expected: the todo count equals `clippings scan` on the same settings (489 on 2026-09-23), the first full tree arrives in well under a second, and the exit code is 0. The prototype measured 278 ms, including process start and the 50 ms rebuild window.

- [ ] **Step 4: Record the results**

Write `docs/benchmarks/2026-09-server.md` with a short table: machine, date, the two performance-test medians against their targets, and the smoke test's todo count, time to first tree and exit code. If a target was missed, say so plainly.

- [ ] **Step 5: Commit**

```bash
git add -A
git commit -F - <<'EOF'
test(core): server performance checks and tilliX smoke test

Co-Authored-By: <your model attribution>
EOF
```

### Task 17: Record this plan's rulings in the spec

The spec is the binding authority, so the eight rulings at the top of this plan, plus plan 1's deferred ruling on fancy-regex runtime errors, go into it.

**Files:**
- Modify: `docs/superpowers/specs/2026-09-23-clippings-design.md`

- [ ] **Step 1: Edit the spec**

Make these edits, each in plain prose matching the spec's existing style:

1. **Section 5.4:** change "Runtime errors such as a backtrack limit are logged per file and skip that file" to say they are logged and skip that match, and add "so one pathological line does not hide a file's other todos".
2. **Section 5.10, Panics paragraph:** say that full walks and git polls run on worker threads, while buffer scans, view builds and decoration computation run on the scheduler thread under `catch_unwind`. A panic there is logged, and the index is dropped and rebuilt by a full rescan.
3. **Section 5.12, Node IDs:** add that a todo whose parent is not a file node (the tags-only view) uses the own key `t:<uri>:<line>:<column>`. Also add that with grouping by both tag and sub-tag, tag grouping wins and no sub-tag level or pseudo-folder is created.
4. **Section 5.12, Rendering, `command` bullet:** say "end of todo" reveals at the end of the line on which the match starts, carried as `textEnd` on each todo.
5. **Section 5.13, Styles:** say the auto-contrast foreground applies to hex and `rgb()` backgrounds only; named colours get none, as in todo-tree.
6. **Section 5.14:** say icon-name validation happens in the extension, which owns the octicon set. List the status bar tooltips and the badge tooltip (`N todos`). Say the spacing matches todo-tree, including its double spaces.
7. **Section 6.3:** add the configuration object's exact shape: `general`, `highlights`, `filtering`, `tree` and `regex` as nested objects mirroring the settings, plus `viewState { flat, tagsOnly, expanded, groupedByTag, groupedBySubTag, filter, includeGlobs, excludeGlobs }`, `filesExclude`, `searchExclude` and `explorerCompactFolders`. Unknown fields are ignored.
8. **Section 13 table:** change the view rebuild target to "under 15 ms", with a sentence noting the prototype measured 12.5 ms and that sharing todo data between the index and view is plan 4's optimisation.

- [ ] **Step 2: Check it reads cleanly**

Re-read each edited section in full. Check that no sentence contradicts the edit, for example an older "5 ms" elsewhere, and that every edit states a behaviour, not a history.

- [ ] **Step 3: Commit**

```bash
git add -A
git commit -F - <<'EOF'
docs(spec): record the server plan's rulings

Co-Authored-By: <your model attribution>
EOF
```
