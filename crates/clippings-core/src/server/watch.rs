//! File watching (spec section 5.10): client-side registration payloads and
//! the `notify` fallback used when the client cannot register watchers and
//! by `clippings watch`.

use crate::protocol::{FileEvent, FILE_CHANGED, FILE_CREATED, FILE_DELETED};
use crate::uri::{file_uri, uri_to_path};
use notify::event::{EventKind, ModifyKind};
use notify::{RecursiveMode, Watcher};
use serde_json::{json, Value};
use std::path::{Path, PathBuf};

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

/// Each root paired with its canonical path, for roots reached through a
/// symlink. Backends such as FSEvents report canonical paths.
pub fn canonical_roots(roots: &[PathBuf]) -> Vec<(PathBuf, PathBuf)> {
    roots
        .iter()
        .filter_map(|r| {
            let canonical = std::fs::canonicalize(r).ok()?;
            (canonical != *r).then(|| (canonical, r.clone()))
        })
        .collect()
}

/// Rewrites event paths under a root's canonical path to lie under the root
/// as configured, so they compare equal to the walked roots.
pub fn rebase_events(events: &mut [FileEvent], roots: &[(PathBuf, PathBuf)]) {
    for e in events {
        let Some(path) = uri_to_path(&e.uri) else {
            continue;
        };
        let rebased = roots.iter().find_map(|(canonical, root)| {
            (!path.starts_with(root))
                .then(|| path.strip_prefix(canonical).ok())
                .flatten()
                .map(|rest| rebase(root, rest))
        });
        if let Some(p) = rebased {
            e.uri = file_uri(&p);
        }
    }
}

fn rebase(root: &Path, rest: &Path) -> PathBuf {
    if rest.as_os_str().is_empty() {
        root.to_path_buf()
    } else {
        root.join(rest)
    }
}

/// Forwards one `notify` result. Backends report a queue overflow as an
/// `Ok` event flagged for rescan with no paths, so that is a failure too.
fn dispatch(res: notify::Result<notify::Event>, on_events: &impl Fn(Option<Vec<FileEvent>>)) {
    match res {
        Ok(e) if e.need_rescan() => on_events(None),
        Ok(e) => {
            let events = to_file_events(&e);
            if !events.is_empty() {
                on_events(Some(events));
            }
        }
        Err(_) => on_events(None),
    }
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
        let mut watcher = notify::recommended_watcher(move |res| dispatch(res, &on_events))?;
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
                seen |= events.iter().any(|e| e.uri == want);
            }
        }
        assert!(seen, "no event for {want}");
    }

    #[test]
    fn events_under_a_canonical_root_are_rebased_onto_the_root() {
        let roots = [(PathBuf::from("/private/w"), PathBuf::from("/w"))];
        let mut events: Vec<FileEvent> = ["/private/w/a.ts", "/private/w", "/w/b.ts", "/x/c.ts"]
            .iter()
            .map(|p| FileEvent {
                uri: file_uri(Path::new(p)),
                kind: FILE_CHANGED,
            })
            .collect();
        rebase_events(&mut events, &roots);
        let uris: Vec<&str> = events.iter().map(|e| e.uri.as_str()).collect();
        assert_eq!(
            uris,
            vec![
                "file:///w/a.ts",
                "file:///w",
                "file:///w/b.ts",
                "file:///x/c.ts"
            ]
        );
    }

    #[test]
    fn overflow_and_errors_report_failure() {
        use notify::event::{EventKind, Flag};
        use std::cell::RefCell;
        let got = RefCell::new(Vec::new());
        let record = |e: Option<Vec<FileEvent>>| got.borrow_mut().push(e.map(|v| v.len()));
        dispatch(
            Ok(notify::Event::new(EventKind::Other).set_flag(Flag::Rescan)),
            &record,
        );
        dispatch(Err(notify::Error::generic("boom")), &record);
        // An event with no paths and no rescan flag is dropped.
        dispatch(Ok(notify::Event::new(EventKind::Other)), &record);
        assert_eq!(*got.borrow(), vec![None, None]);
    }
}
