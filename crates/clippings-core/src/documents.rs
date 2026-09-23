//! Open documents and incremental text sync.

use crate::model::Todo;
use crate::position::LineIndex;
use crate::protocol::ContentChange;
use std::collections::BTreeMap;
use std::path::PathBuf;

#[derive(Clone, Debug)]
pub struct Document {
    pub uri: String,
    pub path: Option<PathBuf>,
    pub version: i32,
    pub text: String,
    /// The last scan of this buffer, used for decorations.
    pub todos: Vec<Todo>,
    /// Whether the document passes the open-buffer admission rules and schemes.
    pub admitted: bool,
}

/// Applies LSP content changes in order. Ranges use UTF-16 positions.
pub fn apply_changes(text: &mut String, changes: &[ContentChange]) {
    for c in changes {
        match c.range {
            None => *text = c.text.clone(),
            Some(r) => {
                let li = LineIndex::new(text.as_bytes());
                let a = li.offset(r.start).min(text.len());
                let b = li.offset(r.end).clamp(a, text.len());
                text.replace_range(a..b, &c.text);
            }
        }
    }
}

#[derive(Default)]
pub struct Documents {
    docs: BTreeMap<String, Document>,
}

impl Documents {
    pub fn insert(&mut self, doc: Document) {
        self.docs.insert(doc.uri.clone(), doc);
    }
    pub fn get(&self, uri: &str) -> Option<&Document> {
        self.docs.get(uri)
    }
    pub fn get_mut(&mut self, uri: &str) -> Option<&mut Document> {
        self.docs.get_mut(uri)
    }
    pub fn remove(&mut self, uri: &str) -> Option<Document> {
        self.docs.remove(uri)
    }
    pub fn iter(&self) -> impl Iterator<Item = &Document> {
        self.docs.values()
    }
    pub fn uris(&self) -> Vec<String> {
        self.docs.keys().cloned().collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::position::{Position, Range};

    fn change(l1: u32, c1: u32, l2: u32, c2: u32, text: &str) -> ContentChange {
        ContentChange {
            range: Some(Range {
                start: Position {
                    line: l1,
                    character: c1,
                },
                end: Position {
                    line: l2,
                    character: c2,
                },
            }),
            text: text.into(),
        }
    }

    #[test]
    fn incremental_edits_in_utf16() {
        let mut t = "é // TODO a\nline two\n".to_string();
        apply_changes(&mut t, &[change(0, 10, 0, 11, "b")]);
        assert_eq!(t, "é // TODO b\nline two\n");
        apply_changes(
            &mut t,
            &[change(1, 0, 1, 4, "LINE"), change(0, 0, 0, 0, "x")],
        );
        assert_eq!(t, "xé // TODO b\nLINE two\n");
        apply_changes(&mut t, &[change(0, 12, 1, 0, " ")]);
        assert_eq!(t, "xé // TODO b LINE two\n");
        apply_changes(
            &mut t,
            &[ContentChange {
                range: None,
                text: "new".into(),
            }],
        );
        assert_eq!(t, "new");
    }
}
