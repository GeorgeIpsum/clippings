//! Wire types (spec section 6): the standard LSP payloads Clippings uses and
//! the custom `clippings/*` messages. Mirrored in `extension/src/protocol.ts`.

use crate::position::{Position, Range};
use crate::settings::Settings;
use crate::status::{Badge, StatusBar};
use crate::styles::DecorationStyle;
use crate::view::ViewNode;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

pub mod method {
    pub const CONFIGURE: &str = "clippings/configure";
    pub const ACTIVE_EDITOR: &str = "clippings/activeEditor";
    pub const RESCAN: &str = "clippings/rescan";
    pub const STOP_SCAN: &str = "clippings/stopScan";
    pub const CHILDREN: &str = "clippings/children";
    pub const FIND: &str = "clippings/find";
    pub const NAVIGATE: &str = "clippings/navigate";
    pub const EXPORT: &str = "clippings/export";
    pub const TREE_CHANGED: &str = "clippings/treeChanged";
    pub const STYLES: &str = "clippings/styles";
    pub const DECORATIONS: &str = "clippings/decorations";
    pub const STATUS: &str = "clippings/status";
    pub const DID_OPEN: &str = "textDocument/didOpen";
    pub const DID_CHANGE: &str = "textDocument/didChange";
    pub const DID_CLOSE: &str = "textDocument/didClose";
    pub const DID_CHANGE_WATCHED_FILES: &str = "workspace/didChangeWatchedFiles";
    pub const DID_CHANGE_WORKSPACE_FOLDERS: &str = "workspace/didChangeWorkspaceFolders";
    pub const REGISTER_CAPABILITY: &str = "client/registerCapability";
    pub const UNREGISTER_CAPABILITY: &str = "client/unregisterCapability";
}

// ---- Standard LSP payloads ----

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkspaceFolder {
    pub uri: String,
    pub name: String,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InitializationOptions {
    pub protocol_version: u32,
    #[serde(default)]
    pub settings: Settings,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InitializeParams {
    #[serde(default)]
    pub workspace_folders: Option<Vec<WorkspaceFolder>>,
    #[serde(default)]
    pub initialization_options: Option<InitializationOptions>,
    #[serde(default)]
    pub capabilities: serde_json::Value,
}

impl InitializeParams {
    /// Whether the client can register file watchers dynamically.
    pub fn dynamic_watchers(&self) -> bool {
        self.capabilities
            .pointer("/workspace/didChangeWatchedFiles/dynamicRegistration")
            .and_then(|v| v.as_bool())
            .unwrap_or(false)
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TextDocumentItem {
    pub uri: String,
    #[serde(default)]
    pub language_id: String,
    pub version: i32,
    pub text: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DidOpenParams {
    pub text_document: TextDocumentItem,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct VersionedDocument {
    pub uri: String,
    pub version: i32,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DocumentId {
    pub uri: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ContentChange {
    /// Absent for a full-text replacement.
    #[serde(default)]
    pub range: Option<Range>,
    pub text: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DidChangeParams {
    pub text_document: VersionedDocument,
    pub content_changes: Vec<ContentChange>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DidCloseParams {
    pub text_document: DocumentId,
}

/// `1` created, `2` changed, `3` deleted.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FileEvent {
    pub uri: String,
    #[serde(rename = "type")]
    pub kind: u8,
}

pub const FILE_CREATED: u8 = 1;
pub const FILE_CHANGED: u8 = 2;
pub const FILE_DELETED: u8 = 3;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DidChangeWatchedFilesParams {
    pub changes: Vec<FileEvent>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct WorkspaceFoldersChange {
    pub added: Vec<WorkspaceFolder>,
    pub removed: Vec<WorkspaceFolder>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DidChangeWorkspaceFoldersParams {
    pub event: WorkspaceFoldersChange,
}

// ---- Custom messages ----

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ActiveEditorParams {
    pub uri: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ChildrenParams {
    pub parent: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ChildrenResult {
    pub nodes: Vec<ViewNode>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FindParams {
    pub uri: String,
    pub line: Option<u32>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FindResult {
    pub paths: Vec<Vec<ViewNode>>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct NavigateParams {
    pub uri: String,
    pub positions: Vec<Position>,
    pub direction: crate::navigate::Direction,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct NavigateResult {
    pub ranges: Option<Vec<Range>>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ExportResult {
    pub path: String,
    pub content: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TreeChangedParams {
    pub refresh: Vec<Option<String>>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct StylesParams {
    pub generation: u64,
    pub reset: bool,
    pub styles: BTreeMap<String, DecorationStyle>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct DecorationsParams {
    pub uri: String,
    pub version: i32,
    pub generation: u64,
    pub ranges: BTreeMap<String, Vec<Range>>,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StatusParams {
    pub instance: String,
    pub scanning: bool,
    pub interrupted: bool,
    pub needs_scan: bool,
    pub error: Option<String>,
    pub warnings: Vec<String>,
    pub status_bar: StatusBar,
    pub badge: Badge,
    pub view_title: String,
    pub has_sub_tags: bool,
    pub is_empty: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_initialize_params() {
        let p: InitializeParams = serde_json::from_str(
            r#"{"processId":1,"workspaceFolders":[{"uri":"file:///w","name":"w"}],
                "capabilities":{"workspace":{"didChangeWatchedFiles":{"dynamicRegistration":true}}},
                "initializationOptions":{"protocolVersion":1,"settings":{"general":{"tags":["TODO"]}}}}"#,
        )
        .unwrap();
        assert!(p.dynamic_watchers());
        let o = p.initialization_options.unwrap();
        assert_eq!(o.protocol_version, 1);
        assert_eq!(o.settings.general.tags, vec!["TODO"]);
    }

    #[test]
    fn status_is_camel_case() {
        let v = serde_json::to_value(StatusParams {
            needs_scan: true,
            view_title: "Tree".into(),
            ..Default::default()
        })
        .unwrap();
        assert_eq!(v["needsScan"], true);
        assert_eq!(v["viewTitle"], "Tree");
        assert_eq!(v["statusBar"]["visible"], false);
    }
}
