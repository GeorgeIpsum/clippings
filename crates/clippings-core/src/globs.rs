//! Exclusion layers 2 to 4 (user globs, temporary globs, VS Code excludes)
//! and layer 1, the built-in never-index list (spec section 5.3).

use crate::config::CoreConfig;
use crate::CoreError;
use globset::{GlobBuilder, GlobSet, GlobSetBuilder};
use std::collections::HashSet;
use std::path::Path;

/// A path as a string with `/` separators.
pub fn slash_path(p: &Path) -> String {
    let s = p.to_string_lossy();
    if cfg!(windows) {
        s.replace('\\', "/")
    } else {
        s.into_owned()
    }
}

/// Compiles globs with literal separators and globset's platform default for
/// backslashes. Empty strings are skipped.
pub fn compile_set(globs: &[String]) -> Result<GlobSet, CoreError> {
    let mut b = GlobSetBuilder::new();
    for g in globs.iter().filter(|g| !g.trim().is_empty()) {
        let glob = GlobBuilder::new(g)
            .literal_separator(true)
            .build()
            .map_err(|e| CoreError::InvalidGlob {
                glob: g.clone(),
                message: e.to_string(),
            })?;
        b.add(glob);
    }
    b.build().map_err(|e| CoreError::InvalidGlob {
        glob: globs.join(", "),
        message: e.to_string(),
    })
}

/// Glob for "Only Show This Folder" (`recursive = false`) or
/// "Only Show This Folder And Subfolders" / "Hide This Folder" (`recursive = true`).
pub fn folder_glob(folder: &Path, recursive: bool) -> String {
    let base = globset::escape(&slash_path(folder));
    if recursive {
        format!("{base}/**/*")
    } else {
        format!("{base}/*")
    }
}

/// Glob for "Hide This File".
pub fn file_glob(file: &Path) -> String {
    globset::escape(&slash_path(file))
}

pub struct GlobLayers {
    has_includes: bool,
    includes: GlobSet,
    excludes: GlobSet,
    /// Exclude globs ending in `/**` or `/**/*` with that suffix removed.
    dir_excludes: GlobSet,
}

impl GlobLayers {
    pub fn new(cfg: &CoreConfig) -> Result<Self, CoreError> {
        let includes: Vec<String> = cfg
            .include_globs
            .iter()
            .chain(&cfg.temp_include_globs)
            .cloned()
            .collect();
        let excludes: Vec<String> = cfg
            .exclude_globs
            .iter()
            .chain(&cfg.temp_exclude_globs)
            .cloned()
            .chain(cfg.vscode_excludes())
            .collect();
        let dir_excludes: Vec<String> = excludes
            .iter()
            .filter_map(|g| g.strip_suffix("/**/*").or_else(|| g.strip_suffix("/**")))
            .filter(|g| !g.is_empty())
            .map(str::to_string)
            .collect();
        Ok(Self {
            has_includes: includes.iter().any(|g| !g.trim().is_empty()),
            includes: compile_set(&includes)?,
            excludes: compile_set(&excludes)?,
            dir_excludes: compile_set(&dir_excludes)?,
        })
    }

    fn any(set: &GlobSet, abs: &str, rel: Option<&str>) -> bool {
        set.is_match(abs) || rel.is_some_and(|r| set.is_match(r))
    }

    /// Include and exclude check for a file. `rel` is the path relative to
    /// its scan root, when it has one.
    pub fn file_allowed(&self, abs: &str, rel: Option<&str>) -> bool {
        if self.has_includes && !Self::any(&self.includes, abs, rel) {
            return false;
        }
        !Self::any(&self.excludes, abs, rel)
    }

    /// Whether a directory is pruned by an exclude glob.
    pub fn dir_pruned(&self, abs: &str, rel: Option<&str>) -> bool {
        Self::any(&self.excludes, abs, rel) || Self::any(&self.dir_excludes, abs, rel)
    }

    /// File check plus a prune check on every ancestor directory. Ancestors
    /// are those strictly below `root` when given, else all ancestors.
    pub fn path_allowed(&self, file: &Path, root: Option<&Path>) -> bool {
        let rel_of = |p: &Path| root.and_then(|r| p.strip_prefix(r).ok()).map(slash_path);
        if !self.file_allowed(&slash_path(file), rel_of(file).as_deref()) {
            return false;
        }
        let mut dir = file.parent();
        while let Some(d) = dir {
            if let Some(r) = root {
                if d == r || !d.starts_with(r) {
                    break;
                }
            }
            if self.dir_pruned(&slash_path(d), rel_of(d).as_deref()) {
                return false;
            }
            dir = d.parent();
        }
        true
    }
}

/// Layer 1. Matched only against components below a scan root.
pub struct BuiltInExcludes {
    names: HashSet<String>,
    suffixes: Vec<Vec<String>>,
}

impl BuiltInExcludes {
    pub fn new(entries: &[String]) -> Self {
        let mut names = HashSet::new();
        let mut suffixes = Vec::new();
        for e in entries
            .iter()
            .map(|e| e.trim().trim_matches('/'))
            .filter(|e| !e.is_empty())
        {
            if e.contains('/') {
                suffixes.push(e.split('/').map(str::to_string).collect());
            } else {
                names.insert(e.to_string());
            }
        }
        Self { names, suffixes }
    }

    /// `rel_dir` holds the components of a directory below its scan root.
    pub fn dir_excluded(&self, rel_dir: &[&str]) -> bool {
        let Some(last) = rel_dir.last() else {
            return false;
        };
        if self.names.contains(*last) {
            return true;
        }
        self.suffixes.iter().any(|s| {
            s.len() <= rel_dir.len()
                && rel_dir[rel_dir.len() - s.len()..]
                    .iter()
                    .zip(s)
                    .all(|(a, b)| a == b)
        })
    }

    /// Whether any directory on the way from the root to `rel_file` is excluded.
    pub fn file_excluded(&self, rel_file: &[&str]) -> bool {
        (1..rel_file.len()).any(|n| self.dir_excluded(&rel_file[..n]))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn layers(f: impl FnOnce(&mut CoreConfig)) -> GlobLayers {
        let mut c = CoreConfig::default();
        f(&mut c);
        GlobLayers::new(&c).unwrap()
    }

    #[test]
    fn star_does_not_cross_separator() {
        let l = layers(|c| c.exclude_globs = vec!["src/*.rs".into()]);
        assert!(!l.file_allowed("/r/src/a.rs", Some("src/a.rs")));
        assert!(l.file_allowed("/r/src/x/a.rs", Some("src/x/a.rs")));
    }

    #[test]
    fn matches_absolute_or_relative() {
        let l = layers(|c| c.exclude_globs = vec!["gen/**".into(), "/abs/only.ts".into()]);
        assert!(!l.file_allowed("/r/gen/a.ts", Some("gen/a.ts")));
        assert!(!l.file_allowed("/abs/only.ts", None));
    }

    #[test]
    fn default_node_modules_glob_prunes_package_dirs() {
        let l = layers(|_| {});
        assert!(l.dir_pruned("/r/node_modules/pkg", Some("node_modules/pkg")));
        assert!(!l.file_allowed("/r/node_modules/pkg/a.js", Some("node_modules/pkg/a.js")));
    }

    #[test]
    fn bare_directory_glob_prunes_directory() {
        let l = layers(|c| {
            c.use_built_in_excludes = crate::config::UseBuiltInExcludes::FileExcludes;
            c.files_exclude = vec!["**/dist".into()];
        });
        assert!(l.dir_pruned("/r/pkg/dist", Some("pkg/dist")));
        assert!(!l.path_allowed(Path::new("/r/pkg/dist/a.js"), Some(Path::new("/r"))));
    }

    #[test]
    fn dot_segments_match_stars() {
        let l = layers(|c| c.exclude_globs = vec!["**/*.rs".into()]);
        assert!(!l.file_allowed("/r/.hidden/a.rs", Some(".hidden/a.rs")));
    }

    #[test]
    fn includes_restrict_files() {
        let l = layers(|c| c.include_globs = vec!["**/src/**".into()]);
        assert!(l.file_allowed("/r/src/a.rs", Some("src/a.rs")));
        assert!(!l.file_allowed("/r/docs/a.md", Some("docs/a.md")));
    }

    #[test]
    fn escaped_folder_glob_matches_bracket_folder() {
        let dir = PathBuf::from("/r/app/[slug]");
        let l = layers(|c| c.temp_exclude_globs = vec![folder_glob(&dir, true)]);
        assert!(!l.file_allowed("/r/app/[slug]/page.tsx", Some("app/[slug]/page.tsx")));
        assert!(l.file_allowed("/r/app/s/page.tsx", Some("app/s/page.tsx")));
        let f =
            layers(|c| c.temp_exclude_globs = vec![file_glob(Path::new("/r/app/[slug]/page.tsx"))]);
        assert!(!f.file_allowed("/r/app/[slug]/page.tsx", None));
    }

    #[test]
    fn built_in_excludes_match_names_and_suffixes_below_root() {
        let b = BuiltInExcludes::new(&["node_modules".into(), ".yarn/cache".into()]);
        assert!(b.dir_excluded(&["a", "node_modules"]));
        assert!(b.dir_excluded(&[".yarn", "cache"]));
        assert!(!b.dir_excluded(&[".yarn"]));
        assert!(b.file_excluded(&["node_modules", "x", "a.js"]));
        assert!(!b.file_excluded(&["src", "node_modules.ts"]));
        assert!(!b.file_excluded(&[]));
    }
}
