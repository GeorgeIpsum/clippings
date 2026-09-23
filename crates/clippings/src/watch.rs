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
            // Drop events a walk would never reach before any stat, as the
            // server's `process_events` does (spec 5.10), so `node_modules`
            // and the rest of the never-index list never get reported.
            if admission.inside_pruned_dir(&path) {
                continue;
            }
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
