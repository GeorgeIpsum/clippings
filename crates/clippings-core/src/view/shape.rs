//! Shaping (spec section 5.12): filter and visibility, counts, sort order,
//! folder and root compaction, and the status pseudo-nodes.

use super::{Arena, Kind, Node};
use crate::config::ScanMode;
use crate::settings::Settings;
use crate::styles::Resolver;
use regex::{Regex, RegexBuilder};
use std::cmp::Ordering;

/// The arena after shaping: which nodes show, in what order, with what counts.
#[derive(Clone, Debug, Default)]
pub struct Shaped {
    pub visible: Vec<bool>,
    /// Count of visible, counted todos at or below each node.
    pub counts: Vec<usize>,
    /// Displayed children per node, sorted, with compacted chains collapsed.
    pub children: Vec<Vec<usize>>,
    /// Displayed top-level nodes, status nodes first.
    pub top: Vec<usize>,
    /// Displayed parent per node.
    pub parent: Vec<Option<usize>>,
    /// Label override for the end of a compacted folder chain.
    pub compact_label: Vec<Option<String>>,
}

/// The filter as a regex: case-sensitive per `tree.filterCaseSensitive`,
/// literal text when it is not a valid regex, `None` when empty.
pub fn filter_regex(settings: &Settings) -> Option<Regex> {
    let text = settings.view_state.filter.as_str();
    if text.is_empty() {
        return None;
    }
    let ci = !settings.tree.filter_case_sensitive;
    RegexBuilder::new(text)
        .case_insensitive(ci)
        .build()
        .or_else(|_| {
            RegexBuilder::new(&regex::escape(text))
                .case_insensitive(ci)
                .build()
        })
        .ok()
}

/// Everything the comparator needs, computed once per node.
struct SortKey {
    folder_like: bool,
    tagged: bool,
    tag_index: i64,
    path: String,
    pos: (u32, u32),
}

fn sort_keys(a: &Arena, settings: &Settings, tags_only: bool) -> Vec<SortKey> {
    let order: std::collections::HashMap<String, i64> = settings
        .tags()
        .into_iter()
        .enumerate()
        .map(|(i, t)| (t, i as i64))
        .collect();
    a.nodes
        .iter()
        .map(|n| SortKey {
            folder_like: n.is_folder_like(),
            tagged: n.kind == Kind::Tag || (tags_only && n.kind == Kind::Todo),
            tag_index: n
                .key
                .as_ref()
                .and_then(|k| order.get(k).copied())
                .unwrap_or(-1),
            path: n
                .path
                .as_ref()
                .map(|p| p.to_string_lossy().into_owned())
                .unwrap_or_default(),
            pos: n
                .todo
                .as_ref()
                .map(|t| (t.start.line, t.start.character))
                .unwrap_or((0, 0)),
        })
        .collect()
}

fn compare(
    a: &Arena,
    keys: &[SortKey],
    settings: &Settings,
    x: usize,
    y: usize,
    tags_only: bool,
) -> Ordering {
    let (n, m) = (&keys[x], &keys[y]);
    let by_location = || {
        n.path
            .cmp(&m.path)
            .then(n.pos.cmp(&m.pos))
            .then(a.nodes[x].id.cmp(&a.nodes[y].id))
    };
    if !settings.tree.sort {
        return by_location();
    }
    let folders = m.folder_like.cmp(&n.folder_like);
    if folders != Ordering::Equal {
        return folders;
    }
    if tags_only && settings.tree.sort_tags_only_view_alphabetically {
        return a.nodes[x].name.cmp(&a.nodes[y].name).then_with(by_location);
    }
    if n.tagged && m.tagged && n.tag_index != m.tag_index {
        return n.tag_index.cmp(&m.tag_index);
    }
    by_location()
}

pub fn shape(a: &mut Arena, settings: &Settings) -> Shaped {
    let resolver = Resolver::new(settings);
    let filter = filter_regex(settings);
    let tags_only = settings.view().tags_only;
    let n = a.nodes.len();
    let keys = sort_keys(a, settings, tags_only);
    let mut visible = vec![false; n];
    let mut counts = vec![0usize; n];

    let mut hide_count: std::collections::HashMap<String, bool> = std::collections::HashMap::new();
    // Todos and extra lines: filter and hide flags.
    for i in 0..n {
        let node = &a.nodes[i];
        if node.kind != Kind::Todo {
            continue;
        }
        let matches = |text: &str| filter.as_ref().is_none_or(|re| re.is_match(text));
        let own = matches(&node.name);
        let extra = node.children.iter().any(|&c| matches(&a.nodes[c].name));
        let show = !node.hidden && (own || extra);
        visible[i] = show;
        for &c in &node.children {
            visible[c] = show;
        }
        let key = node.key.as_deref().unwrap_or("");
        let hidden_count = *hide_count
            .entry(key.to_string())
            .or_insert_with(|| resolver.flag(key, |x| x.hide_from_activity_bar));
        if show && !hidden_count {
            counts[i] = 1;
        }
    }
    // Containers: visible when any descendant is; counts sum bottom-up.
    fn settle(a: &Arena, i: usize, visible: &mut [bool], counts: &mut [usize]) {
        if !a.nodes[i].is_container() {
            return;
        }
        let mut any = false;
        let mut sum = 0;
        for &c in &a.nodes[i].children {
            settle(a, c, visible, counts);
            any |= visible[c];
            sum += counts[c];
        }
        visible[i] = any;
        counts[i] = sum;
    }
    for &t in &a.top.clone() {
        settle(a, t, &mut visible, &mut counts);
    }

    // Sorted, visible children.
    let order = |a: &Arena, list: &[usize]| {
        let mut v: Vec<usize> = list.iter().copied().filter(|&c| visible[c]).collect();
        v.sort_by(|&x, &y| compare(a, &keys, settings, x, y, tags_only));
        v
    };
    let mut children: Vec<Vec<usize>> = (0..n).map(|i| order(a, &a.nodes[i].children)).collect();
    let mut compact_label = vec![None; n];

    // Compact folders: a folder whose only child is a folder merges with it.
    if settings.explorer_compact_folders && !settings.tree.disable_compact_folders {
        for i in 0..n {
            children[i] = children[i]
                .iter()
                .map(|&c| {
                    if a.nodes[c].kind != Kind::Folder {
                        return c;
                    }
                    let mut end = c;
                    let mut label = a.nodes[c].name.clone();
                    while children[end].len() == 1 && a.nodes[children[end][0]].kind == Kind::Folder
                    {
                        end = children[end][0];
                        label = format!("{label}/{}", a.nodes[end].name);
                    }
                    // Ancestors come first in the arena, so the first label
                    // set is the whole chain's.
                    if end != c && compact_label[end].is_none() {
                        compact_label[end] = Some(label);
                    }
                    end
                })
                .collect();
        }
    }

    // Top level: root compaction of tag and sub-tag levels with one child.
    let mut top: Vec<usize> = order(a, &a.top)
        .into_iter()
        .map(|t| {
            let node = &a.nodes[t];
            if matches!(node.kind, Kind::Tag | Kind::SubTag) && children[t].len() == 1 {
                children[t][0]
            } else {
                t
            }
        })
        .collect();

    // Status pseudo-nodes.
    let vs = &settings.view_state;
    let mut total_filters = vs.include_globs.len() + vs.exclude_globs.len();
    let mut tooltip = String::new();
    if !vs.filter.is_empty() {
        tooltip.push_str(&format!("Tree Filter: \"{}\"\n", vs.filter));
        total_filters += 1;
    }
    for g in &vs.include_globs {
        tooltip.push_str(&format!("Include: {g}\n"));
    }
    for g in &vs.exclude_globs {
        tooltip.push_str(&format!("Exclude: {g}\n"));
    }
    let mut label = String::new();
    let mut icon = "filter";
    let mut status_tooltip = None;
    if total_filters > 0 {
        label = format!(
            "{total_filters} filter{} active",
            if total_filters == 1 { "" } else { "s" }
        );
        status_tooltip = Some(format!("{tooltip}\nRight click for filter options"));
    }
    if top.is_empty() {
        if !label.is_empty() {
            label.push_str(", ");
        }
        label.push_str("Nothing found");
        icon = "issues";
    }
    let mut status = Vec::new();
    if settings.tree.show_current_scan_mode {
        let mode = match settings.tree.scan_mode {
            ScanMode::Workspace => "workspace and open files",
            ScanMode::WorkspaceOnly => "workspace only",
            ScanMode::OpenFiles => "open files",
            ScanMode::CurrentFile => "current file",
        };
        status.push(a.get_or_add(None, "status:scan-mode", || {
            let mut n = Node::new(Kind::Status, "");
            n.status = Some((format!("Scan mode: {mode}"), "search", None));
            n
        }));
    }
    if !label.is_empty() {
        status.push(a.get_or_add(None, "status:filter", || {
            let mut n = Node::new(Kind::Status, "");
            n.status = Some((label.clone(), icon, status_tooltip.clone()));
            n
        }));
    }
    let added = a.nodes.len() - n;
    visible.extend(std::iter::repeat_n(true, added));
    counts.extend(std::iter::repeat_n(0, added));
    children.extend(std::iter::repeat_n(Vec::new(), added));
    compact_label.extend(std::iter::repeat_n(None, added));
    status.extend(top);
    top = status;

    // Displayed parents, walking down from the displayed top level.
    let mut parent = vec![None; a.nodes.len()];
    let mut stack: Vec<usize> = top.clone();
    while let Some(p) = stack.pop() {
        for &c in &children[p] {
            parent[c] = Some(p);
            stack.push(c);
        }
    }
    Shaped {
        visible,
        counts,
        children,
        top,
        parent,
        compact_label,
    }
}
