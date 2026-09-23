//! Byte offsets to LSP positions: 0-based line, 0-based UTF-16 column.

use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct Position {
    pub line: u32,
    pub character: u32,
}

/// A half-open range of positions.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct Range {
    pub start: Position,
    pub end: Position,
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

    /// Number of lines (a text ending in a newline has an empty last line).
    pub fn line_count(&self) -> usize {
        self.starts.len()
    }

    /// Byte offset of a position, clamped to its line. The inverse of `position`.
    pub fn offset(&self, pos: Position) -> usize {
        let line = (pos.line as usize).min(self.starts.len() - 1);
        let (start, end) = self.line_range(line);
        let text = String::from_utf8_lossy(&self.text[start..end]);
        let mut units = 0usize;
        for (i, ch) in text.char_indices() {
            if units >= pos.character as usize {
                return start + i;
            }
            units += ch.len_utf16();
        }
        end
    }

    /// Position of the end of a line's content.
    pub fn line_end(&self, line: usize) -> Position {
        let line = line.min(self.starts.len() - 1);
        self.position(self.line_range(line).1)
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

    #[test]
    fn offset_inverts_position() {
        let t = "ééé // TODO\n😀 x".as_bytes();
        let li = LineIndex::new(t);
        for off in [0, 2, 4, 7, 11, 14, 16, 20] {
            if std::str::from_utf8(&t[..off]).is_ok() {
                assert_eq!(li.offset(li.position(off)), off, "offset {off}");
            }
        }
        assert_eq!(
            li.line_end(0),
            Position {
                line: 0,
                character: 11
            }
        );
        assert_eq!(
            li.offset(Position {
                line: 0,
                character: 99
            }),
            14,
            "clamped to line end"
        );
    }
}
