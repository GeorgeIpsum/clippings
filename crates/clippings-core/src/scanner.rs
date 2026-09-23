//! Scanning one file's contents into todos (spec section 5.5).

use crate::comments::strip_line_comment;
use crate::extract::extract;
use crate::fs::Fs;
use crate::model::{ExtraLine, Todo};
use crate::pattern::{Engine, Lossy, ScanPattern};
use crate::position::LineIndex;
use grep_matcher::{LineTerminator, Matcher};
use grep_searcher::{BinaryDetection, MmapChoice, Searcher, SearcherBuilder, Sink, SinkMatch};
use std::io;
use std::path::Path;

/// Files larger than this are streamed in line mode and skipped in multi-line mode.
pub const HEAP_LIMIT: usize = 64 * 1024 * 1024;
/// Label text is capped this many bytes after the match start.
pub const WINDOW_CAP: usize = 1000;

/// Decodes file bytes to UTF-8: transcodes UTF-16 with a BOM, strips a UTF-8
/// BOM. Returns `None` for binary content (a NUL byte anywhere).
pub fn decode(bytes: Vec<u8>) -> Option<Vec<u8>> {
    let text = if bytes.starts_with(&[0xFF, 0xFE]) || bytes.starts_with(&[0xFE, 0xFF]) {
        let enc = if bytes[0] == 0xFF {
            encoding_rs::UTF_16LE
        } else {
            encoding_rs::UTF_16BE
        };
        enc.decode_with_bom_removal(&bytes)
            .0
            .into_owned()
            .into_bytes()
    } else if bytes.starts_with(&[0xEF, 0xBB, 0xBF]) {
        bytes[3..].to_vec()
    } else {
        bytes
    };
    memchr::memchr(0, &text).is_none().then_some(text)
}

fn floor_char_boundary(b: &[u8], mut i: usize) -> usize {
    i = i.min(b.len());
    while i > 0 && i < b.len() && (b[i] & 0b1100_0000) == 0b1000_0000 {
        i -= 1;
    }
    i
}

fn ceil_char_boundary(b: &[u8], mut i: usize) -> usize {
    while i < b.len() && (b[i] & 0b1100_0000) == 0b1000_0000 {
        i += 1;
    }
    i
}

/// Builds todos from one searcher match: `bytes` starts at a line start,
/// `first_line` is its 0-based line number.
fn todos_from_lines(
    p: &ScanPattern,
    bytes: &[u8],
    first_line: u32,
    path: &str,
    out: &mut Vec<Todo>,
) {
    let li = LineIndex::new(bytes);
    let shift = |mut pos: crate::position::Position| {
        pos.line += first_line;
        pos
    };
    let _ = p.matcher.find_iter(bytes, |m| {
        let (s, e) = (m.start(), m.end());
        if s == e || (!p.multi_line && bytes[s..e].contains(&b'\n')) {
            return true;
        }
        let start_line = li.line_of(s);
        let end_line = li.line_of(e - 1);
        let (_, first_end) = li.line_range(start_line);
        let win_end = floor_char_boundary(bytes, first_end.max(s).min(s + WINDOW_CAP));
        let window = &bytes[s..win_end];
        // Lossy conversion may change offsets; `lossy` maps them back.
        let (text, lossy) = match std::str::from_utf8(window) {
            Ok(t) => (t, None),
            Err(_) => ("", Some(Lossy::new(window))),
        };
        let text = lossy.as_ref().map_or(text, |l| l.text.as_str());
        let match_len = lossy
            .as_ref()
            .map_or(e.min(win_end) - s, |l| l.to_str(e.min(win_end) - s));
        let ex = extract(p, text, match_len, path);

        let (line_start, _) = li.line_range(start_line);
        let before_start = ceil_char_boundary(bytes, s.saturating_sub(WINDOW_CAP).max(line_start));
        let before = String::from_utf8_lossy(&bytes[before_start..s])
            .trim()
            .to_string();

        let extra_lines = if end_line > start_line {
            (start_line + 1..=end_line)
                .filter_map(|l| {
                    let (a, b) = li.line_range(l);
                    let b = floor_char_boundary(bytes, b.min(a + WINDOW_CAP));
                    let text = strip_line_comment(&String::from_utf8_lossy(&bytes[a..b]), path);
                    (!text.is_empty() && text != ex.tag).then(|| ExtraLine {
                        line: l as u32 + first_line,
                        text,
                    })
                })
                .collect()
        } else {
            Vec::new()
        };
        let to_byte = |i: usize| lossy.as_ref().map_or(i, |l| l.to_byte(i));
        let tag_pos = ex.tag_range.map(|(a, b)| {
            (
                shift(li.position(s + to_byte(a))),
                shift(li.position(s + to_byte(b))),
            )
        });
        out.push(Todo {
            start: shift(li.position(s)),
            end: shift(li.position(e)),
            text_end: shift(li.position(first_end.max(s))),
            tag_start: tag_pos.map(|t| t.0),
            tag_end: tag_pos.map(|t| t.1),
            tag: ex.tag,
            sub_tag: ex.sub_tag,
            before,
            after: ex.after,
            extra_lines,
        });
        true
    });
}

struct TodoSink<'a> {
    pattern: &'a ScanPattern,
    path: &'a str,
    todos: Vec<Todo>,
    binary: bool,
}

impl Sink for TodoSink<'_> {
    type Error = io::Error;

    fn matched(&mut self, _: &Searcher, m: &SinkMatch<'_>) -> Result<bool, io::Error> {
        let first_line = m.line_number().unwrap_or(1).saturating_sub(1) as u32;
        todos_from_lines(
            self.pattern,
            m.bytes(),
            first_line,
            self.path,
            &mut self.todos,
        );
        Ok(true)
    }

    fn binary_data(&mut self, _: &Searcher, _: u64) -> Result<bool, io::Error> {
        self.binary = true;
        Ok(false)
    }
}

/// A searcher configured for `p`. Build one per thread and reuse it
/// across files with [`scan_file_with`].
pub fn searcher(p: &ScanPattern) -> Searcher {
    let term = match (p.multi_line, p.engine) {
        (true, _) | (false, Engine::Fancy) => LineTerminator::byte(b'\n'),
        (false, Engine::Grep) => LineTerminator::crlf(),
    };
    SearcherBuilder::new()
        .line_number(true)
        .multi_line(p.multi_line)
        .line_terminator(term)
        .memory_map(MmapChoice::never())
        .binary_detection(BinaryDetection::quit(b'\x00'))
        .heap_limit(Some(HEAP_LIMIT))
        .build()
}

fn scan_text_with(s: &mut Searcher, p: &ScanPattern, text: &[u8], path: &str) -> Vec<Todo> {
    let mut sink = TodoSink {
        pattern: p,
        path,
        todos: Vec::new(),
        binary: false,
    };
    // Binary detection was done by `decode`; the searcher only sees text.
    s.set_binary_detection(BinaryDetection::none());
    let _ = s.search_slice(&p.matcher, text, &mut sink);
    sink.todos
}

/// Scans decoded UTF-8 text held in memory, such as an open buffer.
pub fn scan_text(p: &ScanPattern, text: &[u8], path: &str) -> Vec<Todo> {
    scan_text_with(&mut searcher(p), p, text, path)
}

/// Scans a file on disk. `Ok(None)` means binary or skipped.
pub fn scan_file(fs: &dyn Fs, p: &ScanPattern, path: &Path) -> io::Result<Option<Vec<Todo>>> {
    scan_file_with(&mut searcher(p), fs, p, path)
}

/// [`scan_file`] with a reused searcher, which must come from
/// [`searcher`] for the same pattern.
pub fn scan_file_with(
    s: &mut Searcher,
    fs: &dyn Fs,
    p: &ScanPattern,
    path: &Path,
) -> io::Result<Option<Vec<Todo>>> {
    let path_str = path.to_string_lossy();
    let len = fs.len(path)? as usize;
    if len <= HEAP_LIMIT {
        return Ok(decode(fs.read(path)?).map(|text| scan_text_with(s, p, &text, &path_str)));
    }
    if p.multi_line {
        tracing::debug!("skipping {path_str}: larger than the multi-line heap limit");
        return Ok(None);
    }
    let reader = fs.open(path)?;
    let mut sink = TodoSink {
        pattern: p,
        path: &path_str,
        todos: Vec::new(),
        binary: false,
    };
    s.set_binary_detection(BinaryDetection::quit(b'\x00'));
    if let Err(e) = s.search_reader(&p.matcher, reader, &mut sink) {
        tracing::debug!("skipping {path_str}: {e}");
        return Ok(None);
    }
    Ok((!sink.binary).then_some(sink.todos))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::CoreConfig;
    use crate::pattern::build;
    use crate::position::Position;

    fn scan(cfg: CoreConfig, text: &str, path: &str) -> Vec<Todo> {
        scan_text(&build(&cfg).unwrap(), text.as_bytes(), path)
    }

    #[test]
    fn two_tags_on_one_line_give_two_todos() {
        let t = scan(CoreConfig::default(), "a // TODO one # FIXME two\n", "a.py");
        assert_eq!(t.len(), 2);
        assert_eq!((t[0].tag.as_str(), t[1].tag.as_str()), ("TODO", "FIXME"));
        assert_eq!(
            t[1].start,
            Position {
                line: 0,
                character: 14
            }
        );
    }

    #[test]
    fn positions_and_before_after() {
        let t = scan(
            CoreConfig::default(),
            "x\nlet y = 1; // TODO rename y\n",
            "a.ts",
        );
        assert_eq!(t.len(), 1);
        assert_eq!(
            t[0].start,
            Position {
                line: 1,
                character: 11
            }
        );
        assert_eq!(
            t[0].tag_start,
            Some(Position {
                line: 1,
                character: 14
            })
        );
        assert_eq!(t[0].before, "let y = 1;");
        assert_eq!(t[0].after, "rename y");
    }

    #[test]
    fn crlf_is_not_part_of_after() {
        let t = scan(CoreConfig::default(), "// TODO a\r\n// TODO b\r\n", "a.ts");
        assert_eq!(
            t.iter().map(|t| t.after.as_str()).collect::<Vec<_>>(),
            vec!["a", "b"]
        );
    }

    #[test]
    fn non_ascii_before_tag_uses_utf16_columns() {
        let t = scan(CoreConfig::default(), "ééé // TODO utf\n", "a.ts");
        assert_eq!(
            t[0].start,
            Position {
                line: 0,
                character: 4
            }
        );
        assert_eq!(
            t[0].tag_start,
            Some(Position {
                line: 0,
                character: 7
            })
        );
    }

    #[test]
    fn long_lines_are_capped() {
        let text = format!("// TODO {}\n", "x".repeat(5000));
        let t = scan(CoreConfig::default(), &text, "a.ts");
        assert!(t[0].after.len() <= WINDOW_CAP);
        assert!(t[0].after.starts_with("xxx"));
    }

    #[test]
    fn multi_line_match_has_extra_lines() {
        let cfg = CoreConfig {
            regex: r"(//)\s*($TAGS).*(\n\s*//\s{2,}.*)*".into(),
            ..Default::default()
        };
        let t = scan(
            cfg,
            "// TODO first\n//   second\n//   third\ncode\n",
            "a.ts",
        );
        assert_eq!(t.len(), 1);
        assert_eq!(t[0].after, "first");
        assert_eq!(
            t[0].extra_lines,
            vec![
                ExtraLine {
                    line: 1,
                    text: "second".into()
                },
                ExtraLine {
                    line: 2,
                    text: "third".into()
                },
            ]
        );
    }

    #[test]
    fn fancy_multi_line_match_reaches_across_lines() {
        // Look-behind needs fancy-regex; the match spans three lines.
        let cfg = CoreConfig {
            regex: r"(?<=// )($TAGS).*(\n\s*//\s{2,}.*)*".into(),
            ..Default::default()
        };
        let t = scan(
            cfg,
            "// TODO first\n//   second\n//   third\ncode\n",
            "a.ts",
        );
        assert_eq!(t.len(), 1);
        assert_eq!(t[0].end.line, 2);
        assert_eq!(t[0].extra_lines.len(), 2);
    }

    #[test]
    fn adjacent_multi_line_matches_stay_separate() {
        let cfg = CoreConfig {
            regex: r"(//)\s*($TAGS).*(\n\s*//\s{2,}.*)*".into(),
            ..Default::default()
        };
        let t = scan(cfg, "// TODO a\n//   a2\n// FIXME b\n//   b2\n", "a.ts");
        assert_eq!(t.len(), 2);
        assert_eq!(t[1].start.line, 2);
        assert_eq!(t[1].extra_lines[0].text, "b2");
    }

    #[test]
    fn decode_detects_binary_anywhere() {
        let mut b = vec![b'a'; 70_000];
        b.extend_from_slice(b"\0 // TODO");
        assert!(decode(b).is_none());
        assert!(decode(b"// TODO".to_vec()).is_some());
    }

    #[test]
    fn decode_transcodes_utf16_and_strips_bom() {
        let mut b = vec![0xFF, 0xFE];
        for u in "// TODO wide".encode_utf16() {
            b.extend_from_slice(&u.to_le_bytes());
        }
        assert_eq!(decode(b).unwrap(), b"// TODO wide");
        assert_eq!(decode(b"\xEF\xBB\xBF// TODO".to_vec()).unwrap(), b"// TODO");
    }

    #[test]
    fn markdown_checkbox_tag() {
        let t = scan(
            CoreConfig::default(),
            "- [ ] write docs\n- [x] done\n",
            "a.md",
        );
        assert_eq!(
            t.iter()
                .map(|t| (t.tag.as_str(), t.after.as_str()))
                .collect::<Vec<_>>(),
            vec![("[ ]", "write docs"), ("[x]", "done")]
        );
    }

    #[test]
    fn latin1_bytes_do_not_hide_todos() {
        let p = build(&CoreConfig::default()).unwrap();
        let t = scan_text(&p, b"caf\xe9 // TODO latin1\n", "a.c");
        assert_eq!(t.len(), 1);
        assert_eq!(t[0].after, "latin1");
    }

    #[test]
    fn invalid_utf8_after_the_tag_keeps_tag_positions() {
        let p = build(&CoreConfig::default()).unwrap();
        let t = scan_text(&p, b"// TODO caf\xe9\n", "a.c");
        assert_eq!(t.len(), 1);
        assert_eq!(
            (t[0].tag_start, t[0].tag_end),
            (
                Some(Position {
                    line: 0,
                    character: 3
                }),
                Some(Position {
                    line: 0,
                    character: 7
                })
            )
        );
    }

    #[test]
    fn tag_at_end_of_file_without_newline() {
        let t = scan(CoreConfig::default(), "x\n// TODO last", "a.ts");
        assert_eq!(t.len(), 1);
        assert_eq!(t[0].after, "last");
        assert!(scan(CoreConfig::default(), "", "a.ts").is_empty());
    }

    #[test]
    fn empty_matching_regex_terminates() {
        let cfg = CoreConfig {
            regex: "($TAGS)?".into(),
            ..Default::default()
        };
        let t = scan(cfg, "abc\n// TODO x\n", "a.ts");
        assert_eq!(
            t.iter().map(|t| t.tag.as_str()).collect::<Vec<_>>(),
            vec!["TODO"]
        );
    }

    #[test]
    fn a_reused_searcher_scans_like_a_fresh_one() {
        let dir = tempfile::tempdir().unwrap();
        let big = dir.path().join("big.txt");
        let mut body = "// TODO first\n".to_string();
        while body.len() <= HEAP_LIMIT {
            body.push_str(&format!("{}\n", "a".repeat(1023)));
        }
        body.push_str("\0\n// FIXME after nul\n");
        std::fs::write(&big, &body).unwrap();
        let small = dir.path().join("a.ts");
        std::fs::write(&small, "// TODO a\0").unwrap();
        let text = dir.path().join("b.ts");
        std::fs::write(&text, "x\n// TODO b\n").unwrap();
        let p = build(&CoreConfig::default()).unwrap();
        let fs = crate::fs::NativeFs;
        let mut s = searcher(&p);
        for path in [&text, &big, &small, &text, &big] {
            assert_eq!(
                scan_file_with(&mut s, &fs, &p, path).unwrap(),
                scan_file(&fs, &p, path).unwrap(),
                "{}",
                path.display()
            );
        }
        assert!(scan_file(&fs, &p, &big).unwrap().is_none(), "binary");
    }

    #[test]
    fn files_over_the_heap_limit_are_streamed_in_line_mode() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("big.txt");
        let mut body = "// TODO first\n".to_string();
        let filler = format!("{}\n", "a".repeat(1023));
        while body.len() <= HEAP_LIMIT {
            body.push_str(&filler);
        }
        body.push_str("// FIXME last\n");
        std::fs::write(&path, &body).unwrap();
        let p = build(&CoreConfig::default()).unwrap();
        let t = scan_file(&crate::fs::NativeFs, &p, &path).unwrap().unwrap();
        assert_eq!(
            t.iter().map(|t| t.tag.as_str()).collect::<Vec<_>>(),
            vec!["TODO", "FIXME"]
        );
        assert_eq!(t[1].start.line as usize, body.lines().count() - 1);
        let multi = build(&CoreConfig {
            enable_multi_line: true,
            ..Default::default()
        })
        .unwrap();
        assert!(scan_file(&crate::fs::NativeFs, &multi, &path)
            .unwrap()
            .is_none());
    }
}
