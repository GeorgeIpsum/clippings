//! Regex construction (spec section 5.4): `($TAGS)` expansion, flags,
//! engine selection and the fancy-regex fallback.

use crate::config::CoreConfig;
use crate::CoreError;
use grep_matcher::{Match, Matcher, NoCaptures, NoError};
use grep_regex::{ErrorKind, RegexMatcher, RegexMatcherBuilder};

pub const TAGS_TOKEN: &str = "($TAGS)";

/// Tags escaped and joined with `|`, longest first, ties in configured order.
pub fn tag_alternation(tags: &[String]) -> String {
    let mut sorted: Vec<&String> = tags.iter().collect();
    sorted.sort_by_key(|t| std::cmp::Reverse(t.chars().count()));
    sorted
        .iter()
        .map(|t| regex::escape(t))
        .collect::<Vec<_>>()
        .join("|")
}

/// Replaces the literal token `($TAGS)` with `(<alternation>)`.
pub fn expand_tags(source: &str, tags: &[String]) -> String {
    source.replace(TAGS_TOKEN, &format!("({})", tag_alternation(tags)))
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Engine {
    Grep,
    Fancy,
}

/// Lossy UTF-8 conversion of bytes with a map between string and byte offsets.
pub(crate) struct Lossy {
    pub text: String,
    /// (string offset, byte offset) at the start of each valid run and each
    /// replacement character, then at the end.
    map: Vec<(usize, usize)>,
}

impl Lossy {
    pub fn new(bytes: &[u8]) -> Self {
        let mut text = String::with_capacity(bytes.len());
        let mut map = Vec::new();
        let mut byte = 0;
        for chunk in bytes.utf8_chunks() {
            map.push((text.len(), byte));
            text.push_str(chunk.valid());
            byte += chunk.valid().len();
            for _ in chunk.invalid() {
                map.push((text.len(), byte));
                text.push('\u{FFFD}');
                byte += 1;
            }
        }
        map.push((text.len(), byte));
        Self { text, map }
    }

    /// The byte offset of string offset `s`.
    pub fn to_byte(&self, s: usize) -> usize {
        let i = self.map.partition_point(|&(so, _)| so <= s) - 1;
        let (so, bo) = self.map[i];
        // Inside a valid chunk offsets advance together; a replacement
        // character (3 bytes) stands for one invalid byte.
        if self
            .map
            .get(i + 1)
            .is_some_and(|&(nso, nbo)| nso - so != nbo - bo)
        {
            bo
        } else {
            bo + (s - so)
        }
    }

    /// The string offset of byte offset `b`.
    pub fn to_str(&self, b: usize) -> usize {
        let i = self.map.partition_point(|&(_, bo)| bo <= b) - 1;
        let (so, bo) = self.map[i];
        so + (b - bo)
    }
}

/// fancy-regex behind the `grep_matcher::Matcher` trait.
#[derive(Clone, Debug)]
pub struct FancyMatcher {
    re: fancy_regex::Regex,
}

impl FancyMatcher {
    fn find_str(&self, text: &str, at: usize) -> Option<(usize, usize)> {
        match self.re.find_from_pos(text, at) {
            Ok(Some(m)) => Some((m.start(), m.end())),
            Ok(None) => None,
            Err(e) => {
                tracing::debug!("fancy-regex runtime error: {e}");
                None
            }
        }
    }
}

impl Matcher for FancyMatcher {
    type Captures = NoCaptures;
    type Error = NoError;

    fn find_at(&self, haystack: &[u8], at: usize) -> Result<Option<Match>, NoError> {
        if let Ok(text) = std::str::from_utf8(haystack) {
            return Ok(self.find_str(text, at).map(|(s, e)| Match::new(s, e)));
        }
        let lossy = Lossy::new(haystack);
        Ok(self
            .find_str(&lossy.text, lossy.to_str(at))
            .map(|(s, e)| Match::new(lossy.to_byte(s), lossy.to_byte(e))))
    }

    fn new_captures(&self) -> Result<NoCaptures, NoError> {
        Ok(NoCaptures::new())
    }

    fn line_terminator(&self) -> Option<grep_matcher::LineTerminator> {
        Some(grep_matcher::LineTerminator::byte(b'\n'))
    }
}

#[derive(Clone, Debug)]
pub enum PatternMatcher {
    Grep(RegexMatcher),
    Fancy(FancyMatcher),
}

impl Matcher for PatternMatcher {
    type Captures = NoCaptures;
    type Error = NoError;

    fn find_at(&self, haystack: &[u8], at: usize) -> Result<Option<Match>, NoError> {
        match self {
            PatternMatcher::Grep(m) => Ok(m.find_at(haystack, at).unwrap_or(None)),
            PatternMatcher::Fancy(m) => m.find_at(haystack, at),
        }
    }

    fn new_captures(&self) -> Result<NoCaptures, NoError> {
        Ok(NoCaptures::new())
    }

    fn line_terminator(&self) -> Option<grep_matcher::LineTerminator> {
        match self {
            PatternMatcher::Grep(m) => m.line_terminator(),
            PatternMatcher::Fancy(m) => m.line_terminator(),
        }
    }

    fn find_candidate_line(
        &self,
        haystack: &[u8],
    ) -> Result<Option<grep_matcher::LineMatchKind>, NoError> {
        match self {
            PatternMatcher::Grep(m) => Ok(m.find_candidate_line(haystack).unwrap_or(None)),
            PatternMatcher::Fancy(m) => Ok(m
                .find_at(haystack, 0)?
                .map(|m| grep_matcher::LineMatchKind::Confirmed(m.start()))),
        }
    }
}

#[derive(Clone, Debug)]
pub struct ScanPattern {
    /// The expanded pattern source.
    pub source: String,
    pub matcher: PatternMatcher,
    pub engine: Engine,
    /// Whether searches run in multi-line mode.
    pub multi_line: bool,
    /// Configured tags, in configured order.
    pub tags: Vec<String>,
    /// Tag alternation for extraction, when the source used `($TAGS)` or `$TAGS`.
    pub tag_re: Option<regex::Regex>,
    pub sub_tag_re: Option<regex::Regex>,
    pub case_sensitive: bool,
    /// Inline flags for the source: `(?m)` plus `i` and `s` as configured.
    pub flags: String,
}

fn grep_builder(cfg: &CoreConfig, multi_line: bool) -> RegexMatcherBuilder {
    let mut b = RegexMatcherBuilder::new();
    b.multi_line(true)
        .case_insensitive(!cfg.regex_case_sensitive)
        .dot_matches_new_line(cfg.enable_multi_line)
        .crlf(true);
    if multi_line {
        b.line_terminator(None);
    }
    b
}

fn uses_unsupported_feature(source: &str) -> bool {
    use regex_syntax::ast::{parse::Parser, ErrorKind as Ast};
    matches!(
        Parser::new().parse(source).map_err(|e| e.kind().clone()),
        Err(Ast::UnsupportedLookAround) | Err(Ast::UnsupportedBackreference)
    )
}

pub fn flag_prefix(cfg: &CoreConfig) -> String {
    let mut f = String::from("(?m");
    if !cfg.regex_case_sensitive {
        f.push('i');
    }
    if cfg.enable_multi_line {
        f.push('s');
    }
    f.push(')');
    f
}

pub fn build(cfg: &CoreConfig) -> Result<ScanPattern, CoreError> {
    let tags = cfg.effective_tags();
    let source = expand_tags(&cfg.regex, &tags);
    let mut multi_line = cfg.regex.contains(r"\n") || cfg.enable_multi_line;

    let (matcher, engine) = if uses_unsupported_feature(&source) {
        let re = fancy_regex::Regex::new(&format!("{}{}", flag_prefix(cfg), source))
            .map_err(|e| CoreError::InvalidRegex(format!("regex: {e}")))?;
        (PatternMatcher::Fancy(FancyMatcher { re }), Engine::Fancy)
    } else {
        let built = match grep_builder(cfg, multi_line).build(&source) {
            Err(e) if !multi_line && matches!(e.kind(), ErrorKind::NotAllowed(_)) => {
                multi_line = true;
                grep_builder(cfg, true).build(&source)
            }
            other => other,
        };
        let m = built.map_err(|e| CoreError::InvalidRegex(format!("regex: {e}")))?;
        (PatternMatcher::Grep(m), Engine::Grep)
    };

    let ci = if cfg.regex_case_sensitive { "" } else { "(?i)" };
    let tag_re = cfg
        .regex
        .contains("$TAGS")
        .then(|| regex::Regex::new(&format!("{ci}(?:{})", tag_alternation(&tags))))
        .transpose()
        .map_err(|e| CoreError::InvalidRegex(format!("tags: {e}")))?;
    let sub_tag_re = (!cfg.sub_tag_regex.is_empty())
        .then(|| regex::Regex::new(&format!("{ci}{}", cfg.sub_tag_regex)))
        .transpose()
        .map_err(|e| CoreError::InvalidRegex(format!("subTagRegex: {e}")))?;

    Ok(ScanPattern {
        source,
        matcher,
        engine,
        multi_line,
        tags,
        tag_re,
        sub_tag_re,
        case_sensitive: cfg.regex_case_sensitive,
        flags: flag_prefix(cfg),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn find_all(p: &ScanPattern, text: &str) -> Vec<String> {
        let mut out = Vec::new();
        p.matcher
            .find_iter(text.as_bytes(), |m| {
                out.push(text[m.start()..m.end()].to_string());
                true
            })
            .unwrap();
        out
    }

    #[test]
    fn alternation_is_longest_first_and_escaped() {
        let tags: Vec<String> = ["TODO", "[ ]", "TODOS", "FIX"]
            .iter()
            .map(|s| s.to_string())
            .collect();
        assert_eq!(tag_alternation(&tags), r"TODOS|TODO|\[ \]|FIX");
    }

    #[test]
    fn default_pattern_matches_comment_tags() {
        let p = build(&CoreConfig::default()).unwrap();
        assert_eq!(p.engine, Engine::Grep);
        assert!(!p.multi_line);
        assert_eq!(
            find_all(&p, "x(); // TODO fix\n# FIXME y\n- [ ] task\nno TODO here"),
            vec!["// TODO", "# FIXME", "- [ ]"]
        );
    }

    #[test]
    fn line_mode_never_crosses_newline() {
        let p = build(&CoreConfig::default()).unwrap();
        assert!(find_all(&p, "//\nTODO").iter().all(|m| !m.contains('\n')));
    }

    #[test]
    fn case_insensitive_flag() {
        let cfg = CoreConfig {
            regex_case_sensitive: false,
            ..Default::default()
        };
        let p = build(&cfg).unwrap();
        assert_eq!(find_all(&p, "// todo lower"), vec!["// todo"]);
    }

    #[test]
    fn literal_newline_in_source_switches_to_multi_line() {
        let cfg = CoreConfig {
            regex: r"(//)\s*($TAGS).*(\n\s*//\s{2,}.*)*".into(),
            ..Default::default()
        };
        let p = build(&cfg).unwrap();
        assert!(p.multi_line);
        assert_eq!(
            find_all(&p, "// TODO a\n//   b\nx"),
            vec!["// TODO a\n//   b"]
        );
    }

    #[test]
    fn escaped_newline_byte_switches_to_multi_line() {
        let cfg = CoreConfig {
            regex: r"($TAGS)\x0a".into(),
            ..Default::default()
        };
        let p = build(&cfg).unwrap();
        assert!(p.multi_line);
    }

    #[test]
    fn lookaround_falls_back_to_fancy() {
        let cfg = CoreConfig {
            regex: r"(?<=// )($TAGS)".into(),
            ..Default::default()
        };
        let p = build(&cfg).unwrap();
        assert_eq!(p.engine, Engine::Fancy);
        assert_eq!(find_all(&p, "x // TODO y"), vec!["TODO"]);
    }

    #[test]
    fn fancy_maps_offsets_across_invalid_utf8() {
        let cfg = CoreConfig {
            regex: r"(?<=// )($TAGS)".into(),
            ..Default::default()
        };
        let p = build(&cfg).unwrap();
        let hay = b"\xff\xfe // TODO";
        let m = p.matcher.find(hay).unwrap().unwrap();
        assert_eq!(&hay[m.start()..m.end()], b"TODO");
    }

    #[test]
    fn invalid_pattern_errors() {
        let cfg = CoreConfig {
            regex: "(unclosed".into(),
            ..Default::default()
        };
        assert!(matches!(build(&cfg), Err(CoreError::InvalidRegex(_))));
    }

    #[test]
    fn invalid_regex_errors_name_the_setting() {
        let sub = CoreConfig {
            sub_tag_regex: "(unclosed".into(),
            ..Default::default()
        };
        let msg = build(&sub).unwrap_err().to_string();
        assert!(msg.starts_with("invalid regex: subTagRegex: "), "{msg}");
        let main = CoreConfig {
            regex: "(unclosed".into(),
            ..Default::default()
        };
        let msg = build(&main).unwrap_err().to_string();
        assert!(msg.starts_with("invalid regex: regex: "), "{msg}");
        let fancy = CoreConfig {
            regex: "(?<=x)(unclosed".into(),
            ..Default::default()
        };
        let msg = build(&fancy).unwrap_err().to_string();
        assert!(msg.starts_with("invalid regex: regex: "), "{msg}");
    }

    #[test]
    fn zero_width_fancy_pattern_terminates() {
        let cfg = CoreConfig {
            regex: "(?!zz)($TAGS)?".into(),
            ..Default::default()
        };
        let p = build(&cfg).unwrap();
        assert_eq!(p.engine, Engine::Fancy);
        for text in [
            "é😀 // TODO x\n".as_bytes(),
            b"\xe9\xff // TODO x\n".as_slice(),
        ] {
            let t = crate::scanner::scan_text(&p, text, "a.ts");
            assert_eq!(
                t.iter().map(|t| t.tag.as_str()).collect::<Vec<_>>(),
                vec!["TODO"],
                "{text:?}"
            );
        }
    }

    #[test]
    fn tag_and_sub_tag_regexes() {
        let cfg = CoreConfig {
            sub_tag_regex: r"^\s*\((.*?)\)".into(),
            regex_case_sensitive: false,
            ..Default::default()
        };
        let p = build(&cfg).unwrap();
        assert!(p.tag_re.as_ref().unwrap().is_match("todo"));
        assert!(p.sub_tag_re.as_ref().unwrap().is_match(" (ME) x"));
    }
}
