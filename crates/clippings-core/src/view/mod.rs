//! The tree view model (spec section 5.12): placement, filtering, sorting,
//! counts, compaction, rendering, deltas and export.
//!
//! A build runs in three passes. `place` puts every effective todo into an
//! arena of nodes with stable IDs. `shape` computes visibility, counts,
//! order and compaction. `render` turns the shaped arena into the
//! `ViewNode`s the client displays.

pub mod delta;
pub mod export;
pub mod place;
pub mod render;
pub mod shape;
#[cfg(test)]
mod tests;

use crate::position::Position;
use std::collections::HashMap;
use std::path::PathBuf;

pub use render::{NodeCommand, ViewNode};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    /// A workspace folder.
    Root,
    /// A tag or tag-group level (`g:`).
    Tag,
    /// A sub-tag level or pseudo-folder (`s:`).
    SubTag,
    Folder,
    File,
    Todo,
    /// A continuation line of a multi-line todo.
    Extra,
    Status,
}

/// What a todo or extra-line node needs for labels, tooltips and commands.
#[derive(Clone, Debug, PartialEq)]
pub struct TodoData {
    /// The document URI to reveal: the buffer URI, or the file's `file:` URI.
    pub uri: String,
    pub start: Position,
    pub text_end: Position,
    /// The actual tag, before group mapping; empty when untagged.
    pub tag: String,
    pub sub_tag: Option<String>,
    pub before: String,
    pub after: String,
    /// For todo nodes: whether the match had continuation lines.
    pub multi_line: bool,
    /// For extra-line nodes: the line text.
    pub text: String,
    /// The todo came from a notebook cell: its URI differs from its file
    /// node's URI, so its ID and export key carry the URI.
    pub cell: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Node {
    pub kind: Kind,
    /// The node's ID: its parent's ID, `/`, and its own key.
    pub id: String,
    /// The display name before decoration: folder or file name, tag, sub-tag.
    pub name: String,
    pub parent: Option<usize>,
    pub children: Vec<usize>,
    /// Filesystem path for roots, folders, files and todos.
    pub path: Option<PathBuf>,
    /// Document URI for file and todo nodes.
    pub uri: Option<String>,
    /// The decoration key (group or tag) for tag levels and todos.
    pub key: Option<String>,
    pub sub_tag: Option<String>,
    /// ` (dir)` suffix for files in the flat view and outside every root.
    pub path_label: Option<String>,
    pub todo: Option<TodoData>,
    /// Hidden by `hideFromTree`.
    pub hidden: bool,
    /// Status node text and icon.
    pub status: Option<(String, &'static str, Option<String>)>,
}

/// A built view: the arena plus the shaped order.
#[derive(Clone, Debug, Default)]
pub struct Arena {
    pub nodes: Vec<Node>,
    pub top: Vec<usize>,
    pub by_id: HashMap<String, usize>,
}

impl Arena {
    /// Returns the existing node with this parent and key, or adds one.
    pub fn get_or_add(
        &mut self,
        parent: Option<usize>,
        key: &str,
        make: impl FnOnce() -> Node,
    ) -> usize {
        let id = match parent {
            Some(p) => format!("{}/{}", self.nodes[p].id, key),
            None => key.to_string(),
        };
        if let Some(&i) = self.by_id.get(&id) {
            return i;
        }
        let mut node = make();
        node.id = id.clone();
        node.parent = parent;
        let i = self.nodes.len();
        self.nodes.push(node);
        match parent {
            Some(p) => self.nodes[p].children.push(i),
            None => self.top.push(i),
        }
        self.by_id.insert(id, i);
        i
    }
}

impl Node {
    pub fn new(kind: Kind, name: impl Into<String>) -> Self {
        Node {
            kind,
            id: String::new(),
            name: name.into(),
            parent: None,
            children: Vec::new(),
            path: None,
            uri: None,
            key: None,
            sub_tag: None,
            path_label: None,
            todo: None,
            hidden: false,
            status: None,
        }
    }

    pub fn is_container(&self) -> bool {
        !matches!(self.kind, Kind::Todo | Kind::Extra | Kind::Status)
    }

    pub fn is_folder_like(&self) -> bool {
        matches!(self.kind, Kind::Root | Kind::Folder | Kind::SubTag)
    }
}
