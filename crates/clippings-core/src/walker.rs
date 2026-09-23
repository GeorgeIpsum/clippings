//! Parallel walk and scan of the walked roots (spec section 5.3 and 5.5).

use crate::admission::rel_components;
use crate::config::CoreConfig;
use crate::fs::Fs;
use crate::globs::{slash_path, BuiltInExcludes, GlobLayers};
use crate::model::FileResult;
use crate::pattern::ScanPattern;
use crate::roots::deepest_root;
use crate::scanner::{scan_file_with, searcher};
use crate::CoreError;
use ignore::{DirEntry, WalkBuilder, WalkState};
use std::collections::{BTreeMap, BTreeSet};
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
        .threads(std::thread::available_parallelism().map_or(4, |n| n.get()));
    if cfg.respect_ignore_files {
        builder.add_custom_ignore_filename(".rgignore");
    } else {
        builder
            .ignore(false)
            .git_ignore(false)
            .git_exclude(false)
            .git_global(false)
            .parents(false);
    }
    let f = filter.clone();
    builder.filter_entry(move |e| f.keep(e));

    // Keyed by path: overlapping roots reach the same file more than once.
    let results: Mutex<BTreeMap<PathBuf, FileResult>> = Mutex::new(BTreeMap::new());
    let seen: Mutex<BTreeSet<PathBuf>> = Mutex::new(BTreeSet::new());
    let cancelled = AtomicBool::new(false);
    builder.build_parallel().run(|| {
        let fs = fs.clone();
        let mut searcher = searcher(pattern);
        let (results, seen, cancelled) = (&results, &seen, &cancelled);
        Box::new(move |entry| {
            if cancel.load(Ordering::Relaxed) {
                cancelled.store(true, Ordering::Relaxed);
                return WalkState::Quit;
            }
            let entry = match entry {
                Ok(e) => e,
                Err(e) => {
                    tracing::debug!("walk: {e}");
                    return WalkState::Continue;
                }
            };
            if !entry.file_type().is_some_and(|t| t.is_file()) {
                return WalkState::Continue;
            }
            match scan_file_with(&mut searcher, fs.as_ref(), pattern, entry.path()) {
                Ok(Some(todos)) => {
                    let path = entry.path().to_path_buf();
                    seen.lock().unwrap().insert(path.clone());
                    if !todos.is_empty() {
                        results
                            .lock()
                            .unwrap()
                            .insert(path.clone(), FileResult { path, todos });
                    }
                }
                Ok(None) => {}
                Err(e) => tracing::debug!("skipping {}: {e}", entry.path().display()),
            }
            WalkState::Continue
        })
    });
    Ok(WalkOutcome {
        files: results.into_inner().unwrap().into_values().collect(),
        seen: seen.into_inner().unwrap().into_iter().collect(),
        cancelled: cancelled.into_inner(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fs::NativeFs;
    use crate::pattern;

    #[test]
    fn nested_roots_yield_each_file_once() {
        let t = tempfile::tempdir().unwrap();
        let root = dunce::canonicalize(t.path()).unwrap();
        let inner = root.join("inner");
        std::fs::create_dir_all(&inner).unwrap();
        std::fs::write(root.join("a.ts"), "// TODO outer\n").unwrap();
        std::fs::write(inner.join("b.ts"), "// TODO inner\n").unwrap();
        std::fs::write(inner.join("c.ts"), "no tags\n").unwrap();
        let cfg = CoreConfig::default();
        let p = pattern::build(&cfg).unwrap();
        let out = walk_and_scan(
            &cfg,
            &[root.clone(), inner.clone()],
            &p,
            Arc::new(NativeFs),
            &AtomicBool::new(false),
        )
        .unwrap();
        let files: Vec<_> = out.files.iter().map(|f| f.path.clone()).collect();
        assert_eq!(files, vec![root.join("a.ts"), inner.join("b.ts")]);
        assert_eq!(
            out.seen,
            vec![root.join("a.ts"), inner.join("b.ts"), inner.join("c.ts")]
        );
    }

    #[test]
    fn a_cancelled_walk_reports_it_and_scans_nothing() {
        let t = tempfile::tempdir().unwrap();
        std::fs::write(t.path().join("a.ts"), "// TODO\n").unwrap();
        let cfg = CoreConfig::default();
        let p = crate::pattern::build(&cfg).unwrap();
        let out = walk_and_scan(
            &cfg,
            &[t.path().to_path_buf()],
            &p,
            Arc::new(NativeFs),
            &AtomicBool::new(true),
        )
        .unwrap();
        assert!(out.cancelled);
        assert!(out.files.is_empty() && out.seen.is_empty());
    }
}
