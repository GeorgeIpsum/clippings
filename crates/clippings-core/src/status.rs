//! Status bar text, activity badge and view title (spec section 5.14),
//! ported from todo-tree's `updateInformation`.

use crate::config::ScanMode;
use crate::settings::{Settings, StatusBarMode};
use crate::styles::Resolver;
use crate::view::render::View;
use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StatusBar {
    pub text: String,
    pub tooltip: String,
    pub visible: bool,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Badge {
    pub value: usize,
    pub tooltip: String,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Summary {
    pub status_bar: StatusBar,
    pub badge: Badge,
    pub view_title: String,
}

pub fn summarize(view: &View, settings: &Settings, active_file: Option<&Path>) -> Summary {
    let activity = view.counts(settings, |a| a.hide_from_activity_bar, None);
    let total: usize = activity.iter().map(|(_, c)| c).sum();
    let mode = settings.general.status_bar;
    // Current file mode counts the active file; with no active editor
    // todo-tree counts the whole workspace.
    let file = match mode {
        StatusBarMode::CurrentFile => active_file,
        _ => None,
    };
    let counts = view.counts(settings, |a| a.hide_from_status_bar, file);
    let title_total = if mode == StatusBarMode::CurrentFile {
        counts.iter().map(|(_, c)| c).sum()
    } else {
        total
    };
    let v = settings.view();
    let mut title = if v.flat {
        "Flat"
    } else if v.tags_only {
        "Tags"
    } else {
        "Tree"
    }
    .to_string();
    if title_total > 0 && settings.tree.show_counts_in_tree {
        title = format!("{title} ({title_total})");
    }

    let resolver = Resolver::new(settings);
    let default_icon = settings.highlights.default_highlight.icon.clone();
    let show_icons = settings.general.show_icons_instead_of_tags_in_status_bar;
    let count_of = |tag: &str| counts.iter().find(|(k, _)| k == tag).map_or(0, |(_, c)| *c);
    let (mut text, tooltip, visible) = match mode {
        StatusBarMode::None => (String::new(), String::new(), false),
        StatusBarMode::Total => (
            format!("$(check) {}", counts.iter().map(|(_, c)| c).sum::<usize>()),
            "Clippings total".to_string(),
            true,
        ),
        _ => {
            let order: Vec<String> = if mode == StatusBarMode::TopThree {
                let mut c = counts.clone();
                c.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
                c.into_iter().take(3).map(|(k, _)| k).collect()
            } else {
                settings.tags()
            };
            let mut text = String::new();
            for tag in order {
                let n = count_of(&tag);
                if n == 0 {
                    continue;
                }
                if !text.is_empty() {
                    text.push(' ');
                }
                let icon = resolver.icon_name(&tag);
                match icon.filter(|i| show_icons && Some(i) != default_icon.as_ref()) {
                    Some(i) => {
                        let i = if i.trim().starts_with("$(") {
                            i
                        } else {
                            format!("$({i})")
                        };
                        text.push_str(&format!("{i} {n}  "));
                    }
                    None => text.push_str(&format!("{tag}: {n} ")),
                }
            }
            let mut text = if show_icons {
                text.trim().to_string()
            } else {
                format!("$(check) {}", text.trim())
            };
            if counts.is_empty() {
                text = "$(check) 0".to_string();
            }
            let tooltip = match mode {
                StatusBarMode::CurrentFile => "Clippings tags counts in current file",
                StatusBarMode::TopThree => "Clippings top three tag counts",
                _ => "Clippings tags counts",
            };
            (text, tooltip.to_string(), true)
        }
    };
    if visible {
        match settings.tree.scan_mode {
            ScanMode::OpenFiles => text.push_str(" (in open files)"),
            ScanMode::CurrentFile => text.push_str(" (in current file)"),
            _ => {}
        }
    }
    let badge_value = if settings.general.show_activity_bar_badge {
        total
    } else {
        0
    };
    Summary {
        status_bar: StatusBar {
            text,
            tooltip,
            visible,
        },
        badge: Badge {
            value: badge_value,
            tooltip: format!("{badge_value} todos"),
        },
        view_title: title,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::ScanMode;
    use crate::index::{EffectiveFile, Source, SourcedTodo};
    use crate::model::Todo;
    use crate::position::Position;
    use crate::settings::{Attributes, StatusBarMode};
    use crate::view::render::View;
    use std::path::{Path, PathBuf};

    fn todo(line: u32, tag: &str) -> Todo {
        let p = Position { line, character: 0 };
        Todo {
            start: p,
            end: p,
            text_end: p,
            tag_start: None,
            tag_end: None,
            tag: tag.into(),
            sub_tag: None,
            before: String::new(),
            after: "x".into(),
            extra_lines: vec![],
        }
    }

    fn summary(f: impl FnOnce(&mut Settings), active: Option<&str>) -> Summary {
        let a = [todo(0, "TODO"), todo(1, "TODO"), todo(2, "FIXME")];
        let b = [todo(0, "BUG"), todo(1, "TODO")];
        let files = vec![
            EffectiveFile {
                path: Some(Path::new("/w/a.ts")),
                uri: None,
                source: Source::Disk,
                todos: a
                    .iter()
                    .map(|t| SourcedTodo {
                        buffer_uri: None,
                        todo: t,
                    })
                    .collect(),
            },
            EffectiveFile {
                path: Some(Path::new("/w/b.ts")),
                uri: None,
                source: Source::Disk,
                todos: b
                    .iter()
                    .map(|t| SourcedTodo {
                        buffer_uri: None,
                        todo: t,
                    })
                    .collect(),
            },
        ];
        let mut s = Settings::default();
        f(&mut s);
        let v = View::build(&s, &files, &[PathBuf::from("/w")]);
        summarize(&v, &s, active.map(Path::new))
    }

    // Spacing matches todo-tree exactly: each item ends in a space (two after
    // an icon) and items are joined with another space.
    #[test]
    fn total_and_tags_modes() {
        let t = summary(|s| s.general.status_bar = StatusBarMode::Total, None);
        assert_eq!(
            (t.status_bar.text.as_str(), t.status_bar.tooltip.as_str()),
            ("$(check) 5", "Clippings total")
        );
        let t = summary(|s| s.general.status_bar = StatusBarMode::Tags, None);
        assert_eq!(t.status_bar.text, "$(check) BUG: 1  FIXME: 1  TODO: 3");
        let t = summary(|s| s.general.status_bar = StatusBarMode::TopThree, None);
        assert_eq!(t.status_bar.text, "$(check) TODO: 3  BUG: 1  FIXME: 1");
        let t = summary(|s| s.general.status_bar = StatusBarMode::None, None);
        assert!(!t.status_bar.visible);
    }

    #[test]
    fn icons_and_current_file_and_scan_mode_suffix() {
        let t = summary(
            |s| {
                s.general.status_bar = StatusBarMode::Tags;
                s.general.show_icons_instead_of_tags_in_status_bar = true;
            },
            None,
        );
        assert_eq!(t.status_bar.text, "$(bug) 1   $(flame) 1   TODO: 3");
        let t = summary(
            |s| {
                s.general.status_bar = StatusBarMode::CurrentFile;
                s.tree.scan_mode = ScanMode::OpenFiles;
                s.tree.show_counts_in_tree = true;
            },
            Some("/w/b.ts"),
        );
        assert_eq!(
            t.status_bar.text,
            "$(check) BUG: 1  TODO: 1 (in open files)"
        );
        assert_eq!(t.view_title, "Tree (2)");
    }

    #[test]
    fn badge_and_hide_flags() {
        let t = summary(
            |s| {
                s.general.show_activity_bar_badge = true;
                s.general.status_bar = StatusBarMode::Total;
                s.highlights.custom_highlight.insert(
                    "TODO".into(),
                    Attributes {
                        hide_from_activity_bar: Some(true),
                        ..Default::default()
                    },
                );
            },
            None,
        );
        assert_eq!(t.badge.value, 2);
        // hideFromActivityBar affects only the badge (spec 11.2).
        assert_eq!(t.status_bar.text, "$(check) 5");
        let t = summary(
            |s| {
                s.general.show_activity_bar_badge = true;
                s.general.status_bar = StatusBarMode::Total;
                s.highlights.custom_highlight.insert(
                    "TODO".into(),
                    Attributes {
                        hide_from_status_bar: Some(true),
                        ..Default::default()
                    },
                );
            },
            None,
        );
        assert_eq!(t.badge.value, 5);
        assert_eq!(t.status_bar.text, "$(check) 2");
        let t = summary(
            |s| {
                s.general.status_bar = StatusBarMode::Tags;
                s.highlights.custom_highlight.insert(
                    "TODO".into(),
                    Attributes {
                        hide_from_status_bar: Some(true),
                        ..Default::default()
                    },
                );
            },
            None,
        );
        assert_eq!(t.status_bar.text, "$(check) BUG: 1  FIXME: 1");
    }

    #[test]
    fn current_file_mode_without_an_editor_counts_the_workspace() {
        let t = summary(|s| s.general.status_bar = StatusBarMode::CurrentFile, None);
        assert_eq!(t.status_bar.text, "$(check) BUG: 1  FIXME: 1  TODO: 3");
    }
}
