//! Decoration ranges per open document (spec section 5.13).

use crate::model::Todo;
use crate::pattern::ScanPattern;
use crate::position::{LineIndex, Position, Range};
use crate::settings::Settings;
use crate::styles::Resolver;
use std::collections::BTreeMap;

/// The main pattern with capture groups, for `capture-groups:n,m` highlights.
pub enum CaptureRegex {
    Std(regex::bytes::Regex),
    Fancy(fancy_regex::Regex),
}

impl CaptureRegex {
    pub fn new(p: &ScanPattern) -> Option<CaptureRegex> {
        let source = format!("{}{}", p.flags, p.source);
        match regex::bytes::Regex::new(&source) {
            Ok(re) => Some(CaptureRegex::Std(re)),
            Err(_) => fancy_regex::Regex::new(&source)
                .ok()
                .map(CaptureRegex::Fancy),
        }
    }

    /// Byte ranges of each group for the match that starts at `start`.
    pub fn groups_at(&self, text: &[u8], start: usize) -> Option<Vec<Option<(usize, usize)>>> {
        match self {
            CaptureRegex::Std(re) => {
                let c = re.captures_at(text, start)?;
                (c.get(0)?.start() == start).then(|| {
                    (0..c.len())
                        .map(|i| c.get(i).map(|m| (m.start(), m.end())))
                        .collect()
                })
            }
            CaptureRegex::Fancy(re) => {
                let s = std::str::from_utf8(text).ok()?;
                let c = re.captures_from_pos(s, start).ok()??;
                (c.get(0)?.start() == start).then(|| {
                    (0..c.len())
                        .map(|i| c.get(i).map(|m| (m.start(), m.end())))
                        .collect()
                })
            }
        }
    }
}

/// Decoration keys and ranges for one document. Keys absent from the result
/// have no ranges. Returns an empty map when highlights are disabled.
pub fn decorate(
    text: &[u8],
    todos: &[Todo],
    settings: &Settings,
    pattern: &ScanPattern,
) -> BTreeMap<String, Vec<Range>> {
    let mut out: BTreeMap<String, Vec<Range>> = BTreeMap::new();
    if !settings.highlights.enabled {
        return out;
    }
    let li = LineIndex::new(text);
    let resolver = Resolver::new(settings);
    let mut captures: Option<Option<CaptureRegex>> = None;
    for t in todos {
        let s = li.offset(t.start);
        let e = li.offset(t.end).max(s);
        let raw = String::from_utf8_lossy(&text[s..e]).into_owned();
        let (key, tag_range) = match (t.tag.is_empty(), t.tag_start, t.tag_end) {
            (false, Some(a), Some(b)) => (
                settings.key_of(&t.tag).to_string(),
                Range { start: a, end: b },
            ),
            _ => {
                let lead = raw.len() - raw.trim_start().len();
                let trail = raw.len() - raw.trim_end().len();
                let range = Range {
                    start: li.position(s + lead),
                    end: li.position(e - trail),
                };
                (raw.trim().to_string(), range)
            }
        };
        let end_line = if t.end.character == 0 && t.end.line > t.start.line {
            t.end.line - 1
        } else {
            t.end.line
        };
        let line_end = li.line_end(end_line as usize);
        let kind = resolver.kind(&key).unwrap_or_default();
        let mut add = |k: &str, r: Range| out.entry(k.to_string()).or_default().push(r);
        match kind.as_str() {
            "none" => {}
            "text" => add(
                &key,
                Range {
                    start: tag_range.start,
                    end: line_end,
                },
            ),
            "tag-and-comment" => add(
                &key,
                Range {
                    start: t.start,
                    end: tag_range.end,
                },
            ),
            "text-and-comment" => add(
                &key,
                Range {
                    start: t.start,
                    end: line_end,
                },
            ),
            "line" | "whole-line" => add(
                &key,
                Range {
                    start: Position {
                        line: tag_range.start.line,
                        character: 0,
                    },
                    end: line_end,
                },
            ),
            "tag-and-subTag" | "tag-and-subtag" => {
                add(&key, tag_range);
                if let Some(sub) = t
                    .sub_tag
                    .as_deref()
                    .filter(|s| settings.custom(s).is_some())
                {
                    let from = li.offset(tag_range.end);
                    let (_, eol) = li.line_range(li.line_of(from));
                    let hay = String::from_utf8_lossy(&text[from..eol.max(from)]).into_owned();
                    if let Some(i) = hay.find(sub) {
                        let a = from + i;
                        add(
                            sub,
                            Range {
                                start: li.position(a),
                                end: li.position(a + sub.len()),
                            },
                        );
                    }
                }
            }
            k if k.starts_with("capture-groups:") => {
                let re = captures.get_or_insert_with(|| CaptureRegex::new(pattern));
                if let Some(groups) = re.as_ref().and_then(|re| re.groups_at(text, s)) {
                    for n in k["capture-groups:".len()..]
                        .split(',')
                        .filter_map(|n| n.trim().parse::<usize>().ok())
                    {
                        if let Some(Some((a, b))) = groups.get(n) {
                            add(
                                &key,
                                Range {
                                    start: li.position(*a),
                                    end: li.position(*b),
                                },
                            );
                        }
                    }
                }
            }
            _ => add(&key, tag_range),
        }
    }
    for ranges in out.values_mut() {
        ranges.sort();
        ranges.dedup();
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pattern::build;
    use crate::position::{Position, Range};
    use crate::scanner::scan_text;
    use crate::settings::{Attributes, Settings};

    fn run(
        text: &str,
        kind: Option<&str>,
        f: impl FnOnce(&mut Settings),
    ) -> BTreeMap<String, Vec<Range>> {
        let mut s = Settings::default();
        if let Some(k) = kind {
            s.highlights.default_highlight = Attributes {
                kind: Some(k.into()),
                ..Default::default()
            };
        }
        f(&mut s);
        let p = build(&s.core()).unwrap();
        let todos = scan_text(&p, text.as_bytes(), "a.ts");
        decorate(text.as_bytes(), &todos, &s, &p)
    }

    fn r(l1: u32, c1: u32, l2: u32, c2: u32) -> Range {
        Range {
            start: Position {
                line: l1,
                character: c1,
            },
            end: Position {
                line: l2,
                character: c2,
            },
        }
    }

    const TEXT: &str = "x();  // TODO fix this\n";

    #[test]
    fn tag_is_the_default() {
        assert_eq!(
            run(TEXT, None, |_| {}),
            BTreeMap::from([("TODO".to_string(), vec![r(0, 9, 0, 13)])])
        );
    }

    #[test]
    fn range_types() {
        assert_eq!(
            run(TEXT, Some("text"), |_| {})["TODO"],
            vec![r(0, 9, 0, 22)]
        );
        assert_eq!(
            run(TEXT, Some("tag-and-comment"), |_| {})["TODO"],
            vec![r(0, 6, 0, 13)]
        );
        assert_eq!(
            run(TEXT, Some("text-and-comment"), |_| {})["TODO"],
            vec![r(0, 6, 0, 22)]
        );
        assert_eq!(
            run(TEXT, Some("line"), |_| {})["TODO"],
            vec![r(0, 0, 0, 22)]
        );
        assert_eq!(
            run(TEXT, Some("whole-line"), |_| {})["TODO"],
            vec![r(0, 0, 0, 22)]
        );
        assert!(run(TEXT, Some("none"), |_| {}).is_empty());
        // Group 2 of the default regex is the nested list-marker group; the tag is group 3.
        assert_eq!(
            run(TEXT, Some("capture-groups:1,3"), |_| {})["TODO"],
            vec![r(0, 6, 0, 8), r(0, 9, 0, 13)]
        );
    }

    #[test]
    fn groups_key_by_group_name() {
        let d = run(TEXT, None, |s| {
            s.general
                .tag_groups
                .insert("WORK".into(), vec!["TODO".into()]);
        });
        assert!(d.contains_key("WORK") && !d.contains_key("TODO"));
    }

    #[test]
    fn sub_tags_get_their_own_key_when_configured() {
        let text = "// TODO(alice) ship\n";
        let d = run(text, Some("tag-and-subTag"), |s| {
            s.regex.sub_tag_regex = r"^\s*\((.*?)\)".into();
            s.highlights
                .custom_highlight
                .insert("alice".into(), Attributes::default());
        });
        assert_eq!(d["TODO"], vec![r(0, 3, 0, 7)]);
        assert_eq!(d["alice"], vec![r(0, 8, 0, 13)]);
    }

    #[test]
    fn disabled_highlights_clear_everything() {
        assert!(run(TEXT, None, |s| s.highlights.enabled = false).is_empty());
    }

    #[test]
    fn matches_without_tags_token_use_the_whole_match() {
        // Without `$TAGS` the whole match is the tag, keyed by its trimmed text.
        let d = run("  NOTE(bob) read\n", None, |s| {
            s.regex.regex = r"\s*NOTE\(\w+\)".into()
        });
        assert_eq!(d["NOTE(bob)"], vec![r(0, 0, 0, 11)]);
    }
}
