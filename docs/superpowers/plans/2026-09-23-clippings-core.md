# Clippings Core Implementation Plan (Plan 1 of 4)

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build the `clippings-core` Rust library and the `clippings scan` and `clippings probe` commands: roots, exclusion layers, ignore rules, the admission predicate, regex construction, scanning, tag extraction and the index, producing a JSON report that matches ripgrep's matches on the benchmark repository.

**Architecture:** A Cargo workspace with a library crate, `clippings-core`, and a binary crate, `clippings`. The walker uses the `ignore` crate's parallel walker with the exclusion layers in `filter_entry`. Each file is decoded and searched with `grep-searcher` and a `grep-regex` matcher, falling back to `fancy-regex` for look-around and backreferences. The matcher is re-run inside each searcher match to get per-match offsets. A single-path admission predicate reproduces the walker's decisions for later file events.

**Tech Stack:** Rust 2021, minimum Rust 1.85. Crates: ignore 0.4.33, globset 0.4.20, grep-searcher 0.1.17, grep-regex 0.1.14, grep-matcher 0.1.9, regex 1.13, regex-syntax 0.8.11, fancy-regex 0.19, encoding_rs 0.8.41, memchr 2.8, serde, serde_json, thiserror 2, tracing, clap 4.6. Tests: insta 1.48 with the json feature, tempfile 3.

**Spec:** `docs/superpowers/specs/2026-09-23-clippings-design.md`. Sections 5.1 to 5.9 and 12.1 to 12.2 are implemented here. Section references below are to that spec.

## Plan sequence

The spec's milestones map onto four plans. Each is written when the previous one has landed, so it can build on the real interfaces.

1. **Core (this plan)**: spec milestone 1.
2. **Server**: view model, node IDs and deltas, decorations and styles, status, navigation, export, protocol types, scheduler, file watching, `clippings lsp` and `clippings watch`, protocol integration tests. Spec milestone 2.
3. **Extension**: manifest, settings and import, server resolution and lifecycle, tree provider, decorations, status bar, commands, menus and context keys, extension tests. Spec milestone 3.
4. **Packaging and parity polish**: release CI for ten targets, `clippings bench`, the parity oracle, notebooks, icons, scopes and export polish. Spec milestones 4 and 5.

## Global Constraints

- Minimum Rust 1.85, edition 2021. `cargo fmt --all` must leave no diff, and `cargo clippy --all-targets -- -D warnings` must pass before every commit.
- Every regex, including the fancy-regex fallback, comes from `pattern::build`. No other module compiles a search regex from user settings.
- The scanner never memory-maps files: `MmapChoice::never()`.
- Positions on the wire are 0-based line and 0-based UTF-16 column (`position::Position`).
- Label text is capped 1,000 bytes after the match start (`scanner::WINDOW_CAP`). Files above 64 MiB (`scanner::HEAP_LIMIT`) are streamed in line mode and skipped in multi-line mode.
- Built-in excludes match only path components below a scan root, never the root or its ancestors.
- Settings defaults equal todo-tree v0.0.224: tags `BUG, HACK, FIXME, TODO, XXX, [ ], [x]`, regex `(//|#|<!--|;|/\*|^|^[ \t]*(-|\d+.))\s*($TAGS)`, case-sensitive, exclude glob `**/node_modules/*/**`.
- `PROTOCOL_VERSION` is `1`.
- CI runs on Linux, macOS and Windows. A test that fails only on Windows is a bug to fix, not a test to skip, unless it exercises a Unix-only feature such as symlinks.

## Review Focus

These inputs are implied by the spec but easy to miss. Each has a test in the task that owns the code.

1. **Latin-1 or other invalid UTF-8 bytes** before a tag must not hide the todo or break its label. Test `latin1_bytes_do_not_hide_todos` in Task 9.
2. **A tag at the very end of a file with no trailing newline**, and an empty file, must scan correctly. Test `tag_at_end_of_file_without_newline` in Task 9.
3. **A user regex that can match the empty string**, such as `($TAGS)?`, must terminate and report only real tags. Test `empty_matching_regex_terminates` in Task 9.
4. **A file larger than 64 MiB** must still be scanned in line mode, with correct line numbers, and skipped in multi-line mode. Test `files_over_the_heap_limit_are_streamed_in_line_mode` in Task 9.
5. **A root given through a symlink**, such as macOS `/tmp`, must report paths relative to the root. Test `scan_through_a_symlinked_root_reports_relative_paths` in Task 12.

---

### Task 1: Workspace scaffold and configuration

**Files:**
- Create: `Cargo.toml`, `.gitignore`
- Create: `crates/clippings-core/Cargo.toml`, `crates/clippings-core/src/lib.rs`, `crates/clippings-core/src/error.rs`, `crates/clippings-core/src/fs.rs`, `crates/clippings-core/src/config.rs`
- Create: `crates/clippings/Cargo.toml`, `crates/clippings/src/main.rs`

**Interfaces:**
- Produces: `config::CoreConfig` (serde camelCase, `Default` = todo-tree defaults), `CoreConfig::effective_tags() -> Vec<String>`, `CoreConfig::vscode_excludes() -> Vec<String>`, `CoreConfig::group_of(&str) -> Option<&str>`, `config::ScanMode` (`Workspace`, `WorkspaceOnly`, `OpenFiles`, `CurrentFile`), `config::UseBuiltInExcludes`, constants `DEFAULT_REGEX`, `DEFAULT_TAGS`, `DEFAULT_BUILT_IN_EXCLUDES`.
- Produces: `error::CoreError` with variants `InvalidRegex(String)`, `InvalidGlob { glob, message }`, `Io { path, source }`, re-exported as `clippings_core::CoreError`.
- Produces: `fs::Fs` trait (`read`, `len`, `exists`, `is_dir`) and `fs::NativeFs`.
- Produces: `clippings_core::PROTOCOL_VERSION: u32 = 1`.

- [ ] **Step 1: Create the workspace files**

`Cargo.toml`:

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
clap = { version = "4.6", features = ["derive"] }
dunce = "1.0"
encoding_rs = "0.8.41"
fancy-regex = "0.19"
globset = "0.4.20"
grep-matcher = "0.1.9"
grep-regex = "0.1.14"
grep-searcher = "0.1.17"
ignore = "0.4.33"
insta = { version = "1.48", features = ["json"] }
memchr = "2.8"
regex = "1.13"
regex-syntax = "0.8.11"
serde = { version = "1", features = ["derive"] }
serde_json = "1"
tempfile = "3"
thiserror = "2"
tracing = "0.1"

[profile.release]
lto = "fat"
codegen-units = 1
panic = "unwind"
strip = "symbols"
```

`.gitignore`:

```gitignore
/target
node_modules/
*.vsix
/extension/bin/
/extension/dist/
*.pending-snap

```

`crates/clippings-core/Cargo.toml`:

```toml
[package]
name = "clippings-core"
version.workspace = true
edition.workspace = true
license.workspace = true
rust-version.workspace = true

[dependencies]
encoding_rs.workspace = true
fancy-regex.workspace = true
globset.workspace = true
grep-matcher.workspace = true
grep-regex.workspace = true
grep-searcher.workspace = true
ignore.workspace = true
memchr.workspace = true
regex.workspace = true
regex-syntax.workspace = true
serde.workspace = true
serde_json.workspace = true
thiserror.workspace = true
tracing.workspace = true

[dev-dependencies]
dunce.workspace = true
insta.workspace = true
tempfile.workspace = true
```

`crates/clippings/Cargo.toml`:

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

[dev-dependencies]
insta.workspace = true
tempfile.workspace = true
```

`crates/clippings/src/main.rs`, a placeholder binary until Task 12:

```rust
fn main() {}
```

`crates/clippings-core/src/error.rs`:

```rust
use thiserror::Error;

#[derive(Debug, Error)]
pub enum CoreError {
    #[error("invalid regex: {0}")]
    InvalidRegex(String),
    #[error("invalid glob {glob:?}: {message}")]
    InvalidGlob { glob: String, message: String },
    #[error("io error on {path}: {source}")]
    Io {
        path: String,
        #[source]
        source: std::io::Error,
    },
}
```

`crates/clippings-core/src/fs.rs`:

```rust
//! File content access. The scanner reads bytes only through [`Fs`], so a
//! later web build can serve reads from the client. The walker module uses
//! the `ignore` crate, which touches `std::fs` itself, and is native-only.

use std::io;
use std::path::Path;

pub trait Fs: Send + Sync {
    /// Reads a whole file.
    fn read(&self, path: &Path) -> io::Result<Vec<u8>>;
    /// File size in bytes, without reading it.
    fn len(&self, path: &Path) -> io::Result<u64>;
    /// Whether a path exists (file, directory or symlink).
    fn exists(&self, path: &Path) -> bool;
    /// Whether a path is a directory.
    fn is_dir(&self, path: &Path) -> bool;
}

#[derive(Clone, Copy, Debug, Default)]
pub struct NativeFs;

impl Fs for NativeFs {
    fn read(&self, path: &Path) -> io::Result<Vec<u8>> {
        std::fs::read(path)
    }
    fn len(&self, path: &Path) -> io::Result<u64> {
        Ok(std::fs::metadata(path)?.len())
    }
    fn exists(&self, path: &Path) -> bool {
        std::fs::symlink_metadata(path).is_ok()
    }
    fn is_dir(&self, path: &Path) -> bool {
        std::fs::metadata(path).map(|m| m.is_dir()).unwrap_or(false)
    }
}
```

`crates/clippings-core/src/lib.rs`:

```rust
//! Core of Clippings: configuration, walking, scanning, tag extraction and the index.

pub mod config;
pub mod error;
pub mod fs;

pub use error::CoreError;

/// Version of the client-server protocol, checked at `initialize` and by `clippings probe`.
pub const PROTOCOL_VERSION: u32 = 1;
```

- [ ] **Step 2: Write the failing tests**

Create `crates/clippings-core/src/config.rs` containing only this test module:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_match_todo_tree() {
        let c = CoreConfig::default();
        assert_eq!(
            c.tags,
            vec!["BUG", "HACK", "FIXME", "TODO", "XXX", "[ ]", "[x]"]
        );
        assert_eq!(c.regex, r"(//|#|<!--|;|/\*|^|^[ \t]*(-|\d+.))\s*($TAGS)");
        assert!(c.regex_case_sensitive);
        assert_eq!(c.exclude_globs, vec!["**/node_modules/*/**"]);
        assert_eq!(c.built_in_excludes.len(), 25);
        assert_eq!(c.scan_mode, ScanMode::Workspace);
    }

    #[test]
    fn deserializes_camel_case_with_defaults() {
        let c: CoreConfig = serde_json::from_str(
            r#"{"tags":["TODO"],"scanMode":"workspace only","useBuiltInExcludes":"file excludes","filesExclude":["**/.git"]}"#,
        )
        .unwrap();
        assert_eq!(c.tags, vec!["TODO"]);
        assert_eq!(c.scan_mode, ScanMode::WorkspaceOnly);
        assert_eq!(c.vscode_excludes(), vec!["**/.git"]);
        assert!(c.regex_case_sensitive);
    }

    #[test]
    fn empty_tags_mean_todo() {
        let c = CoreConfig {
            tags: vec![],
            ..Default::default()
        };
        assert_eq!(c.effective_tags(), vec!["TODO"]);
    }

    #[test]
    fn group_lookup() {
        let mut c = CoreConfig::default();
        c.tag_groups
            .insert("FIX".into(), vec!["FIXME".into(), "BUG".into()]);
        assert_eq!(c.group_of("BUG"), Some("FIX"));
        assert_eq!(c.group_of("TODO"), None);
    }
}
```

- [ ] **Step 3: Run the tests to verify they fail**

Run: `cargo test -p clippings-core config`
Expected: compile error, `CoreConfig` and `ScanMode` not found.

- [ ] **Step 4: Write the implementation**

Put this above the test module in `crates/clippings-core/src/config.rs`:

```rust
//! Resolved configuration that the core needs. Field names mirror the
//! `clippings.*` settings; serde uses camelCase so the extension can send
//! the object as-is.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

pub const DEFAULT_REGEX: &str = r"(//|#|<!--|;|/\*|^|^[ \t]*(-|\d+.))\s*($TAGS)";

pub const DEFAULT_TAGS: [&str; 7] = ["BUG", "HACK", "FIXME", "TODO", "XXX", "[ ]", "[x]"];

pub const DEFAULT_BUILT_IN_EXCLUDES: [&str; 25] = [
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
    ".vs",
];

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum ScanMode {
    #[default]
    #[serde(rename = "workspace")]
    Workspace,
    #[serde(rename = "workspace only")]
    WorkspaceOnly,
    #[serde(rename = "open files")]
    OpenFiles,
    #[serde(rename = "current file")]
    CurrentFile,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum UseBuiltInExcludes {
    #[default]
    #[serde(rename = "none")]
    None,
    #[serde(rename = "file excludes")]
    FileExcludes,
    #[serde(rename = "search excludes")]
    SearchExcludes,
    #[serde(rename = "file and search excludes")]
    FileAndSearchExcludes,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct CoreConfig {
    pub tags: Vec<String>,
    pub regex: String,
    pub regex_case_sensitive: bool,
    pub sub_tag_regex: String,
    pub enable_multi_line: bool,
    pub tag_groups: BTreeMap<String, Vec<String>>,
    pub include_globs: Vec<String>,
    pub exclude_globs: Vec<String>,
    pub temp_include_globs: Vec<String>,
    pub temp_exclude_globs: Vec<String>,
    /// Keys of `files.exclude` whose value is exactly `true`.
    pub files_exclude: Vec<String>,
    /// Keys of `search.exclude` whose value is exactly `true`.
    pub search_exclude: Vec<String>,
    pub use_built_in_excludes: UseBuiltInExcludes,
    pub built_in_excludes: Vec<String>,
    pub include_hidden_files: bool,
    pub ignore_git_submodules: bool,
    pub root_folder: String,
    pub included_workspaces: Vec<String>,
    pub excluded_workspaces: Vec<String>,
    pub scan_mode: ScanMode,
    /// Not a user setting: `clippings scan --no-ignore` turns it off.
    pub respect_ignore_files: bool,
}

impl Default for CoreConfig {
    fn default() -> Self {
        Self {
            tags: DEFAULT_TAGS.iter().map(|s| s.to_string()).collect(),
            regex: DEFAULT_REGEX.to_string(),
            regex_case_sensitive: true,
            sub_tag_regex: String::new(),
            enable_multi_line: false,
            tag_groups: BTreeMap::new(),
            include_globs: Vec::new(),
            exclude_globs: vec!["**/node_modules/*/**".to_string()],
            temp_include_globs: Vec::new(),
            temp_exclude_globs: Vec::new(),
            files_exclude: Vec::new(),
            search_exclude: Vec::new(),
            use_built_in_excludes: UseBuiltInExcludes::None,
            built_in_excludes: DEFAULT_BUILT_IN_EXCLUDES
                .iter()
                .map(|s| s.to_string())
                .collect(),
            include_hidden_files: false,
            ignore_git_submodules: false,
            root_folder: String::new(),
            included_workspaces: Vec::new(),
            excluded_workspaces: Vec::new(),
            scan_mode: ScanMode::Workspace,
            respect_ignore_files: true,
        }
    }
}

impl CoreConfig {
    /// Tags to search for: `tags`, or `["TODO"]` when empty.
    pub fn effective_tags(&self) -> Vec<String> {
        if self.tags.is_empty() {
            vec!["TODO".to_string()]
        } else {
            self.tags.clone()
        }
    }

    /// The VS Code exclude keys that `use_built_in_excludes` pulls in.
    pub fn vscode_excludes(&self) -> Vec<String> {
        let mut out = Vec::new();
        match self.use_built_in_excludes {
            UseBuiltInExcludes::None => {}
            UseBuiltInExcludes::FileExcludes => out.extend(self.files_exclude.iter().cloned()),
            UseBuiltInExcludes::SearchExcludes => out.extend(self.search_exclude.iter().cloned()),
            UseBuiltInExcludes::FileAndSearchExcludes => {
                out.extend(self.files_exclude.iter().cloned());
                out.extend(self.search_exclude.iter().cloned());
            }
        }
        out
    }

    /// Group name for a tag, if `tag_groups` maps it.
    pub fn group_of(&self, tag: &str) -> Option<&str> {
        self.tag_groups
            .iter()
            .find(|(_, tags)| tags.iter().any(|t| t == tag))
            .map(|(group, _)| group.as_str())
    }
}
```

- [ ] **Step 5: Run the tests to verify they pass**

Run: `cargo test -p clippings-core config`
Expected: 4 tests pass.
Then run: `cargo fmt --all && cargo clippy --all-targets -- -D warnings`
Expected: no diffs, no warnings. The first build downloads dependencies.

- [ ] **Step 6: Commit**

```bash
git add -A
git commit -m "feat(core): scaffold workspace and configuration"
```

### Task 2: Glob layers and built-in excludes

Implements exclusion layers 1 to 4 and the escaped temporary globs of spec section 5.3.

**Files:**
- Create: `crates/clippings-core/src/globs.rs`
- Modify: `crates/clippings-core/src/lib.rs` (add the module declaration)

**Interfaces:**
- Consumes: `CoreConfig` fields `include_globs`, `exclude_globs`, `temp_include_globs`, `temp_exclude_globs`, `vscode_excludes()`; `CoreError::InvalidGlob`.
- Produces: `slash_path(&Path) -> String`, `compile_set(&[String]) -> Result<GlobSet, CoreError>`, `folder_glob(&Path, recursive: bool) -> String`, `file_glob(&Path) -> String`.
- Produces: `GlobLayers::new(&CoreConfig) -> Result<GlobLayers, CoreError>`, `file_allowed(&self, abs: &str, rel: Option<&str>) -> bool`, `dir_pruned(&self, abs: &str, rel: Option<&str>) -> bool`, `path_allowed(&self, file: &Path, root: Option<&Path>) -> bool`.
- Produces: `BuiltInExcludes::new(&[String])`, `dir_excluded(&self, rel_dir: &[&str]) -> bool`, `file_excluded(&self, rel_file: &[&str]) -> bool`.

- [ ] **Step 1: Write the failing tests**

Add `pub mod globs;` to `crates/clippings-core/src/lib.rs`, keeping the declarations sorted. Create `crates/clippings-core/src/globs.rs` containing only this test module for now:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn layers(f: impl FnOnce(&mut CoreConfig)) -> GlobLayers {
        let mut c = CoreConfig::default();
        f(&mut c);
        GlobLayers::new(&c).unwrap()
    }

    #[test]
    fn star_does_not_cross_separator() {
        let l = layers(|c| c.exclude_globs = vec!["src/*.rs".into()]);
        assert!(!l.file_allowed("/r/src/a.rs", Some("src/a.rs")));
        assert!(l.file_allowed("/r/src/x/a.rs", Some("src/x/a.rs")));
    }

    #[test]
    fn matches_absolute_or_relative() {
        let l = layers(|c| c.exclude_globs = vec!["gen/**".into(), "/abs/only.ts".into()]);
        assert!(!l.file_allowed("/r/gen/a.ts", Some("gen/a.ts")));
        assert!(!l.file_allowed("/abs/only.ts", None));
    }

    #[test]
    fn default_node_modules_glob_prunes_package_dirs() {
        let l = layers(|_| {});
        assert!(l.dir_pruned("/r/node_modules/pkg", Some("node_modules/pkg")));
        assert!(!l.file_allowed("/r/node_modules/pkg/a.js", Some("node_modules/pkg/a.js")));
    }

    #[test]
    fn bare_directory_glob_prunes_directory() {
        let l = layers(|c| {
            c.use_built_in_excludes = crate::config::UseBuiltInExcludes::FileExcludes;
            c.files_exclude = vec!["**/dist".into()];
        });
        assert!(l.dir_pruned("/r/pkg/dist", Some("pkg/dist")));
        assert!(!l.path_allowed(Path::new("/r/pkg/dist/a.js"), Some(Path::new("/r"))));
    }

    #[test]
    fn dot_segments_match_stars() {
        let l = layers(|c| c.exclude_globs = vec!["**/*.rs".into()]);
        assert!(!l.file_allowed("/r/.hidden/a.rs", Some(".hidden/a.rs")));
    }

    #[test]
    fn includes_restrict_files() {
        let l = layers(|c| c.include_globs = vec!["**/src/**".into()]);
        assert!(l.file_allowed("/r/src/a.rs", Some("src/a.rs")));
        assert!(!l.file_allowed("/r/docs/a.md", Some("docs/a.md")));
    }

    #[test]
    fn escaped_folder_glob_matches_bracket_folder() {
        let dir = PathBuf::from("/r/app/[slug]");
        let l = layers(|c| c.temp_exclude_globs = vec![folder_glob(&dir, true)]);
        assert!(!l.file_allowed("/r/app/[slug]/page.tsx", Some("app/[slug]/page.tsx")));
        assert!(l.file_allowed("/r/app/s/page.tsx", Some("app/s/page.tsx")));
        let f =
            layers(|c| c.temp_exclude_globs = vec![file_glob(Path::new("/r/app/[slug]/page.tsx"))]);
        assert!(!f.file_allowed("/r/app/[slug]/page.tsx", None));
    }

    #[test]
    fn built_in_excludes_match_names_and_suffixes_below_root() {
        let b = BuiltInExcludes::new(&["node_modules".into(), ".yarn/cache".into()]);
        assert!(b.dir_excluded(&["a", "node_modules"]));
        assert!(b.dir_excluded(&[".yarn", "cache"]));
        assert!(!b.dir_excluded(&[".yarn"]));
        assert!(b.file_excluded(&["node_modules", "x", "a.js"]));
        assert!(!b.file_excluded(&["src", "node_modules.ts"]));
        assert!(!b.file_excluded(&[]));
    }
}
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p clippings-core globs`
Expected: compile error, `GlobLayers`, `BuiltInExcludes` and `folder_glob` not found.

- [ ] **Step 3: Write the implementation**

Put this above the test module in `crates/clippings-core/src/globs.rs`:

```rust
//! Exclusion layers 2 to 4 (user globs, temporary globs, VS Code excludes)
//! and layer 1, the built-in never-index list (spec section 5.3).

use crate::config::CoreConfig;
use crate::CoreError;
use globset::{GlobBuilder, GlobSet, GlobSetBuilder};
use std::collections::HashSet;
use std::path::Path;

/// A path as a string with `/` separators.
pub fn slash_path(p: &Path) -> String {
    let s = p.to_string_lossy();
    if cfg!(windows) {
        s.replace('\\', "/")
    } else {
        s.into_owned()
    }
}

/// Compiles globs with literal separators and globset's platform default for
/// backslashes. Empty strings are skipped.
pub fn compile_set(globs: &[String]) -> Result<GlobSet, CoreError> {
    let mut b = GlobSetBuilder::new();
    for g in globs.iter().filter(|g| !g.trim().is_empty()) {
        let glob = GlobBuilder::new(g)
            .literal_separator(true)
            .build()
            .map_err(|e| CoreError::InvalidGlob {
                glob: g.clone(),
                message: e.to_string(),
            })?;
        b.add(glob);
    }
    b.build().map_err(|e| CoreError::InvalidGlob {
        glob: globs.join(", "),
        message: e.to_string(),
    })
}

/// Glob for "Only Show This Folder" (`recursive = false`) or
/// "Only Show This Folder And Subfolders" / "Hide This Folder" (`recursive = true`).
pub fn folder_glob(folder: &Path, recursive: bool) -> String {
    let base = globset::escape(&slash_path(folder));
    if recursive {
        format!("{base}/**/*")
    } else {
        format!("{base}/*")
    }
}

/// Glob for "Hide This File".
pub fn file_glob(file: &Path) -> String {
    globset::escape(&slash_path(file))
}

pub struct GlobLayers {
    has_includes: bool,
    includes: GlobSet,
    excludes: GlobSet,
    /// Exclude globs ending in `/**` or `/**/*` with that suffix removed.
    dir_excludes: GlobSet,
}

impl GlobLayers {
    pub fn new(cfg: &CoreConfig) -> Result<Self, CoreError> {
        let includes: Vec<String> = cfg
            .include_globs
            .iter()
            .chain(&cfg.temp_include_globs)
            .cloned()
            .collect();
        let excludes: Vec<String> = cfg
            .exclude_globs
            .iter()
            .chain(&cfg.temp_exclude_globs)
            .cloned()
            .chain(cfg.vscode_excludes())
            .collect();
        let dir_excludes: Vec<String> = excludes
            .iter()
            .filter_map(|g| g.strip_suffix("/**/*").or_else(|| g.strip_suffix("/**")))
            .filter(|g| !g.is_empty())
            .map(str::to_string)
            .collect();
        Ok(Self {
            has_includes: includes.iter().any(|g| !g.trim().is_empty()),
            includes: compile_set(&includes)?,
            excludes: compile_set(&excludes)?,
            dir_excludes: compile_set(&dir_excludes)?,
        })
    }

    fn any(set: &GlobSet, abs: &str, rel: Option<&str>) -> bool {
        set.is_match(abs) || rel.is_some_and(|r| set.is_match(r))
    }

    /// Include and exclude check for a file. `rel` is the path relative to
    /// its scan root, when it has one.
    pub fn file_allowed(&self, abs: &str, rel: Option<&str>) -> bool {
        if self.has_includes && !Self::any(&self.includes, abs, rel) {
            return false;
        }
        !Self::any(&self.excludes, abs, rel)
    }

    /// Whether a directory is pruned by an exclude glob.
    pub fn dir_pruned(&self, abs: &str, rel: Option<&str>) -> bool {
        Self::any(&self.excludes, abs, rel) || Self::any(&self.dir_excludes, abs, rel)
    }

    /// File check plus a prune check on every ancestor directory. Ancestors
    /// are those strictly below `root` when given, else all ancestors.
    pub fn path_allowed(&self, file: &Path, root: Option<&Path>) -> bool {
        let rel_of = |p: &Path| root.and_then(|r| p.strip_prefix(r).ok()).map(slash_path);
        if !self.file_allowed(&slash_path(file), rel_of(file).as_deref()) {
            return false;
        }
        let mut dir = file.parent();
        while let Some(d) = dir {
            if let Some(r) = root {
                if d == r || !d.starts_with(r) {
                    break;
                }
            }
            if self.dir_pruned(&slash_path(d), rel_of(d).as_deref()) {
                return false;
            }
            dir = d.parent();
        }
        true
    }
}

/// Layer 1. Matched only against components below a scan root.
pub struct BuiltInExcludes {
    names: HashSet<String>,
    suffixes: Vec<Vec<String>>,
}

impl BuiltInExcludes {
    pub fn new(entries: &[String]) -> Self {
        let mut names = HashSet::new();
        let mut suffixes = Vec::new();
        for e in entries
            .iter()
            .map(|e| e.trim().trim_matches('/'))
            .filter(|e| !e.is_empty())
        {
            if e.contains('/') {
                suffixes.push(e.split('/').map(str::to_string).collect());
            } else {
                names.insert(e.to_string());
            }
        }
        Self { names, suffixes }
    }

    /// `rel_dir` holds the components of a directory below its scan root.
    pub fn dir_excluded(&self, rel_dir: &[&str]) -> bool {
        let Some(last) = rel_dir.last() else {
            return false;
        };
        if self.names.contains(*last) {
            return true;
        }
        self.suffixes.iter().any(|s| {
            s.len() <= rel_dir.len()
                && rel_dir[rel_dir.len() - s.len()..]
                    .iter()
                    .zip(s)
                    .all(|(a, b)| a == b)
        })
    }

    /// Whether any directory on the way from the root to `rel_file` is excluded.
    pub fn file_excluded(&self, rel_file: &[&str]) -> bool {
        (1..rel_file.len()).any(|n| self.dir_excluded(&rel_file[..n]))
    }
}
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test -p clippings-core globs`
Expected: all tests in the module pass.
Then run: `cargo fmt --all && cargo clippy --all-targets -- -D warnings`
Expected: no diffs, no warnings.

- [ ] **Step 5: Commit**

```bash
git add -A
git commit -m "feat(core): glob exclusion layers and built-in excludes"
```

### Task 3: Tree, scan and walked roots

Implements spec section 5.2 and the walked-roots column of section 5.9.

**Files:**
- Create: `crates/clippings-core/src/roots.rs`
- Modify: `crates/clippings-core/src/lib.rs` (add the module declaration)

**Interfaces:**
- Consumes: `globs::compile_set`, `globs::slash_path`, `CoreConfig::{root_folder, included_workspaces, excluded_workspaces}`, `ScanMode`.
- Produces: `Roots { tree_roots: Vec<PathBuf>, scan_roots: Vec<PathBuf>, from_root_folder: bool }`, `expand_env(&str, &dyn Fn(&str) -> Option<String>) -> String`, `resolve_roots(&[PathBuf], &CoreConfig, env) -> Result<Roots, CoreError>`, `walked_roots(&Roots, ScanMode) -> Vec<PathBuf>`, `deepest_root(&Path, &[PathBuf]) -> Option<&Path>`.

- [ ] **Step 1: Write the failing tests**

Add `pub mod roots;` to `crates/clippings-core/src/lib.rs`, keeping the declarations sorted. Create `crates/clippings-core/src/roots.rs` containing only this test module for now:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    fn no_env(_: &str) -> Option<String> {
        None
    }

    #[test]
    fn expands_env_but_not_workspace_folder() {
        let env = |n: &str| (n == "HOME").then(|| "/home/u".to_string());
        assert_eq!(
            expand_env("${HOME}/x/${workspaceFolder}/${NOPE}", &env),
            "/home/u/x/${workspaceFolder}/"
        );
    }

    #[test]
    fn tree_roots_are_scan_roots_without_root_folder() {
        let folders = vec![PathBuf::from("/w/a"), PathBuf::from("/w/b")];
        let r = resolve_roots(&folders, &CoreConfig::default(), &no_env).unwrap();
        assert_eq!(r.tree_roots, folders);
        assert_eq!(r.scan_roots, folders);
        assert!(!r.from_root_folder);
    }

    #[test]
    fn excluded_workspaces_filter_by_path_glob() {
        let folders = vec![PathBuf::from("/w/a"), PathBuf::from("/w/skip")];
        let cfg = CoreConfig {
            excluded_workspaces: vec!["**/skip".into()],
            ..Default::default()
        };
        let r = resolve_roots(&folders, &cfg, &no_env).unwrap();
        assert_eq!(r.tree_roots, vec![PathBuf::from("/w/a")]);
    }

    #[test]
    fn root_folder_expands_per_workspace_folder() {
        let folders = vec![PathBuf::from("/w/a"), PathBuf::from("/w/b")];
        let cfg = CoreConfig {
            root_folder: "${workspaceFolder}/src".into(),
            ..Default::default()
        };
        let r = resolve_roots(&folders, &cfg, &no_env).unwrap();
        assert_eq!(r.tree_roots, folders);
        assert_eq!(
            r.scan_roots,
            vec![PathBuf::from("/w/a/src"), PathBuf::from("/w/b/src")]
        );
        assert!(r.from_root_folder);
    }

    #[test]
    fn open_file_modes_walk_only_root_folder() {
        let folders = vec![PathBuf::from("/w/a")];
        let plain = resolve_roots(&folders, &CoreConfig::default(), &no_env).unwrap();
        assert!(walked_roots(&plain, ScanMode::OpenFiles).is_empty());
        assert_eq!(walked_roots(&plain, ScanMode::WorkspaceOnly), folders);
        let cfg = CoreConfig {
            root_folder: "/elsewhere".into(),
            ..Default::default()
        };
        let rf = resolve_roots(&folders, &cfg, &no_env).unwrap();
        assert_eq!(
            walked_roots(&rf, ScanMode::CurrentFile),
            vec![PathBuf::from("/elsewhere")]
        );
    }

    #[test]
    fn deepest_root_wins() {
        let roots = vec![PathBuf::from("/w"), PathBuf::from("/w/inner")];
        assert_eq!(
            deepest_root(Path::new("/w/inner/x.rs"), &roots),
            Some(Path::new("/w/inner"))
        );
        assert_eq!(
            deepest_root(Path::new("/w/y.rs"), &roots),
            Some(Path::new("/w"))
        );
        assert_eq!(deepest_root(Path::new("/other/y.rs"), &roots), None);
    }
}
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p clippings-core roots`
Expected: compile error, `resolve_roots` and `walked_roots` not found.

- [ ] **Step 3: Write the implementation**

Put this above the test module in `crates/clippings-core/src/roots.rs`:

```rust
//! Tree roots, scan roots and walked roots (spec section 5.2 and 5.9).

use crate::config::{CoreConfig, ScanMode};
use crate::globs::compile_set;
use crate::CoreError;
use std::path::{Path, PathBuf};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Roots {
    /// Workspace folders shown as root nodes, after include/exclude filtering.
    pub tree_roots: Vec<PathBuf>,
    /// What the walker walks: the `rootFolder` expansion, or the tree roots.
    pub scan_roots: Vec<PathBuf>,
    /// True when `scan_roots` came from `rootFolder`.
    pub from_root_folder: bool,
}

/// Replaces `${NAME}` with the environment variable NAME (empty if unset).
/// `${workspaceFolder}` is left alone; the caller expands it.
pub fn expand_env(input: &str, env: &dyn Fn(&str) -> Option<String>) -> String {
    let mut out = String::with_capacity(input.len());
    let mut rest = input;
    while let Some(start) = rest.find("${") {
        out.push_str(&rest[..start]);
        let after = &rest[start + 2..];
        match after.find('}') {
            Some(end) => {
                let name = &after[..end];
                if name == "workspaceFolder" {
                    out.push_str("${workspaceFolder}");
                } else {
                    out.push_str(&env(name).unwrap_or_default());
                }
                rest = &after[end + 1..];
            }
            None => {
                out.push_str(&rest[start..]);
                rest = "";
            }
        }
    }
    out.push_str(rest);
    out
}

pub fn resolve_roots(
    workspace_folders: &[PathBuf],
    cfg: &CoreConfig,
    env: &dyn Fn(&str) -> Option<String>,
) -> Result<Roots, CoreError> {
    let includes = compile_set(&cfg.included_workspaces)?;
    let excludes = compile_set(&cfg.excluded_workspaces)?;
    let allowed = |p: &Path| {
        let s = crate::globs::slash_path(p);
        (cfg.included_workspaces.is_empty() || includes.is_match(&s)) && !excludes.is_match(&s)
    };
    let tree_roots: Vec<PathBuf> = workspace_folders
        .iter()
        .filter(|p| allowed(p))
        .cloned()
        .collect();

    let root_folder = cfg.root_folder.trim();
    if root_folder.is_empty() {
        return Ok(Roots {
            scan_roots: tree_roots.clone(),
            tree_roots,
            from_root_folder: false,
        });
    }
    let expanded = expand_env(root_folder, env);
    let scan_roots: Vec<PathBuf> = if expanded.contains("${workspaceFolder}") {
        workspace_folders
            .iter()
            .map(|f| PathBuf::from(expanded.replace("${workspaceFolder}", &f.to_string_lossy())))
            .filter(|p| allowed(p))
            .collect()
    } else {
        vec![PathBuf::from(expanded)]
    };
    Ok(Roots {
        tree_roots,
        scan_roots,
        from_root_folder: true,
    })
}

/// Scan roots that `mode` walks: all of them in the workspace modes, only a
/// `rootFolder` root in the open-file modes.
pub fn walked_roots(roots: &Roots, mode: ScanMode) -> Vec<PathBuf> {
    match mode {
        ScanMode::Workspace | ScanMode::WorkspaceOnly => roots.scan_roots.clone(),
        ScanMode::OpenFiles | ScanMode::CurrentFile => {
            if roots.from_root_folder {
                roots.scan_roots.clone()
            } else {
                Vec::new()
            }
        }
    }
}

/// The deepest root that contains `path`.
pub fn deepest_root<'a>(path: &Path, roots: &'a [PathBuf]) -> Option<&'a Path> {
    roots
        .iter()
        .filter(|r| path.starts_with(r))
        .max_by_key(|r| r.components().count())
        .map(|r| r.as_path())
}
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test -p clippings-core roots`
Expected: all tests in the module pass.
Then run: `cargo fmt --all && cargo clippy --all-targets -- -D warnings`
Expected: no diffs, no warnings.

- [ ] **Step 5: Commit**

```bash
git add -A
git commit -m "feat(core): tree, scan and walked roots"
```

### Task 4: Ignore-file rules for single paths

The walker gets ignore rules from the `ignore` crate. File events and document closes in plan 2 check one path at a time, so this module answers the same question for a single path. The global excludes file is rooted at the process's working directory, so it is matched with `matched` on each ancestor, because `matched_path_or_any_parents` panics for paths outside its root.

**Files:**
- Create: `crates/clippings-core/src/ignore_rules.rs`
- Modify: `crates/clippings-core/src/lib.rs` (add the module declaration)

**Interfaces:**
- Produces: `IgnoreRules::new()`, `IgnoreRules::clear(&self)`, `IgnoreRules::is_ignored(&self, path: &Path, is_dir: bool) -> bool`. Rules in deeper directories win; `.gitignore`, `.git/info/exclude` and the global excludes file apply only inside a git repository; `.ignore` and `.rgignore` always apply.

- [ ] **Step 1: Write the failing tests**

Add `pub mod ignore_rules;` to `crates/clippings-core/src/lib.rs`, keeping the declarations sorted. Create `crates/clippings-core/src/ignore_rules.rs` containing only this test module for now:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn gitignore_applies_only_in_repo() {
        let t = tempfile::tempdir().unwrap();
        let root = t.path();
        fs::write(root.join(".gitignore"), "dist/\n*.log\n").unwrap();
        fs::create_dir_all(root.join("dist")).unwrap();
        let rules = IgnoreRules::new();
        assert!(!rules.is_ignored(&root.join("dist/a.js"), false));
        fs::create_dir(root.join(".git")).unwrap();
        rules.clear();
        assert!(rules.is_ignored(&root.join("dist/a.js"), false));
        assert!(rules.is_ignored(&root.join("x.log"), false));
        assert!(!rules.is_ignored(&root.join("src/a.js"), false));
    }

    #[test]
    fn ignore_and_rgignore_apply_without_repo_and_deeper_wins() {
        let t = tempfile::tempdir().unwrap();
        let root = t.path();
        fs::create_dir_all(root.join("sub")).unwrap();
        fs::write(root.join(".ignore"), "*.gen.ts\n").unwrap();
        fs::write(root.join("sub/.rgignore"), "!keep.gen.ts\n").unwrap();
        let rules = IgnoreRules::new();
        assert!(rules.is_ignored(&root.join("a.gen.ts"), false));
        assert!(!rules.is_ignored(&root.join("sub/keep.gen.ts"), false));
    }

    #[test]
    fn git_info_exclude_applies() {
        let t = tempfile::tempdir().unwrap();
        let root = t.path();
        fs::create_dir_all(root.join(".git/info")).unwrap();
        fs::write(root.join(".git/info/exclude"), ".claude/worktrees/\n").unwrap();
        let rules = IgnoreRules::new();
        assert!(rules.is_ignored(&root.join(".claude/worktrees/x/a.ts"), false));
    }
}
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p clippings-core ignore_rules`
Expected: compile error, `IgnoreRules` not found.

- [ ] **Step 3: Write the implementation**

Put this above the test module in `crates/clippings-core/src/ignore_rules.rs`:

```rust
//! Ignore-file rules evaluated for a single path, matching what the
//! `ignore` crate's walker would decide (spec section 5.3, admission
//! predicate). Used for file events and document closes, where no walk runs.

use ignore::gitignore::{Gitignore, GitignoreBuilder};
use ignore::Match;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

struct DirRules {
    /// `.rgignore`, `.ignore`, `.gitignore`, highest precedence first.
    files: Vec<(Gitignore, bool)>,
    /// `.git/info/exclude` when this directory holds a `.git` directory.
    git_exclude: Option<Gitignore>,
    has_git: bool,
}

pub struct IgnoreRules {
    cache: Mutex<HashMap<PathBuf, Arc<DirRules>>>,
    global: Gitignore,
}

fn load(dir: &Path, name: &str) -> Option<Gitignore> {
    let path = dir.join(name);
    if !path.is_file() {
        return None;
    }
    let mut b = GitignoreBuilder::new(dir);
    b.add(&path);
    b.build().ok()
}

impl Default for IgnoreRules {
    fn default() -> Self {
        Self::new()
    }
}

impl IgnoreRules {
    pub fn new() -> Self {
        Self {
            cache: Mutex::new(HashMap::new()),
            global: Gitignore::global().0,
        }
    }

    /// Forgets cached rules, for when an ignore file changes.
    pub fn clear(&self) {
        self.cache.lock().unwrap().clear();
    }

    fn rules(&self, dir: &Path) -> Arc<DirRules> {
        if let Some(r) = self.cache.lock().unwrap().get(dir) {
            return r.clone();
        }
        let git = dir.join(".git");
        let has_git = git.exists();
        let mut files = Vec::new();
        for (name, git_only) in [
            (".rgignore", false),
            (".ignore", false),
            (".gitignore", true),
        ] {
            if let Some(g) = load(dir, name) {
                files.push((g, git_only));
            }
        }
        let git_exclude = if git.is_dir() {
            let p = git.join("info").join("exclude");
            p.is_file()
                .then(|| {
                    let mut b = GitignoreBuilder::new(dir);
                    b.add(&p);
                    b.build().ok()
                })
                .flatten()
        } else {
            None
        };
        let r = Arc::new(DirRules {
            files,
            git_exclude,
            has_git,
        });
        self.cache
            .lock()
            .unwrap()
            .insert(dir.to_path_buf(), r.clone());
        r
    }

    /// Whether ignore files hide `path`. Rules in deeper directories win.
    /// `.gitignore`, git excludes and the global excludes file apply only
    /// inside a git repository, as with ripgrep's defaults.
    pub fn is_ignored(&self, path: &Path, is_dir: bool) -> bool {
        let ancestors: Vec<&Path> = path.ancestors().skip(1).collect();
        let in_repo = ancestors.iter().any(|d| self.rules(d).has_git);
        for dir in &ancestors {
            let rules = self.rules(dir);
            for (g, git_only) in &rules.files {
                if *git_only && !in_repo {
                    continue;
                }
                match g.matched_path_or_any_parents(path, is_dir) {
                    Match::None => {}
                    m => return m.is_ignore(),
                }
            }
        }
        if in_repo {
            for dir in &ancestors {
                if let Some(g) = &self.rules(dir).git_exclude {
                    match g.matched_path_or_any_parents(path, is_dir) {
                        Match::None => {}
                        m => return m.is_ignore(),
                    }
                }
            }
            // The global file is rooted at the process's current directory, so
            // test the path and each ancestor with `matched`, which never panics.
            let mut is_dir_now = is_dir;
            for p in path.ancestors() {
                match self.global.matched(p, is_dir_now) {
                    Match::None => {}
                    m => return m.is_ignore(),
                }
                is_dir_now = true;
            }
        }
        false
    }
}
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test -p clippings-core ignore_rules`
Expected: all tests in the module pass.
Then run: `cargo fmt --all && cargo clippy --all-targets -- -D warnings`
Expected: no diffs, no warnings.

- [ ] **Step 5: Commit**

```bash
git add -A
git commit -m "feat(core): single-path ignore-file rules"
```

### Task 5: Admission predicate

Implements the admission predicate and open-buffer rules of spec section 5.3. Task 10 adds a test that checks this predicate agrees with the walker on every fixture file.

**Files:**
- Create: `crates/clippings-core/src/admission.rs`
- Modify: `crates/clippings-core/src/lib.rs` (add the module declaration)

**Interfaces:**
- Consumes: `GlobLayers`, `BuiltInExcludes`, `IgnoreRules`, `deepest_root`, `Fs`.
- Produces: `rel_components(&Path, root: &Path) -> Vec<String>`, `buffer_hidden(&Path) -> bool`, `Admission::new(&CoreConfig, walked_roots: Vec<PathBuf>, Arc<dyn Fs>) -> Result<Admission, CoreError>`, `admits_disk(&self, &Path) -> bool`, `admits_buffer(&self, Option<&Path>, tree_roots: &[PathBuf]) -> bool`, `walked_roots(&self) -> &[PathBuf]`, `clear_ignore_cache(&self)`.

- [ ] **Step 1: Write the failing tests**

Add `pub mod admission;` to `crates/clippings-core/src/lib.rs`, keeping the declarations sorted. Create `crates/clippings-core/src/admission.rs` containing only this test module for now:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::fs::NativeFs;
    use std::fs;

    fn admission(root: &Path, f: impl FnOnce(&mut CoreConfig)) -> Admission {
        let mut c = CoreConfig::default();
        f(&mut c);
        Admission::new(&c, vec![root.to_path_buf()], Arc::new(NativeFs)).unwrap()
    }

    #[test]
    fn disk_rules() {
        let t = tempfile::tempdir().unwrap();
        let r = t.path();
        fs::create_dir_all(r.join(".git")).unwrap();
        fs::write(r.join(".gitignore"), "dist/\n").unwrap();
        let a = admission(r, |_| {});
        assert!(a.admits_disk(&r.join("src/a.ts")));
        assert!(!a.admits_disk(&r.join("dist/a.js")), "gitignored");
        assert!(!a.admits_disk(&r.join(".github/ci.yml")), "hidden");
        assert!(!a.admits_disk(&r.join("node_modules/x/a.js")), "built-in");
        assert!(
            !a.admits_disk(&r.join(".claude/worktrees/w/a.ts")),
            "built-in and hidden"
        );
        assert!(
            !a.admits_disk(Path::new("/elsewhere/a.ts")),
            "outside walked roots"
        );
        let hidden_ok = admission(r, |c| c.include_hidden_files = true);
        assert!(hidden_ok.admits_disk(&r.join(".github/ci.yml")));
    }

    #[test]
    fn built_in_excludes_ignore_root_ancestors() {
        let t = tempfile::tempdir().unwrap();
        let r = t.path().join("target/.claude/worktrees/w");
        fs::create_dir_all(&r).unwrap();
        let a = admission(&r, |c| c.include_hidden_files = true);
        assert!(a.admits_disk(&r.join("src/a.ts")));
    }

    #[test]
    fn submodules_are_skipped_when_asked() {
        let t = tempfile::tempdir().unwrap();
        let r = t.path();
        fs::create_dir_all(r.join("vendor/lib")).unwrap();
        fs::write(r.join("vendor/lib/.git"), "gitdir: x").unwrap();
        assert!(admission(r, |_| {}).admits_disk(&r.join("vendor/lib/a.c")));
        assert!(!admission(r, |c| c.ignore_git_submodules = true)
            .admits_disk(&r.join("vendor/lib/a.c")));
    }

    #[test]
    fn buffer_rules_ignore_gitignore_and_built_ins() {
        let t = tempfile::tempdir().unwrap();
        let r = t.path();
        fs::create_dir_all(r.join(".git")).unwrap();
        fs::write(r.join(".gitignore"), "dist/\n").unwrap();
        let a = admission(r, |_| {});
        let roots = vec![r.to_path_buf()];
        assert!(a.admits_buffer(Some(&r.join("dist/a.js")), &roots));
        assert!(a.admits_buffer(Some(&r.join("target/x.rs")), &roots));
        assert!(a.admits_buffer(Some(&r.join(".eslintrc.json")), &roots));
        assert!(!a.admits_buffer(Some(&r.join(".env")), &roots));
        assert!(
            !a.admits_buffer(Some(&r.join("node_modules/p/a.js")), &roots),
            "default exclude glob"
        );
        assert!(a.admits_buffer(None, &roots));
    }
}
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p clippings-core admission`
Expected: compile error, `Admission` not found.

- [ ] **Step 3: Write the implementation**

Put this above the test module in `crates/clippings-core/src/admission.rs`:

```rust
//! Admission rules (spec section 5.3): which disk paths and open buffers
//! may enter the index. The walker applies the same rules during a walk;
//! tests check the two agree.

use crate::config::CoreConfig;
use crate::fs::Fs;
use crate::globs::{BuiltInExcludes, GlobLayers};
use crate::ignore_rules::IgnoreRules;
use crate::roots::deepest_root;
use crate::CoreError;
use std::path::{Path, PathBuf};
use std::sync::Arc;

/// Components of `path` below `root`, as strings.
pub fn rel_components(path: &Path, root: &Path) -> Vec<String> {
    path.strip_prefix(root)
        .map(|r| {
            r.components()
                .map(|c| c.as_os_str().to_string_lossy().into_owned())
                .collect()
        })
        .unwrap_or_default()
}

/// todo-tree's rule for open buffers: a dot-prefixed base name without an extension.
pub fn buffer_hidden(path: &Path) -> bool {
    let name = path
        .file_name()
        .map(|n| n.to_string_lossy())
        .unwrap_or_default();
    name.starts_with('.') && path.extension().is_none()
}

pub struct Admission {
    walked_roots: Vec<PathBuf>,
    pub(crate) layers: GlobLayers,
    pub(crate) built_in: BuiltInExcludes,
    ignore: IgnoreRules,
    include_hidden: bool,
    ignore_submodules: bool,
    respect_ignore_files: bool,
    fs: Arc<dyn Fs>,
}

impl Admission {
    pub fn new(
        cfg: &CoreConfig,
        walked_roots: Vec<PathBuf>,
        fs: Arc<dyn Fs>,
    ) -> Result<Self, CoreError> {
        Ok(Self {
            walked_roots,
            layers: GlobLayers::new(cfg)?,
            built_in: BuiltInExcludes::new(&cfg.built_in_excludes),
            ignore: IgnoreRules::new(),
            include_hidden: cfg.include_hidden_files,
            ignore_submodules: cfg.ignore_git_submodules,
            respect_ignore_files: cfg.respect_ignore_files,
            fs,
        })
    }

    pub fn walked_roots(&self) -> &[PathBuf] {
        &self.walked_roots
    }

    /// Forgets cached ignore-file rules, for when an ignore file changes.
    pub fn clear_ignore_cache(&self) {
        self.ignore.clear();
    }

    /// Whether a file on disk may enter the index. Binary detection happens later, in the scanner.
    pub fn admits_disk(&self, path: &Path) -> bool {
        let Some(root) = deepest_root(path, &self.walked_roots) else {
            return false;
        };
        let rel = rel_components(path, root);
        if rel.is_empty() {
            return false;
        }
        let rel_refs: Vec<&str> = rel.iter().map(String::as_str).collect();
        if !self.include_hidden && rel_refs.iter().any(|c| c.starts_with('.')) {
            return false;
        }
        if self.built_in.file_excluded(&rel_refs) {
            return false;
        }
        if !self.layers.path_allowed(path, Some(root)) {
            return false;
        }
        if self.ignore_submodules {
            let mut dir = path.parent();
            while let Some(d) = dir {
                if d == root || !d.starts_with(root) {
                    break;
                }
                if self.fs.exists(&d.join(".git")) {
                    return false;
                }
                dir = d.parent();
            }
        }
        !(self.respect_ignore_files && self.ignore.is_ignored(path, false))
    }

    /// Whether an open buffer may feed the tree and get decorations. `path`
    /// is `None` for documents without a file path, such as `untitled:`.
    pub fn admits_buffer(&self, path: Option<&Path>, tree_roots: &[PathBuf]) -> bool {
        let Some(path) = path else { return true };
        let root = deepest_root(path, tree_roots);
        self.layers.path_allowed(path, root) && !buffer_hidden(path)
    }
}
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test -p clippings-core admission`
Expected: all tests in the module pass.
Then run: `cargo fmt --all && cargo clippy --all-targets -- -D warnings`
Expected: no diffs, no warnings.

- [ ] **Step 5: Commit**

```bash
git add -A
git commit -m "feat(core): admission predicate for disk paths and buffers"
```

### Task 6: Regex construction and engine selection

Implements spec section 5.4. Line mode uses `crlf(true)` so matches never contain `\r` or `\n`. Multi-line mode is chosen when the source contains the two characters `\n`, when `enableMultiLine` is set, or when grep-regex rejects the pattern in line mode with `NotAllowed`. fancy-regex is used only when `regex-syntax` reports look-around or a backreference.

**Files:**
- Create: `crates/clippings-core/src/pattern.rs`
- Modify: `crates/clippings-core/src/lib.rs` (add the module declaration)

**Interfaces:**
- Consumes: `CoreConfig::{regex, regex_case_sensitive, sub_tag_regex, enable_multi_line, effective_tags()}`, `CoreError::InvalidRegex`.
- Produces: `tag_alternation(&[String]) -> String`, `expand_tags(&str, &[String]) -> String`, `Engine { Grep, Fancy }`, `PatternMatcher` implementing `grep_matcher::Matcher<Captures = NoCaptures, Error = NoError>`, `build(&CoreConfig) -> Result<ScanPattern, CoreError>`.
- Produces: `ScanPattern { source: String, matcher: PatternMatcher, engine: Engine, multi_line: bool, tags: Vec<String>, tag_re: Option<regex::Regex>, sub_tag_re: Option<regex::Regex>, case_sensitive: bool }`.

- [ ] **Step 1: Write the failing tests**

Add `pub mod pattern;` to `crates/clippings-core/src/lib.rs`, keeping the declarations sorted. Create `crates/clippings-core/src/pattern.rs` containing only this test module for now:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    fn find_all(p: &ScanPattern, text: &str) -> Vec<String> {
        let mut out = Vec::new();
        p.matcher
            .find_iter(text.as_bytes(), |m| {
                out.push(text[m.start()..m.end()].to_string());
                true
            })
            .unwrap();
        out
    }

    #[test]
    fn alternation_is_longest_first_and_escaped() {
        let tags: Vec<String> = ["TODO", "[ ]", "TODOS", "FIX"]
            .iter()
            .map(|s| s.to_string())
            .collect();
        assert_eq!(tag_alternation(&tags), r"TODOS|TODO|\[ \]|FIX");
    }

    #[test]
    fn default_pattern_matches_comment_tags() {
        let p = build(&CoreConfig::default()).unwrap();
        assert_eq!(p.engine, Engine::Grep);
        assert!(!p.multi_line);
        assert_eq!(
            find_all(&p, "x(); // TODO fix\n# FIXME y\n- [ ] task\nno TODO here"),
            vec!["// TODO", "# FIXME", "- [ ]"]
        );
    }

    #[test]
    fn line_mode_never_crosses_newline() {
        let p = build(&CoreConfig::default()).unwrap();
        assert!(find_all(&p, "//\nTODO").iter().all(|m| !m.contains('\n')));
    }

    #[test]
    fn case_insensitive_flag() {
        let cfg = CoreConfig {
            regex_case_sensitive: false,
            ..Default::default()
        };
        let p = build(&cfg).unwrap();
        assert_eq!(find_all(&p, "// todo lower"), vec!["// todo"]);
    }

    #[test]
    fn literal_newline_in_source_switches_to_multi_line() {
        let cfg = CoreConfig {
            regex: r"(//)\s*($TAGS).*(\n\s*//\s{2,}.*)*".into(),
            ..Default::default()
        };
        let p = build(&cfg).unwrap();
        assert!(p.multi_line);
        assert_eq!(
            find_all(&p, "// TODO a\n//   b\nx"),
            vec!["// TODO a\n//   b"]
        );
    }

    #[test]
    fn escaped_newline_byte_switches_to_multi_line() {
        let cfg = CoreConfig {
            regex: r"($TAGS)\x0a".into(),
            ..Default::default()
        };
        let p = build(&cfg).unwrap();
        assert!(p.multi_line);
    }

    #[test]
    fn lookaround_falls_back_to_fancy() {
        let cfg = CoreConfig {
            regex: r"(?<=// )($TAGS)".into(),
            ..Default::default()
        };
        let p = build(&cfg).unwrap();
        assert_eq!(p.engine, Engine::Fancy);
        assert_eq!(find_all(&p, "x // TODO y"), vec!["TODO"]);
    }

    #[test]
    fn fancy_maps_offsets_across_invalid_utf8() {
        let cfg = CoreConfig {
            regex: r"(?<=// )($TAGS)".into(),
            ..Default::default()
        };
        let p = build(&cfg).unwrap();
        let hay = b"\xff\xfe // TODO";
        let m = p.matcher.find(hay).unwrap().unwrap();
        assert_eq!(&hay[m.start()..m.end()], b"TODO");
    }

    #[test]
    fn invalid_pattern_errors() {
        let cfg = CoreConfig {
            regex: "(unclosed".into(),
            ..Default::default()
        };
        assert!(matches!(build(&cfg), Err(CoreError::InvalidRegex(_))));
    }

    #[test]
    fn tag_and_sub_tag_regexes() {
        let cfg = CoreConfig {
            sub_tag_regex: r"^\s*\((.*?)\)".into(),
            regex_case_sensitive: false,
            ..Default::default()
        };
        let p = build(&cfg).unwrap();
        assert!(p.tag_re.as_ref().unwrap().is_match("todo"));
        assert!(p.sub_tag_re.as_ref().unwrap().is_match(" (ME) x"));
    }
}
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p clippings-core pattern`
Expected: compile error, `build`, `tag_alternation` and `Engine` not found.

- [ ] **Step 3: Write the implementation**

Put this above the test module in `crates/clippings-core/src/pattern.rs`:

```rust
//! Regex construction (spec section 5.4): `($TAGS)` expansion, flags,
//! engine selection and the fancy-regex fallback.

use crate::config::CoreConfig;
use crate::CoreError;
use grep_matcher::{Match, Matcher, NoCaptures, NoError};
use grep_regex::{ErrorKind, RegexMatcher, RegexMatcherBuilder};

pub const TAGS_TOKEN: &str = "($TAGS)";

/// Tags escaped and joined with `|`, longest first, ties in configured order.
pub fn tag_alternation(tags: &[String]) -> String {
    let mut sorted: Vec<&String> = tags.iter().collect();
    sorted.sort_by_key(|t| std::cmp::Reverse(t.chars().count()));
    sorted
        .iter()
        .map(|t| regex::escape(t))
        .collect::<Vec<_>>()
        .join("|")
}

/// Replaces the literal token `($TAGS)` with `(<alternation>)`.
pub fn expand_tags(source: &str, tags: &[String]) -> String {
    source.replace(TAGS_TOKEN, &format!("({})", tag_alternation(tags)))
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Engine {
    Grep,
    Fancy,
}

/// fancy-regex behind the `grep_matcher::Matcher` trait.
#[derive(Clone, Debug)]
pub struct FancyMatcher {
    re: fancy_regex::Regex,
}

impl FancyMatcher {
    fn find_str(&self, text: &str, at: usize) -> Option<(usize, usize)> {
        match self.re.find_from_pos(text, at) {
            Ok(Some(m)) => Some((m.start(), m.end())),
            Ok(None) => None,
            Err(e) => {
                tracing::debug!("fancy-regex runtime error: {e}");
                None
            }
        }
    }
}

impl Matcher for FancyMatcher {
    type Captures = NoCaptures;
    type Error = NoError;

    fn find_at(&self, haystack: &[u8], at: usize) -> Result<Option<Match>, NoError> {
        if let Ok(text) = std::str::from_utf8(haystack) {
            return Ok(self.find_str(text, at).map(|(s, e)| Match::new(s, e)));
        }
        // Lossy conversion with an offset map back to bytes.
        let mut text = String::with_capacity(haystack.len());
        let mut map: Vec<(usize, usize)> = Vec::new(); // (string offset, byte offset)
        let mut byte = 0;
        for chunk in haystack.utf8_chunks() {
            map.push((text.len(), byte));
            text.push_str(chunk.valid());
            byte += chunk.valid().len();
            for _ in chunk.invalid() {
                map.push((text.len(), byte));
                text.push('\u{FFFD}');
                byte += 1;
            }
        }
        map.push((text.len(), byte));
        let to_byte = |s: usize| {
            let i = map.partition_point(|&(so, _)| so <= s) - 1;
            let (so, bo) = map[i];
            // Inside a valid chunk offsets advance together; a replacement
            // character (3 bytes) stands for one invalid byte.
            if map
                .get(i + 1)
                .is_some_and(|&(nso, nbo)| nso - so != nbo - bo)
            {
                bo
            } else {
                bo + (s - so)
            }
        };
        let to_str = |b: usize| {
            let i = map.partition_point(|&(_, bo)| bo <= b) - 1;
            let (so, bo) = map[i];
            so + (b - bo)
        };
        Ok(self
            .find_str(&text, to_str(at))
            .map(|(s, e)| Match::new(to_byte(s), to_byte(e))))
    }

    fn new_captures(&self) -> Result<NoCaptures, NoError> {
        Ok(NoCaptures::new())
    }

    fn line_terminator(&self) -> Option<grep_matcher::LineTerminator> {
        Some(grep_matcher::LineTerminator::byte(b'\n'))
    }
}

#[derive(Clone, Debug)]
pub enum PatternMatcher {
    Grep(RegexMatcher),
    Fancy(FancyMatcher),
}

impl Matcher for PatternMatcher {
    type Captures = NoCaptures;
    type Error = NoError;

    fn find_at(&self, haystack: &[u8], at: usize) -> Result<Option<Match>, NoError> {
        match self {
            PatternMatcher::Grep(m) => Ok(m.find_at(haystack, at).unwrap_or(None)),
            PatternMatcher::Fancy(m) => m.find_at(haystack, at),
        }
    }

    fn new_captures(&self) -> Result<NoCaptures, NoError> {
        Ok(NoCaptures::new())
    }

    fn line_terminator(&self) -> Option<grep_matcher::LineTerminator> {
        match self {
            PatternMatcher::Grep(m) => m.line_terminator(),
            PatternMatcher::Fancy(m) => m.line_terminator(),
        }
    }

    fn find_candidate_line(
        &self,
        haystack: &[u8],
    ) -> Result<Option<grep_matcher::LineMatchKind>, NoError> {
        match self {
            PatternMatcher::Grep(m) => Ok(m.find_candidate_line(haystack).unwrap_or(None)),
            PatternMatcher::Fancy(m) => Ok(m
                .find_at(haystack, 0)?
                .map(|m| grep_matcher::LineMatchKind::Confirmed(m.start()))),
        }
    }
}

#[derive(Clone, Debug)]
pub struct ScanPattern {
    /// The expanded pattern source.
    pub source: String,
    pub matcher: PatternMatcher,
    pub engine: Engine,
    /// Whether searches run in multi-line mode.
    pub multi_line: bool,
    /// Configured tags, in configured order.
    pub tags: Vec<String>,
    /// Tag alternation for extraction, when the source used `($TAGS)` or `$TAGS`.
    pub tag_re: Option<regex::Regex>,
    pub sub_tag_re: Option<regex::Regex>,
    pub case_sensitive: bool,
}

fn grep_builder(cfg: &CoreConfig, multi_line: bool) -> RegexMatcherBuilder {
    let mut b = RegexMatcherBuilder::new();
    b.multi_line(true)
        .case_insensitive(!cfg.regex_case_sensitive)
        .dot_matches_new_line(cfg.enable_multi_line)
        .crlf(true);
    if multi_line {
        b.line_terminator(None);
    }
    b
}

fn uses_unsupported_feature(source: &str) -> bool {
    use regex_syntax::ast::{parse::Parser, ErrorKind as Ast};
    matches!(
        Parser::new().parse(source).map_err(|e| e.kind().clone()),
        Err(Ast::UnsupportedLookAround) | Err(Ast::UnsupportedBackreference)
    )
}

fn flag_prefix(cfg: &CoreConfig) -> String {
    let mut f = String::from("(?m");
    if !cfg.regex_case_sensitive {
        f.push('i');
    }
    if cfg.enable_multi_line {
        f.push('s');
    }
    f.push(')');
    f
}

pub fn build(cfg: &CoreConfig) -> Result<ScanPattern, CoreError> {
    let tags = cfg.effective_tags();
    let source = expand_tags(&cfg.regex, &tags);
    let mut multi_line = cfg.regex.contains(r"\n") || cfg.enable_multi_line;

    let (matcher, engine) = if uses_unsupported_feature(&source) {
        let re = fancy_regex::Regex::new(&format!("{}{}", flag_prefix(cfg), source))
            .map_err(|e| CoreError::InvalidRegex(e.to_string()))?;
        (PatternMatcher::Fancy(FancyMatcher { re }), Engine::Fancy)
    } else {
        let built = match grep_builder(cfg, multi_line).build(&source) {
            Err(e) if !multi_line && matches!(e.kind(), ErrorKind::NotAllowed(_)) => {
                multi_line = true;
                grep_builder(cfg, true).build(&source)
            }
            other => other,
        };
        let m = built.map_err(|e| CoreError::InvalidRegex(e.to_string()))?;
        (PatternMatcher::Grep(m), Engine::Grep)
    };

    let ci = if cfg.regex_case_sensitive { "" } else { "(?i)" };
    let tag_re = cfg
        .regex
        .contains("$TAGS")
        .then(|| regex::Regex::new(&format!("{ci}(?:{})", tag_alternation(&tags))))
        .transpose()
        .map_err(|e| CoreError::InvalidRegex(e.to_string()))?;
    let sub_tag_re = (!cfg.sub_tag_regex.is_empty())
        .then(|| regex::Regex::new(&format!("{ci}{}", cfg.sub_tag_regex)))
        .transpose()
        .map_err(|e| CoreError::InvalidRegex(e.to_string()))?;

    Ok(ScanPattern {
        source,
        matcher,
        engine,
        multi_line,
        tags,
        tag_re,
        sub_tag_re,
        case_sensitive: cfg.regex_case_sensitive,
    })
}
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test -p clippings-core pattern`
Expected: all tests in the module pass.
Then run: `cargo fmt --all && cargo clippy --all-targets -- -D warnings`
Expected: no diffs, no warnings.

- [ ] **Step 5: Commit**

```bash
git add -A
git commit -m "feat(core): regex construction with fancy-regex fallback"
```

### Task 7: Positions and comment syntax

Implements the UTF-16 positions of spec section 5.7 and the comment-leader table of section 5.6.

**Files:**
- Create: `crates/clippings-core/src/position.rs`, `crates/clippings-core/src/comments.rs`
- Modify: `crates/clippings-core/src/lib.rs` (add both module declarations)

**Interfaces:**
- Produces: `position::Position { line: u32, character: u32 }` (serde, `Ord`), `LineIndex::new(&[u8])`, `line_of(&self, offset) -> usize`, `line_start(&self, line) -> usize`, `line_range(&self, line) -> (usize, usize)` excluding `\n` or `\r\n`, `position(&self, offset) -> Position`, `utf16_len(&[u8]) -> usize`.
- Produces: `comments::CommentSyntax { line: &[&str], block: &[(&str, &str)] }`, `syntax_for(path: &str) -> &'static CommentSyntax`, `strip_line_comment(&str, path) -> String`, `strip_block_comment(&str, path) -> String`.

- [ ] **Step 1: Write the failing tests**

Add `pub mod comments;` and `pub mod position;` to `crates/clippings-core/src/lib.rs`. Create `crates/clippings-core/src/position.rs` containing only:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ascii_positions() {
        let t = b"ab\ncd // TODO x\n";
        let li = LineIndex::new(t);
        assert_eq!(
            li.position(9),
            Position {
                line: 1,
                character: 6
            }
        );
        assert_eq!(li.line_range(1), (3, 15));
    }

    #[test]
    fn non_ascii_counts_utf16_units() {
        let t = "ééé // TODO utf\n😀 // TODO".as_bytes();
        let li = LineIndex::new(t);
        let first = t.windows(4).position(|w| w == b"TODO").unwrap();
        assert_eq!(
            li.position(first),
            Position {
                line: 0,
                character: 7
            }
        );
        let second = first
            + 4
            + t[first + 4..]
                .windows(4)
                .position(|w| w == b"TODO")
                .unwrap();
        assert_eq!(
            li.position(second),
            Position {
                line: 1,
                character: 6
            }
        );
    }

    #[test]
    fn crlf_line_range_excludes_cr() {
        let t = b"a\r\nb";
        let li = LineIndex::new(t);
        assert_eq!(li.line_range(0), (0, 1));
        assert_eq!(li.line_range(1), (3, 4));
    }
}
```

Create `crates/clippings-core/src/comments.rs` containing only:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strips_line_leaders_by_extension() {
        assert_eq!(strip_line_comment("   // more text ", "a.ts"), "more text");
        assert_eq!(strip_line_comment("# more", "a.py"), "more");
        assert_eq!(strip_line_comment("-- more", "q.sql"), "more");
        assert_eq!(strip_line_comment("// kept", "a.unknown"), "// kept");
    }

    #[test]
    fn strips_block_markers() {
        assert_eq!(strip_block_comment("/* TODO x */", "a.c"), "TODO x");
        assert_eq!(strip_block_comment("<!-- TODO y -->", "a.md"), "TODO y");
        assert_eq!(strip_block_comment("{- TODO z -}", "a.hs"), "TODO z");
    }
}
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p clippings-core position comments`
Expected: compile error, `LineIndex` and `strip_line_comment` not found.

- [ ] **Step 3: Write the position implementation**

Put this above the tests in `crates/clippings-core/src/position.rs`:

```rust
//! Byte offsets to LSP positions: 0-based line, 0-based UTF-16 column.

use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct Position {
    pub line: u32,
    pub character: u32,
}

/// Line start offsets of a UTF-8 text.
pub struct LineIndex<'a> {
    text: &'a [u8],
    starts: Vec<usize>,
}

impl<'a> LineIndex<'a> {
    pub fn new(text: &'a [u8]) -> Self {
        let mut starts = vec![0];
        starts.extend(memchr::memchr_iter(b'\n', text).map(|i| i + 1));
        Self { text, starts }
    }

    /// 0-based line containing byte `offset`.
    pub fn line_of(&self, offset: usize) -> usize {
        match self.starts.binary_search(&offset) {
            Ok(l) => l,
            Err(l) => l - 1,
        }
    }

    pub fn line_start(&self, line: usize) -> usize {
        self.starts[line]
    }

    /// Byte range of a line's content, without its `\n` or `\r\n`.
    pub fn line_range(&self, line: usize) -> (usize, usize) {
        let start = self.starts[line];
        let mut end = self.starts.get(line + 1).map_or(self.text.len(), |s| s - 1);
        if end > start && self.text[end - 1] == b'\r' {
            end -= 1;
        }
        (start, end)
    }

    pub fn position(&self, offset: usize) -> Position {
        let line = self.line_of(offset);
        let start = self.starts[line];
        let character = utf16_len(&self.text[start..offset]);
        Position {
            line: line as u32,
            character: character as u32,
        }
    }
}

/// Number of UTF-16 code units in a UTF-8 byte slice (lossy on bad bytes).
pub fn utf16_len(bytes: &[u8]) -> usize {
    String::from_utf8_lossy(bytes).encode_utf16().count()
}
```

- [ ] **Step 4: Write the comment syntax implementation**

Put this above the tests in `crates/clippings-core/src/comments.rs`:

```rust
//! Comment leaders by file extension, a port of the parts of the
//! `comment-patterns` table todo-tree uses: single-line leaders for extra
//! lines and block markers for the primary text.

pub struct CommentSyntax {
    pub line: &'static [&'static str],
    pub block: &'static [(&'static str, &'static str)],
}

const C_LIKE: CommentSyntax = CommentSyntax {
    line: &["//"],
    block: &[("/*", "*/")],
};
const HASH: CommentSyntax = CommentSyntax {
    line: &["#"],
    block: &[],
};
const HTML: CommentSyntax = CommentSyntax {
    line: &[],
    block: &[("<!--", "-->")],
};
const DASH: CommentSyntax = CommentSyntax {
    line: &["--"],
    block: &[],
};
const SEMI: CommentSyntax = CommentSyntax {
    line: &[";"],
    block: &[],
};
const PERCENT: CommentSyntax = CommentSyntax {
    line: &["%"],
    block: &[],
};
const QUOTE: CommentSyntax = CommentSyntax {
    line: &["\""],
    block: &[],
};
const HASKELL: CommentSyntax = CommentSyntax {
    line: &["--"],
    block: &[("{-", "-}")],
};
const PYTHON: CommentSyntax = CommentSyntax {
    line: &["#"],
    block: &[("\"\"\"", "\"\"\""), ("'''", "'''")],
};
const LUA: CommentSyntax = CommentSyntax {
    line: &["--"],
    block: &[("--[[", "]]")],
};
const NONE: CommentSyntax = CommentSyntax {
    line: &[],
    block: &[],
};

pub fn syntax_for(path: &str) -> &'static CommentSyntax {
    let ext = path
        .rsplit_once('.')
        .map(|(_, e)| e.to_ascii_lowercase())
        .unwrap_or_default();
    match ext.as_str() {
        "c" | "h" | "cc" | "cpp" | "cxx" | "hpp" | "cs" | "java" | "js" | "jsx" | "mjs" | "cjs"
        | "ts" | "tsx" | "mts" | "cts" | "jsonc" | "go" | "rs" | "swift" | "kt" | "kts"
        | "scala" | "dart" | "php" | "css" | "scss" | "less" | "groovy" | "gradle" | "proto"
        | "zig" | "sol" | "prisma" => &C_LIKE,
        "py" | "pyw" => &PYTHON,
        "sh" | "bash" | "zsh" | "fish" | "rb" | "pl" | "pm" | "r" | "yaml" | "yml" | "toml"
        | "ini" | "cfg" | "conf" | "dockerfile" | "mk" | "cmake" | "nix" | "ps1" | "tf" | "ex"
        | "exs" | "jl" => &HASH,
        "html" | "htm" | "xml" | "svg" | "vue" | "md" | "markdown" | "svelte" => &HTML,
        "sql" | "ada" | "elm" => &DASH,
        "lua" => &LUA,
        "hs" => &HASKELL,
        "lisp" | "clj" | "cljs" | "el" | "scm" | "asm" => &SEMI,
        "tex" | "erl" | "m" => &PERCENT,
        "vim" => &QUOTE,
        _ => &NONE,
    }
}

/// Trims `text` and strips one leading single-line comment leader.
pub fn strip_line_comment(text: &str, path: &str) -> String {
    let t = text.trim();
    for leader in syntax_for(path).line {
        if let Some(rest) = t.strip_prefix(leader) {
            return rest.trim().to_string();
        }
    }
    t.to_string()
}

/// Strips a leading block-comment opener and trailing closer from `text`.
pub fn strip_block_comment(text: &str, path: &str) -> String {
    let t = text.trim();
    for (open, close) in syntax_for(path).block {
        if let Some(rest) = t.strip_prefix(open) {
            let rest = rest.trim_end();
            return rest.strip_suffix(close).unwrap_or(rest).trim().to_string();
        }
    }
    t.to_string()
}
```

- [ ] **Step 5: Run the tests to verify they pass**

Run: `cargo test -p clippings-core position comments`
Expected: 5 tests pass. Then `cargo fmt --all && cargo clippy --all-targets -- -D warnings` shows nothing.

- [ ] **Step 6: Commit**

```bash
git add -A
git commit -m "feat(core): UTF-16 positions and comment syntax table"
```

### Task 8: Tag extraction

Ports todo-tree's `extractTag` (spec section 5.6). The window starts at the match start and ends at the end of the match's first line. `before` is computed by the scanner in Task 9, because it needs the line text before the match.

**Files:**
- Create: `crates/clippings-core/src/extract.rs`
- Modify: `crates/clippings-core/src/lib.rs` (add the module declaration)

**Interfaces:**
- Consumes: `ScanPattern::{tags, tag_re, sub_tag_re, case_sensitive}`, `comments::syntax_for`.
- Produces: `Extracted { tag: String, tag_range: Option<(usize, usize)>, sub_tag: Option<String>, after: String }`, `extract(&ScanPattern, window: &str, match_len: usize, path: &str) -> Extracted`. `tag_range` is a byte range within `window`.

- [ ] **Step 1: Write the failing tests**

Add `pub mod extract;` to `crates/clippings-core/src/lib.rs`, keeping the declarations sorted. Create `crates/clippings-core/src/extract.rs` containing only this test module for now:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::CoreConfig;
    use crate::pattern::build;

    fn ex(cfg: CoreConfig, window: &str, path: &str) -> Extracted {
        extract(&build(&cfg).unwrap(), window, 0, path)
    }

    #[test]
    fn tag_and_after() {
        let e = ex(CoreConfig::default(), "// TODO fix the thing", "a.ts");
        assert_eq!(e.tag, "TODO");
        assert_eq!(e.tag_range, Some((3, 7)));
        assert_eq!(e.after, "fix the thing");
        assert_eq!(e.sub_tag, None);
    }

    #[test]
    fn configured_spelling_when_case_insensitive() {
        let cfg = CoreConfig {
            regex_case_sensitive: false,
            ..Default::default()
        };
        assert_eq!(ex(cfg, "# todo lower", "a.py").tag, "TODO");
    }

    #[test]
    fn sub_tag_is_group_one_and_removed_from_after() {
        let cfg = CoreConfig {
            sub_tag_regex: r"^\s*\((.*?)\)".into(),
            ..Default::default()
        };
        let e = ex(cfg, "// TODO(alice) ship it", "a.rs");
        assert_eq!(e.sub_tag.as_deref(), Some("alice"));
        assert_eq!(e.after, "ship it");
    }

    #[test]
    fn block_comment_closer_is_stripped() {
        assert_eq!(
            ex(CoreConfig::default(), "/* FIXME later */", "a.c").after,
            "later"
        );
        assert_eq!(
            ex(CoreConfig::default(), "<!-- TODO doc -->", "a.md").after,
            "doc"
        );
    }

    #[test]
    fn without_tags_token_whole_match_is_tag() {
        let cfg = CoreConfig {
            regex: r"NOTE\(\w+\)".into(),
            ..Default::default()
        };
        let p = build(&cfg).unwrap();
        let e = extract(&p, "NOTE(bob) read this", 9, "a.ts");
        assert_eq!(e.tag, "NOTE(bob)");
        assert_eq!(e.after, "read this");
    }

    #[test]
    fn longest_tag_wins() {
        let cfg = CoreConfig {
            tags: vec!["TODO".into(), "TODOS".into()],
            ..Default::default()
        };
        assert_eq!(ex(cfg, "// TODOS list", "a.ts").tag, "TODOS");
    }
}
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p clippings-core extract`
Expected: compile error, `extract` not found.

- [ ] **Step 3: Write the implementation**

Put this above the test module in `crates/clippings-core/src/extract.rs`:

```rust
//! Tag extraction (spec section 5.6), a port of todo-tree's `extractTag`.

use crate::comments::syntax_for;
use crate::pattern::ScanPattern;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Extracted {
    pub tag: String,
    /// Byte range of the tag within the window.
    pub tag_range: Option<(usize, usize)>,
    pub sub_tag: Option<String>,
    pub after: String,
}

fn configured_spelling(p: &ScanPattern, found: &str) -> String {
    p.tags
        .iter()
        .find(|t| {
            if p.case_sensitive {
                t.as_str() == found
            } else {
                t.to_lowercase() == found.to_lowercase()
            }
        })
        .cloned()
        .unwrap_or_else(|| found.to_string())
}

/// `window` starts at the match start and ends at the end of the match's
/// first line, capped. `match_len` is the match length within the window.
pub fn extract(p: &ScanPattern, window: &str, match_len: usize, path: &str) -> Extracted {
    let (tag, tag_range, right) = match &p.tag_re {
        Some(re) => match re.find(window) {
            Some(m) => (
                configured_spelling(p, m.as_str()),
                Some((m.start(), m.end())),
                &window[m.end()..],
            ),
            None => (String::new(), None, window),
        },
        None => {
            let end = match_len.min(window.len());
            let end = (0..=end)
                .rev()
                .find(|i| window.is_char_boundary(*i))
                .unwrap_or(0);
            (
                window[..end].trim().to_string(),
                Some((0, end)),
                &window[end..],
            )
        }
    };
    let mut right = right.trim().to_string();
    // A block comment opened before the tag closes at the end of the line.
    let opened_before = &window[..tag_range.map_or(0, |r| r.0)];
    for (open, close) in syntax_for(path).block {
        if opened_before.contains(open) {
            if let Some(stripped) = right.strip_suffix(close) {
                right = stripped.trim_end().to_string();
            }
        }
    }
    let (sub_tag, after) = match &p.sub_tag_re {
        Some(re) => {
            let sub = re
                .captures(&right)
                .and_then(|c| c.get(1))
                .map(|m| m.as_str().to_string());
            (sub, re.replace(&right, "").trim().to_string())
        }
        None => (None, right),
    };
    Extracted {
        tag,
        tag_range,
        sub_tag,
        after,
    }
}
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test -p clippings-core extract`
Expected: all tests in the module pass.
Then run: `cargo fmt --all && cargo clippy --all-targets -- -D warnings`
Expected: no diffs, no warnings.

- [ ] **Step 5: Commit**

```bash
git add -A
git commit -m "feat(core): tag extraction"
```

### Task 9: Scanner and scan results

Implements spec section 5.5. grep-searcher reports whole matching lines or merged line groups, not match offsets, so the sink re-runs the matcher inside each `SinkMatch`. For a multi-line match the primary text is the match's first line and the following lines become extra lines. Binary content is detected by `decode` over the whole buffer, and by `BinaryDetection::quit` when streaming files over 64 MiB.

**Files:**
- Create: `crates/clippings-core/src/model.rs`, `crates/clippings-core/src/scanner.rs`
- Modify: `crates/clippings-core/src/lib.rs` (add `model` and `scanner`)

**Interfaces:**
- Consumes: `ScanPattern`, `Engine`, `extract`, `LineIndex`, `strip_line_comment`, `Fs`.
- Produces: `model::Todo { start, end: Position, tag_start, tag_end: Option<Position>, tag: String, sub_tag: Option<String>, before: String, after: String, extra_lines: Vec<ExtraLine> }`, `model::ExtraLine { line: u32, text: String }`, `model::FileResult { path: PathBuf, todos: Vec<Todo> }`, all serde camelCase.
- Produces: `scanner::HEAP_LIMIT = 64 MiB`, `scanner::WINDOW_CAP = 1000`, `decode(Vec<u8>) -> Option<Vec<u8>>`, `scan_text(&ScanPattern, text: &[u8], path: &str) -> Vec<Todo>`, `scan_file(&dyn Fs, &ScanPattern, &Path) -> io::Result<Option<Vec<Todo>>>` where `Ok(None)` means binary or skipped.

- [ ] **Step 1: Create the result types**

`crates/clippings-core/src/model.rs`:

```rust
//! Scan results.

use crate::position::Position;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExtraLine {
    pub line: u32,
    pub text: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Todo {
    /// Start of the regex match.
    pub start: Position,
    /// End of the regex match.
    pub end: Position,
    /// The tag's range, when a tag was found.
    pub tag_start: Option<Position>,
    pub tag_end: Option<Position>,
    /// Configured spelling of the tag, or the whole match without `$TAGS`; empty if none.
    pub tag: String,
    pub sub_tag: Option<String>,
    pub before: String,
    pub after: String,
    /// Continuation lines of a multi-line match, comment-stripped.
    pub extra_lines: Vec<ExtraLine>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FileResult {
    pub path: PathBuf,
    pub todos: Vec<Todo>,
}
```

- [ ] **Step 2: Write the failing tests**

Add `pub mod model;` and `pub mod scanner;` to `lib.rs`. Create `crates/clippings-core/src/scanner.rs` containing only this test module. The last four tests cover Review Focus items 1 to 4:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::CoreConfig;
    use crate::pattern::build;
    use crate::position::Position;

    fn scan(cfg: CoreConfig, text: &str, path: &str) -> Vec<Todo> {
        scan_text(&build(&cfg).unwrap(), text.as_bytes(), path)
    }

    #[test]
    fn two_tags_on_one_line_give_two_todos() {
        let t = scan(CoreConfig::default(), "a // TODO one # FIXME two\n", "a.py");
        assert_eq!(t.len(), 2);
        assert_eq!((t[0].tag.as_str(), t[1].tag.as_str()), ("TODO", "FIXME"));
        assert_eq!(
            t[1].start,
            Position {
                line: 0,
                character: 14
            }
        );
    }

    #[test]
    fn positions_and_before_after() {
        let t = scan(
            CoreConfig::default(),
            "x\nlet y = 1; // TODO rename y\n",
            "a.ts",
        );
        assert_eq!(t.len(), 1);
        assert_eq!(
            t[0].start,
            Position {
                line: 1,
                character: 11
            }
        );
        assert_eq!(
            t[0].tag_start,
            Some(Position {
                line: 1,
                character: 14
            })
        );
        assert_eq!(t[0].before, "let y = 1;");
        assert_eq!(t[0].after, "rename y");
    }

    #[test]
    fn crlf_is_not_part_of_after() {
        let t = scan(CoreConfig::default(), "// TODO a\r\n// TODO b\r\n", "a.ts");
        assert_eq!(
            t.iter().map(|t| t.after.as_str()).collect::<Vec<_>>(),
            vec!["a", "b"]
        );
    }

    #[test]
    fn non_ascii_before_tag_uses_utf16_columns() {
        let t = scan(CoreConfig::default(), "ééé // TODO utf\n", "a.ts");
        assert_eq!(
            t[0].start,
            Position {
                line: 0,
                character: 4
            }
        );
        assert_eq!(
            t[0].tag_start,
            Some(Position {
                line: 0,
                character: 7
            })
        );
    }

    #[test]
    fn long_lines_are_capped() {
        let text = format!("// TODO {}\n", "x".repeat(5000));
        let t = scan(CoreConfig::default(), &text, "a.ts");
        assert!(t[0].after.len() <= WINDOW_CAP);
        assert!(t[0].after.starts_with("xxx"));
    }

    #[test]
    fn multi_line_match_has_extra_lines() {
        let cfg = CoreConfig {
            regex: r"(//)\s*($TAGS).*(\n\s*//\s{2,}.*)*".into(),
            ..Default::default()
        };
        let t = scan(
            cfg,
            "// TODO first\n//   second\n//   third\ncode\n",
            "a.ts",
        );
        assert_eq!(t.len(), 1);
        assert_eq!(t[0].after, "first");
        assert_eq!(
            t[0].extra_lines,
            vec![
                ExtraLine {
                    line: 1,
                    text: "second".into()
                },
                ExtraLine {
                    line: 2,
                    text: "third".into()
                },
            ]
        );
    }

    #[test]
    fn adjacent_multi_line_matches_stay_separate() {
        let cfg = CoreConfig {
            regex: r"(//)\s*($TAGS).*(\n\s*//\s{2,}.*)*".into(),
            ..Default::default()
        };
        let t = scan(cfg, "// TODO a\n//   a2\n// FIXME b\n//   b2\n", "a.ts");
        assert_eq!(t.len(), 2);
        assert_eq!(t[1].start.line, 2);
        assert_eq!(t[1].extra_lines[0].text, "b2");
    }

    #[test]
    fn decode_detects_binary_anywhere() {
        let mut b = vec![b'a'; 70_000];
        b.extend_from_slice(b"\0 // TODO");
        assert!(decode(b).is_none());
        assert!(decode(b"// TODO".to_vec()).is_some());
    }

    #[test]
    fn decode_transcodes_utf16_and_strips_bom() {
        let mut b = vec![0xFF, 0xFE];
        for u in "// TODO wide".encode_utf16() {
            b.extend_from_slice(&u.to_le_bytes());
        }
        assert_eq!(decode(b).unwrap(), b"// TODO wide");
        assert_eq!(decode(b"\xEF\xBB\xBF// TODO".to_vec()).unwrap(), b"// TODO");
    }

    #[test]
    fn markdown_checkbox_tag() {
        let t = scan(
            CoreConfig::default(),
            "- [ ] write docs\n- [x] done\n",
            "a.md",
        );
        assert_eq!(
            t.iter()
                .map(|t| (t.tag.as_str(), t.after.as_str()))
                .collect::<Vec<_>>(),
            vec![("[ ]", "write docs"), ("[x]", "done")]
        );
    }

    #[test]
    fn latin1_bytes_do_not_hide_todos() {
        let p = build(&CoreConfig::default()).unwrap();
        let t = scan_text(&p, b"caf\xe9 // TODO latin1\n", "a.c");
        assert_eq!(t.len(), 1);
        assert_eq!(t[0].after, "latin1");
    }

    #[test]
    fn tag_at_end_of_file_without_newline() {
        let t = scan(CoreConfig::default(), "x\n// TODO last", "a.ts");
        assert_eq!(t.len(), 1);
        assert_eq!(t[0].after, "last");
        assert!(scan(CoreConfig::default(), "", "a.ts").is_empty());
    }

    #[test]
    fn empty_matching_regex_terminates() {
        let cfg = CoreConfig {
            regex: "($TAGS)?".into(),
            ..Default::default()
        };
        let t = scan(cfg, "abc\n// TODO x\n", "a.ts");
        assert_eq!(
            t.iter().map(|t| t.tag.as_str()).collect::<Vec<_>>(),
            vec!["TODO"]
        );
    }

    #[test]
    fn files_over_the_heap_limit_are_streamed_in_line_mode() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("big.txt");
        let mut body = "// TODO first\n".to_string();
        let filler = format!("{}\n", "a".repeat(1023));
        while body.len() <= HEAP_LIMIT {
            body.push_str(&filler);
        }
        body.push_str("// FIXME last\n");
        std::fs::write(&path, &body).unwrap();
        let p = build(&CoreConfig::default()).unwrap();
        let t = scan_file(&crate::fs::NativeFs, &p, &path).unwrap().unwrap();
        assert_eq!(
            t.iter().map(|t| t.tag.as_str()).collect::<Vec<_>>(),
            vec!["TODO", "FIXME"]
        );
        assert_eq!(t[1].start.line as usize, body.lines().count() - 1);
        let multi = build(&CoreConfig {
            enable_multi_line: true,
            ..Default::default()
        })
        .unwrap();
        assert!(scan_file(&crate::fs::NativeFs, &multi, &path)
            .unwrap()
            .is_none());
    }
}
```

- [ ] **Step 3: Run the tests to verify they fail**

Run: `cargo test -p clippings-core scanner`
Expected: compile error, `scan_text`, `decode` and `WINDOW_CAP` not found.

- [ ] **Step 4: Write the implementation**

Put this above the tests in `crates/clippings-core/src/scanner.rs`:

```rust
//! Scanning one file's contents into todos (spec section 5.5).

use crate::comments::strip_line_comment;
use crate::extract::extract;
use crate::fs::Fs;
use crate::model::{ExtraLine, Todo};
use crate::pattern::{Engine, ScanPattern};
use crate::position::LineIndex;
use grep_matcher::{LineTerminator, Matcher};
use grep_searcher::{BinaryDetection, MmapChoice, Searcher, SearcherBuilder, Sink, SinkMatch};
use std::io;
use std::path::Path;

/// Files larger than this are streamed in line mode and skipped in multi-line mode.
pub const HEAP_LIMIT: usize = 64 * 1024 * 1024;
/// Label text is capped this many bytes after the match start.
pub const WINDOW_CAP: usize = 1000;

/// Decodes file bytes to UTF-8: transcodes UTF-16 with a BOM, strips a UTF-8
/// BOM. Returns `None` for binary content (a NUL byte anywhere).
pub fn decode(bytes: Vec<u8>) -> Option<Vec<u8>> {
    let text = if bytes.starts_with(&[0xFF, 0xFE]) || bytes.starts_with(&[0xFE, 0xFF]) {
        let enc = if bytes[0] == 0xFF {
            encoding_rs::UTF_16LE
        } else {
            encoding_rs::UTF_16BE
        };
        enc.decode_with_bom_removal(&bytes)
            .0
            .into_owned()
            .into_bytes()
    } else if bytes.starts_with(&[0xEF, 0xBB, 0xBF]) {
        bytes[3..].to_vec()
    } else {
        bytes
    };
    memchr::memchr(0, &text).is_none().then_some(text)
}

fn floor_char_boundary(b: &[u8], mut i: usize) -> usize {
    i = i.min(b.len());
    while i > 0 && i < b.len() && (b[i] & 0b1100_0000) == 0b1000_0000 {
        i -= 1;
    }
    i
}

fn ceil_char_boundary(b: &[u8], mut i: usize) -> usize {
    while i < b.len() && (b[i] & 0b1100_0000) == 0b1000_0000 {
        i += 1;
    }
    i
}

/// Builds todos from one searcher match: `bytes` starts at a line start,
/// `first_line` is its 0-based line number.
fn todos_from_lines(
    p: &ScanPattern,
    bytes: &[u8],
    first_line: u32,
    path: &str,
    out: &mut Vec<Todo>,
) {
    let li = LineIndex::new(bytes);
    let shift = |mut pos: crate::position::Position| {
        pos.line += first_line;
        pos
    };
    let _ = p.matcher.find_iter(bytes, |m| {
        let (s, e) = (m.start(), m.end());
        if s == e || (!p.multi_line && bytes[s..e].contains(&b'\n')) {
            return true;
        }
        let start_line = li.line_of(s);
        let end_line = li.line_of(e - 1);
        let (_, first_end) = li.line_range(start_line);
        let win_end = floor_char_boundary(bytes, first_end.max(s).min(s + WINDOW_CAP));
        let window = String::from_utf8_lossy(&bytes[s..win_end]);
        let match_len = e.min(win_end) - s;
        let ex = extract(p, &window, match_len, path);

        let (line_start, _) = li.line_range(start_line);
        let before_start = ceil_char_boundary(bytes, s.saturating_sub(WINDOW_CAP).max(line_start));
        let before = String::from_utf8_lossy(&bytes[before_start..s])
            .trim()
            .to_string();

        let extra_lines = if end_line > start_line {
            (start_line + 1..=end_line)
                .filter_map(|l| {
                    let (a, b) = li.line_range(l);
                    let b = floor_char_boundary(bytes, b.min(a + WINDOW_CAP));
                    let text = strip_line_comment(&String::from_utf8_lossy(&bytes[a..b]), path);
                    (!text.is_empty() && text != ex.tag).then(|| ExtraLine {
                        line: l as u32 + first_line,
                        text,
                    })
                })
                .collect()
        } else {
            Vec::new()
        };
        // Window bytes equal source bytes unless lossy conversion changed them.
        let tag_pos = ex
            .tag_range
            .filter(|_| window.len() == win_end - s)
            .map(|(a, b)| (shift(li.position(s + a)), shift(li.position(s + b))));
        out.push(Todo {
            start: shift(li.position(s)),
            end: shift(li.position(e)),
            tag_start: tag_pos.map(|t| t.0),
            tag_end: tag_pos.map(|t| t.1),
            tag: ex.tag,
            sub_tag: ex.sub_tag,
            before,
            after: ex.after,
            extra_lines,
        });
        true
    });
}

struct TodoSink<'a> {
    pattern: &'a ScanPattern,
    path: &'a str,
    todos: Vec<Todo>,
    binary: bool,
}

impl Sink for TodoSink<'_> {
    type Error = io::Error;

    fn matched(&mut self, _: &Searcher, m: &SinkMatch<'_>) -> Result<bool, io::Error> {
        let first_line = m.line_number().unwrap_or(1).saturating_sub(1) as u32;
        todos_from_lines(
            self.pattern,
            m.bytes(),
            first_line,
            self.path,
            &mut self.todos,
        );
        Ok(true)
    }

    fn binary_data(&mut self, _: &Searcher, _: u64) -> Result<bool, io::Error> {
        self.binary = true;
        Ok(false)
    }
}

fn searcher(p: &ScanPattern) -> Searcher {
    let term = match (p.multi_line, p.engine) {
        (true, _) | (false, Engine::Fancy) => LineTerminator::byte(b'\n'),
        (false, Engine::Grep) => LineTerminator::crlf(),
    };
    SearcherBuilder::new()
        .line_number(true)
        .multi_line(p.multi_line)
        .line_terminator(term)
        .memory_map(MmapChoice::never())
        .binary_detection(BinaryDetection::quit(b'\x00'))
        .heap_limit(Some(HEAP_LIMIT))
        .build()
}

/// Scans decoded UTF-8 text held in memory, such as an open buffer.
pub fn scan_text(p: &ScanPattern, text: &[u8], path: &str) -> Vec<Todo> {
    let mut sink = TodoSink {
        pattern: p,
        path,
        todos: Vec::new(),
        binary: false,
    };
    let mut s = searcher(p);
    // Binary detection was done by `decode`; the searcher only sees text.
    s.set_binary_detection(BinaryDetection::none());
    let _ = s.search_slice(&p.matcher, text, &mut sink);
    sink.todos
}

/// Scans a file on disk. `Ok(None)` means binary or skipped.
pub fn scan_file(fs: &dyn Fs, p: &ScanPattern, path: &Path) -> io::Result<Option<Vec<Todo>>> {
    let path_str = path.to_string_lossy();
    let len = fs.len(path)? as usize;
    if len <= HEAP_LIMIT {
        return Ok(decode(fs.read(path)?).map(|text| scan_text(p, &text, &path_str)));
    }
    if p.multi_line {
        tracing::debug!("skipping {path_str}: larger than the multi-line heap limit");
        return Ok(None);
    }
    let file = std::fs::File::open(path)?;
    let mut sink = TodoSink {
        pattern: p,
        path: &path_str,
        todos: Vec::new(),
        binary: false,
    };
    if let Err(e) = searcher(p).search_file(&p.matcher, &file, &mut sink) {
        tracing::debug!("skipping {path_str}: {e}");
        return Ok(None);
    }
    Ok((!sink.binary).then_some(sink.todos))
}
```

- [ ] **Step 5: Run the tests to verify they pass**

Run: `cargo test -p clippings-core scanner`
Expected: 14 tests pass. The heap-limit test writes a 64 MiB temporary file and takes a few seconds. Then `cargo fmt --all && cargo clippy --all-targets -- -D warnings` shows nothing.

- [ ] **Step 6: Commit**

```bash
git add -A
git commit -m "feat(core): scanner with per-match offsets, binary detection and streaming"
```

### Task 10: Parallel walker, scan report and golden tests

Implements the walk of spec section 5.3 and the fixture and golden tests of section 12.2. The fixture is generated in a temporary directory at test time, because it contains nested git repositories and binary files that cannot be committed as a plain directory tree. It covers every fixture case the spec lists except notebooks, which plan 4 adds.

**Files:**
- Create: `crates/clippings-core/src/walker.rs`, `crates/clippings-core/src/report.rs`
- Create: `crates/clippings-core/tests/support/mod.rs`, `crates/clippings-core/tests/golden.rs`
- Create (generated): `crates/clippings-core/tests/snapshots/golden__default_scan.snap`, `crates/clippings-core/tests/snapshots/golden__sub_tag_multi_line_scan.snap`
- Modify: `crates/clippings-core/src/lib.rs` (add `report` and `walker`)

**Interfaces:**
- Consumes: `rel_components`, `GlobLayers`, `BuiltInExcludes`, `deepest_root`, `slash_path`, `scan_file`, `pattern::build`, `Admission` (tests).
- Produces: `walker::walk_and_scan(&CoreConfig, roots: &[PathBuf], &ScanPattern, Arc<dyn Fs>, cancel: &AtomicBool) -> Result<WalkOutcome, CoreError>`, `WalkOutcome { files: Vec<FileResult>, seen: Vec<PathBuf>, cancelled: bool }` where `files` holds files with todos and `seen` every admitted text file, both sorted.
- Produces: `report::scan_report(&[PathBuf], &CoreConfig, Arc<dyn Fs>) -> Result<ScanReport, CoreError>`, `ScanReport { files: Vec<ReportFile> }`, `ReportFile { path: String, todos: Vec<Todo> }` with paths relative to the deepest root and `/` separators.

- [ ] **Step 1: Write the fixture builder**

`crates/clippings-core/tests/support/mod.rs`:

```rust
//! Builds the fixture workspace from spec section 12.2 in a temporary directory.

use std::fs;
use std::path::Path;
use std::process::Command;

fn write(root: &Path, rel: &str, contents: impl AsRef<[u8]>) {
    let p = root.join(rel);
    fs::create_dir_all(p.parent().unwrap()).unwrap();
    fs::write(p, contents).unwrap();
}

fn git_init(dir: &Path) {
    let ok = Command::new("git")
        .args(["init", "-q"])
        .current_dir(dir)
        .status()
        .unwrap()
        .success();
    assert!(ok, "git init failed in {}", dir.display());
}

pub fn build_fixture(root: &Path) {
    git_init(root);
    write(root, ".gitignore", "dist/\n*.log\n");
    write(
        root,
        "src/main.ts",
        "export const x = 1; // TODO rename x\n/* FIXME block comment */\n",
    );
    write(
        root,
        "src/lib.rs",
        "fn a() {} // TODO(alice) sub tag\n// HACK: two // XXX on one line\n",
    );
    write(
        root,
        "src/app.py",
        "# BUG crashes on empty input\nprint('no tag')\n",
    );
    write(
        root,
        "src/unicode.ts",
        "const é = 'ééé'; // TODO after non-ascii\n",
    );
    write(
        root,
        "src/crlf.ts",
        "// TODO windows line\r\nconst y = 2;\r\n",
    );
    write(
        root,
        "src/long.ts",
        format!("// TODO {}\n", "long ".repeat(400)),
    );
    write(root, "src/multi.ts", "// TODO first line\n//   second line\n//   third line\n// FIXME next\n//   next continued\n");
    write(
        root,
        "docs/notes.md",
        "# Notes\n- [ ] write docs\n- [x] done item\n<!-- TODO html comment -->\n",
    );
    write(root, "app/[slug]/page.tsx", "// TODO dynamic route\n");
    write(root, "dist/bundle.js", "// TODO ignored by gitignore\n");
    write(root, "debug.log", "TODO ignored log\n");
    write(
        root,
        "node_modules/pkg/index.js",
        "// TODO in node_modules\n",
    );
    write(root, ".claude/notes.md", "- [ ] hidden and built-in\n");
    write(
        root,
        ".github/workflows/ci.yml",
        "# TODO hidden directory\n",
    );
    let mut bin = b"\x00\x01binary // TODO in binary".to_vec();
    bin.extend_from_slice(b"\n");
    write(root, "assets/early-nul.bin", bin);
    let mut late = b"// TODO before late nul\n".to_vec();
    late.extend(std::iter::repeat_n(b'a', 70_000));
    late.extend_from_slice(b"\n\x00\n// TODO after late nul\n");
    write(root, "assets/late-nul.dat", late);
    let nested = root.join("vendor/nested");
    fs::create_dir_all(&nested).unwrap();
    git_init(&nested);
    write(root, "vendor/nested/lib.c", "/* TODO nested repo */\n");
}
```

- [ ] **Step 2: Write the failing tests**

`crates/clippings-core/tests/golden.rs`. The third test is the check that the walker and the admission predicate agree on every non-binary fixture file, under four configurations:

```rust
mod support;

use clippings_core::admission::Admission;
use clippings_core::config::CoreConfig;
use clippings_core::fs::NativeFs;
use clippings_core::pattern;
use clippings_core::report::scan_report;
use clippings_core::walker::walk_and_scan;
use std::path::PathBuf;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;

fn fixture() -> (tempfile::TempDir, PathBuf) {
    let t = tempfile::tempdir().unwrap();
    let root = dunce::canonicalize(t.path()).unwrap();
    support::build_fixture(&root);
    (t, root)
}

#[test]
fn default_scan_matches_golden() {
    let (_t, root) = fixture();
    let report = scan_report(&[root], &CoreConfig::default(), Arc::new(NativeFs)).unwrap();
    insta::assert_json_snapshot!("default_scan", report);
}

#[test]
fn sub_tag_and_multi_line_scan_matches_golden() {
    let (_t, root) = fixture();
    let cfg = CoreConfig {
        sub_tag_regex: r"^\s*\((.*?)\)".into(),
        regex: r"(//|#|<!--|;|/\*|^|^[ \t]*(-|\d+.))\s*($TAGS).*(\n\s*//\s{2,}.*)*".into(),
        ..Default::default()
    };
    let report = scan_report(&[root], &cfg, Arc::new(NativeFs)).unwrap();
    insta::assert_json_snapshot!("sub_tag_multi_line_scan", report);
}

#[test]
fn walk_and_admission_agree_on_every_file() {
    let (_t, root) = fixture();
    for cfg in [
        CoreConfig::default(),
        CoreConfig {
            include_hidden_files: true,
            ..Default::default()
        },
        CoreConfig {
            ignore_git_submodules: true,
            ..Default::default()
        },
        CoreConfig {
            built_in_excludes: vec![],
            include_hidden_files: true,
            ..Default::default()
        },
    ] {
        let p = pattern::build(&cfg).unwrap();
        let fs = Arc::new(NativeFs);
        let walk = walk_and_scan(
            &cfg,
            std::slice::from_ref(&root),
            &p,
            fs.clone(),
            &AtomicBool::new(false),
        )
        .unwrap();
        let admission = Admission::new(&cfg, vec![root.clone()], fs).unwrap();
        for entry in walkdir(&root) {
            let binary = entry.extension().is_some_and(|e| e == "bin" || e == "dat");
            let admitted = admission.admits_disk(&entry);
            let walked = walk.seen.binary_search(&entry).is_ok();
            if !binary {
                assert_eq!(
                    admitted,
                    walked,
                    "{} under {:?}",
                    entry.display(),
                    cfg.built_in_excludes.len()
                );
            }
        }
    }
}

fn walkdir(root: &std::path::Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(d) = stack.pop() {
        for e in std::fs::read_dir(&d).unwrap() {
            let p = e.unwrap().path();
            if p.file_name().is_some_and(|n| n == ".git") {
                continue;
            }
            if p.is_dir() {
                stack.push(p)
            } else {
                out.push(p)
            }
        }
    }
    out
}
```

- [ ] **Step 3: Run the tests to verify they fail**

Run: `cargo test -p clippings-core --test golden`
Expected: compile error, unresolved imports `clippings_core::report` and `clippings_core::walker`.

- [ ] **Step 4: Write the walker**

Add `pub mod report;` and `pub mod walker;` to `lib.rs`. `crates/clippings-core/src/walker.rs`:

```rust
//! Parallel walk and scan of the walked roots (spec section 5.3 and 5.5).

use crate::admission::rel_components;
use crate::config::CoreConfig;
use crate::fs::Fs;
use crate::globs::{slash_path, BuiltInExcludes, GlobLayers};
use crate::model::FileResult;
use crate::pattern::ScanPattern;
use crate::roots::deepest_root;
use crate::scanner::scan_file;
use crate::CoreError;
use ignore::{DirEntry, WalkBuilder, WalkState};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

pub struct WalkOutcome {
    /// Files with at least one todo, sorted by path.
    pub files: Vec<FileResult>,
    /// Every admitted text file seen, sorted, including files without todos.
    pub seen: Vec<PathBuf>,
    pub cancelled: bool,
}

struct EntryFilter {
    roots: Vec<PathBuf>,
    layers: GlobLayers,
    built_in: BuiltInExcludes,
    ignore_submodules: bool,
}

impl EntryFilter {
    fn keep(&self, e: &DirEntry) -> bool {
        if e.depth() == 0 {
            return true;
        }
        let path = e.path();
        let Some(root) = deepest_root(path, &self.roots) else {
            return true;
        };
        let rel = rel_components(path, root);
        let rel_refs: Vec<&str> = rel.iter().map(String::as_str).collect();
        let rel_str = path.strip_prefix(root).ok().map(slash_path);
        let abs = slash_path(path);
        if e.file_type().is_some_and(|t| t.is_dir()) {
            if self.built_in.dir_excluded(&rel_refs)
                || self.layers.dir_pruned(&abs, rel_str.as_deref())
            {
                return false;
            }
            return !(self.ignore_submodules && path.join(".git").exists());
        }
        self.layers.file_allowed(&abs, rel_str.as_deref())
    }
}

pub fn walk_and_scan(
    cfg: &CoreConfig,
    roots: &[PathBuf],
    pattern: &ScanPattern,
    fs: Arc<dyn Fs>,
    cancel: &AtomicBool,
) -> Result<WalkOutcome, CoreError> {
    if roots.is_empty() {
        return Ok(WalkOutcome {
            files: Vec::new(),
            seen: Vec::new(),
            cancelled: false,
        });
    }
    let filter = Arc::new(EntryFilter {
        roots: roots.to_vec(),
        layers: GlobLayers::new(cfg)?,
        built_in: BuiltInExcludes::new(&cfg.built_in_excludes),
        ignore_submodules: cfg.ignore_git_submodules,
    });
    let mut builder = WalkBuilder::new(&roots[0]);
    for r in &roots[1..] {
        builder.add(r);
    }
    builder
        .hidden(!cfg.include_hidden_files)
        .add_custom_ignore_filename(".rgignore")
        .threads(std::thread::available_parallelism().map_or(4, |n| n.get()));
    if !cfg.respect_ignore_files {
        builder
            .ignore(false)
            .git_ignore(false)
            .git_exclude(false)
            .git_global(false)
            .parents(false);
    }
    let f = filter.clone();
    builder.filter_entry(move |e| f.keep(e));

    let results: Mutex<Vec<FileResult>> = Mutex::new(Vec::new());
    let seen: Mutex<Vec<PathBuf>> = Mutex::new(Vec::new());
    let cancelled = AtomicBool::new(false);
    builder.build_parallel().run(|| {
        let fs = fs.clone();
        let (results, seen, cancelled) = (&results, &seen, &cancelled);
        Box::new(move |entry| {
            if cancel.load(Ordering::Relaxed) {
                cancelled.store(true, Ordering::Relaxed);
                return WalkState::Quit;
            }
            let Ok(entry) = entry else {
                return WalkState::Continue;
            };
            if !entry.file_type().is_some_and(|t| t.is_file()) {
                return WalkState::Continue;
            }
            match scan_file(fs.as_ref(), pattern, entry.path()) {
                Ok(Some(todos)) => {
                    seen.lock().unwrap().push(entry.path().to_path_buf());
                    if !todos.is_empty() {
                        results.lock().unwrap().push(FileResult {
                            path: entry.path().to_path_buf(),
                            todos,
                        });
                    }
                }
                Ok(None) => {}
                Err(e) => tracing::debug!("skipping {}: {e}", entry.path().display()),
            }
            WalkState::Continue
        })
    });
    let mut files = results.into_inner().unwrap();
    files.sort_by(|a, b| a.path.cmp(&b.path));
    let mut seen = seen.into_inner().unwrap();
    seen.sort();
    Ok(WalkOutcome {
        files,
        seen,
        cancelled: cancelled.into_inner(),
    })
}
```

- [ ] **Step 5: Write the report**

`crates/clippings-core/src/report.rs`:

```rust
//! The `clippings scan --json` report.

use crate::config::CoreConfig;
use crate::fs::Fs;
use crate::globs::slash_path;
use crate::model::Todo;
use crate::pattern;
use crate::roots::deepest_root;
use crate::walker::walk_and_scan;
use crate::CoreError;
use serde::Serialize;
use std::path::PathBuf;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;

#[derive(Debug, Serialize)]
pub struct ScanReport {
    pub files: Vec<ReportFile>,
}

#[derive(Debug, Serialize)]
pub struct ReportFile {
    /// Path relative to its root, with `/` separators.
    pub path: String,
    pub todos: Vec<Todo>,
}

pub fn scan_report(
    roots: &[PathBuf],
    cfg: &CoreConfig,
    fs: Arc<dyn Fs>,
) -> Result<ScanReport, CoreError> {
    let pattern = pattern::build(cfg)?;
    let outcome = walk_and_scan(cfg, roots, &pattern, fs, &AtomicBool::new(false))?;
    let files = outcome
        .files
        .into_iter()
        .map(|f| {
            let rel = deepest_root(&f.path, roots)
                .and_then(|r| f.path.strip_prefix(r).ok())
                .map(slash_path)
                .unwrap_or_else(|| slash_path(&f.path));
            ReportFile {
                path: rel,
                todos: f.todos,
            }
        })
        .collect();
    Ok(ScanReport { files })
}
```

- [ ] **Step 6: Record and review the golden snapshots**

Run: `INSTA_UPDATE=always cargo test -p clippings-core --test golden`
Expected: 3 tests pass and two `.snap` files are written under `crates/clippings-core/tests/snapshots/`.

Review them before committing. A snapshot records whatever the code did, bugs included. Print a summary:

```bash
python3 - <<'EOF'
import json, pathlib
for f in sorted(pathlib.Path("crates/clippings-core/tests/snapshots").glob("golden__*.snap")):
    d = json.loads(f.read_text().split("---", 2)[2])
    print("==", f.name)
    for fl in d["files"]:
        for t in fl["todos"]:
            print(f'{fl["path"]}:{t["start"]["line"]+1}:{t["start"]["character"]+1} {t["tag"]!r} sub={t["subTag"]!r} after={t["after"][:30]!r} extra={[e["text"] for e in t["extraLines"]]}')
EOF
```

Expected for `golden__default_scan.snap`, exactly these 16 todos:

```text
app/[slug]/page.tsx:1:1 'TODO' sub=None after='dynamic route' extra=[]
docs/notes.md:2:1 '[ ]' sub=None after='write docs' extra=[]
docs/notes.md:3:1 '[x]' sub=None after='done item' extra=[]
docs/notes.md:4:1 'TODO' sub=None after='html comment' extra=[]
src/app.py:1:1 'BUG' sub=None after='crashes on empty input' extra=[]
src/crlf.ts:1:1 'TODO' sub=None after='windows line' extra=[]
src/lib.rs:1:11 'TODO' sub=None after='(alice) sub tag' extra=[]
src/lib.rs:2:1 'HACK' sub=None after=': two // XXX on one line' extra=[]
src/lib.rs:2:14 'XXX' sub=None after='on one line' extra=[]
src/long.ts:1:1 'TODO' sub=None after='long long long long long long ' extra=[]
src/main.ts:1:21 'TODO' sub=None after='rename x' extra=[]
src/main.ts:2:1 'FIXME' sub=None after='block comment' extra=[]
src/multi.ts:1:1 'TODO' sub=None after='first line' extra=[]
src/multi.ts:4:1 'FIXME' sub=None after='next' extra=[]
src/unicode.ts:1:18 'TODO' sub=None after='after non-ascii' extra=[]
vendor/nested/lib.c:1:1 'TODO' sub=None after='nested repo' extra=[]
```

Expected for `golden__sub_tag_multi_line_scan.snap`: the same list except that `src/lib.rs:1:11` has `sub='alice'` and `after='sub tag'`, the `XXX` todo on `src/lib.rs:2` is absent because the greedy `.*` of that regex consumes it, `src/multi.ts:1:1` has `extra=['second line', 'third line']`, and `src/multi.ts:4:1` has `extra=['next continued']`.

Nothing from `dist/`, `debug.log`, `node_modules/`, `.claude/`, `.github/` or `assets/` may appear. If the output differs, fix the code, not the expectation.

- [ ] **Step 7: Run all tests**

Run: `cargo test -p clippings-core`
Expected: all unit and golden tests pass. Then `cargo fmt --all && cargo clippy --all-targets -- -D warnings` shows nothing.

- [ ] **Step 8: Commit**

```bash
git add -A
git commit -m "feat(core): parallel walker, scan report and golden tests"
```

### Task 11: Index and effective results

Implements spec sections 5.8 and 5.9. Buffers are keyed by the exact URI string, so notebook cells sharing one path stay separate. `apply_walk` implements the cancelled-rescan rule of section 5.10: entries for unseen files are removed only when the walk completed.

**Files:**
- Create: `crates/clippings-core/src/index.rs`
- Modify: `crates/clippings-core/src/lib.rs` (add the module declaration)

**Interfaces:**
- Consumes: `ScanMode`, `Todo`, `FileResult`, `Position` (tests).
- Produces: `BufferEntry { uri: String, path: Option<PathBuf>, version: i32, todos: Vec<Todo> }`, `SourcedTodo { buffer_uri: Option<String>, todo: Todo }`, `Source { Disk, Buffers }`, `EffectiveFile { path: Option<PathBuf>, uri: Option<String>, source: Source, todos: Vec<SourcedTodo> }`, `EffectiveContext { mode: ScanMode, walked_roots: &[PathBuf], active_uri: Option<&str> }`.
- Produces: `Index::new()`, `set_disk(PathBuf, Vec<Todo>)` (empty removes), `remove_disk(&Path)`, `remove_disk_prefix(&Path)`, `apply_walk(roots, files, seen, complete: bool)`, `disk(&Path) -> Option<&[Todo]>`, `set_buffer(BufferEntry)`, `buffer(&str) -> Option<&BufferEntry>`, `remove_buffer(&str) -> Option<PathBuf>` (the path when no other buffer shares it, so plan 2 can rescan it from disk), `effective(&EffectiveContext) -> Vec<EffectiveFile>` sorted by path then URI.

- [ ] **Step 1: Write the failing tests**

Add `pub mod index;` to `crates/clippings-core/src/lib.rs`, keeping the declarations sorted. Create `crates/clippings-core/src/index.rs` containing only this test module for now:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::position::Position;

    fn todo(after: &str) -> Todo {
        let p = Position {
            line: 0,
            character: 0,
        };
        Todo {
            start: p,
            end: p,
            tag_start: None,
            tag_end: None,
            tag: "TODO".into(),
            sub_tag: None,
            before: String::new(),
            after: after.into(),
            extra_lines: vec![],
        }
    }

    fn buf(uri: &str, path: Option<&str>, after: &str) -> BufferEntry {
        BufferEntry {
            uri: uri.into(),
            path: path.map(PathBuf::from),
            version: 1,
            todos: vec![todo(after)],
        }
    }

    fn afters(files: &[EffectiveFile]) -> Vec<String> {
        files
            .iter()
            .flat_map(|f| f.todos.iter().map(|t| t.todo.after.clone()))
            .collect()
    }

    fn setup() -> Index {
        let mut i = Index::new();
        i.set_disk("/r/a.ts".into(), vec![todo("disk a")]);
        i.set_disk("/r/b.ts".into(), vec![todo("disk b")]);
        i.set_buffer(buf("file:///r/a.ts", Some("/r/a.ts"), "buffer a"));
        i.set_buffer(buf("untitled:Untitled-1", None, "untitled"));
        i
    }

    #[test]
    fn workspace_mode_buffers_shadow_disk() {
        let i = setup();
        let roots = vec![PathBuf::from("/r")];
        let ctx = EffectiveContext {
            mode: ScanMode::Workspace,
            walked_roots: &roots,
            active_uri: None,
        };
        assert_eq!(
            afters(&i.effective(&ctx)),
            vec!["untitled", "buffer a", "disk b"]
        );
    }

    #[test]
    fn workspace_only_ignores_buffers() {
        let i = setup();
        let roots = vec![PathBuf::from("/r")];
        let ctx = EffectiveContext {
            mode: ScanMode::WorkspaceOnly,
            walked_roots: &roots,
            active_uri: None,
        };
        assert_eq!(afters(&i.effective(&ctx)), vec!["disk a", "disk b"]);
    }

    #[test]
    fn open_files_without_walked_roots_shows_only_buffers() {
        let i = setup();
        let ctx = EffectiveContext {
            mode: ScanMode::OpenFiles,
            walked_roots: &[],
            active_uri: None,
        };
        assert_eq!(afters(&i.effective(&ctx)), vec!["untitled", "buffer a"]);
    }

    #[test]
    fn current_file_shows_only_active_buffer() {
        let i = setup();
        let ctx = EffectiveContext {
            mode: ScanMode::CurrentFile,
            walked_roots: &[],
            active_uri: Some("file:///r/a.ts"),
        };
        assert_eq!(afters(&i.effective(&ctx)), vec!["buffer a"]);
        let none = EffectiveContext {
            mode: ScanMode::CurrentFile,
            walked_roots: &[],
            active_uri: None,
        };
        assert!(i.effective(&none).is_empty());
    }

    #[test]
    fn notebook_cells_union_under_one_path() {
        let mut i = Index::new();
        i.set_buffer(buf(
            "vscode-notebook-cell:/r/n.ipynb#c1",
            Some("/r/n.ipynb"),
            "cell 1",
        ));
        i.set_buffer(buf(
            "vscode-notebook-cell:/r/n.ipynb#c2",
            Some("/r/n.ipynb"),
            "cell 2",
        ));
        let ctx = EffectiveContext {
            mode: ScanMode::Workspace,
            walked_roots: &[],
            active_uri: None,
        };
        let eff = i.effective(&ctx);
        assert_eq!(eff.len(), 1);
        assert_eq!(
            eff[0].todos[1].buffer_uri.as_deref(),
            Some("vscode-notebook-cell:/r/n.ipynb#c2")
        );
        assert_eq!(
            i.remove_buffer("vscode-notebook-cell:/r/n.ipynb#c1"),
            None,
            "another cell still open"
        );
        assert_eq!(
            i.remove_buffer("vscode-notebook-cell:/r/n.ipynb#c2"),
            Some(PathBuf::from("/r/n.ipynb"))
        );
    }

    #[test]
    fn apply_walk_removes_unseen_only_when_complete() {
        let mut i = setup();
        let roots = vec![PathBuf::from("/r")];
        let seen = vec![PathBuf::from("/r/a.ts")];
        i.apply_walk(
            &roots,
            vec![FileResult {
                path: "/r/a.ts".into(),
                todos: vec![todo("new a")],
            }],
            &seen,
            false,
        );
        assert!(
            i.disk(Path::new("/r/b.ts")).is_some(),
            "cancelled walk keeps unseen"
        );
        i.apply_walk(&roots, vec![], &seen, true);
        assert!(i.disk(Path::new("/r/b.ts")).is_none());
        assert!(i.disk(Path::new("/r/a.ts")).is_none(), "seen without todos");
    }

    #[test]
    fn remove_prefix_handles_deleted_directories() {
        let mut i = Index::new();
        i.set_disk("/r/d/a.ts".into(), vec![todo("a")]);
        i.set_disk("/r/d2/b.ts".into(), vec![todo("b")]);
        i.remove_disk_prefix(Path::new("/r/d"));
        assert!(i.disk(Path::new("/r/d/a.ts")).is_none());
        assert!(
            i.disk(Path::new("/r/d2/b.ts")).is_some(),
            "sibling with shared prefix string survives"
        );
    }
}
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p clippings-core index`
Expected: compile error, `Index` and `EffectiveContext` not found.

- [ ] **Step 3: Write the implementation**

Put this above the test module in `crates/clippings-core/src/index.rs`:

```rust
//! The index (spec section 5.8): disk results per path, buffer results per
//! document URI, and the effective result the tree shows per scan mode.

use crate::config::ScanMode;
use crate::model::{FileResult, Todo};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BufferEntry {
    /// The exact URI string received from the client.
    pub uri: String,
    /// File path of the document; `None` for `untitled:` and similar.
    pub path: Option<PathBuf>,
    pub version: i32,
    pub todos: Vec<Todo>,
}

/// A todo with the document it came from: `None` for disk results, the
/// buffer URI (for example a notebook cell) for buffer results.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SourcedTodo {
    pub buffer_uri: Option<String>,
    pub todo: Todo,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Source {
    Disk,
    Buffers,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EffectiveFile {
    /// File path, or `None` for a document without one.
    pub path: Option<PathBuf>,
    /// Set for documents without a path.
    pub uri: Option<String>,
    pub source: Source,
    pub todos: Vec<SourcedTodo>,
}

pub struct EffectiveContext<'a> {
    pub mode: ScanMode,
    pub walked_roots: &'a [PathBuf],
    /// URI of the active editor's document, used in `current file` mode.
    pub active_uri: Option<&'a str>,
}

#[derive(Default)]
pub struct Index {
    disk: BTreeMap<PathBuf, Vec<Todo>>,
    buffers: BTreeMap<String, BufferEntry>,
}

impl Index {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn set_disk(&mut self, path: PathBuf, todos: Vec<Todo>) {
        if todos.is_empty() {
            self.disk.remove(&path);
        } else {
            self.disk.insert(path, todos);
        }
    }

    pub fn remove_disk(&mut self, path: &Path) {
        self.disk.remove(path);
    }

    /// Removes `path` and every entry below it, for a Deleted event.
    pub fn remove_disk_prefix(&mut self, path: &Path) {
        self.disk.retain(|p, _| !p.starts_with(path));
    }

    /// Applies a walk: replaces entries for scanned files and, when the walk
    /// completed, removes entries under `roots` that it did not see.
    pub fn apply_walk(
        &mut self,
        roots: &[PathBuf],
        files: Vec<FileResult>,
        seen: &[PathBuf],
        complete: bool,
    ) {
        for p in seen {
            self.disk.remove(p);
        }
        for f in files {
            self.disk.insert(f.path, f.todos);
        }
        if complete {
            self.disk.retain(|p, _| {
                !roots.iter().any(|r| p.starts_with(r)) || seen.binary_search(p).is_ok()
            });
        }
    }

    pub fn disk(&self, path: &Path) -> Option<&[Todo]> {
        self.disk.get(path).map(Vec::as_slice)
    }

    pub fn set_buffer(&mut self, entry: BufferEntry) {
        self.buffers.insert(entry.uri.clone(), entry);
    }

    pub fn buffer(&self, uri: &str) -> Option<&BufferEntry> {
        self.buffers.get(uri)
    }

    /// Removes a buffer. Returns its path when no other open buffer shares
    /// it, so the caller can rescan that file from disk.
    pub fn remove_buffer(&mut self, uri: &str) -> Option<PathBuf> {
        let entry = self.buffers.remove(uri)?;
        let path = entry.path?;
        (!self
            .buffers
            .values()
            .any(|b| b.path.as_deref() == Some(path.as_path())))
        .then_some(path)
    }

    fn buffer_feeds(&self, b: &BufferEntry, ctx: &EffectiveContext) -> bool {
        match ctx.mode {
            ScanMode::Workspace | ScanMode::OpenFiles => true,
            ScanMode::WorkspaceOnly => false,
            ScanMode::CurrentFile => ctx.active_uri == Some(b.uri.as_str()),
        }
    }

    /// What the tree shows, ordered by path then URI.
    pub fn effective(&self, ctx: &EffectiveContext) -> Vec<EffectiveFile> {
        let mut by_path: BTreeMap<PathBuf, Vec<&BufferEntry>> = BTreeMap::new();
        let mut pathless: Vec<&BufferEntry> = Vec::new();
        for b in self.buffers.values().filter(|b| self.buffer_feeds(b, ctx)) {
            match &b.path {
                Some(p) => by_path.entry(p.clone()).or_default().push(b),
                None => pathless.push(b),
            }
        }
        let mut out: Vec<EffectiveFile> = Vec::new();
        for (path, todos) in &self.disk {
            if by_path.contains_key(path) || !ctx.walked_roots.iter().any(|r| path.starts_with(r)) {
                continue;
            }
            out.push(EffectiveFile {
                path: Some(path.clone()),
                uri: None,
                source: Source::Disk,
                todos: todos
                    .iter()
                    .map(|t| SourcedTodo {
                        buffer_uri: None,
                        todo: t.clone(),
                    })
                    .collect(),
            });
        }
        for (path, buffers) in by_path {
            let todos: Vec<SourcedTodo> = buffers
                .iter()
                .flat_map(|b| {
                    b.todos.iter().map(|t| SourcedTodo {
                        buffer_uri: Some(b.uri.clone()),
                        todo: t.clone(),
                    })
                })
                .collect();
            if !todos.is_empty() {
                out.push(EffectiveFile {
                    path: Some(path),
                    uri: None,
                    source: Source::Buffers,
                    todos,
                });
            }
        }
        for b in pathless.into_iter().filter(|b| !b.todos.is_empty()) {
            out.push(EffectiveFile {
                path: None,
                uri: Some(b.uri.clone()),
                source: Source::Buffers,
                todos: b
                    .todos
                    .iter()
                    .map(|t| SourcedTodo {
                        buffer_uri: Some(b.uri.clone()),
                        todo: t.clone(),
                    })
                    .collect(),
            });
        }
        out.sort_by(|a, b| (&a.path, &a.uri).cmp(&(&b.path, &b.uri)));
        out
    }
}
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test -p clippings-core index`
Expected: all tests in the module pass.
Then run: `cargo fmt --all && cargo clippy --all-targets -- -D warnings`
Expected: no diffs, no warnings.

- [ ] **Step 5: Commit**

```bash
git add -A
git commit -m "feat(core): index with buffer shadowing and scan-mode selection"
```

### Task 12: `clippings scan` and `clippings probe`, CI, and the first benchmark check

**Files:**
- Modify: `crates/clippings/src/main.rs` (replace the placeholder)
- Create: `crates/clippings/tests/cli.rs`, `.github/workflows/ci.yml`, `docs/benchmarks/2026-09-core.md`

**Interfaces:**
- Consumes: `CoreConfig` (deserialized from `--config`), `report::scan_report`, `NativeFs`, `PROTOCOL_VERSION`.
- Produces: the command line `clippings scan <ROOTS>... [--config FILE] [--hidden] [--no-ignore] [--json]` and `clippings probe`, which prints `{"version", "target", "protocolVersion"}`. Plan 3's binary check depends on the probe output shape.

- [ ] **Step 1: Write the failing tests**

`crates/clippings/tests/cli.rs`. The last test covers Review Focus item 5:

```rust
use std::process::Command;

fn bin() -> Command {
    Command::new(env!("CARGO_BIN_EXE_clippings"))
}

#[test]
fn probe_prints_versions() {
    let out = bin().arg("probe").output().unwrap();
    assert!(out.status.success());
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(v["protocolVersion"], 1);
    assert_eq!(v["version"], env!("CARGO_PKG_VERSION"));
}

#[test]
fn scan_json_lists_todos_with_relative_paths() {
    let t = tempfile::tempdir().unwrap();
    std::fs::create_dir(t.path().join("src")).unwrap();
    std::fs::write(t.path().join("src/a.ts"), "// TODO hello\n").unwrap();
    let out = bin()
        .args(["scan", "--json"])
        .arg(t.path())
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(v["files"][0]["path"], "src/a.ts");
    assert_eq!(v["files"][0]["todos"][0]["tag"], "TODO");
    assert_eq!(v["files"][0]["todos"][0]["after"], "hello");
}

#[test]
fn scan_text_output_and_flags() {
    let t = tempfile::tempdir().unwrap();
    std::fs::create_dir(t.path().join(".hidden")).unwrap();
    std::fs::write(t.path().join(".hidden/a.ts"), "// FIXME hidden\n").unwrap();
    let plain = bin().arg("scan").arg(t.path()).output().unwrap();
    assert_eq!(String::from_utf8_lossy(&plain.stdout), "");
    let hidden = bin()
        .args(["scan", "--hidden"])
        .arg(t.path())
        .output()
        .unwrap();
    assert_eq!(
        String::from_utf8_lossy(&hidden.stdout),
        ".hidden/a.ts:1:1: FIXME hidden\n"
    );
}

#[test]
fn bad_regex_config_fails_cleanly() {
    let t = tempfile::tempdir().unwrap();
    let cfg = t.path().join("c.json");
    std::fs::write(&cfg, r#"{"regex":"(unclosed"}"#).unwrap();
    let out = bin()
        .args(["scan", "--config"])
        .arg(&cfg)
        .arg(t.path())
        .output()
        .unwrap();
    assert!(!out.status.success());
    assert!(String::from_utf8_lossy(&out.stderr).contains("invalid regex"));
}

#[cfg(unix)]
#[test]
fn scan_through_a_symlinked_root_reports_relative_paths() {
    let t = tempfile::tempdir().unwrap();
    let real = t.path().join("real");
    std::fs::create_dir_all(real.join("src")).unwrap();
    std::fs::write(real.join("src/a.ts"), "// TODO via link\n").unwrap();
    let link = t.path().join("link");
    std::os::unix::fs::symlink(&real, &link).unwrap();
    let out = bin().args(["scan", "--json"]).arg(&link).output().unwrap();
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(v["files"][0]["path"], "src/a.ts");
}
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p clippings`
Expected: every test fails, because the placeholder binary prints nothing and exits 0.

- [ ] **Step 3: Write the command line**

Replace `crates/clippings/src/main.rs`:

```rust
use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use clippings_core::config::CoreConfig;
use clippings_core::fs::NativeFs;
use clippings_core::report::scan_report;
use std::path::PathBuf;
use std::sync::Arc;

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
            let mut cfg: CoreConfig = match config {
                Some(p) => serde_json::from_slice(
                    &std::fs::read(&p).with_context(|| format!("reading {}", p.display()))?,
                )?,
                None => CoreConfig::default(),
            };
            cfg.include_hidden_files |= hidden;
            cfg.respect_ignore_files &= !no_ignore;
            let roots: Vec<PathBuf> = roots
                .iter()
                .map(|r| dunce::canonicalize(r).with_context(|| format!("root {}", r.display())))
                .collect::<Result<_>>()?;
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
    }
    Ok(())
}
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test --all`
Expected: every test in both crates passes. Then `cargo fmt --all && cargo clippy --all-targets -- -D warnings` shows nothing.

- [ ] **Step 5: Add CI**

`.github/workflows/ci.yml`:

```yaml
name: CI

on:
  push:
    branches: [main]
  pull_request:

jobs:
  rust:
    strategy:
      fail-fast: false
      matrix:
        os: [ubuntu-latest, macos-latest, windows-latest]
    runs-on: ${{ matrix.os }}
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
        with:
          components: rustfmt, clippy
      - uses: Swatinem/rust-cache@v2
      - run: git config --global init.defaultBranch main
      - run: cargo fmt --all --check
      - run: cargo clippy --all-targets -- -D warnings
      - run: cargo test --all
        env:
          INSTA_UPDATE: "no"
```

- [ ] **Step 6: Check speed and parity on the benchmark repository**

This is a manual check on the maintainer's machine, where the tilliX monorepo lives at `~/tilli/tilliX`. It is not a CI step. Build and write the two configurations:

```bash
cargo build --release
mkdir -p target/bench
cat > target/bench/tags.json <<'EOF'
{"tags":["BUG","HACK","FIXME","TODO","XXX","[ ]","QUESTION"]}
EOF
cat > target/bench/wide.json <<'EOF'
{"tags":["BUG","HACK","FIXME","TODO","XXX","[ ]","QUESTION"],"builtInExcludes":[".git","node_modules",".turbo",".next"],"excludeGlobs":[]}
EOF
```

Parity: count todos and compare with ripgrep running the same expanded regex. The two lines must be identical.

```bash
T=~/tilli/tilliX
R='(//|#|<!--|;|/\*|^|^[ \t]*(-|\d+.))\s*(QUESTION|FIXME|HACK|TODO|BUG|XXX|\[ \])'
target/release/clippings scan --json --config target/bench/tags.json "$T" \
  | python3 -c 'import json,sys; d=json.load(sys.stdin); print(sum(len(f["todos"]) for f in d["files"]), "in", len(d["files"]), "files")'
rg --count-matches --max-columns=1000 -e "$R" "$T" | awk -F: '{s+=$NF; f++} END {print s, "in", f, "files"}'

```

Speed: time three warm runs of each. Use `hyperfine` if it is installed, otherwise the shell's `time`.

```bash
hyperfine -w 2 -r 10 \
  "target/release/clippings scan --json --config target/bench/tags.json $T" \
  "rg -c --max-columns=1000 -e '$R' $T" \
  "target/release/clippings scan --json --hidden --no-ignore --config target/bench/wide.json $T" \
  "rg -c --max-columns=1000 --hidden --no-ignore -g '!node_modules' -g '!.turbo' -g '!.next' -g '!.git' -e '$R' $T"
```

Expected: identical counts. Each clippings run takes at most 1.25 times its ripgrep counterpart. On 2026-09-23 the prototype of this plan measured 489 todos in 83 files for both tools, about 30 ms against 25 ms for the default set, and about 160 ms against 152 ms for the wide set.

Record the file counts from `rg --files | wc -l` for each set, the timings and the machine in `docs/benchmarks/2026-09-core.md` as a small table. If a clippings run exceeds 1.25 times ripgrep, record that too and say so in the task report. Plan 4 turns this into the `clippings bench` command.

- [ ] **Step 7: Commit**

```bash
git add -A
git commit -m "feat: clippings scan and probe commands, CI, first benchmark"
```
