//! Scheduler tests that drive `Server` directly, to control the order of
//! file events, worker results and timers.

use super::*;
use crate::fs::NativeFs;
use crate::model::FileResult;
use crate::uri::file_uri;
use serde_json::{json, Value};
use std::cell::RefCell;
use std::sync::Mutex;

thread_local! {
    /// URI whose scan panics, on this test's thread only.
    static PANIC_ON: RefCell<Option<String>> = const { RefCell::new(None) };
}

/// Called from `scan_document`: the seam for injecting a panicking scan.
pub(super) fn maybe_panic(uri: &str) {
    if PANIC_ON.with(|p| p.borrow().as_deref() == Some(uri)) {
        panic!("injected scan panic for {uri}");
    }
}

/// A native file system that records every `is_dir` call and every read.
#[derive(Default)]
struct RecordingFs {
    is_dir_calls: Mutex<Vec<PathBuf>>,
    reads: Mutex<Vec<PathBuf>>,
}

impl Fs for RecordingFs {
    fn read(&self, path: &Path) -> std::io::Result<Vec<u8>> {
        self.reads.lock().unwrap().push(path.to_path_buf());
        NativeFs.read(path)
    }
    fn len(&self, path: &Path) -> std::io::Result<u64> {
        NativeFs.len(path)
    }
    fn open(&self, path: &Path) -> std::io::Result<Box<dyn std::io::Read + Send>> {
        NativeFs.open(path)
    }
    fn exists(&self, path: &Path) -> bool {
        NativeFs.exists(path)
    }
    fn is_dir(&self, path: &Path) -> bool {
        self.is_dir_calls.lock().unwrap().push(path.to_path_buf());
        NativeFs.is_dir(path)
    }
}

fn workspace() -> (tempfile::TempDir, PathBuf) {
    let t = tempfile::tempdir().unwrap();
    let root = dunce::canonicalize(t.path()).unwrap();
    write(&root.join("src/a.ts"), "// TODO alpha\n");
    write(&root.join("b.ts"), "// FIXME beta\n");
    (t, root)
}

fn write(path: &Path, text: &str) {
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, text).unwrap();
}

fn server(root: &Path, settings: Value, fs: Arc<dyn Fs>) -> (Server, Receiver<Message>) {
    let params: InitializeParams = serde_json::from_value(json!({
        "workspaceFolders": [{ "uri": file_uri(root), "name": "w" }],
        "initializationOptions": { "protocolVersion": 1, "settings": settings },
    }))
    .unwrap();
    let (tx, rx) = unbounded();
    let env: Env = Arc::new(|_| None);
    (Server::new(tx, fs, env, &params), rx)
}

fn notification(method: &str, params: Value) -> Notification {
    Notification::new(method.to_string(), params)
}

/// Delivers file events and runs the event timer.
fn events(s: &mut Server, changes: &[(&Path, u8)]) {
    let now = Instant::now();
    let changes: Vec<Value> = changes
        .iter()
        .map(|(p, kind)| json!({ "uri": file_uri(p), "type": kind }))
        .collect();
    s.handle_notification(
        notification(
            method::DID_CHANGE_WATCHED_FILES,
            json!({ "changes": changes }),
        ),
        now,
    );
    s.tick(now + EVENT_DELAY);
}

fn afters(s: &Server, path: &Path) -> Option<Vec<String>> {
    s.index
        .disk(path)
        .map(|todos| todos.iter().map(|t| t.after.clone()).collect())
}

/// The next walk result from the worker thread.
fn next_walk(s: &Server) -> Work {
    loop {
        let w = s
            .work_rx
            .recv_timeout(Duration::from_secs(10))
            .expect("walk result");
        if matches!(w, Work::Walk { .. }) {
            return w;
        }
    }
}

fn walk(generation: u64, roots: Vec<PathBuf>, outcome: Result<WalkOutcome, String>) -> Work {
    Work::Walk {
        generation,
        roots,
        outcome,
    }
}

fn fixed_outcome(files: Vec<FileResult>) -> Result<WalkOutcome, String> {
    let seen = files.iter().map(|f| f.path.clone()).collect();
    Ok(WalkOutcome {
        files,
        seen,
        cancelled: false,
    })
}

fn todo_file(path: PathBuf, after: &str) -> FileResult {
    let text = format!("// TODO {after}\n");
    let pattern = pattern::build(&crate::config::CoreConfig::default()).unwrap();
    FileResult {
        todos: scan_text(&pattern, text.as_bytes(), &path.to_string_lossy()),
        path,
    }
}

#[test]
fn an_invalid_glob_is_reported_instead_of_crashing() {
    let (_t, root) = workspace();
    let (mut s, _rx) = server(
        &root,
        json!({ "filtering": { "excludeGlobs": ["src/[abc"] } }),
        Arc::new(NativeFs),
    );
    s.send_status();
    let error = s.last_status.as_ref().unwrap().error.clone().unwrap();
    assert!(error.contains("src/[abc"), "{error}");
}

#[test]
fn events_in_pruned_directories_are_dropped_before_stat_or_rewalk() {
    let (_t, root) = workspace();
    let nm = root.join("node_modules");
    let generated = root.join("gen");
    write(&nm.join("pkg/a.ts"), "// TODO from node_modules\n");
    write(&generated.join("x/a.ts"), "// TODO generated\n");
    let fs = Arc::new(RecordingFs::default());
    let (mut s, _rx) = server(
        &root,
        json!({ "filtering": { "excludeGlobs": ["**/node_modules/*/**", "gen/**"] } }),
        fs.clone(),
    );
    write(&root.join("src/c.ts"), "// TODO control\n");
    let started = Instant::now();
    events(
        &mut s,
        &[
            (&nm, p::FILE_CREATED),
            (&nm.join("pkg"), p::FILE_CREATED),
            (&nm.join("pkg/a.ts"), p::FILE_CREATED),
            (&generated, p::FILE_CREATED),
            (&generated.join("x/a.ts"), p::FILE_CREATED),
            (&root.join("src/c.ts"), p::FILE_CREATED),
        ],
    );
    let answer = s.handle_request(Request::new(
        RequestId::from(1),
        method::CHILDREN.to_string(),
        json!({ "parent": null }),
    ));
    assert!(answer.response_result.is_ok());
    assert!(
        started.elapsed() < Duration::from_secs(2),
        "the scheduler stays responsive"
    );

    assert_eq!(
        afters(&s, &root.join("src/c.ts")),
        Some(vec!["control".to_string()]),
        "control event processed"
    );
    assert_eq!(afters(&s, &nm.join("pkg/a.ts")), None);
    assert_eq!(afters(&s, &generated.join("x/a.ts")), None);
    let calls = fs.is_dir_calls.lock().unwrap().clone();
    let inside: Vec<_> = calls
        .iter()
        .filter(|c| {
            (c.starts_with(&nm) && **c != nm) || (c.starts_with(&generated) && **c != generated)
        })
        .collect();
    assert!(inside.is_empty(), "statted inside pruned dirs: {inside:?}");

    // A rewalk prunes as a full walk does, and never enters a pruned directory.
    let visited = s.rewalk_candidates(&root).unwrap();
    assert!(visited.contains(&root.join("src/a.ts")));
    assert!(
        !visited
            .iter()
            .any(|f| f.starts_with(&nm) || f.starts_with(&generated)),
        "{visited:?}"
    );
    assert!(s.rewalk_candidates(&nm).is_none());
    assert!(s.rewalk_candidates(&generated).is_none());
}

#[test]
fn ignore_file_events_rescan_only_when_they_can_matter() {
    let (_t, root) = workspace();
    write(&root.join("node_modules/x/.gitignore"), "*.ts\n");
    let (mut s, _rx) = server(&root, json!({}), Arc::new(NativeFs));

    events(
        &mut s,
        &[(&root.join("node_modules/x/.gitignore"), p::FILE_CHANGED)],
    );
    assert_eq!(s.scan_generation, 0, "inside node_modules");
    assert!(!s.scanning && !s.needs_scan);

    // Without ignore-file support, an ignore file is an ordinary file.
    let mut core = s.settings.core();
    core.respect_ignore_files = false;
    s.admission = Arc::new(Admission::new(&core, s.walked.clone(), s.fs.clone()).unwrap());
    events(&mut s, &[(&root.join(".gitignore"), p::FILE_CHANGED)]);
    assert_eq!(s.scan_generation, 0, "ignore files not respected");

    // Control: a respected ignore file at the root does rescan.
    s.rebuild_scan_state();
    events(&mut s, &[(&root.join(".gitignore"), p::FILE_CHANGED)]);
    assert_eq!(s.scan_generation, 1);
}

#[test]
fn file_events_during_a_walk_survive_its_application() {
    let (_t, root) = workspace();
    let (mut s, _rx) = server(&root, json!({}), Arc::new(NativeFs));
    s.full_rescan();
    let w = next_walk(&s);

    write(&root.join("c.ts"), "// TODO created\n");
    std::fs::remove_file(root.join("b.ts")).unwrap();
    write(&root.join("src/a.ts"), "// TODO edited\n");
    write(&root.join("lib/d.ts"), "// TODO in new dir\n");
    events(
        &mut s,
        &[
            (&root.join("c.ts"), p::FILE_CREATED),
            (&root.join("b.ts"), p::FILE_DELETED),
            (&root.join("src/a.ts"), p::FILE_CHANGED),
            (&root.join("lib"), p::FILE_CREATED),
        ],
    );
    s.handle_work(w, Instant::now());

    assert!(!s.scanning);
    assert_eq!(
        afters(&s, &root.join("c.ts")),
        Some(vec!["created".to_string()])
    );
    assert_eq!(afters(&s, &root.join("b.ts")), None, "no ghost");
    assert_eq!(
        afters(&s, &root.join("src/a.ts")),
        Some(vec!["edited".to_string()])
    );
    assert_eq!(
        afters(&s, &root.join("lib/d.ts")),
        Some(vec!["in new dir".to_string()])
    );

    // A new generation forgets paths touched during the one it replaced:
    // its walk started after those events, so it applies in full.
    s.full_rescan();
    write(
        &root.join("e.ts"),
        "// TODO touched in the old generation\n",
    );
    events(&mut s, &[(&root.join("e.ts"), p::FILE_CREATED)]);
    s.full_rescan();
    let generation = s.scan_generation;
    s.handle_work(
        walk(generation, s.walked.clone(), fixed_outcome(vec![])),
        Instant::now(),
    );
    assert_eq!(afters(&s, &root.join("e.ts")), None);
}

#[test]
fn a_panicking_scan_keeps_the_documents_previous_state() {
    let (_t, root) = workspace();
    let (mut s, rx) = server(
        &root,
        json!({ "highlights": { "highlightDelay": 0 } }),
        Arc::new(NativeFs),
    );
    let good = file_uri(&root.join("good.ts"));
    let bad = file_uri(&root.join("bad.ts"));
    let now = Instant::now();
    for uri in [&good, &bad] {
        s.handle_notification(
            notification(
                method::DID_OPEN,
                json!({ "textDocument": { "uri": uri, "languageId": "typescript", "version": 1, "text": "// TODO one\n" } }),
            ),
            now,
        );
    }
    s.tick(now);
    while rx.try_recv().is_ok() {}

    PANIC_ON.with(|p| *p.borrow_mut() = Some(bad.clone()));
    s.handle_notification(
        notification(
            method::DID_CHANGE,
            json!({ "textDocument": { "uri": bad, "version": 2 }, "contentChanges": [{ "text": "// TODO two\n" }] }),
        ),
        now,
    );
    s.tick(now + BUFFER_DELAY);
    let sent: Vec<Message> = rx.try_iter().collect();
    assert!(
        !sent
            .iter()
            .any(|m| matches!(m, Message::Notification(n) if n.method == method::DECORATIONS)),
        "no decorations for the panicking document"
    );
    let todos = |s: &Server, uri: &str| -> Vec<String> {
        s.index
            .buffer(uri)
            .map(|b| b.todos.iter().map(|t| t.after.clone()).collect())
            .unwrap_or_default()
    };
    assert_eq!(todos(&s, &bad), vec!["one"], "previous todos kept");
    assert_eq!(s.docs.get(&bad).unwrap().todos[0].after, "one");

    s.recover(now);
    PANIC_ON.with(|p| *p.borrow_mut() = None);
    assert_eq!(todos(&s, &good), vec!["one"], "rescanned");
    assert_eq!(todos(&s, &bad), vec!["one"], "previous entry kept");
    assert!(s.docs.get(&good).is_some() && s.docs.get(&bad).is_some());
    assert!(s.scanning, "recover starts a full rescan");
}

#[test]
fn a_stale_walk_is_discarded() {
    let (_t, root) = workspace();
    let (mut s, _rx) = server(&root, json!({}), Arc::new(NativeFs));
    s.scan_generation = 1;
    s.scanning = true;
    s.full_rescan();
    let ghost = root.join("ghost.ts");
    s.handle_work(
        walk(
            1,
            s.walked.clone(),
            fixed_outcome(vec![todo_file(ghost.clone(), "old")]),
        ),
        Instant::now(),
    );
    assert!(s.scanning, "still waiting for generation 2");
    assert_eq!(afters(&s, &ghost), None);

    s.handle_work(
        walk(
            2,
            s.walked.clone(),
            fixed_outcome(vec![todo_file(ghost.clone(), "new")]),
        ),
        Instant::now(),
    );
    assert!(!s.scanning);
    assert_eq!(afters(&s, &ghost), Some(vec!["new".to_string()]));
}

#[test]
fn a_successful_walk_clears_a_walk_error() {
    let (_t, root) = workspace();
    let (mut s, _rx) = server(&root, json!({}), Arc::new(NativeFs));
    s.full_rescan();
    let generation = s.scan_generation;
    s.handle_work(
        walk(generation, s.walked.clone(), Err("walk failed".into())),
        Instant::now(),
    );
    assert_eq!(
        s.last_status.as_ref().unwrap().error.as_deref(),
        Some("walk failed")
    );
    s.full_rescan();
    let generation = s.scan_generation;
    s.handle_work(
        walk(generation, s.walked.clone(), fixed_outcome(vec![])),
        Instant::now(),
    );
    assert_eq!(s.last_status.as_ref().unwrap().error, None);
}

#[test]
fn a_batch_handles_each_path_once_and_skips_files_under_rewalked_dirs() {
    let (_t, root) = workspace();
    let fs = Arc::new(RecordingFs::default());
    let (mut s, _rx) = server(&root, json!({}), fs.clone());
    write(&root.join("src/a.ts"), "// TODO edited\n");
    write(&root.join("lib/d.ts"), "// TODO in lib\n");
    write(&root.join("gone.ts"), "// TODO gone\n");
    let gone = todo_file(root.join("gone.ts"), "gone");
    s.index.set_disk(gone.path, gone.todos);
    std::fs::remove_file(root.join("gone.ts")).unwrap();
    events(
        &mut s,
        &[
            (&root.join("src/a.ts"), p::FILE_CHANGED),
            (&root.join("lib/d.ts"), p::FILE_CREATED),
            (&root.join("src/a.ts"), p::FILE_CHANGED),
            (&root.join("lib"), p::FILE_CREATED),
            (&root.join("gone.ts"), p::FILE_CREATED),
            (&root.join("gone.ts"), p::FILE_DELETED),
        ],
    );
    let calls = fs.is_dir_calls.lock().unwrap().clone();
    let count = |p: &Path| calls.iter().filter(|c| c.as_path() == p).count();
    assert_eq!(count(&root.join("src/a.ts")), 1, "deduplicated: {calls:?}");
    assert_eq!(
        count(&root.join("lib/d.ts")),
        0,
        "covered by the rewalk: {calls:?}"
    );
    assert_eq!(count(&root.join("gone.ts")), 0, "last event was a deletion");
    assert_eq!(
        afters(&s, &root.join("src/a.ts")),
        Some(vec!["edited".to_string()])
    );
    assert_eq!(
        afters(&s, &root.join("lib/d.ts")),
        Some(vec!["in lib".to_string()])
    );
    assert_eq!(afters(&s, &root.join("gone.ts")), None);
}

#[test]
fn an_ignore_file_under_a_rewalked_dir_still_rescans() {
    let (_t, root) = workspace();
    std::fs::create_dir_all(root.join(".git")).unwrap();
    write(&root.join("lib/x.ts"), "// TODO x\n");
    let (mut s, _rx) = server(&root, json!({}), Arc::new(NativeFs));
    assert!(
        s.admission.admits_disk(&root.join("lib/x.ts")),
        "rules cached"
    );
    write(&root.join("lib/.gitignore"), "x.ts\n");
    assert!(
        s.admission.admits_disk(&root.join("lib/x.ts")),
        "still cached"
    );
    events(
        &mut s,
        &[
            (&root.join("lib"), p::FILE_CREATED),
            (&root.join("lib/.gitignore"), p::FILE_CREATED),
        ],
    );
    assert!(
        !s.admission.admits_disk(&root.join("lib/x.ts")),
        "ignore cache cleared"
    );
    assert_eq!(s.scan_generation, 1, "full rescan started");
    assert!(s.scanning);
}

#[test]
fn a_changed_event_on_a_directory_does_not_rewalk_it() {
    let (_t, root) = workspace();
    let fs = Arc::new(RecordingFs::default());
    let (mut s, _rx) = server(&root, json!({}), fs.clone());
    // Written without an event of its own: only a rewalk would find it.
    write(&root.join("src/unseen.ts"), "// TODO unseen\n");
    events(
        &mut s,
        &[
            (&root, p::FILE_CHANGED),
            (&root.join("src"), p::FILE_CHANGED),
        ],
    );
    assert_eq!(afters(&s, &root.join("src/unseen.ts")), None);
    assert_eq!(
        *fs.reads.lock().unwrap(),
        Vec::<PathBuf>::new(),
        "no file read"
    );

    // Control: a Created event on the same directory rewalks it.
    events(&mut s, &[(&root.join("src"), p::FILE_CREATED)]);
    assert_eq!(
        afters(&s, &root.join("src/unseen.ts")),
        Some(vec!["unseen".to_string()])
    );
}
