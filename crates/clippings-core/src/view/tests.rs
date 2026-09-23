use super::render::{NodeCommand, View};
use crate::index::{EffectiveFile, Source, SourcedTodo};
use crate::model::{ExtraLine, Todo};
use crate::position::Position;
use crate::settings::{Attributes, RevealBehaviour, Settings};
use crate::styles::IconDescriptor;
use std::path::PathBuf;

fn pos(line: u32, character: u32) -> Position {
    Position { line, character }
}

fn todo(line: u32, tag: &str, after: &str) -> Todo {
    Todo {
        start: pos(line, 0),
        end: pos(line, 2 + tag.len() as u32),
        text_end: pos(line, 3 + (tag.len() + after.len()) as u32),
        tag_start: Some(pos(line, 3)),
        tag_end: Some(pos(line, 3 + tag.len() as u32)),
        tag: tag.into(),
        sub_tag: None,
        before: String::new(),
        after: after.into(),
        extra_lines: vec![],
    }
}

struct Fixture {
    files: Vec<(PathBuf, Vec<Todo>)>,
}

impl Fixture {
    fn new() -> Self {
        Fixture { files: Vec::new() }
    }
    fn file(mut self, path: &str, todos: Vec<Todo>) -> Self {
        self.files.push((PathBuf::from(path), todos));
        self
    }
    fn build(&self, s: &Settings) -> View {
        let files: Vec<EffectiveFile> = self
            .files
            .iter()
            .map(|(p, t)| EffectiveFile {
                path: Some(p.as_path()),
                uri: None,
                source: Source::Disk,
                todos: t
                    .iter()
                    .map(|todo| SourcedTodo {
                        buffer_uri: None,
                        todo,
                    })
                    .collect(),
            })
            .collect();
        View::build(s, &files, &[PathBuf::from("/w")])
    }
}

fn quiet() -> Settings {
    let mut s = Settings::default();
    s.tree.show_current_scan_mode = false;
    s
}

fn labels(v: &View, parent: Option<&str>) -> Vec<String> {
    v.children_of(parent).into_iter().map(|n| n.label).collect()
}

fn ids(v: &View, parent: Option<&str>) -> Vec<String> {
    v.children_of(parent).into_iter().map(|n| n.id).collect()
}

fn sample() -> Fixture {
    Fixture::new()
        .file(
            "/w/src/b.ts",
            vec![todo(4, "FIXME", "later"), todo(1, "TODO", "first")],
        )
        .file("/w/src/a.ts", vec![todo(0, "BUG", "")])
        .file("/w/README.md", vec![todo(2, "TODO", "docs")])
}

#[test]
fn tree_view_places_root_folders_files_and_todos() {
    let v = sample().build(&quiet());
    assert_eq!(ids(&v, None), vec!["w:file:///w"]);
    assert_eq!(
        labels(&v, Some("w:file:///w")),
        vec!["src", "README.md"],
        "folders before files"
    );
    let src = "w:file:///w/d:/w/src";
    assert_eq!(labels(&v, Some(src)), vec!["a.ts", "b.ts"]);
    let b = format!("{src}/f:/w/src/b.ts");
    assert_eq!(
        ids(&v, Some(&b)),
        vec![format!("{b}/t:1:0"), format!("{b}/t:4:0")]
    );
    assert_eq!(labels(&v, Some(&b)), vec!["TODO first", "FIXME later"]);
    assert_eq!(
        labels(&v, Some(&format!("{src}/f:/w/src/a.ts"))),
        vec!["BUG "],
        "empty after keeps the format"
    );
    let root = &v.nodes["w:file:///w"];
    assert_eq!(root.context_value.as_deref(), Some("folder"));
    assert_eq!(root.resource_uri.as_deref(), Some("file:///w"));
    assert_eq!(v.nodes[&b].context_value.as_deref(), Some("file"));
    assert_eq!(v.nodes[&b].icon, Some(IconDescriptor::File));
    assert!(!v.is_empty);
}

#[test]
fn raw_label_when_format_is_empty() {
    let mut s = quiet();
    s.tree.label_format = String::new();
    let v = sample().build(&s);
    let a = "w:file:///w/d:/w/src/f:/w/src/a.ts";
    assert_eq!(labels(&v, Some(a)), vec!["BUG line 1"]);
}

#[test]
fn flat_view_uses_path_labels() {
    let mut s = quiet();
    s.view_state.flat = Some(true);
    let v = sample().build(&s);
    assert_eq!(
        labels(&v, Some("w:file:///w")),
        vec!["README.md", "a.ts (src)", "b.ts (src)"]
    );
}

#[test]
fn files_outside_every_root_are_top_level_with_absolute_dir() {
    let v = Fixture::new()
        .file("/elsewhere/x.ts", vec![todo(0, "TODO", "out")])
        .build(&quiet());
    assert_eq!(labels(&v, None), vec!["x.ts (/elsewhere)"]);
}

#[test]
fn grouping_by_tag_and_root_compaction_in_tags_only() {
    let mut s = quiet();
    s.view_state.tags_only = Some(true);
    s.view_state.grouped_by_tag = Some(true);
    let v = sample().build(&s);
    // TODO has two todos; BUG and FIXME have one each, so their tag nodes are
    // replaced by the todo. Order follows general.tags: BUG, HACK, FIXME, TODO.
    assert_eq!(labels(&v, None), vec!["BUG ", "FIXME later", "TODO"]);
    assert_eq!(labels(&v, Some("g:TODO")), vec!["TODO docs", "TODO first"]);
}

#[test]
fn tags_only_ungrouped_todo_ids_include_the_uri() {
    let mut s = quiet();
    s.view_state.tags_only = Some(true);
    let v = sample().build(&s);
    let top = ids(&v, None);
    assert!(
        top.contains(&"t:file:///w/src/b.ts:1:0".to_string()),
        "{top:?}"
    );
    assert_eq!(top.len(), 4);
}

#[test]
fn sub_tags_make_pseudo_folders_or_levels() {
    let mut t = todo(0, "TODO", "ship");
    t.sub_tag = Some("alice".into());
    let fx = Fixture::new().file("/w/a.ts", vec![t]);
    let v = fx.build(&quiet());
    let file = "w:file:///w/f:/w/a.ts";
    assert_eq!(labels(&v, Some(file)), vec!["alice"]);
    assert!(v.has_sub_tags);
    let mut s = quiet();
    s.view_state.grouped_by_sub_tag = Some(true);
    s.tree.sub_tag_click_url = "https://x/${subtag}".into();
    let v = fx.build(&s);
    let level = "w:file:///w/s:alice";
    assert_eq!(ids(&v, Some("w:file:///w")), vec![level]);
    assert_eq!(
        v.nodes[level].command,
        Some(NodeCommand::OpenUrl {
            url: "https://x/alice".into()
        })
    );
    assert_eq!(
        v.nodes[level].tooltip.as_deref(),
        Some("Click to open https://x/alice")
    );
    assert_eq!(v.nodes[level].context_value, None);
}

#[test]
fn filter_hides_non_matching_and_falls_back_to_literal() {
    let mut s = quiet();
    s.view_state.filter = "LATER".into();
    let v = sample().build(&s);
    let src = "w:file:///w/d:/w/src";
    assert_eq!(labels(&v, Some(src)), vec!["b.ts"]);
    s.view_state.filter = "(".into();
    let v = sample().build(&s);
    assert!(v.is_empty);
    let status = v.children_of(None);
    assert_eq!(
        status[0].description.as_deref(),
        Some("1 filter active, Nothing found")
    );
    assert_eq!(
        status[0].icon,
        Some(IconDescriptor::Codicon {
            name: "issues".into(),
            colour: None
        })
    );
}

#[test]
fn hide_from_tree_and_counts() {
    let mut s = quiet();
    s.tree.show_counts_in_tree = true;
    s.highlights.custom_highlight.insert(
        "FIXME".into(),
        Attributes {
            hide_from_tree: Some(true),
            ..Default::default()
        },
    );
    s.highlights.custom_highlight.insert(
        "BUG".into(),
        Attributes {
            hide_from_activity_bar: Some(true),
            ..Default::default()
        },
    );
    let v = sample().build(&s);
    let b = "w:file:///w/d:/w/src/f:/w/src/b.ts";
    assert_eq!(labels(&v, Some(b)), vec!["TODO first"]);
    assert_eq!(
        v.nodes["w:file:///w"].description.as_deref(),
        Some("2"),
        "BUG not counted, FIXME hidden"
    );
}

#[test]
fn compact_folders_join_single_folder_chains() {
    let mut s = quiet();
    s.explorer_compact_folders = true;
    let v = Fixture::new()
        .file("/w/a/b/c/x.ts", vec![todo(0, "TODO", "deep")])
        .build(&s);
    let kids = v.children_of(Some("w:file:///w"));
    assert_eq!(kids.len(), 1);
    assert_eq!(kids[0].label, "a/b/c");
    assert!(kids[0].id.ends_with("d:/w/a/b/c"));
    assert_eq!(labels(&v, Some(&kids[0].id)), vec!["x.ts"]);
}

#[test]
fn status_nodes_come_first() {
    let mut s = Settings::default();
    s.view_state.exclude_globs = vec!["**/gen/**".into()];
    let v = sample().build(&s);
    let top = v.children_of(None);
    assert_eq!(
        top[0].description.as_deref(),
        Some("Scan mode: workspace and open files")
    );
    assert_eq!(top[1].description.as_deref(), Some("1 filter active"));
    assert_eq!(
        top[1].tooltip.as_deref(),
        Some("Exclude: **/gen/**\n\nRight click for filter options")
    );
    assert_eq!(top[0].label, "");
}

#[test]
fn multi_line_todos_show_the_tag_and_extra_lines() {
    let mut t = todo(3, "TODO", "first");
    t.extra_lines = vec![
        ExtraLine {
            line: 4,
            text: "second".into(),
        },
        ExtraLine {
            line: 5,
            text: "third".into(),
        },
    ];
    let v = Fixture::new().file("/w/m.ts", vec![t]).build(&quiet());
    let file = "w:file:///w/f:/w/m.ts";
    let node = &v.children_of(Some(file))[0];
    assert_eq!(node.label, "TODO");
    assert!(node.has_children && node.default_expanded);
    assert_eq!(labels(&v, Some(&node.id)), vec!["second", "third"]);
}

#[test]
fn reveal_positions_follow_reveal_behaviour() {
    let fx = Fixture::new().file("/w/a.ts", vec![todo(2, "TODO", "go")]);
    let id = "w:file:///w/f:/w/a.ts/t:2:0";
    for (b, p) in [
        (RevealBehaviour::StartOfTodo, pos(2, 0)),
        (RevealBehaviour::StartOfLine, pos(2, 0)),
        (RevealBehaviour::EndOfTodo, pos(2, 9)),
    ] {
        let mut s = quiet();
        s.general.reveal_behaviour = b;
        let v = fx.build(&s);
        assert_eq!(
            v.nodes[id].command,
            Some(NodeCommand::Reveal {
                uri: "file:///w/a.ts".into(),
                position: p
            })
        );
    }
    let v = fx.build(&quiet());
    assert_eq!(v.nodes[id].tooltip.as_deref(), Some("/w/a.ts, line 3"));
}

#[test]
fn find_returns_paths_from_the_top() {
    let v = sample().build(&quiet());
    let paths = v.find("file:///w/src/b.ts", None);
    assert_eq!(paths.len(), 1);
    assert_eq!(
        paths[0]
            .iter()
            .map(|n| n.label.as_str())
            .collect::<Vec<_>>(),
        vec!["w", "src", "b.ts"]
    );
    let todos = v.find("file:///w/src/b.ts", Some(4));
    assert_eq!(todos[0].last().unwrap().label, "FIXME later");
    assert!(v.find("file:///w/nope.ts", None).is_empty());
}

#[test]
fn counts_includes_all_visible_todos_in_first_seen_order() {
    let mut s = quiet();
    s.tree.show_counts_in_tree = true;
    s.highlights.custom_highlight.insert(
        "BUG".into(),
        Attributes {
            hide_from_activity_bar: Some(true),
            ..Default::default()
        },
    );
    let v = sample().build(&s);
    let all_counts = v.counts(&s, |a| a.hide_from_activity_bar, None);
    assert_eq!(
        all_counts,
        vec![("FIXME".into(), 1), ("TODO".into(), 2)],
        "counts per key in first-seen order, BUG excluded by hide_from_activity_bar"
    );

    let file_a = std::path::PathBuf::from("/w/src/a.ts");
    let file_counts = v.counts(&s, |a| a.hide_from_activity_bar, Some(&file_a));
    assert_eq!(file_counts, vec![], "BUG hidden, no other todos in a.ts");

    let file_b = std::path::PathBuf::from("/w/src/b.ts");
    let file_b_counts = v.counts(&s, |a| a.hide_from_activity_bar, Some(&file_b));
    assert_eq!(
        file_b_counts,
        vec![("FIXME".into(), 1), ("TODO".into(), 1)],
        "file filter includes only b.ts todos"
    );
}

#[test]
fn tag_grouping_wins_over_sub_tag_grouping() {
    let mut t1 = todo(0, "TODO", "one");
    t1.sub_tag = Some("alice".into());
    let mut t2 = todo(1, "FIXME", "two");
    t2.sub_tag = Some("bob".into());

    // Multiple files prevent tag compaction (each tag has 2+ children: files + todos)
    let fx = Fixture::new()
        .file("/w/a.ts", vec![t1.clone()])
        .file("/w/b.ts", vec![t2.clone()]);

    // Tree view with both grouped: tag grouping wins (tags under root)
    let mut s_both = quiet();
    s_both.view_state.grouped_by_tag = Some(true);
    s_both.view_state.grouped_by_sub_tag = Some(true);

    let v = fx.build(&s_both);
    let root_id = "w:file:///w";
    let root_children = ids(&v, Some(root_id));
    assert!(
        root_children.iter().any(|id| id.contains("/g:")),
        "tag levels present under root when tag grouping wins in tree view, got: {:?}",
        root_children
    );

    // Tags-only view with both groupings: tag grouping wins
    let mut s_tags_both = quiet();
    s_tags_both.view_state.tags_only = Some(true);
    s_tags_both.view_state.grouped_by_tag = Some(true);
    s_tags_both.view_state.grouped_by_sub_tag = Some(true);

    let v = fx.build(&s_tags_both);
    let top = ids(&v, None);
    assert!(
        top.iter().any(|id| id.starts_with("g:")),
        "tag grouping visible at top in tags-only when both set"
    );
}
