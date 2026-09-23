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
    /// completed, removes entries under `roots` that it did not see. `seen` may be in any order.
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
            let seen_set: std::collections::HashSet<&Path> =
                seen.iter().map(PathBuf::as_path).collect();
            self.disk.retain(|p, _| {
                !roots.iter().any(|r| p.starts_with(r)) || seen_set.contains(p.as_path())
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

    #[test]
    fn apply_walk_accepts_unsorted_seen() {
        let mut i = Index::new();
        let roots = vec![PathBuf::from("/r")];
        let seen = vec![
            PathBuf::from("/r/z.ts"),
            PathBuf::from("/r/a.ts"),
            PathBuf::from("/r/m.ts"),
        ];
        let files = seen
            .iter()
            .map(|p| FileResult {
                path: p.clone(),
                todos: vec![todo("x")],
            })
            .collect();
        i.apply_walk(&roots, files, &seen, true);
        for p in &seen {
            assert!(i.disk(p).is_some(), "{} kept", p.display());
        }
    }
}
