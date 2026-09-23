//! Scan results.

use crate::position::Position;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExtraLine {
    pub line: u32,
    pub text: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Todo {
    /// Start of the regex match.
    pub start: Position,
    /// End of the regex match.
    pub end: Position,
    /// End of the line on which the match starts: where "reveal at end of todo" lands.
    pub text_end: Position,
    /// The tag's range, when a tag was found.
    pub tag_start: Option<Position>,
    pub tag_end: Option<Position>,
    /// Configured spelling of the tag, or the whole match without `$TAGS`; empty if none.
    pub tag: String,
    pub sub_tag: Option<String>,
    pub before: String,
    pub after: String,
    /// Continuation lines of a multi-line match, comment-stripped.
    pub extra_lines: Vec<ExtraLine>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FileResult {
    pub path: PathBuf,
    pub todos: Vec<Todo>,
}
