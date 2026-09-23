//! The full resolved configuration the extension sends in `initialize` and
//! `clippings/configure` (spec section 6.3). The nesting mirrors the
//! `clippings.*` settings; serde uses camelCase and fills defaults, which
//! equal todo-tree v0.0.224's. Unknown fields are ignored.

use crate::config::{
    CoreConfig, ScanMode, UseBuiltInExcludes, DEFAULT_BUILT_IN_EXCLUDES, DEFAULT_REGEX,
    DEFAULT_TAGS,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum RevealBehaviour {
    #[serde(rename = "start of line")]
    StartOfLine,
    #[default]
    #[serde(rename = "start of todo")]
    StartOfTodo,
    #[serde(rename = "end of todo")]
    EndOfTodo,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum StatusBarMode {
    #[default]
    #[serde(rename = "none")]
    None,
    #[serde(rename = "total")]
    Total,
    #[serde(rename = "tags")]
    Tags,
    #[serde(rename = "top three")]
    TopThree,
    #[serde(rename = "current file")]
    CurrentFile,
}

/// Per-tag highlight attributes (`customHighlight.<key>` and `defaultHighlight`).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Attributes {
    #[serde(rename = "type")]
    pub kind: Option<String>,
    pub foreground: Option<String>,
    pub background: Option<String>,
    pub opacity: Option<f64>,
    pub ruler_colour: Option<String>,
    pub ruler_opacity: Option<f64>,
    /// A lane number or one of `none`, `left`, `center`, `right`, `full`.
    pub ruler_lane: Option<Value>,
    pub border_radius: Option<String>,
    pub font_style: Option<String>,
    pub font_weight: Option<String>,
    pub text_decoration: Option<String>,
    pub gutter_icon: Option<bool>,
    pub icon: Option<String>,
    pub icon_colour: Option<String>,
    /// US spelling, checked before `iconColour` as in todo-tree.
    pub icon_color: Option<String>,
    pub hide_from_tree: Option<bool>,
    pub hide_from_status_bar: Option<bool>,
    pub hide_from_activity_bar: Option<bool>,
}

fn icon(name: &str) -> Attributes {
    Attributes {
        icon: Some(name.to_string()),
        ..Default::default()
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct General {
    pub automatic_git_refresh_interval: u64,
    pub periodic_refresh_interval: u64,
    pub reveal_behaviour: RevealBehaviour,
    pub export_path: String,
    pub root_folder: String,
    pub schemes: Vec<String>,
    pub status_bar: StatusBarMode,
    pub show_icons_instead_of_tags_in_status_bar: bool,
    pub tag_groups: BTreeMap<String, Vec<String>>,
    pub tags: Vec<String>,
    pub show_activity_bar_badge: bool,
}

impl Default for General {
    fn default() -> Self {
        Self {
            automatic_git_refresh_interval: 0,
            periodic_refresh_interval: 0,
            reveal_behaviour: RevealBehaviour::StartOfTodo,
            export_path: "~/todo-tree-%Y%m%d-%H%M.txt".to_string(),
            root_folder: String::new(),
            schemes: ["file", "ssh", "untitled", "vscode-notebook-cell"]
                .map(String::from)
                .to_vec(),
            status_bar: StatusBarMode::None,
            show_icons_instead_of_tags_in_status_bar: false,
            tag_groups: BTreeMap::new(),
            tags: DEFAULT_TAGS.map(String::from).to_vec(),
            show_activity_bar_badge: false,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Highlights {
    pub custom_highlight: BTreeMap<String, Attributes>,
    pub default_highlight: Attributes,
    pub enabled: bool,
    pub highlight_delay: u64,
    pub use_colour_scheme: bool,
    pub foreground_colour_scheme: Vec<String>,
    pub background_colour_scheme: Vec<String>,
}

impl Default for Highlights {
    fn default() -> Self {
        let custom = [
            ("BUG", "bug"),
            ("HACK", "tools"),
            ("FIXME", "flame"),
            ("XXX", "x"),
            ("[ ]", "issue-draft"),
            ("[x]", "issue-closed"),
        ];
        Self {
            custom_highlight: custom
                .iter()
                .map(|(t, i)| (t.to_string(), icon(i)))
                .collect(),
            default_highlight: Attributes::default(),
            enabled: true,
            highlight_delay: 500,
            use_colour_scheme: false,
            foreground_colour_scheme: [
                "white", "black", "black", "white", "white", "white", "black",
            ]
            .map(String::from)
            .to_vec(),
            background_colour_scheme: [
                "red", "orange", "yellow", "green", "blue", "indigo", "violet",
            ]
            .map(String::from)
            .to_vec(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Filtering {
    pub excluded_workspaces: Vec<String>,
    pub exclude_globs: Vec<String>,
    pub ignore_git_submodules: bool,
    pub included_workspaces: Vec<String>,
    pub include_globs: Vec<String>,
    pub include_hidden_files: bool,
    pub use_built_in_excludes: UseBuiltInExcludes,
    pub built_in_excludes: Vec<String>,
}

impl Default for Filtering {
    fn default() -> Self {
        Self {
            excluded_workspaces: Vec::new(),
            exclude_globs: vec!["**/node_modules/*/**".to_string()],
            ignore_git_submodules: false,
            included_workspaces: Vec::new(),
            include_globs: Vec::new(),
            include_hidden_files: false,
            use_built_in_excludes: UseBuiltInExcludes::None,
            built_in_excludes: DEFAULT_BUILT_IN_EXCLUDES.map(String::from).to_vec(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Tree {
    pub auto_refresh: bool,
    pub disable_compact_folders: bool,
    pub expanded: bool,
    pub filter_case_sensitive: bool,
    pub flat: bool,
    pub grouped_by_tag: bool,
    pub grouped_by_sub_tag: bool,
    pub hide_icons_when_grouped_by_tag: bool,
    pub hide_tree_when_empty: bool,
    pub label_format: String,
    pub scan_at_startup: bool,
    pub scan_mode: ScanMode,
    pub show_badges: bool,
    pub show_counts_in_tree: bool,
    pub show_current_scan_mode: bool,
    pub sub_tag_click_url: String,
    pub sort_tags_only_view_alphabetically: bool,
    pub sort: bool,
    pub tags_only: bool,
    pub tooltip_format: String,
    pub track_file: bool,
}

impl Default for Tree {
    fn default() -> Self {
        Self {
            auto_refresh: true,
            disable_compact_folders: false,
            expanded: false,
            filter_case_sensitive: false,
            flat: false,
            grouped_by_tag: false,
            grouped_by_sub_tag: false,
            hide_icons_when_grouped_by_tag: false,
            hide_tree_when_empty: false,
            label_format: "${tag} ${after}".to_string(),
            scan_at_startup: true,
            scan_mode: ScanMode::Workspace,
            show_badges: true,
            show_counts_in_tree: false,
            show_current_scan_mode: true,
            sub_tag_click_url: String::new(),
            sort_tags_only_view_alphabetically: false,
            sort: true,
            tags_only: false,
            tooltip_format: "${filepath}, line ${line}".to_string(),
            track_file: true,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct RegexSettings {
    pub regex: String,
    pub regex_case_sensitive: bool,
    pub sub_tag_regex: String,
    pub enable_multi_line: bool,
}

impl Default for RegexSettings {
    fn default() -> Self {
        Self {
            regex: DEFAULT_REGEX.to_string(),
            regex_case_sensitive: true,
            sub_tag_regex: String::new(),
            enable_multi_line: false,
        }
    }
}

/// View state the user set by clicking view buttons, kept in the client's
/// workspace storage. A set value overrides the matching `tree.*` setting.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct ViewState {
    pub flat: Option<bool>,
    pub tags_only: Option<bool>,
    pub expanded: Option<bool>,
    pub grouped_by_tag: Option<bool>,
    pub grouped_by_sub_tag: Option<bool>,
    /// The tree filter text; empty means no filter.
    pub filter: String,
    /// Temporary include globs from the folder context menu and scopes.
    pub include_globs: Vec<String>,
    /// Temporary exclude globs from the folder and file context menus and scopes.
    pub exclude_globs: Vec<String>,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Settings {
    pub general: General,
    pub highlights: Highlights,
    pub filtering: Filtering,
    pub tree: Tree,
    pub regex: RegexSettings,
    pub view_state: ViewState,
    /// Keys of `files.exclude` whose value is exactly `true`.
    pub files_exclude: Vec<String>,
    /// Keys of `search.exclude` whose value is exactly `true`.
    pub search_exclude: Vec<String>,
    /// `explorer.compactFolders`.
    pub explorer_compact_folders: bool,
}

/// The effective view options after applying the view state over the settings.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ViewOptions {
    pub flat: bool,
    pub tags_only: bool,
    pub expanded: bool,
    pub grouped_by_tag: bool,
    pub grouped_by_sub_tag: bool,
}

/// What a configuration change requires (spec section 6.3). Every flag whose
/// fields changed is set; callers apply all of them.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Changes {
    pub rescan: bool,
    pub styles: bool,
    pub view: bool,
    pub status: bool,
    pub timers: bool,
    /// The view mode or grouping changed, so the whole tree is replaced.
    pub view_mode: bool,
}

impl Changes {
    pub fn any(&self) -> bool {
        self.rescan || self.styles || self.view || self.status || self.timers
    }
}

impl Settings {
    /// Tags to search for: `general.tags`, or `["TODO"]` when empty.
    pub fn tags(&self) -> Vec<String> {
        if self.general.tags.is_empty() {
            vec!["TODO".to_string()]
        } else {
            self.general.tags.clone()
        }
    }

    pub fn view(&self) -> ViewOptions {
        let v = &self.view_state;
        let t = &self.tree;
        ViewOptions {
            flat: v.flat.unwrap_or(t.flat),
            tags_only: v.tags_only.unwrap_or(t.tags_only),
            expanded: v.expanded.unwrap_or(t.expanded),
            grouped_by_tag: v.grouped_by_tag.unwrap_or(t.grouped_by_tag),
            grouped_by_sub_tag: v.grouped_by_sub_tag.unwrap_or(t.grouped_by_sub_tag),
        }
    }

    /// The scanner's configuration.
    pub fn core(&self) -> CoreConfig {
        CoreConfig {
            tags: self.general.tags.clone(),
            regex: self.regex.regex.clone(),
            regex_case_sensitive: self.regex.regex_case_sensitive,
            sub_tag_regex: self.regex.sub_tag_regex.clone(),
            enable_multi_line: self.regex.enable_multi_line,
            tag_groups: self.general.tag_groups.clone(),
            include_globs: self.filtering.include_globs.clone(),
            exclude_globs: self.filtering.exclude_globs.clone(),
            temp_include_globs: self.view_state.include_globs.clone(),
            temp_exclude_globs: self.view_state.exclude_globs.clone(),
            files_exclude: self.files_exclude.clone(),
            search_exclude: self.search_exclude.clone(),
            use_built_in_excludes: self.filtering.use_built_in_excludes,
            built_in_excludes: self.filtering.built_in_excludes.clone(),
            include_hidden_files: self.filtering.include_hidden_files,
            ignore_git_submodules: self.filtering.ignore_git_submodules,
            root_folder: self.general.root_folder.clone(),
            included_workspaces: self.filtering.included_workspaces.clone(),
            excluded_workspaces: self.filtering.excluded_workspaces.clone(),
            scan_mode: self.tree.scan_mode,
            respect_ignore_files: true,
        }
    }

    /// Attributes for a decoration or tree key: an exact `customHighlight`
    /// entry, if any.
    pub fn custom(&self, key: &str) -> Option<&Attributes> {
        self.highlights.custom_highlight.get(key)
    }

    /// Group name for a tag, if `tagGroups` maps it.
    pub fn group_of(&self, tag: &str) -> Option<&str> {
        self.general
            .tag_groups
            .iter()
            .find(|(_, tags)| tags.iter().any(|t| t == tag))
            .map(|(g, _)| g.as_str())
    }

    /// The decoration and tree key for a tag: its group, else the tag.
    pub fn key_of<'a>(&'a self, tag: &'a str) -> &'a str {
        self.group_of(tag).unwrap_or(tag)
    }

    /// What changed between `old` and `self` (spec section 6.3).
    pub fn changes_from(&self, old: &Settings) -> Changes {
        let mut c = Changes::default();
        let old_core = old.core();
        let new_core = self.core();
        let rescan_fields_changed = {
            let mut a = old_core.clone();
            let mut b = new_core.clone();
            // Tag groups apply at view build, never rescan.
            a.tag_groups.clear();
            b.tag_groups.clear();
            a != b
        };
        c.rescan = rescan_fields_changed || (!old.tree.auto_refresh && self.tree.auto_refresh);
        let hide = |s: &Settings, f: fn(&Attributes) -> Option<bool>| {
            s.highlights
                .custom_highlight
                .iter()
                .map(|(k, a)| (k.clone(), f(a)))
                .collect::<Vec<_>>()
        };
        c.styles = old.general.tags != self.general.tags
            || old.general.tag_groups != self.general.tag_groups
            || old.regex != self.regex
            || old_core.include_globs != new_core.include_globs
            || old_core.exclude_globs != new_core.exclude_globs
            || old_core.temp_include_globs != new_core.temp_include_globs
            || old_core.temp_exclude_globs != new_core.temp_exclude_globs
            || old_core.vscode_excludes() != new_core.vscode_excludes()
            || old.highlights != self.highlights
            || old.general.schemes != self.general.schemes;
        c.view = old.general.tag_groups != self.general.tag_groups
            || old.view_state != self.view_state
            || old.tree != self.tree
            || old.general.reveal_behaviour != self.general.reveal_behaviour
            || old.general.status_bar != self.general.status_bar
            || old.general.tags != self.general.tags
            || old.explorer_compact_folders != self.explorer_compact_folders
            || old.highlights.custom_highlight != self.highlights.custom_highlight
            || old.highlights.default_highlight != self.highlights.default_highlight
            || old.highlights.use_colour_scheme != self.highlights.use_colour_scheme
            || old.highlights.background_colour_scheme != self.highlights.background_colour_scheme
            || c.rescan;
        c.status = old.general.status_bar != self.general.status_bar
            || old.general.show_icons_instead_of_tags_in_status_bar
                != self.general.show_icons_instead_of_tags_in_status_bar
            || old.general.show_activity_bar_badge != self.general.show_activity_bar_badge
            || hide(old, |a| a.hide_from_status_bar) != hide(self, |a| a.hide_from_status_bar)
            || old.highlights.default_highlight.hide_from_status_bar
                != self.highlights.default_highlight.hide_from_status_bar
            || c.view;
        c.timers = old.general.automatic_git_refresh_interval
            != self.general.automatic_git_refresh_interval
            || old.general.periodic_refresh_interval != self.general.periodic_refresh_interval;
        c.view_mode = old.view() != self.view() || old.tree.scan_mode != self.tree.scan_mode;
        c
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_match_todo_tree() {
        let s = Settings::default();
        assert_eq!(s.general.tags.len(), 7);
        assert_eq!(
            s.general.schemes,
            vec!["file", "ssh", "untitled", "vscode-notebook-cell"]
        );
        assert_eq!(s.highlights.highlight_delay, 500);
        assert!(s.highlights.enabled);
        assert_eq!(s.custom("BUG").unwrap().icon.as_deref(), Some("bug"));
        assert_eq!(s.tree.label_format, "${tag} ${after}");
        assert_eq!(s.tree.tooltip_format, "${filepath}, line ${line}");
        assert!(
            s.tree.show_badges && s.tree.sort && s.tree.show_current_scan_mode && s.tree.track_file
        );
        assert_eq!(s.general.export_path, "~/todo-tree-%Y%m%d-%H%M.txt");
        assert_eq!(s.core(), CoreConfig::default());
    }

    #[test]
    fn deserializes_the_client_shape() {
        let s: Settings = serde_json::from_str(
            r#"{"general":{"tags":["TODO","FIXME"],"statusBar":"top three","revealBehaviour":"end of todo"},
                "highlights":{"customHighlight":{"TODO":{"type":"text","iconColour":"red","rulerLane":"left"}}},
                "tree":{"scanMode":"workspace only","showCountsInTree":true},
                "viewState":{"flat":true,"filter":"fix"},
                "filesExclude":["**/.git"],"explorerCompactFolders":true,
                "server":{"logLevel":"debug"}}"#,
        )
        .unwrap();
        assert_eq!(s.general.status_bar, StatusBarMode::TopThree);
        assert_eq!(s.general.reveal_behaviour, RevealBehaviour::EndOfTodo);
        let todo = s.custom("TODO").unwrap();
        assert_eq!(todo.kind.as_deref(), Some("text"));
        assert_eq!(todo.icon_colour.as_deref(), Some("red"));
        assert_eq!(todo.ruler_lane, Some(Value::String("left".into())));
        assert!(s.view().flat);
        assert!(!s.view().tags_only);
        assert_eq!(s.core().scan_mode, ScanMode::WorkspaceOnly);
        assert!(s.explorer_compact_folders);
    }

    #[test]
    fn view_state_overrides_settings() {
        let mut s = Settings::default();
        s.tree.flat = true;
        assert!(s.view().flat);
        s.view_state.flat = Some(false);
        assert!(!s.view().flat);
    }

    #[test]
    fn group_and_key() {
        let mut s = Settings::default();
        s.general
            .tag_groups
            .insert("FIX".into(), vec!["FIXME".into(), "BUG".into()]);
        assert_eq!(s.key_of("BUG"), "FIX");
        assert_eq!(s.key_of("TODO"), "TODO");
    }

    #[test]
    fn change_classification() {
        let base = Settings::default();
        let mut s = base.clone();
        s.regex.regex = r"($TAGS)".into();
        let c = s.changes_from(&base);
        assert!(c.rescan && c.styles && c.view && c.status);

        let mut s = base.clone();
        s.general.tag_groups.insert("G".into(), vec!["TODO".into()]);
        let c = s.changes_from(&base);
        assert!(!c.rescan && c.styles && c.view);

        let mut s = base.clone();
        s.highlights.default_highlight.foreground = Some("red".into());
        let c = s.changes_from(&base);
        assert!(!c.rescan && c.styles);

        let mut s = base.clone();
        s.general.status_bar = StatusBarMode::Total;
        let c = s.changes_from(&base);
        assert!(!c.rescan && !c.styles && c.status);

        let mut s = base.clone();
        s.general.periodic_refresh_interval = 5;
        let c = s.changes_from(&base);
        assert!(c.timers && !c.rescan && !c.view);

        let mut s = base.clone();
        s.view_state.flat = Some(true);
        let c = s.changes_from(&base);
        assert!(c.view && c.view_mode && !c.rescan);

        let mut off = base.clone();
        off.tree.auto_refresh = false;
        assert!(
            base.changes_from(&off).rescan,
            "autoRefresh false -> true rescans"
        );

        assert!(!base.changes_from(&base).any());
    }
}
