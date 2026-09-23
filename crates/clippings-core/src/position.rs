//! Byte offsets to LSP positions: 0-based line, 0-based UTF-16 column.

use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct Position {
    pub line: u32,
    pub character: u32,
}

/// Line start offsets of a UTF-8 text.
pub struct LineIndex<'a> {
    text: &'a [u8],
    starts: Vec<usize>,
}

impl<'a> LineIndex<'a> {
    pub fn new(text: &'a [u8]) -> Self {
        let mut starts = vec![0];
        starts.extend(memchr::memchr_iter(b'\n', text).map(|i| i + 1));
        Self { text, starts }
    }

    /// 0-based line containing byte `offset`.
    pub fn line_of(&self, offset: usize) -> usize {
        match self.starts.binary_search(&offset) {
            Ok(l) => l,
            Err(l) => l - 1,
        }
    }

    pub fn line_start(&self, line: usize) -> usize {
        self.starts[line]
    }

    /// Byte range of a line's content, without its `\n` or `\r\n`.
    pub fn line_range(&self, line: usize) -> (usize, usize) {
        let start = self.starts[line];
        let mut end = self.starts.get(line + 1).map_or(self.text.len(), |s| s - 1);
        if end > start && self.text[end - 1] == b'\r' {
            end -= 1;
        }
        (start, end)
    }

    pub fn position(&self, offset: usize) -> Position {
        let line = self.line_of(offset);
        let start = self.starts[line];
        let character = utf16_len(&self.text[start..offset]);
        Position {
            line: line as u32,
            character: character as u32,
        }
    }
}

/// Number of UTF-16 code units in a UTF-8 byte slice (lossy on bad bytes).
pub fn utf16_len(bytes: &[u8]) -> usize {
    String::from_utf8_lossy(bytes).encode_utf16().count()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ascii_positions() {
        let t = b"ab\ncd // TODO x\n";
        let li = LineIndex::new(t);
        assert_eq!(
            li.position(9),
            Position {
                line: 1,
                character: 6
            }
        );
        assert_eq!(li.line_range(1), (3, 15));
    }

    #[test]
    fn non_ascii_counts_utf16_units() {
        let t = "ééé // TODO utf\n😀 // TODO".as_bytes();
        let li = LineIndex::new(t);
        let first = t.windows(4).position(|w| w == b"TODO").unwrap();
        assert_eq!(
            li.position(first),
            Position {
                line: 0,
                character: 7
            }
        );
        let second = first
            + 4
            + t[first + 4..]
                .windows(4)
                .position(|w| w == b"TODO")
                .unwrap();
        assert_eq!(
            li.position(second),
            Position {
                line: 1,
                character: 6
            }
        );
    }

    #[test]
    fn crlf_line_range_excludes_cr() {
        let t = b"a\r\nb";
        let li = LineIndex::new(t);
        assert_eq!(li.line_range(0), (0, 1));
        assert_eq!(li.line_range(1), (3, 4));
    }
}
