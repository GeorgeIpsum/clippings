//! Rendering (spec section 5.12): the `ViewNode`s the client displays, and
//! the built `View` that answers `children` and `find` requests.

use super::place::place;
use super::shape::{shape, Shaped};
use super::{Arena, Kind};
use crate::index::EffectiveFile;
use crate::labels::{self, LabelFields};
use crate::position::Position;
use crate::settings::{RevealBehaviour, Settings};
use crate::styles::{IconDescriptor, Resolver};
use crate::uri::file_uri;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", tag = "kind")]
pub enum NodeCommand {
    /// Open `uri` with the cursor at `position`.
    Reveal {
        uri: String,
        position: Position,
    },
    OpenUrl {
        url: String,
    },
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ViewNode {
    pub id: String,
    pub label: String,
    pub description: Option<String>,
    pub tooltip: Option<String>,
    pub icon: Option<IconDescriptor>,
    pub has_children: bool,
    pub default_expanded: bool,
    pub context_value: Option<String>,
    pub resource_uri: Option<String>,
    pub command: Option<NodeCommand>,
}

/// A built, shaped and rendered view.
#[derive(Clone, Debug, Default)]
pub struct View {
    pub arena: Arena,
    pub shaped: Shaped,
    /// Rendered nodes by ID, displayed nodes only.
    pub nodes: HashMap<String, ViewNode>,
    /// Displayed child IDs by parent ID; `None` is the top level.
    pub children: HashMap<Option<String>, Vec<String>>,
    /// Displayed parent ID by node ID; `None` for top-level nodes.
    pub parents: HashMap<String, Option<String>>,
    pub has_sub_tags: bool,
    pub is_empty: bool,
}

fn fields<'a>(t: &'a super::TodoData, path: &'a str) -> LabelFields<'a> {
    LabelFields {
        line: t.start.line,
        column: t.start.character + 1,
        tag: &t.tag,
        sub_tag: t.sub_tag.as_deref().unwrap_or(""),
        before: &t.before,
        after: &t.after,
        file_path: path,
    }
}

impl View {
    pub fn build(settings: &Settings, files: &[EffectiveFile], tree_roots: &[PathBuf]) -> View {
        let mut arena = place(settings, files, tree_roots);
        let shaped = shape(&mut arena, settings);
        let view = settings.view();
        let resolver = Resolver::new(settings);
        let grouped = view.grouped_by_tag || view.grouped_by_sub_tag;
        let mut nodes = HashMap::new();
        let mut children: HashMap<Option<String>, Vec<String>> = HashMap::new();
        let mut parents = HashMap::new();
        let mut icons: HashMap<String, IconDescriptor> = HashMap::new();
        let mut icon_of = |key: &str| {
            icons
                .entry(key.to_string())
                .or_insert_with(|| resolver.icon(key))
                .clone()
        };
        let mut has_sub_tags = false;
        let mut is_empty = true;

        let mut stack: Vec<usize> = shaped.top.clone();
        children.insert(
            None,
            shaped
                .top
                .iter()
                .map(|&i| arena.nodes[i].id.clone())
                .collect(),
        );
        while let Some(i) = stack.pop() {
            let n = &arena.nodes[i];
            let kids = &shaped.children[i];
            parents.insert(
                n.id.clone(),
                shaped.parent[i].map(|p| arena.nodes[p].id.clone()),
            );
            stack.extend(kids.iter().copied());
            if !kids.is_empty() {
                children.insert(
                    Some(n.id.clone()),
                    kids.iter().map(|&c| arena.nodes[c].id.clone()).collect(),
                );
            }
            has_sub_tags |= n.sub_tag.is_some();
            is_empty &= n.kind != Kind::Todo;
            let path_str = n.path.as_ref().map(|p| p.to_string_lossy().into_owned());
            let count = (settings.tree.show_counts_in_tree && n.is_container())
                .then(|| shaped.counts[i].to_string());
            let badge_uri = |n: &super::Node| {
                (settings.tree.show_badges
                    && matches!(n.kind, Kind::Root | Kind::Folder | Kind::File))
                .then(|| {
                    n.path
                        .as_ref()
                        .map(|p| file_uri(p))
                        .or_else(|| n.uri.clone())
                })
                .flatten()
            };
            let rendered = match n.kind {
                Kind::Status => {
                    let (text, icon, tooltip) = n.status.clone().unwrap_or_default();
                    ViewNode {
                        id: n.id.clone(),
                        label: String::new(),
                        description: Some(text),
                        tooltip,
                        icon: Some(IconDescriptor::Codicon {
                            name: icon.to_string(),
                            colour: None,
                        }),
                        has_children: false,
                        default_expanded: false,
                        context_value: None,
                        resource_uri: None,
                        command: None,
                    }
                }
                Kind::Todo | Kind::Extra => {
                    let t = n.todo.as_ref().expect("todo data");
                    let path = path_str.clone().unwrap_or_else(|| t.uri.clone());
                    let f = fields(t, &path);
                    let label = if n.kind == Kind::Extra
                        || t.multi_line
                        || settings.tree.label_format.is_empty()
                    {
                        n.name.clone()
                    } else {
                        labels::format(&settings.tree.label_format, &f)
                    };
                    let key = n.key.clone().unwrap_or_default();
                    let icon = (n.kind == Kind::Todo
                        && !(settings.tree.hide_icons_when_grouped_by_tag && grouped))
                        .then(|| icon_of(&key));
                    let position = match settings.general.reveal_behaviour {
                        RevealBehaviour::StartOfLine => Position {
                            line: t.start.line,
                            character: 0,
                        },
                        RevealBehaviour::StartOfTodo => t.start,
                        RevealBehaviour::EndOfTodo => t.text_end,
                    };
                    ViewNode {
                        id: n.id.clone(),
                        label,
                        description: None,
                        tooltip: Some(labels::format(&settings.tree.tooltip_format, &f)),
                        icon,
                        has_children: !kids.is_empty(),
                        default_expanded: t.multi_line,
                        context_value: None,
                        resource_uri: None,
                        command: Some(NodeCommand::Reveal {
                            uri: t.uri.clone(),
                            position,
                        }),
                    }
                }
                _ => {
                    let mut label = shaped.compact_label[i]
                        .clone()
                        .unwrap_or_else(|| n.name.clone());
                    if let Some(pl) = &n.path_label {
                        label = format!("{label} {pl}");
                    }
                    let click = (n.kind == Kind::SubTag
                        && !settings.tree.sub_tag_click_url.is_empty())
                    .then(|| {
                        let sub = n.sub_tag.clone().unwrap_or_default();
                        labels::format(
                            &settings.tree.sub_tag_click_url,
                            &LabelFields {
                                sub_tag: &sub,
                                ..Default::default()
                            },
                        )
                    });
                    let tooltip = match (&click, n.kind) {
                        (Some(url), _) => Some(format!("Click to open {url}")),
                        (None, Kind::Root | Kind::Folder | Kind::File) => {
                            path_str.clone().or_else(|| n.uri.clone())
                        }
                        _ => None,
                    };
                    let icon = match n.kind {
                        Kind::Root => IconDescriptor::Codicon {
                            name: "window".into(),
                            colour: None,
                        },
                        Kind::Tag => icon_of(n.key.as_deref().unwrap_or("")),
                        Kind::File => IconDescriptor::File,
                        _ => IconDescriptor::Folder,
                    };
                    ViewNode {
                        id: n.id.clone(),
                        label,
                        description: count,
                        tooltip,
                        icon: Some(icon),
                        has_children: !kids.is_empty(),
                        default_expanded: view.expanded,
                        context_value: match n.kind {
                            Kind::Root | Kind::Folder => Some("folder".into()),
                            Kind::File => Some("file".into()),
                            _ => None,
                        },
                        resource_uri: badge_uri(n),
                        command: click.map(|url| NodeCommand::OpenUrl { url }),
                    }
                }
            };
            nodes.insert(n.id.clone(), rendered);
        }
        View {
            arena,
            shaped,
            nodes,
            children,
            parents,
            has_sub_tags,
            is_empty,
        }
    }

    /// Displayed children of a node, or of the top level.
    pub fn children_of(&self, parent: Option<&str>) -> Vec<ViewNode> {
        self.children
            .get(&parent.map(str::to_string))
            .map(|ids| {
                ids.iter()
                    .filter_map(|id| self.nodes.get(id).cloned())
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Paths from the top level down to each displayed node for `uri`: file
    /// nodes when `line` is `None`, todos on that line otherwise.
    pub fn find(&self, uri: &str, line: Option<u32>) -> Vec<Vec<ViewNode>> {
        let path = crate::uri::uri_to_path(uri);
        let mut out = Vec::new();
        for (i, n) in self.arena.nodes.iter().enumerate() {
            if !self.nodes.contains_key(&n.id) {
                continue;
            }
            let hit = match line {
                None => {
                    n.kind == Kind::File
                        && (n.uri.as_deref() == Some(uri) || (path.is_some() && n.path == path))
                }
                Some(l) => {
                    n.kind == Kind::Todo
                        && (n.uri.as_deref() == Some(uri) || (path.is_some() && n.path == path))
                        && n.todo.as_ref().is_some_and(|t| t.start.line == l)
                }
            };
            if !hit {
                continue;
            }
            let mut chain = vec![i];
            let mut cur = i;
            while let Some(p) = self.shaped.parent[cur] {
                chain.push(p);
                cur = p;
            }
            chain.reverse();
            out.push(
                chain
                    .iter()
                    .filter_map(|&c| self.nodes.get(&self.arena.nodes[c].id).cloned())
                    .collect(),
            );
        }
        out.sort_by(|a: &Vec<ViewNode>, b| a.last().map(|n| &n.id).cmp(&b.last().map(|n| &n.id)));
        out
    }

    /// Visible todos per key, excluding keys with `hide` set.
    pub fn counts(
        &self,
        settings: &Settings,
        hide: fn(&crate::settings::Attributes) -> Option<bool>,
        file: Option<&std::path::Path>,
    ) -> Vec<(String, usize)> {
        let resolver = Resolver::new(settings);
        let mut counts: Vec<(String, usize)> = Vec::new();
        let mut key_to_index: HashMap<String, usize> = HashMap::new();
        let mut hidden_cache: HashMap<String, bool> = HashMap::new();
        for (i, n) in self.arena.nodes.iter().enumerate() {
            if n.kind != Kind::Todo || !self.shaped.visible[i] {
                continue;
            }
            if let Some(f) = file {
                if n.path.as_deref() != Some(f) {
                    continue;
                }
            }
            let key = n.key.clone().unwrap_or_default();
            let is_hidden = *hidden_cache
                .entry(key.clone())
                .or_insert_with(|| resolver.flag(&key, hide));
            if is_hidden {
                continue;
            }
            match key_to_index.get(&key) {
                Some(&idx) => counts[idx].1 += 1,
                None => {
                    let idx = counts.len();
                    key_to_index.insert(key.clone(), idx);
                    counts.push((key, 1));
                }
            }
        }
        counts
    }
}
