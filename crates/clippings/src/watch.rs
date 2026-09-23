//! `clippings watch`: scan, then print one JSON line per file whose todos
//! change, using the same `notify` watcher the server falls back to.

use crate::{flush, write_line};
use anyhow::Result;
use clippings_core::admission::Admission;
use clippings_core::config::CoreConfig;
use clippings_core::fs::{Fs, NativeFs};
use clippings_core::globs::slash_path;
use clippings_core::pattern;
use clippings_core::roots::deepest_root;
use clippings_core::scanner::scan_file;
use clippings_core::server::watch::NotifyWatcher;
use clippings_core::uri::uri_to_path;
use clippings_core::walker::walk_and_scan;
use serde_json::json;
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
    write_line(
        &mut out,
        &json!({ "event": "ready", "files": outcome.files.len(), "todos": total }).to_string(),
    )?;
    flush(&mut out)?;

    let (tx, rx) = mpsc::channel();
    let _watcher = NotifyWatcher::new(&roots, move |e| {
        let _ = tx.send(e);
    })?;
    for events in rx {
        let Some(events) = events else {
            write_line(&mut out, &json!({ "event": "overflow" }).to_string())?;
            continue;
        };
        for e in events {
            let Some(path) = uri_to_path(&e.uri) else {
                continue;
            };
            // Drop events a walk would never reach before any stat, as the
            // server's `process_events` does (spec 5.10), so `node_modules`
            // and the rest of the never-index list never get reported.
            if admission.inside_pruned_dir(&path) {
                continue;
            }
            // Deciding "removed" vs "updated" (and the never-index checks
            // below) from live disk state rather than `e.kind`: backends
            // such as FSEvents can report a directory removal as several
            // events with different, sometimes misleading kinds all naming
            // the same already-gone path, so the reported kind alone is not
            // a reliable way to tell a deletion apart from a change.
            let exists = fs.exists(&path);
            if exists && fs.is_dir(&path) {
                continue;
            }
            if !exists && admission.prunes_dir(&path) {
                // The path itself, not just an ancestor, is a never-index
                // directory (`rm -rf node_modules`); `inside_pruned_dir`
                // only checks ancestors. It cannot be stat'ed any more, but
                // `prunes_dir`'s name- and glob-based rules need none; its
                // one stat (the submodule check) only runs when those pass,
                // and gracefully reports "not a submodule" for a path that
                // no longer exists, which is the right answer here.
                continue;
            }
            let todos = if !exists || !admission.admits_disk(&path) {
                None
            } else {
                scan_file(fs.as_ref(), &pattern, &path).ok().flatten()
            };
            let line = match todos {
                Some(t) => json!({ "event": "updated", "path": rel(&path, &roots), "todos": t }),
                None => json!({ "event": "removed", "path": rel(&path, &roots) }),
            };
            write_line(&mut out, &line.to_string())?;
        }
        flush(&mut out)?;
    }
    Ok(())
}
