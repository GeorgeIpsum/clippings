//! Go to next and previous todo in an open buffer (spec section 5.15).

use crate::pattern::ScanPattern;
use crate::position::{LineIndex, Position, Range};
use grep_matcher::Matcher;
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Direction {
    Next,
    Previous,
}

/// One target range per cursor, or `None` when any cursor has no match.
pub fn navigate(
    text: &[u8],
    pattern: &ScanPattern,
    cursors: &[Position],
    direction: Direction,
) -> Option<Vec<Range>> {
    let li = LineIndex::new(text);
    let mut matches: Vec<(usize, usize)> = Vec::new();
    let _ = pattern.matcher.find_iter(text, |m| {
        if m.start() != m.end() {
            matches.push((m.start(), m.end()));
        }
        true
    });
    cursors
        .iter()
        .map(|c| {
            let at = li.offset(*c);
            let hit = match direction {
                Direction::Next => matches.iter().find(|m| m.0 > at),
                // todo-tree searches the text before the cursor, so the
                // whole match must end at or before it.
                Direction::Previous => matches.iter().rev().find(|m| m.1 <= at),
            };
            hit.map(|&(s, e)| Range {
                start: li.position(s),
                end: li.position(e),
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::CoreConfig;
    use crate::pattern::build;

    const TEXT: &[u8] = b"a // TODO one\nb\n// FIXME two\n";

    fn p(line: u32, character: u32) -> Position {
        Position { line, character }
    }

    #[test]
    fn next_skips_a_match_at_the_cursor() {
        let pat = build(&CoreConfig::default()).unwrap();
        let r = navigate(TEXT, &pat, &[p(0, 2)], Direction::Next).unwrap();
        assert_eq!(r[0].start, p(2, 0));
        let r = navigate(TEXT, &pat, &[p(0, 0)], Direction::Next).unwrap();
        assert_eq!(r[0].start, p(0, 2));
    }

    #[test]
    fn previous_and_no_wrap() {
        let pat = build(&CoreConfig::default()).unwrap();
        let r = navigate(TEXT, &pat, &[p(2, 0)], Direction::Previous).unwrap();
        assert_eq!(
            r[0],
            Range {
                start: p(0, 2),
                end: p(0, 9)
            }
        );
        assert!(navigate(TEXT, &pat, &[p(2, 5)], Direction::Next).is_none());
        assert!(
            navigate(TEXT, &pat, &[p(0, 0), p(2, 5)], Direction::Next).is_none(),
            "all or nothing"
        );
    }

    #[test]
    fn previous_skips_the_match_around_the_cursor() {
        let pat = build(&CoreConfig::default()).unwrap();
        assert!(navigate(TEXT, &pat, &[p(0, 6)], Direction::Previous).is_none());
        let r = navigate(TEXT, &pat, &[p(2, 4)], Direction::Previous).unwrap();
        assert_eq!(r[0].start, p(0, 2));
        let r = navigate(TEXT, &pat, &[p(0, 9)], Direction::Previous).unwrap();
        assert_eq!(r[0].start, p(0, 2), "a match ending at the cursor counts");
    }

    #[test]
    fn no_matches_means_no_move() {
        let pat = build(&CoreConfig::default()).unwrap();
        for d in [Direction::Next, Direction::Previous] {
            assert!(navigate(b"a\nb\n", &pat, &[p(1, 0)], d).is_none());
        }
    }
}
