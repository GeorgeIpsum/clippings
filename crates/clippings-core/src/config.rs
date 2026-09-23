//! Resolved configuration that the core needs. Field names mirror the
//! `clippings.*` settings; serde uses camelCase so the extension can send
//! the object as-is.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

pub const DEFAULT_REGEX: &str = r"(//|#|<!--|;|/\*|^|^[ \t]*(-|\d+.))\s*($TAGS)";

pub const DEFAULT_TAGS: [&str; 7] = ["BUG", "HACK", "FIXME", "TODO", "XXX", "[ ]", "[x]"];

pub const DEFAULT_BUILT_IN_EXCLUDES: [&str; 25] = [
    ".git",
    ".hg",
    ".svn",
    "node_modules",
    ".pnpm-store",
    ".yarn/cache",
    ".claude",
    ".next",
    ".nuxt",
    ".output",
    ".turbo",
    ".cache",
    ".parcel-cache",
    ".svelte-kit",
    ".angular",
    ".vercel",
    ".sst",
    ".terraform",
    "target",
    "__pycache__",
    ".venv",
    "venv",
    ".gradle",
    ".idea",
    ".vs",
];

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum ScanMode {
    #[default]
    #[serde(rename = "workspace")]
    Workspace,
    #[serde(rename = "workspace only")]
    WorkspaceOnly,
    #[serde(rename = "open files")]
    OpenFiles,
    #[serde(rename = "current file")]
    CurrentFile,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum UseBuiltInExcludes {
    #[default]
    #[serde(rename = "none")]
    None,
    #[serde(rename = "file excludes")]
    FileExcludes,
    #[serde(rename = "search excludes")]
    SearchExcludes,
    #[serde(rename = "file and search excludes")]
    FileAndSearchExcludes,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct CoreConfig {
    pub tags: Vec<String>,
    pub regex: String,
    pub regex_case_sensitive: bool,
    pub sub_tag_regex: String,
    pub enable_multi_line: bool,
    pub tag_groups: BTreeMap<String, Vec<String>>,
    pub include_globs: Vec<String>,
    pub exclude_globs: Vec<String>,
    pub temp_include_globs: Vec<String>,
    pub temp_exclude_globs: Vec<String>,
    /// Keys of `files.exclude` whose value is exactly `true`.
    pub files_exclude: Vec<String>,
    /// Keys of `search.exclude` whose value is exactly `true`.
    pub search_exclude: Vec<String>,
    pub use_built_in_excludes: UseBuiltInExcludes,
    pub built_in_excludes: Vec<String>,
    pub include_hidden_files: bool,
    pub ignore_git_submodules: bool,
    pub root_folder: String,
    pub included_workspaces: Vec<String>,
    pub excluded_workspaces: Vec<String>,
    pub scan_mode: ScanMode,
    /// Not a user setting: `clippings scan --no-ignore` turns it off.
    pub respect_ignore_files: bool,
}

impl Default for CoreConfig {
    fn default() -> Self {
        Self {
            tags: DEFAULT_TAGS.iter().map(|s| s.to_string()).collect(),
            regex: DEFAULT_REGEX.to_string(),
            regex_case_sensitive: true,
            sub_tag_regex: String::new(),
            enable_multi_line: false,
            tag_groups: BTreeMap::new(),
            include_globs: Vec::new(),
            exclude_globs: vec!["**/node_modules/*/**".to_string()],
            temp_include_globs: Vec::new(),
            temp_exclude_globs: Vec::new(),
            files_exclude: Vec::new(),
            search_exclude: Vec::new(),
            use_built_in_excludes: UseBuiltInExcludes::None,
            built_in_excludes: DEFAULT_BUILT_IN_EXCLUDES
                .iter()
                .map(|s| s.to_string())
                .collect(),
            include_hidden_files: false,
            ignore_git_submodules: false,
            root_folder: String::new(),
            included_workspaces: Vec::new(),
            excluded_workspaces: Vec::new(),
            scan_mode: ScanMode::Workspace,
            respect_ignore_files: true,
        }
    }
}

impl CoreConfig {
    /// Tags to search for: `tags`, or `["TODO"]` when empty.
    pub fn effective_tags(&self) -> Vec<String> {
        if self.tags.is_empty() {
            vec!["TODO".to_string()]
        } else {
            self.tags.clone()
        }
    }

    /// The VS Code exclude keys that `use_built_in_excludes` pulls in.
    pub fn vscode_excludes(&self) -> Vec<String> {
        let mut out = Vec::new();
        match self.use_built_in_excludes {
            UseBuiltInExcludes::None => {}
            UseBuiltInExcludes::FileExcludes => out.extend(self.files_exclude.iter().cloned()),
            UseBuiltInExcludes::SearchExcludes => out.extend(self.search_exclude.iter().cloned()),
            UseBuiltInExcludes::FileAndSearchExcludes => {
                out.extend(self.files_exclude.iter().cloned());
                out.extend(self.search_exclude.iter().cloned());
            }
        }
        out
    }

    /// Group name for a tag, if `tag_groups` maps it.
    pub fn group_of(&self, tag: &str) -> Option<&str> {
        self.tag_groups
            .iter()
            .find(|(_, tags)| tags.iter().any(|t| t == tag))
            .map(|(group, _)| group.as_str())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_match_todo_tree() {
        let c = CoreConfig::default();
        assert_eq!(
            c.tags,
            vec!["BUG", "HACK", "FIXME", "TODO", "XXX", "[ ]", "[x]"]
        );
        assert_eq!(c.regex, r"(//|#|<!--|;|/\*|^|^[ \t]*(-|\d+.))\s*($TAGS)");
        assert!(c.regex_case_sensitive);
        assert_eq!(c.exclude_globs, vec!["**/node_modules/*/**"]);
        assert_eq!(c.built_in_excludes.len(), 25);
        assert_eq!(c.scan_mode, ScanMode::Workspace);
    }

    #[test]
    fn deserializes_camel_case_with_defaults() {
        let c: CoreConfig = serde_json::from_str(
            r#"{"tags":["TODO"],"scanMode":"workspace only","useBuiltInExcludes":"file excludes","filesExclude":["**/.git"]}"#,
        )
        .unwrap();
        assert_eq!(c.tags, vec!["TODO"]);
        assert_eq!(c.scan_mode, ScanMode::WorkspaceOnly);
        assert_eq!(c.vscode_excludes(), vec!["**/.git"]);
        assert!(c.regex_case_sensitive);
    }

    #[test]
    fn empty_tags_mean_todo() {
        let c = CoreConfig {
            tags: vec![],
            ..Default::default()
        };
        assert_eq!(c.effective_tags(), vec!["TODO"]);
    }

    #[test]
    fn group_lookup() {
        let mut c = CoreConfig::default();
        c.tag_groups
            .insert("FIX".into(), vec!["FIXME".into(), "BUG".into()]);
        assert_eq!(c.group_of("BUG"), Some("FIX"));
        assert_eq!(c.group_of("TODO"), None);
    }
}
