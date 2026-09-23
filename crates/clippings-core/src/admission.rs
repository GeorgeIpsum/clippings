//! Admission rules (spec section 5.3): which disk paths and open buffers
//! may enter the index. The walker applies the same rules during a walk;
//! tests check the two agree.

use crate::config::CoreConfig;
use crate::fs::Fs;
use crate::globs::{BuiltInExcludes, GlobLayers};
use crate::ignore_rules::IgnoreRules;
use crate::roots::deepest_root;
use crate::CoreError;
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

pub struct Admission {
    walked_roots: Vec<PathBuf>,
    pub(crate) layers: GlobLayers,
    pub(crate) built_in: BuiltInExcludes,
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
            layers: GlobLayers::new(cfg)?,
            built_in: BuiltInExcludes::new(&cfg.built_in_excludes),
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

    /// Whether a file on disk may enter the index. Binary detection happens later, in the scanner.
    pub fn admits_disk(&self, path: &Path) -> bool {
        let Some(root) = deepest_root(path, &self.walked_roots) else {
            return false;
        };
        let rel = rel_components(path, root);
        if rel.is_empty() {
            return false;
        }
        let rel_refs: Vec<&str> = rel.iter().map(String::as_str).collect();
        if !self.include_hidden && rel_refs.iter().any(|c| c.starts_with('.')) {
            return false;
        }
        if self.built_in.file_excluded(&rel_refs) {
            return false;
        }
        if !self.layers.path_allowed(path, Some(root)) {
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
        !(self.respect_ignore_files && self.ignore.is_ignored(path, false))
    }

    /// Whether an open buffer may feed the tree and get decorations. `path`
    /// is `None` for documents without a file path, such as `untitled:`.
    pub fn admits_buffer(&self, path: Option<&Path>, tree_roots: &[PathBuf]) -> bool {
        let Some(path) = path else { return true };
        let root = deepest_root(path, tree_roots);
        self.layers.path_allowed(path, root) && !buffer_hidden(path)
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
