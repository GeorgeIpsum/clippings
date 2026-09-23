//! Placement (spec section 5.12): where each todo goes in the tree, flat
//! and tags-only views, and each node's ID.

use super::{Arena, Kind, Node, TodoData};
use crate::index::EffectiveFile;
use crate::roots::deepest_root;
use crate::settings::Settings;
use crate::styles::Resolver;
use crate::uri::{file_uri, uri_basename};
use std::path::{Path, PathBuf};

fn name_of(path: &Path) -> String {
    path.file_name().map_or_else(
        || path.to_string_lossy().into_owned(),
        |n| n.to_string_lossy().into_owned(),
    )
}

/// The todo's key for decorations, icons and hide flags: its group or tag,
/// or its raw label when untagged.
pub fn todo_key(settings: &Settings, tag: &str, raw_label: &str) -> String {
    if tag.is_empty() {
        raw_label.to_string()
    } else {
        settings.key_of(tag).to_string()
    }
}

/// Raw label (spec section 5.12): `after`, or `line N`, prefixed with the
/// tag unless grouped by tag; a multi-line todo's raw label is its tag.
pub fn raw_label(
    tag: &str,
    after: &str,
    line: u32,
    multi_line: bool,
    grouped_by_tag: bool,
) -> String {
    if multi_line {
        return tag.to_string();
    }
    let text = if after.is_empty() {
        format!("line {}", line + 1)
    } else {
        after.to_string()
    };
    if grouped_by_tag || tag.is_empty() {
        text
    } else {
        format!("{tag} {text}")
    }
}

pub fn place(settings: &Settings, files: &[EffectiveFile], tree_roots: &[PathBuf]) -> Arena {
    let view = settings.view();
    let resolver = Resolver::new(settings);
    let mut a = Arena::default();
    let mut hide_from_tree: std::collections::HashMap<String, bool> =
        std::collections::HashMap::new();
    // Container chains depend only on the file, key and sub-tag; build each once.
    let mut chains: std::collections::HashMap<(usize, String, Option<String>), Option<usize>> =
        std::collections::HashMap::new();
    for (file_i, file) in files.iter().enumerate() {
        let doc_uri = file.uri.map(str::to_string);
        let disk_uri = file.path.map(file_uri);
        for sourced in &file.todos {
            let t = sourced.todo;
            let uri = sourced
                .buffer_uri
                .map(str::to_string)
                .or_else(|| disk_uri.clone())
                .or_else(|| doc_uri.clone())
                .unwrap_or_default();
            let sub = t.sub_tag.clone().filter(|s| !s.is_empty());
            let multi = !t.extra_lines.is_empty();
            let raw = raw_label(&t.tag, &t.after, t.start.line, multi, view.grouped_by_tag);
            let key = todo_key(settings, &t.tag, &raw);

            let chain_key = (file_i, key.clone(), sub.clone());
            let parent = if let Some(p) = chains.get(&chain_key) {
                *p
            } else {
                let p = if view.tags_only {
                    if view.grouped_by_tag {
                        let label = match &sub {
                            Some(s) => format!("{key} ({s})"),
                            None => key.clone(),
                        };
                        Some(a.get_or_add(None, &format!("g:{label}"), || {
                            let mut n = Node::new(Kind::Tag, label.clone());
                            n.key = Some(key.clone());
                            n
                        }))
                    } else if let (true, Some(s)) = (view.grouped_by_sub_tag, &sub) {
                        Some(a.get_or_add(None, &format!("s:{s}"), || {
                            let mut n = Node::new(Kind::SubTag, s.clone());
                            n.sub_tag = Some(s.clone());
                            n
                        }))
                    } else {
                        None
                    }
                } else {
                    let root = file.path.and_then(|p| deepest_root(p, tree_roots));
                    let mut parent = root.map(|r| {
                        a.get_or_add(None, &format!("w:{}", file_uri(r)), || {
                            let mut n = Node::new(Kind::Root, name_of(r));
                            n.path = Some(r.to_path_buf());
                            n
                        })
                    });
                    if view.grouped_by_tag {
                        parent = Some(a.get_or_add(parent, &format!("g:{key}"), || {
                            let mut n = Node::new(Kind::Tag, key.clone());
                            n.key = Some(key.clone());
                            n
                        }));
                    } else if let (true, Some(s)) = (view.grouped_by_sub_tag, &sub) {
                        parent = Some(a.get_or_add(parent, &format!("s:{s}"), || {
                            let mut n = Node::new(Kind::SubTag, s.clone());
                            n.sub_tag = Some(s.clone());
                            n
                        }));
                    }
                    if let (Some(r), Some(path), false) = (root, file.path, view.flat) {
                        let mut dir = r.to_path_buf();
                        if let Ok(rel) = path.parent().unwrap_or(path).strip_prefix(r) {
                            for c in rel.components() {
                                dir.push(c);
                                let d = dir.clone();
                                parent = Some(a.get_or_add(
                                    parent,
                                    &format!("d:{}", d.display()),
                                    || {
                                        let mut n = Node::new(Kind::Folder, name_of(&d));
                                        n.path = Some(d.clone());
                                        n
                                    },
                                ));
                            }
                        }
                    }
                    let file_key = match file.path {
                        Some(p) => format!("f:{}", p.display()),
                        None => format!("f:{}", doc_uri.clone().unwrap_or_default()),
                    };
                    let file_idx = a.get_or_add(parent, &file_key, || {
                        let mut n = Node::new(
                            Kind::File,
                            file.path.map(name_of).unwrap_or_else(|| uri_basename(&uri)),
                        );
                        n.path = file.path.map(Path::to_path_buf);
                        n.uri = Some(disk_uri.clone().unwrap_or_else(|| uri.clone()));
                        let dir = file.path.and_then(|p| p.parent()).map(|d| match root {
                            Some(r) => d
                                .strip_prefix(r)
                                .unwrap_or(d)
                                .to_string_lossy()
                                .into_owned(),
                            None => d.to_string_lossy().into_owned(),
                        });
                        if view.flat || root.is_none() {
                            n.path_label = dir.filter(|d| !d.is_empty()).map(|d| format!("({d})"));
                        }
                        n
                    });
                    let mut parent = Some(file_idx);
                    if let (Some(s), false) = (&sub, view.grouped_by_sub_tag) {
                        parent = Some(a.get_or_add(parent, &format!("s:{s}"), || {
                            let mut n = Node::new(Kind::SubTag, s.clone());
                            n.sub_tag = Some(s.clone());
                            n
                        }));
                    }
                    parent
                };
                chains.insert(chain_key, p);
                p
            };

            let has_file_parent = parent.is_some_and(|p| {
                let k = a.nodes[p].kind;
                k == Kind::File
                    || (k == Kind::SubTag
                        && a.nodes[p]
                            .parent
                            .is_some_and(|g| a.nodes[g].kind == Kind::File))
            });
            let todo_key = if has_file_parent {
                format!("t:{}:{}", t.start.line, t.start.character)
            } else {
                format!("t:{uri}:{}:{}", t.start.line, t.start.character)
            };
            let data = TodoData {
                uri: uri.clone(),
                start: t.start,
                text_end: t.text_end,
                tag: t.tag.clone(),
                sub_tag: sub.clone(),
                before: t.before.clone(),
                after: t.after.clone(),
                multi_line: multi,
                text: raw.clone(),
            };
            let hidden = match hide_from_tree.get(&key) {
                Some(h) => *h,
                None => {
                    let h = resolver.flag(&key, |x| x.hide_from_tree);
                    hide_from_tree.insert(key.clone(), h);
                    h
                }
            };
            let extra_base = (!t.extra_lines.is_empty()).then(|| data.clone());
            let todo_idx = a.get_or_add(parent, &todo_key, || {
                let mut n = Node::new(Kind::Todo, raw);
                n.path = file.path.map(Path::to_path_buf);
                n.uri = Some(uri.clone());
                n.key = Some(key);
                n.sub_tag = sub;
                n.todo = Some(data);
                n.hidden = hidden;
                n
            });
            for (i, extra) in t.extra_lines.iter().enumerate() {
                let mut d = extra_base.clone().expect("extra lines imply a base");
                d.text = extra.text.clone();
                d.start = crate::position::Position {
                    line: extra.line,
                    character: 0,
                };
                d.multi_line = false;
                a.get_or_add(Some(todo_idx), &format!("x:{i}"), || {
                    let mut n = Node::new(Kind::Extra, extra.text.clone());
                    n.path = file.path.map(Path::to_path_buf);
                    n.uri = Some(uri.clone());
                    n.todo = Some(d);
                    n
                });
            }
        }
    }
    a
}
