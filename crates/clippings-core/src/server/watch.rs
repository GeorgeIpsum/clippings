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
