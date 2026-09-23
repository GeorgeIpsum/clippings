//! Label, tooltip and URL placeholders, ported from todo-tree's `formatLabel`.

use regex::Regex;
use std::sync::OnceLock;

/// The values a template can refer to.
#[derive(Clone, Debug, Default)]
pub struct LabelFields<'a> {
    /// 0-based line; `${line}` prints it 1-based.
    pub line: u32,
    /// `${column}` as stored: 1-based UTF-16 column.
    pub column: u32,
    pub tag: &'a str,
    pub sub_tag: &'a str,
    pub before: &'a str,
    pub after: &'a str,
    pub file_path: &'a str,
}

fn capitalize(s: &str) -> String {
    let mut c = s.chars();
    match c.next() {
        Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
        None => String::new(),
    }
}

/// The value for a placeholder name, compared case-insensitively, or `None`
/// for names todo-tree does not know (those stay in the output as written).
fn value(name: &str, f: &LabelFields, tag: &str, sub: &str) -> Option<String> {
    let is = |n: &str| name.eq_ignore_ascii_case(n);
    Some(if is("line") {
        (f.line + 1).to_string()
    } else if is("column") {
        f.column.to_string()
    } else if is("tag") {
        tag.to_string()
    } else if is("tag:uppercase") {
        tag.to_uppercase()
    } else if is("tag:lowercase") {
        tag.to_lowercase()
    } else if is("tag:capitalize") {
        capitalize(tag)
    } else if is("subtag") {
        sub.to_string()
    } else if is("subtag:uppercase") {
        sub.to_uppercase()
    } else if is("subtag:lowercase") {
        sub.to_lowercase()
    } else if is("subtag:capitalize") {
        capitalize(sub)
    } else if is("before") {
        f.before.to_string()
    } else if is("after") {
        f.after.to_string()
    } else if is("afterorbefore") {
        (if f.after.is_empty() {
            f.before
        } else {
            f.after
        })
        .to_string()
    } else if is("filename") {
        f.file_path
            .rsplit(['/', '\\'])
            .next()
            .unwrap_or("")
            .to_string()
    } else if is("filepath") {
        f.file_path.to_string()
    } else {
        return None;
    })
}

pub fn format(template: &str, f: &LabelFields) -> String {
    let tag = f.tag.trim();
    let sub = f.sub_tag.trim();
    let mut out = String::with_capacity(template.len() + f.after.len());
    let mut rest = template;
    while let Some(start) = rest.find("${") {
        out.push_str(&rest[..start]);
        let after = &rest[start + 2..];
        match after.find('}') {
            Some(end) => match value(&after[..end], f, tag, sub) {
                Some(v) => {
                    out.push_str(&v);
                    rest = &after[end + 1..];
                }
                None => {
                    out.push_str("${");
                    rest = after;
                }
            },
            None => {
                out.push_str(&rest[start..]);
                rest = "";
            }
        }
    }
    out.push_str(rest);
    out
}

/// The first unrecognised `${...}` left after formatting, as todo-tree's
/// greedy `\$\{.*\}` reports it.
pub fn unexpected_placeholder(template: &str) -> Option<String> {
    static RE: OnceLock<Regex> = OnceLock::new();
    let re = RE.get_or_init(|| Regex::new(r"\$\{.*\}").unwrap());
    let formatted = format(template, &LabelFields::default());
    re.find(&formatted).map(|m| m.as_str().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fields() -> LabelFields<'static> {
        LabelFields {
            line: 4,
            column: 3,
            tag: "todo",
            sub_tag: "alice",
            before: "x();",
            after: "fix it",
            file_path: "/r/src/a.ts",
        }
    }

    #[test]
    fn substitutes_every_placeholder_case_insensitively() {
        let f = fields();
        assert_eq!(format("${tag} ${after}", &f), "todo fix it");
        assert_eq!(
            format(
                "${TAG:UpperCase}|${subtag:capitalize}|${Line}:${column}",
                &f
            ),
            "TODO|Alice|5:3"
        );
        assert_eq!(format("${filename} ${filepath}", &f), "a.ts /r/src/a.ts");
        assert_eq!(
            format(
                "${afterorbefore}",
                &LabelFields {
                    after: "",
                    ..fields()
                }
            ),
            "x();"
        );
        assert_eq!(format("${unknown} stays", &f), "${unknown} stays");
    }

    #[test]
    fn reports_unexpected_placeholders() {
        assert_eq!(unexpected_placeholder("${tag} ${after}"), None);
        assert_eq!(
            unexpected_placeholder("${tag} ${nope}"),
            Some("${nope}".to_string())
        );
    }
}
