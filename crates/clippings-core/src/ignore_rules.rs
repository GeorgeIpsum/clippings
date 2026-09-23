//! Ignore-file rules evaluated for a single path, matching what the
//! `ignore` crate's walker would decide (spec section 5.3, admission
//! predicate). Used for file events and document closes, where no walk runs.
//!
//! This is a port of `Ignore::matched_ignore` from `ignore` 0.4.33
//! (`src/dir.rs`): each kind of rule takes its deepest match, kinds combine
//! with precedence `.rgignore` > `.ignore` > `.gitignore` > git exclude >
//! global excludes, and `.gitignore` and git excludes are not consulted
//! above the directory holding the nearest `.git`.

use ignore::gitignore::{Gitignore, GitignoreBuilder};
use ignore::Match;
use std::collections::HashMap;
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

/// The rules one directory contributes, as the walker's `IgnoreInner`.
struct DirRules {
    /// `.rgignore`, the walker's custom ignore file name.
    custom: Gitignore,
    /// `.ignore`.
    ignore: Gitignore,
    /// `.gitignore`.
    git_ignore: Gitignore,
    /// `info/exclude` in the git common dir, when this directory holds `.git`.
    git_exclude: Gitignore,
    /// Whether this directory holds a `.git` (directory or file) or `.jj`.
    has_git: bool,
}

pub struct IgnoreRules {
    cache: Mutex<HashMap<PathBuf, Arc<DirRules>>>,
    global: Gitignore,
}

/// Builds a matcher rooted at `dir` from `file`, keeping the globs that
/// parse, as the crate's `create_gitignore` does.
fn load(dir: &Path, file: &Path) -> Gitignore {
    if !file.exists() {
        return Gitignore::empty();
    }
    let mut b = GitignoreBuilder::new(dir);
    if let Some(e) = b.add(file) {
        tracing::debug!("reading {}: {e}", file.display());
    }
    b.build().unwrap_or_else(|e| {
        tracing::debug!("building {}: {e}", file.display());
        Gitignore::empty()
    })
}

fn first_line(path: &Path) -> Option<String> {
    let file = File::open(path).ok()?;
    BufReader::new(file).lines().next()?.ok()
}

/// The directory holding `info/exclude` for the repository whose `.git`
/// is in `dir`, as the crate's `resolve_git_commondir`. When `.git` is a
/// file (worktrees, submodules) it names the real git dir, and that dir's
/// `commondir` file names the common dir; without a `commondir` file there
/// is no exclude file. Paths are taken as written, as the crate does.
fn git_common_dir(dir: &Path) -> Option<PathBuf> {
    let git = dir.join(".git");
    if !git.metadata().is_ok_and(|m| m.is_file()) {
        return Some(git);
    }
    let line = first_line(&git)?;
    let real = PathBuf::from(line.strip_prefix("gitdir: ")?);
    let common = first_line(&real.join("commondir"))?;
    Some(if common.starts_with('.') {
        real.join(common)
    } else {
        PathBuf::from(common)
    })
}

impl Default for IgnoreRules {
    fn default() -> Self {
        Self::new()
    }
}

impl IgnoreRules {
    pub fn new() -> Self {
        let (global, err) = Gitignore::global();
        if let Some(e) = err {
            tracing::debug!("global gitignore: {e}");
        }
        Self {
            cache: Mutex::new(HashMap::new()),
            global,
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
        let git_exists = dir.join(".git").metadata().is_ok();
        let git_exclude = if git_exists {
            git_common_dir(dir)
                .map(|common| load(dir, &common.join("info").join("exclude")))
                .unwrap_or_else(Gitignore::empty)
        } else {
            Gitignore::empty()
        };
        let r = Arc::new(DirRules {
            custom: load(dir, &dir.join(".rgignore")),
            ignore: load(dir, &dir.join(".ignore")),
            git_ignore: load(dir, &dir.join(".gitignore")),
            git_exclude,
            has_git: git_exists || dir.join(".jj").exists(),
        });
        self.cache
            .lock()
            .unwrap()
            .insert(dir.to_path_buf(), r.clone());
        r
    }

    /// What ignore files say about the single entry `path`, found by a walk
    /// of `root`: the rules of `path`'s directory up to `root`, then those of
    /// `root`'s ancestors, as the walker's parent chain. Directories above
    /// `path` are not consulted for their own matches; see [`Self::is_ignored`].
    /// `path` must lie strictly below `root`.
    pub fn matched(&self, root: &Path, path: &Path, is_dir: bool) -> Match<()> {
        let Some(parent) = path.parent().filter(|p| p.starts_with(root)) else {
            return Match::None;
        };
        let chain: Vec<Arc<DirRules>> = parent.ancestors().map(|d| self.rules(d)).collect();
        let any_git = chain.iter().any(|r| r.has_git);
        let (mut custom, mut ignore, mut git_ignore, mut git_exclude) =
            (Match::None, Match::None, Match::None, Match::None);
        let mut saw_git = false;
        for r in &chain {
            if custom.is_none() {
                custom = r.custom.matched(path, is_dir).map(|_| ());
            }
            if ignore.is_none() {
                ignore = r.ignore.matched(path, is_dir).map(|_| ());
            }
            if any_git && !saw_git && git_ignore.is_none() {
                git_ignore = r.git_ignore.matched(path, is_dir).map(|_| ());
            }
            if any_git && !saw_git && git_exclude.is_none() {
                git_exclude = r.git_exclude.matched(path, is_dir).map(|_| ());
            }
            saw_git = saw_git || r.has_git;
        }
        let global = if any_git {
            self.global.matched(path, is_dir).map(|_| ())
        } else {
            Match::None
        };
        custom.or(ignore).or(git_ignore).or(git_exclude).or(global)
    }

    /// Whether ignore files hide `path` from a walk of `root`: `path` or a
    /// directory between `root` and `path` is ignored, since the walker
    /// prunes ignored directories.
    pub fn is_ignored(&self, root: &Path, path: &Path, is_dir: bool) -> bool {
        let Ok(rel) = path.strip_prefix(root) else {
            return false;
        };
        let mut p = root.to_path_buf();
        let n = rel.components().count();
        rel.components().enumerate().any(|(i, c)| {
            p.push(c);
            self.matched(root, &p, i + 1 < n || is_dir).is_ignore()
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn root() -> (tempfile::TempDir, PathBuf) {
        let t = tempfile::tempdir().unwrap();
        let r = dunce::canonicalize(t.path()).unwrap().join("ws");
        fs::create_dir(&r).unwrap();
        (t, r)
    }

    #[test]
    fn gitignore_applies_only_in_repo() {
        let (_t, root) = root();
        let root = &root;
        fs::write(root.join(".gitignore"), "dist/\n*.log\n").unwrap();
        fs::create_dir_all(root.join("dist")).unwrap();
        let rules = IgnoreRules::new();
        assert!(!rules.is_ignored(root, &root.join("dist/a.js"), false));
        fs::create_dir(root.join(".git")).unwrap();
        rules.clear();
        assert!(rules.is_ignored(root, &root.join("dist/a.js"), false));
        assert!(rules.is_ignored(root, &root.join("x.log"), false));
        assert!(!rules.is_ignored(root, &root.join("src/a.js"), false));
    }

    #[test]
    fn ignore_and_rgignore_apply_without_repo_and_deeper_wins() {
        let (_t, root) = root();
        let root = &root;
        fs::create_dir_all(root.join("sub")).unwrap();
        fs::write(root.join(".ignore"), "*.gen.ts\n").unwrap();
        fs::write(root.join("sub/.ignore"), "!keep.gen.ts\n").unwrap();
        let rules = IgnoreRules::new();
        assert!(rules.is_ignored(root, &root.join("a.gen.ts"), false));
        assert!(!rules.is_ignored(root, &root.join("sub/keep.gen.ts"), false));
    }

    #[test]
    fn kinds_combine_by_precedence_not_depth() {
        let (_t, root) = root();
        let root = &root;
        fs::create_dir_all(root.join(".git")).unwrap();
        fs::create_dir_all(root.join("sub")).unwrap();
        fs::write(root.join(".rgignore"), "!*.gen.ts\n").unwrap();
        fs::write(root.join("sub/.ignore"), "*.gen.ts\n").unwrap();
        fs::write(root.join(".ignore"), "!keep.txt\n").unwrap();
        fs::write(root.join("sub/.gitignore"), "keep.txt\n").unwrap();
        let rules = IgnoreRules::new();
        assert!(!rules.is_ignored(root, &root.join("sub/a.gen.ts"), false));
        assert!(!rules.is_ignored(root, &root.join("sub/keep.txt"), false));
    }

    #[test]
    fn gitignore_stops_at_nearest_repo_root() {
        let (_t, root) = root();
        let root = &root;
        fs::create_dir_all(root.join(".git")).unwrap();
        fs::create_dir_all(root.join("vendor/nested/.git")).unwrap();
        fs::write(root.parent().unwrap().join(".gitignore"), "*.txt\n").unwrap();
        fs::write(root.join(".gitignore"), "*.log\n").unwrap();
        let rules = IgnoreRules::new();
        assert!(rules.is_ignored(root, &root.join("a.log"), false));
        assert!(!rules.is_ignored(root, &root.join("vendor/nested/a.log"), false));
        assert!(!rules.is_ignored(root, &root.join("a.txt"), false));
    }

    #[test]
    fn git_info_exclude_applies() {
        let (_t, root) = root();
        let root = &root;
        fs::create_dir_all(root.join(".git/info")).unwrap();
        fs::write(root.join(".git/info/exclude"), ".claude/worktrees/\n").unwrap();
        let rules = IgnoreRules::new();
        assert!(rules.is_ignored(root, &root.join(".claude/worktrees/x/a.ts"), false));
    }

    #[test]
    fn worktree_exclude_is_read_from_the_common_dir() {
        let (_t, root) = root();
        let main = root.join("main");
        let wt = root.join("wt");
        let real = main.join(".git/worktrees/wt");
        fs::create_dir_all(main.join(".git/info")).unwrap();
        fs::create_dir_all(&real).unwrap();
        fs::create_dir_all(&wt).unwrap();
        fs::write(main.join(".git/info/exclude"), "*.secret\n").unwrap();
        fs::write(real.join("commondir"), "../..\n").unwrap();
        fs::write(wt.join(".git"), format!("gitdir: {}\n", real.display())).unwrap();
        let rules = IgnoreRules::new();
        assert!(rules.is_ignored(&wt, &wt.join("a.secret"), false));
        assert!(!rules.is_ignored(&wt, &wt.join("a.txt"), false));
        // Without a commondir file there is no exclude file, as in the crate.
        fs::remove_file(real.join("commondir")).unwrap();
        rules.clear();
        assert!(!rules.is_ignored(&wt, &wt.join("a.secret"), false));
    }
}
