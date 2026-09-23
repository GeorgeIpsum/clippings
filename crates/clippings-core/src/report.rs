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
