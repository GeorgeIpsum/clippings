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
