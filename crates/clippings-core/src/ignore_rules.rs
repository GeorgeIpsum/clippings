//! Ignore-file rules evaluated for a single path, matching what the
//! `ignore` crate's walker would decide (spec section 5.3, admission
//! predicate). Used for file events and document closes, where no walk runs.

use ignore::gitignore::{Gitignore, GitignoreBuilder};
use ignore::Match;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

struct DirRules {
    /// `.rgignore`, `.ignore`, `.gitignore`, highest precedence first.
    files: Vec<(Gitignore, bool)>,
    /// `.git/info/exclude` when this directory holds a `.git` directory.
    git_exclude: Option<Gitignore>,
    has_git: bool,
}

pub struct IgnoreRules {
    cache: Mutex<HashMap<PathBuf, Arc<DirRules>>>,
    global: Gitignore,
}

fn load(dir: &Path, name: &str) -> Option<Gitignore> {
    let path = dir.join(name);
    if !path.is_file() {
        return None;
    }
    let mut b = GitignoreBuilder::new(dir);
    b.add(&path);
    b.build().ok()
}

impl Default for IgnoreRules {
    fn default() -> Self {
        Self::new()
    }
}

impl IgnoreRules {
    pub fn new() -> Self {
        Self {
            cache: Mutex::new(HashMap::new()),
            global: Gitignore::global().0,
        }
    }

    /// Forgets cached rules, for when an ignore file changes.
    pub fn clear(&self) {
        self.cache.lock().unwrap().clear();
    }

    fn rules(&self, dir: &Path) -> Arc<DirRules> {
        if let Some(r) = self.cache.lock().unwrap().get(dir) {
            return r.clone();
        }
        let git = dir.join(".git");
        let has_git = git.exists();
        let mut files = Vec::new();
        for (name, git_only) in [
            (".rgignore", false),
            (".ignore", false),
            (".gitignore", true),
        ] {
            if let Some(g) = load(dir, name) {
                files.push((g, git_only));
            }
        }
        let git_exclude = if git.is_dir() {
            let p = git.join("info").join("exclude");
            p.is_file()
                .then(|| {
                    let mut b = GitignoreBuilder::new(dir);
                    b.add(&p);
                    b.build().ok()
                })
                .flatten()
        } else {
            None
        };
        let r = Arc::new(DirRules {
            files,
            git_exclude,
            has_git,
        });
        self.cache
            .lock()
            .unwrap()
            .insert(dir.to_path_buf(), r.clone());
        r
    }

    /// Whether ignore files hide `path`. Rules in deeper directories win.
    /// `.gitignore`, git excludes and the global excludes file apply only
    /// inside a git repository, as with ripgrep's defaults.
    pub fn is_ignored(&self, path: &Path, is_dir: bool) -> bool {
        let ancestors: Vec<&Path> = path.ancestors().skip(1).collect();
        let in_repo = ancestors.iter().any(|d| self.rules(d).has_git);
        for dir in &ancestors {
            let rules = self.rules(dir);
            for (g, git_only) in &rules.files {
                if *git_only && !in_repo {
                    continue;
                }
                match g.matched_path_or_any_parents(path, is_dir) {
                    Match::None => {}
                    m => return m.is_ignore(),
                }
            }
        }
        if in_repo {
            for dir in &ancestors {
                if let Some(g) = &self.rules(dir).git_exclude {
                    match g.matched_path_or_any_parents(path, is_dir) {
                        Match::None => {}
                        m => return m.is_ignore(),
                    }
                }
            }
            // The global file is rooted at the process's current directory, so
            // test the path and each ancestor with `matched`, which never panics.
            let mut is_dir_now = is_dir;
            for p in path.ancestors() {
                match self.global.matched(p, is_dir_now) {
                    Match::None => {}
                    m => return m.is_ignore(),
                }
                is_dir_now = true;
            }
        }
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn gitignore_applies_only_in_repo() {
        let t = tempfile::tempdir().unwrap();
        let root = t.path();
        fs::write(root.join(".gitignore"), "dist/\n*.log\n").unwrap();
        fs::create_dir_all(root.join("dist")).unwrap();
        let rules = IgnoreRules::new();
        assert!(!rules.is_ignored(&root.join("dist/a.js"), false));
        fs::create_dir(root.join(".git")).unwrap();
        rules.clear();
        assert!(rules.is_ignored(&root.join("dist/a.js"), false));
        assert!(rules.is_ignored(&root.join("x.log"), false));
        assert!(!rules.is_ignored(&root.join("src/a.js"), false));
    }

    #[test]
    fn ignore_and_rgignore_apply_without_repo_and_deeper_wins() {
        let t = tempfile::tempdir().unwrap();
        let root = t.path();
        fs::create_dir_all(root.join("sub")).unwrap();
        fs::write(root.join(".ignore"), "*.gen.ts\n").unwrap();
        fs::write(root.join("sub/.rgignore"), "!keep.gen.ts\n").unwrap();
        let rules = IgnoreRules::new();
        assert!(rules.is_ignored(&root.join("a.gen.ts"), false));
        assert!(!rules.is_ignored(&root.join("sub/keep.gen.ts"), false));
    }

    #[test]
    fn git_info_exclude_applies() {
        let t = tempfile::tempdir().unwrap();
        let root = t.path();
        fs::create_dir_all(root.join(".git/info")).unwrap();
        fs::write(root.join(".git/info/exclude"), ".claude/worktrees/\n").unwrap();
        let rules = IgnoreRules::new();
        assert!(rules.is_ignored(&root.join(".claude/worktrees/x/a.ts"), false));
    }
}
