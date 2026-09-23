//! Tag extraction (spec section 5.6), a port of todo-tree's `extractTag`.

use crate::comments::syntax_for;
use crate::pattern::ScanPattern;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Extracted {
    pub tag: String,
    /// Byte range of the tag within the window.
    pub tag_range: Option<(usize, usize)>,
    pub sub_tag: Option<String>,
    pub after: String,
}

fn configured_spelling(p: &ScanPattern, found: &str) -> String {
    p.tags
        .iter()
        .find(|t| {
            if p.case_sensitive {
                t.as_str() == found
            } else {
                t.to_lowercase() == found.to_lowercase()
            }
        })
        .cloned()
        .unwrap_or_else(|| found.to_string())
}

/// `window` starts at the match start and ends at the end of the match's
/// first line, capped. `match_len` is the match length within the window.
pub fn extract(p: &ScanPattern, window: &str, match_len: usize, path: &str) -> Extracted {
    let (tag, tag_range, right) = match &p.tag_re {
        Some(re) => match re.find(window) {
            Some(m) => (
                configured_spelling(p, m.as_str()),
                Some((m.start(), m.end())),
                &window[m.end()..],
            ),
            None => (String::new(), None, window),
        },
        None => {
            let end = match_len.min(window.len());
            let end = (0..=end)
                .rev()
                .find(|i| window.is_char_boundary(*i))
                .unwrap_or(0);
            (
                window[..end].trim().to_string(),
                Some((0, end)),
                &window[end..],
            )
        }
    };
    let mut right = right.trim().to_string();
    // A block comment opened before the tag closes at the end of the line.
    let opened_before = &window[..tag_range.map_or(0, |r| r.0)];
    for (open, close) in syntax_for(path).block {
        if opened_before.contains(open) {
            if let Some(stripped) = right.strip_suffix(close) {
                right = stripped.trim_end().to_string();
            }
        }
    }
    let (sub_tag, after) = match &p.sub_tag_re {
        Some(re) => {
            let sub = re
                .captures(&right)
                .and_then(|c| c.get(1))
                .map(|m| m.as_str().to_string());
            (sub, re.replace(&right, "").trim().to_string())
        }
        None => (None, right),
    };
    Extracted {
        tag,
        tag_range,
        sub_tag,
        after,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::CoreConfig;
    use crate::pattern::build;

    fn ex(cfg: CoreConfig, window: &str, path: &str) -> Extracted {
        extract(&build(&cfg).unwrap(), window, 0, path)
    }

    #[test]
    fn tag_and_after() {
        let e = ex(CoreConfig::default(), "// TODO fix the thing", "a.ts");
        assert_eq!(e.tag, "TODO");
        assert_eq!(e.tag_range, Some((3, 7)));
        assert_eq!(e.after, "fix the thing");
        assert_eq!(e.sub_tag, None);
    }

    #[test]
    fn configured_spelling_when_case_insensitive() {
        let cfg = CoreConfig {
            regex_case_sensitive: false,
            ..Default::default()
        };
        assert_eq!(ex(cfg, "# todo lower", "a.py").tag, "TODO");
    }

    #[test]
    fn sub_tag_is_group_one_and_removed_from_after() {
        let cfg = CoreConfig {
            sub_tag_regex: r"^\s*\((.*?)\)".into(),
            ..Default::default()
        };
        let e = ex(cfg, "// TODO(alice) ship it", "a.rs");
        assert_eq!(e.sub_tag.as_deref(), Some("alice"));
        assert_eq!(e.after, "ship it");
    }

    #[test]
    fn block_comment_closer_is_stripped() {
        assert_eq!(
            ex(CoreConfig::default(), "/* FIXME later */", "a.c").after,
            "later"
        );
        assert_eq!(
            ex(CoreConfig::default(), "<!-- TODO doc -->", "a.md").after,
            "doc"
        );
    }

    #[test]
    fn without_tags_token_whole_match_is_tag() {
        let cfg = CoreConfig {
            regex: r"NOTE\(\w+\)".into(),
            ..Default::default()
        };
        let p = build(&cfg).unwrap();
        let e = extract(&p, "NOTE(bob) read this", 9, "a.ts");
        assert_eq!(e.tag, "NOTE(bob)");
        assert_eq!(e.after, "read this");
    }

    #[test]
    fn longest_tag_wins() {
        let cfg = CoreConfig {
            tags: vec!["TODO".into(), "TODOS".into()],
            ..Default::default()
        };
        assert_eq!(ex(cfg, "// TODOS list", "a.ts").tag, "TODOS");
    }
}
