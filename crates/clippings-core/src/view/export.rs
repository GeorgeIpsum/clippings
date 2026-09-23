//! Export (spec section 5.15): the visible tree as JSON or as an ASCII tree
//! in treeify's format, and the export path.

use super::render::View;
use super::Kind;
use serde_json::{Map, Value};

fn node_value(view: &View, i: usize, tags_only: bool) -> (String, Value) {
    let n = &view.arena.nodes[i];
    let rendered = &view.nodes[&n.id];
    if n.kind == Kind::Todo {
        let t = n.todo.as_ref().expect("todo data");
        let siblings = view.shaped.parent[i].map_or(&view.shaped.top, |p| &view.shaped.children[p]);
        let shared = siblings.iter().filter(|&&s| {
            let o = &view.arena.nodes[s];
            o.kind == Kind::Todo
                && o.todo
                    .as_ref()
                    .is_some_and(|ot| ot.start.line == t.start.line && o.path == n.path)
        });
        let mut key = if shared.count() > 1 {
            format!("line {}:{}", t.start.line + 1, t.start.character + 1)
        } else {
            format!("line {}", t.start.line + 1)
        };
        if tags_only {
            let file = n
                .path
                .as_ref()
                .map(|p| p.to_string_lossy().into_owned())
                .unwrap_or_else(|| t.uri.clone());
            key = format!("{file} {key}");
        }
        let kids = &view.shaped.children[i];
        let value = if kids.is_empty() {
            Value::String(rendered.label.clone())
        } else {
            let mut extras = Map::new();
            for &c in kids {
                extras.insert(view.arena.nodes[c].name.clone(), Value::Object(Map::new()));
            }
            let mut m = Map::new();
            m.insert(rendered.label.clone(), Value::Object(extras));
            Value::Object(m)
        };
        return (key, value);
    }
    let mut m = Map::new();
    for &c in &view.shaped.children[i] {
        let (k, v) = node_value(view, c, tags_only);
        m.insert(k, v);
    }
    (rendered.label.clone(), Value::Object(m))
}

/// The visible tree, without status nodes, as a JSON object.
pub fn export_value(view: &View, tags_only: bool) -> Value {
    let mut m = Map::new();
    for &t in &view.shaped.top {
        if view.arena.nodes[t].kind == Kind::Status {
            continue;
        }
        let (k, v) = node_value(view, t, tags_only);
        m.insert(k, v);
    }
    Value::Object(m)
}

/// treeify's `asTree(value, true)`.
pub fn treeify(value: &Value) -> String {
    fn grow(key: &str, value: &Value, last: bool, ancestors_last: &[bool], out: &mut String) {
        let mut line = String::new();
        for &l in ancestors_last {
            line.push_str(if l { "   " } else { "│  " });
        }
        line.push_str(if last { "└─ " } else { "├─ " });
        line.push_str(key);
        match value {
            Value::Object(_) => {}
            Value::String(s) => {
                line.push_str(": ");
                line.push_str(s);
            }
            other => {
                line.push_str(": ");
                line.push_str(&other.to_string());
            }
        }
        out.push_str(&line);
        out.push('\n');
        if let Value::Object(m) = value {
            let mut next = ancestors_last.to_vec();
            next.push(last);
            let len = m.len();
            for (i, (k, v)) in m.iter().enumerate() {
                grow(k, v, i + 1 == len, &next, out);
            }
        }
    }
    let mut out = String::new();
    if let Value::Object(m) = value {
        let len = m.len();
        for (i, (k, v)) in m.iter().enumerate() {
            grow(k, v, i + 1 == len, &[], &mut out);
        }
    }
    out
}

/// `general.exportPath` with `~`, `${NAME}` and strftime placeholders expanded.
pub fn export_path(
    template: &str,
    env: &dyn Fn(&str) -> Option<String>,
    home: Option<&str>,
    now: &chrono::DateTime<chrono::Local>,
) -> String {
    let mut s = crate::roots::expand_env(template, env);
    if let (Some(rest), Some(h)) = (s.strip_prefix('~'), home) {
        s = format!("{h}{rest}");
    }
    let mut out = String::new();
    use std::fmt::Write;
    match write!(out, "{}", now.format(&s)) {
        Ok(()) => out,
        Err(_) => s,
    }
}

/// The export document: JSON when the path ends in `.json`, else treeify text.
pub fn export_content(view: &View, tags_only: bool, path: &str) -> String {
    let value = export_value(view, tags_only);
    if path.ends_with(".json") {
        serde_json::to_string_pretty(&value).unwrap_or_default()
    } else {
        treeify(&value)
    }
}
