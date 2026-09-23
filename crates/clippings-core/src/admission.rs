//! Admission rules (spec section 5.3): which disk paths and open buffers
//! may enter the index. The walker applies the same rules during a walk;
//! tests check the two agree.

use crate::config::CoreConfig;
use crate::fs::Fs;
use crate::globs::{slash_path, BuiltInExcludes, GlobLayers};
use crate::ignore_rules::IgnoreRules;
use crate::roots::deepest_root;
use crate::CoreError;
use ignore::Match;
use std::path::{Path, PathBuf};
use std::sync::Arc;

/// Components of `path` below `root`, as strings.
pub fn rel_components(path: &Path, root: &Path) -> Vec<String> {
    path.strip_prefix(root)
        .map(|r| {
            r.components()
                .map(|c| c.as_os_str().to_string_lossy().into_owned())
                .collect()
        })
        .unwrap_or_default()
}

/// todo-tree's rule for open buffers: a dot-prefixed base name without an extension.
pub fn buffer_hidden(path: &Path) -> bool {
    let name = path
        .file_name()
        .map(|n| n.to_string_lossy())
        .unwrap_or_default();
    name.starts_with('.') && path.extension().is_none()
}

/// The directory rules the walker applies in `filter_entry`: layer 1, the
/// directory pruning of layers 2 to 4, and submodule skipping. The walker
/// and [`Admission::prunes_dir`] share them, so the server's file-event
/// filtering prunes exactly what a walk prunes.
pub struct DirPruning {
    pub(crate) layers: GlobLayers,
    pub(crate) built_in: BuiltInExcludes,
    ignore_submodules: bool,
}

impl DirPruning {
    pub fn new(cfg: &CoreConfig) -> Result<Self, CoreError> {
        Ok(Self {
            layers: GlobLayers::new(cfg)?,
            built_in: BuiltInExcludes::new(&cfg.built_in_excludes),
            ignore_submodules: cfg.ignore_git_submodules,
        })
    }

    /// Whether `dir`, a directory whose deepest scan root is `root`, is
    /// pruned. `has_git` says whether a directory contains a `.git` entry.
    pub fn prunes(&self, dir: &Path, root: &Path, has_git: impl FnOnce(&Path) -> bool) -> bool {
        let rel = rel_components(dir, root);
        let rel_refs: Vec<&str> = rel.iter().map(String::as_str).collect();
        let rel_str = dir.strip_prefix(root).ok().map(slash_path);
        self.built_in.dir_excluded(&rel_refs)
            || self.layers.dir_pruned(&slash_path(dir), rel_str.as_deref())
            || (self.ignore_submodules && has_git(dir))
    }

    /// Include and exclude check for a file below `root`, as the walker does it.
    pub fn file_allowed(&self, file: &Path, root: &Path) -> bool {
        let rel_str = file.strip_prefix(root).ok().map(slash_path);
        self.layers
            .file_allowed(&slash_path(file), rel_str.as_deref())
    }
}

pub struct Admission {
    walked_roots: Vec<PathBuf>,
    rules: DirPruning,
    ignore: IgnoreRules,
    include_hidden: bool,
    ignore_submodules: bool,
    respect_ignore_files: bool,
    fs: Arc<dyn Fs>,
}

impl Admission {
    pub fn new(
        cfg: &CoreConfig,
        walked_roots: Vec<PathBuf>,
        fs: Arc<dyn Fs>,
    ) -> Result<Self, CoreError> {
        Ok(Self {
            walked_roots,
            rules: DirPruning::new(cfg)?,
            ignore: IgnoreRules::new(),
            include_hidden: cfg.include_hidden_files,
            ignore_submodules: cfg.ignore_git_submodules,
            respect_ignore_files: cfg.respect_ignore_files,
            fs,
        })
    }

    pub fn walked_roots(&self) -> &[PathBuf] {
        &self.walked_roots
    }

    /// Forgets cached ignore-file rules, for when an ignore file changes.
    pub fn clear_ignore_cache(&self) {
        self.ignore.clear();
    }

    /// Whether `.gitignore`, `.ignore` and `.rgignore` files apply at all.
    pub fn respects_ignore_files(&self) -> bool {
        self.respect_ignore_files
    }

    /// Whether ignore files or the hidden rule keep the entry `entry` (whose
    /// base name is `name`) out of a walk of `root`. An ignore-file match,
    /// even a whitelist, takes precedence over the hidden rule, as in the walker.
    fn ignored_or_hidden(&self, root: &Path, entry: &Path, name: &str, is_dir: bool) -> bool {
        let m = if self.respect_ignore_files {
            self.ignore.matched(root, entry, is_dir)
        } else {
            Match::None
        };
        m.is_ignore() || (m.is_none() && !self.include_hidden && name.starts_with('.'))
    }

    /// Whether a walk would not descend into `dir`, a directory strictly
    /// below a walked root, judging `dir` alone and not its ancestors: the
    /// walker's [`DirPruning`] rules plus the ignore-file and hidden rules.
    /// False for walked roots and paths outside them.
    pub fn prunes_dir(&self, dir: &Path) -> bool {
        let Some(root) = deepest_root(dir, &self.walked_roots) else {
            return false;
        };
        if dir == root {
            return false;
        }
        let name = dir
            .file_name()
            .map(|n| n.to_string_lossy())
            .unwrap_or_default();
        self.rules
            .prunes(dir, root, |d| self.fs.exists(&d.join(".git")))
            || self.ignored_or_hidden(root, dir, &name, true)
    }

    /// Whether a directory strictly between `path`'s walked root and `path`
    /// is pruned, so a walk never reaches `path`. Checks no property of
    /// `path` itself, so it needs no stat of `path`.
    pub fn inside_pruned_dir(&self, path: &Path) -> bool {
        let Some(root) = deepest_root(path, &self.walked_roots) else {
            return false;
        };
        let mut dir = root.to_path_buf();
        let rel = rel_components(path, root);
        for c in rel.iter().take(rel.len().saturating_sub(1)) {
            dir.push(c);
            if self.prunes_dir(&dir) {
                return true;
            }
        }
        false
    }

    /// Whether a file on disk may enter the index. Binary detection happens later, in the scanner.
    ///
    /// Known gap: on Windows the walker also treats files and directories
    /// with the HIDDEN attribute as hidden, while this checks only the
    /// dot-prefix rule, so the two can disagree there.
    pub fn admits_disk(&self, path: &Path) -> bool {
        let Some(root) = deepest_root(path, &self.walked_roots) else {
            return false;
        };
        let rel = rel_components(path, root);
        if rel.is_empty() {
            return false;
        }
        let rel_refs: Vec<&str> = rel.iter().map(String::as_str).collect();
        if self.rules.built_in.file_excluded(&rel_refs) {
            return false;
        }
        if !self.rules.layers.path_allowed(path, Some(root)) {
            return false;
        }
        if self.ignore_submodules {
            let mut dir = path.parent();
            while let Some(d) = dir {
                if d == root || !d.starts_with(root) {
                    break;
                }
                if self.fs.exists(&d.join(".git")) {
                    return false;
                }
                dir = d.parent();
            }
        }
        // Ignore files and hidden names, entry by entry from the root down,
        // as the walker prunes.
        let mut entry = root.to_path_buf();
        for (i, c) in rel_refs.iter().enumerate() {
            entry.push(c);
            if self.ignored_or_hidden(root, &entry, c, i + 1 < rel_refs.len()) {
                return false;
            }
        }
        true
    }

    /// Whether an open buffer may feed the tree and get decorations. `path`
    /// is `None` for documents without a file path, such as `untitled:`.
    pub fn admits_buffer(&self, path: Option<&Path>, tree_roots: &[PathBuf]) -> bool {
        let Some(path) = path else { return true };
        let root = deepest_root(path, tree_roots);
        self.rules.layers.path_allowed(path, root) && !buffer_hidden(path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fs::NativeFs;
    use std::fs;

    fn admission(root: &Path, f: impl FnOnce(&mut CoreConfig)) -> Admission {
        let mut c = CoreConfig::default();
        f(&mut c);
        Admission::new(&c, vec![root.to_path_buf()], Arc::new(NativeFs)).unwrap()
    }

    #[test]
    fn disk_rules() {
        let t = tempfile::tempdir().unwrap();
        let r = t.path();
        fs::create_dir_all(r.join(".git")).unwrap();
        fs::write(r.join(".gitignore"), "dist/\n").unwrap();
        let a = admission(r, |_| {});
        assert!(a.admits_disk(&r.join("src/a.ts")));
        assert!(!a.admits_disk(&r.join("dist/a.js")), "gitignored");
        assert!(!a.admits_disk(&r.join(".github/ci.yml")), "hidden");
        assert!(!a.admits_disk(&r.join("node_modules/x/a.js")), "built-in");
        assert!(
            !a.admits_disk(&r.join(".claude/worktrees/w/a.ts")),
            "built-in and hidden"
        );
        assert!(
            !a.admits_disk(Path::new("/elsewhere/a.ts")),
            "outside walked roots"
        );
        let hidden_ok = admission(r, |c| c.include_hidden_files = true);
        assert!(hidden_ok.admits_disk(&r.join(".github/ci.yml")));
    }

    #[test]
    fn ignore_file_whitelist_beats_the_hidden_rule() {
        let t = tempfile::tempdir().unwrap();
        let r = t.path();
        fs::write(r.join(".ignore"), "!.config\n").unwrap();
        let a = admission(r, |_| {});
        assert!(a.admits_disk(&r.join(".config/a.toml")), "whitelisted");
        assert!(!a.admits_disk(&r.join(".other/a.toml")), "hidden");
        let no_rules = admission(r, |c| c.respect_ignore_files = false);
        assert!(!no_rules.admits_disk(&r.join(".config/a.toml")));
    }

    #[test]
    fn built_in_excludes_ignore_root_ancestors() {
        let t = tempfile::tempdir().unwrap();
        let r = t.path().join("target/.claude/worktrees/w");
        fs::create_dir_all(&r).unwrap();
        let a = admission(&r, |c| c.include_hidden_files = true);
        assert!(a.admits_disk(&r.join("src/a.ts")));
    }

    #[test]
    fn submodules_are_skipped_when_asked() {
        let t = tempfile::tempdir().unwrap();
        let r = t.path();
        fs::create_dir_all(r.join("vendor/lib")).unwrap();
        fs::write(r.join("vendor/lib/.git"), "gitdir: x").unwrap();
        assert!(admission(r, |_| {}).admits_disk(&r.join("vendor/lib/a.c")));
        assert!(!admission(r, |c| c.ignore_git_submodules = true)
            .admits_disk(&r.join("vendor/lib/a.c")));
    }

    #[test]
    fn directory_pruning_matches_the_walker() {
        let t = tempfile::tempdir().unwrap();
        let r = t.path();
        fs::create_dir_all(r.join(".git")).unwrap();
        fs::write(r.join(".gitignore"), "dist/\n").unwrap();
        fs::create_dir_all(r.join("vendor/lib")).unwrap();
        fs::write(r.join("vendor/lib/.git"), "gitdir: x").unwrap();
        let a = admission(r, |c| {
            c.exclude_globs = vec!["gen/**".into()];
            c.ignore_git_submodules = true;
        });
        for pruned in [
            "node_modules",
            "a/node_modules",
            ".yarn/cache",
            "gen",
            "dist",
            ".github",
            "vendor/lib",
        ] {
            assert!(a.prunes_dir(&r.join(pruned)), "{pruned}");
        }
        for kept in ["src", "cache", "vendor", "generated"] {
            assert!(!a.prunes_dir(&r.join(kept)), "{kept}");
        }
        assert!(!a.prunes_dir(r), "a walked root");
        assert!(!a.prunes_dir(Path::new("/elsewhere/node_modules")));
        assert!(a.inside_pruned_dir(&r.join("node_modules/pkg/a.js")));
        assert!(a.inside_pruned_dir(&r.join("gen/x")));
        assert!(
            !a.inside_pruned_dir(&r.join("node_modules")),
            "only ancestors"
        );
        assert!(!a.inside_pruned_dir(&r.join("src/a.ts")));
    }

    #[test]
    fn buffer_rules_ignore_gitignore_and_built_ins() {
        let t = tempfile::tempdir().unwrap();
        let r = t.path();
        fs::create_dir_all(r.join(".git")).unwrap();
        fs::write(r.join(".gitignore"), "dist/\n").unwrap();
        let a = admission(r, |_| {});
        let roots = vec![r.to_path_buf()];
        assert!(a.admits_buffer(Some(&r.join("dist/a.js")), &roots));
        assert!(a.admits_buffer(Some(&r.join("target/x.rs")), &roots));
        assert!(a.admits_buffer(Some(&r.join(".eslintrc.json")), &roots));
        assert!(!a.admits_buffer(Some(&r.join(".env")), &roots));
        assert!(
            !a.admits_buffer(Some(&r.join("node_modules/p/a.js")), &roots),
            "default exclude glob"
        );
        assert!(a.admits_buffer(None, &roots));
    }
}
