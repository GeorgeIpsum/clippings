//! The server's event loop over an `lsp-server` connection.

use super::{Env, Server};
use crate::fs::{Fs, NativeFs};
use crate::protocol::InitializeParams;
use crate::PROTOCOL_VERSION;
use crossbeam_channel::{after, never, select};
use lsp_server::{Connection, ErrorCode, Message, Response};
use serde_json::json;
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::sync::Arc;
use std::time::Instant;

fn capabilities() -> serde_json::Value {
    json!({
        "capabilities": {
            "textDocumentSync": { "openClose": true, "change": 2 },
            "workspace": { "workspaceFolders": { "supported": true, "changeNotifications": true } }
        },
        "serverInfo": { "name": "clippings", "version": env!("CARGO_PKG_VERSION") }
    })
}

/// Runs the handshake and the event loop until `exit`.
pub fn run(connection: Connection, fs: Arc<dyn Fs>, env: Env) -> Result<(), String> {
    let (id, raw) = connection.initialize_start().map_err(|e| e.to_string())?;
    let params: InitializeParams = serde_json::from_value(raw).map_err(|e| e.to_string())?;
    let version = params
        .initialization_options
        .as_ref()
        .map(|o| o.protocol_version);
    if version != Some(PROTOCOL_VERSION) {
        let message =
            format!("protocol version mismatch: client {version:?}, server {PROTOCOL_VERSION}");
        let _ = connection
            .sender
            .send(Response::new_err(id, ErrorCode::InvalidRequest as i32, message.clone()).into());
        return Err(message);
    }
    connection
        .initialize_finish(id, capabilities())
        .map_err(|e| e.to_string())?;
    let mut server = Server::new(connection.sender.clone(), fs, env, &params);
    server.start(Instant::now());
    let work = server.work_rx.clone();
    loop {
        let timeout = match server.next_deadline() {
            Some(d) => after(d.saturating_duration_since(Instant::now())),
            None => never(),
        };
        select! {
            recv(connection.receiver) -> msg => {
                let Ok(msg) = msg else { break };
                match msg {
                    Message::Request(req) => {
                        if connection.handle_shutdown(&req).map_err(|e| e.to_string())? {
                            break;
                        }
                        let id = req.id.clone();
                        let resp = catch_unwind(AssertUnwindSafe(|| server.handle_request(req)))
                            .unwrap_or_else(|_| Response::new_err(id, ErrorCode::InternalError as i32, "request panicked".into()));
                        let _ = connection.sender.send(resp.into());
                    }
                    Message::Notification(n) => {
                        if n.method == "exit" {
                            break;
                        }
                        if catch_unwind(AssertUnwindSafe(|| server.handle_notification(n, Instant::now()))).is_err() {
                            server.recover(Instant::now());
                        }
                    }
                    Message::Response(_) => {}
                }
            }
            recv(work) -> w => {
                if let Ok(w) = w {
                    if catch_unwind(AssertUnwindSafe(|| server.handle_work(w, Instant::now()))).is_err() {
                        server.recover(Instant::now());
                    }
                }
            }
            recv(timeout) -> _ => {
                if catch_unwind(AssertUnwindSafe(|| server.tick(Instant::now()))).is_err() {
                    server.recover(Instant::now());
                }
            }
        }
    }
    Ok(())
}

/// `clippings lsp`: the server over stdin and stdout.
pub fn run_stdio() -> Result<(), String> {
    let (connection, io) = Connection::stdio();
    let env: Env = Arc::new(|n| std::env::var(n).ok());
    let result = run(connection, Arc::new(NativeFs), env);
    io.join().map_err(|e| e.to_string())?;
    result
}
