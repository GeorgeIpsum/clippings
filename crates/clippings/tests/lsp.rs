//! Spawns `clippings lsp` and `clippings watch` and talks to them.

use lsp_server::{Message, Notification, Request, RequestId};
use serde_json::{json, Value};
use std::io::{BufRead, BufReader, Write};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

fn bin() -> Command {
    Command::new(env!("CARGO_BIN_EXE_clippings"))
}

#[test]
fn lsp_over_stdio_initializes_scans_and_shuts_down() {
    let t = tempfile::tempdir().unwrap();
    let root = dunce::canonicalize(t.path()).unwrap();
    std::fs::write(root.join("a.ts"), "// TODO over stdio\n").unwrap();
    let uri = clippings_core::uri::file_uri(&root);
    let mut child = bin()
        .arg("lsp")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    let mut stdin = child.stdin.take().unwrap();
    let mut stdout = BufReader::new(child.stdout.take().unwrap());
    let mut send = |m: Message| m.write(&mut stdin).unwrap();

    send(Request::new(RequestId::from(1), "initialize".into(), json!({
        "workspaceFolders": [{ "uri": uri, "name": "w" }],
        "capabilities": {},
        "initializationOptions": { "protocolVersion": 1, "settings": { "tree": { "showCurrentScanMode": false } } }
    })).into());
    let init = Message::read(&mut stdout).unwrap().unwrap();
    assert!(
        matches!(init, Message::Response(ref r) if r.response_result.is_ok()),
        "{init:?}"
    );
    send(Notification::new("initialized".into(), json!({})).into());

    // Wait until a scan finishes, then ask for the top level.
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        assert!(Instant::now() < deadline, "no finished scan");
        if let Some(Message::Notification(n)) = Message::read(&mut stdout).unwrap() {
            if n.method == "clippings/status" && n.params["scanning"] == false {
                break;
            }
        }
    }
    std::thread::sleep(Duration::from_millis(200));
    send(
        Request::new(
            RequestId::from(2),
            "clippings/children".into(),
            json!({ "parent": null }),
        )
        .into(),
    );
    let nodes = loop {
        if let Some(Message::Response(r)) = Message::read(&mut stdout).unwrap() {
            break r.response_result.unwrap();
        }
    };
    assert_eq!(nodes["nodes"].as_array().unwrap().len(), 1);

    send(Request::new(RequestId::from(3), "shutdown".into(), Value::Null).into());
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        assert!(Instant::now() < deadline, "no shutdown response");
        if let Some(Message::Response(r)) = Message::read(&mut stdout).unwrap() {
            assert_eq!(r.id, RequestId::from(3));
            break;
        }
    }
    send(Notification::new("exit".into(), Value::Null).into());
    drop(stdin);
    assert!(child.wait().unwrap().success());
}

#[test]
fn lsp_rejects_a_mismatched_protocol_version() {
    let mut child = bin()
        .arg("lsp")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    let mut stdin = child.stdin.take().unwrap();
    let mut stdout = BufReader::new(child.stdout.take().unwrap());
    let init: Message = Request::new(
        RequestId::from(1),
        "initialize".into(),
        json!({ "capabilities": {}, "initializationOptions": { "protocolVersion": 0 } }),
    )
    .into();
    init.write(&mut stdin).unwrap();
    let resp = Message::read(&mut stdout).unwrap().unwrap();
    assert!(matches!(resp, Message::Response(ref r) if r.response_result.is_err()));
    drop(stdin);
    assert!(!child.wait().unwrap().success());
}

#[test]
fn watch_prints_changes() {
    let t = tempfile::tempdir().unwrap();
    let root = dunce::canonicalize(t.path()).unwrap();
    std::fs::write(root.join("a.ts"), "// TODO first\n").unwrap();
    let mut child = bin()
        .arg("watch")
        .arg(&root)
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    let mut lines = BufReader::new(child.stdout.take().unwrap()).lines();
    let ready: Value = serde_json::from_str(&lines.next().unwrap().unwrap()).unwrap();
    assert_eq!(ready, json!({ "event": "ready", "files": 1, "todos": 1 }));
    std::thread::sleep(Duration::from_millis(300));
    std::fs::write(root.join("b.ts"), "// FIXME second\n").unwrap();
    let mut seen = false;
    for line in lines.by_ref().take(20) {
        let v: Value = serde_json::from_str(&line.unwrap()).unwrap();
        if v["event"] == "updated" && v["path"] == "b.ts" {
            assert_eq!(v["todos"][0]["tag"], "FIXME");
            seen = true;
            break;
        }
    }
    child.kill().unwrap();
    let _ = child.wait();
    assert!(seen);
    let _ = std::io::stdout().flush();
}

#[test]
fn watch_drops_events_under_a_never_index_directory() {
    let t = tempfile::tempdir().unwrap();
    let root = dunce::canonicalize(t.path()).unwrap();
    std::fs::create_dir_all(root.join("node_modules/pkg")).unwrap();
    let mut child = bin()
        .arg("watch")
        .arg(&root)
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    let mut lines = BufReader::new(child.stdout.take().unwrap()).lines();
    let ready: Value = serde_json::from_str(&lines.next().unwrap().unwrap()).unwrap();
    assert_eq!(ready["event"], "ready");
    std::thread::sleep(Duration::from_millis(300));

    // A created, then changed, then deleted file under node_modules, and
    // finally the whole node_modules directory removed with it.
    let nested = root.join("node_modules/pkg/a.ts");
    std::fs::write(&nested, "// TODO in node_modules\n").unwrap();
    std::thread::sleep(Duration::from_millis(150));
    std::fs::write(&nested, "// FIXME in node_modules\n").unwrap();
    std::thread::sleep(Duration::from_millis(150));
    std::fs::remove_file(&nested).unwrap();
    std::thread::sleep(Duration::from_millis(150));
    std::fs::remove_dir_all(root.join("node_modules")).unwrap();
    std::thread::sleep(Duration::from_millis(150));

    // The barrier: a change to a top-level file. Once its `updated` line
    // arrives, every notify event queued before it has already been
    // processed and either printed or correctly dropped, so the test fails
    // deterministically instead of timing out.
    std::fs::write(root.join("sentinel.ts"), "// TODO sentinel\n").unwrap();

    let mut saw_node_modules_line = false;
    let mut saw_sentinel = false;
    for line in lines.by_ref().take(50) {
        let v: Value = serde_json::from_str(&line.unwrap()).unwrap();
        let path = v["path"].as_str().unwrap_or_default();
        if path == "node_modules" || path.starts_with("node_modules/") {
            saw_node_modules_line = true;
        }
        if v["event"] == "updated" && v["path"] == "sentinel.ts" {
            saw_sentinel = true;
            break;
        }
    }
    child.kill().unwrap();
    let _ = child.wait();
    assert!(saw_sentinel, "the sentinel change was never reported");
    assert!(
        !saw_node_modules_line,
        "an event for a path under node_modules was printed"
    );
}
