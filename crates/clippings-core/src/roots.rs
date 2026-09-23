//! Tree roots, scan roots and walked roots (spec section 5.2 and 5.9).

use crate::config::{CoreConfig, ScanMode};
use crate::globs::compile_set;
use crate::CoreError;
use std::path::{Path, PathBuf};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Roots {
    /// Workspace folders shown as root nodes, after include/exclude filtering.
    pub tree_roots: Vec<PathBuf>,
    /// What the walker walks: the `rootFolder` expansion, or the tree roots.
    pub scan_roots: Vec<PathBuf>,
    /// True when `scan_roots` came from `rootFolder`.
    pub from_root_folder: bool,
}

/// Replaces `${NAME}` with the environment variable NAME (empty if unset).
/// `${workspaceFolder}` is left alone; the caller expands it.
pub fn expand_env(input: &str, env: &dyn Fn(&str) -> Option<String>) -> String {
    let mut out = String::with_capacity(input.len());
    let mut rest = input;
    while let Some(start) = rest.find("${") {
        out.push_str(&rest[..start]);
        let after = &rest[start + 2..];
        match after.find('}') {
            Some(end) => {
                let name = &after[..end];
                if name == "workspaceFolder" {
                    out.push_str("${workspaceFolder}");
                } else {
                    out.push_str(&env(name).unwrap_or_default());
                }
                rest = &after[end + 1..];
            }
            None => {
                out.push_str(&rest[start..]);
                rest = "";
            }
        }
    }
    out.push_str(rest);
    out
}

pub fn resolve_roots(
    workspace_folders: &[PathBuf],
    cfg: &CoreConfig,
    env: &dyn Fn(&str) -> Option<String>,
) -> Result<Roots, CoreError> {
    let includes = compile_set(&cfg.included_workspaces)?;
    let excludes = compile_set(&cfg.excluded_workspaces)?;
    let allowed = |p: &Path| {
        let s = crate::globs::slash_path(p);
        (cfg.included_workspaces.is_empty() || includes.is_match(&s)) && !excludes.is_match(&s)
    };
    let tree_roots: Vec<PathBuf> = workspace_folders
        .iter()
        .filter(|p| allowed(p))
        .cloned()
        .collect();

    let root_folder = cfg.root_folder.trim();
    if root_folder.is_empty() {
        return Ok(Roots {
            scan_roots: tree_roots.clone(),
            tree_roots,
            from_root_folder: false,
        });
    }
    let expanded = expand_env(root_folder, env);
    let scan_roots: Vec<PathBuf> = if expanded.contains("${workspaceFolder}") {
        workspace_folders
            .iter()
            .map(|f| PathBuf::from(expanded.replace("${workspaceFolder}", &f.to_string_lossy())))
            .filter(|p| allowed(p))
            .collect()
    } else {
        vec![PathBuf::from(expanded)]
    };
    Ok(Roots {
        tree_roots,
        scan_roots,
        from_root_folder: true,
    })
}

/// Scan roots that `mode` walks: all of them in the workspace modes, only a
/// `rootFolder` root in the open-file modes.
pub fn walked_roots(roots: &Roots, mode: ScanMode) -> Vec<PathBuf> {
    match mode {
        ScanMode::Workspace | ScanMode::WorkspaceOnly => roots.scan_roots.clone(),
        ScanMode::OpenFiles | ScanMode::CurrentFile => {
            if roots.from_root_folder {
                roots.scan_roots.clone()
            } else {
                Vec::new()
            }
        }
    }
}

/// The deepest root that contains `path`.
pub fn deepest_root<'a>(path: &Path, roots: &'a [PathBuf]) -> Option<&'a Path> {
    roots
        .iter()
        .filter(|r| path.starts_with(r))
        .max_by_key(|r| r.components().count())
        .map(|r| r.as_path())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn no_env(_: &str) -> Option<String> {
        None
    }

    #[test]
    fn expands_env_but_not_workspace_folder() {
        let env = |n: &str| (n == "HOME").then(|| "/home/u".to_string());
        assert_eq!(
            expand_env("${HOME}/x/${workspaceFolder}/${NOPE}", &env),
            "/home/u/x/${workspaceFolder}/"
        );
    }

    #[test]
    fn tree_roots_are_scan_roots_without_root_folder() {
        let folders = vec![PathBuf::from("/w/a"), PathBuf::from("/w/b")];
        let r = resolve_roots(&folders, &CoreConfig::default(), &no_env).unwrap();
        assert_eq!(r.tree_roots, folders);
        assert_eq!(r.scan_roots, folders);
        assert!(!r.from_root_folder);
    }

    #[test]
    fn excluded_workspaces_filter_by_path_glob() {
        let folders = vec![PathBuf::from("/w/a"), PathBuf::from("/w/skip")];
        let cfg = CoreConfig {
            excluded_workspaces: vec!["**/skip".into()],
            ..Default::default()
        };
        let r = resolve_roots(&folders, &cfg, &no_env).unwrap();
        assert_eq!(r.tree_roots, vec![PathBuf::from("/w/a")]);
    }

    #[test]
    fn root_folder_expands_per_workspace_folder() {
        let folders = vec![PathBuf::from("/w/a"), PathBuf::from("/w/b")];
        let cfg = CoreConfig {
            root_folder: "${workspaceFolder}/src".into(),
            ..Default::default()
        };
        let r = resolve_roots(&folders, &cfg, &no_env).unwrap();
        assert_eq!(r.tree_roots, folders);
        assert_eq!(
            r.scan_roots,
            vec![PathBuf::from("/w/a/src"), PathBuf::from("/w/b/src")]
        );
        assert!(r.from_root_folder);
    }

    #[test]
    fn open_file_modes_walk_only_root_folder() {
        let folders = vec![PathBuf::from("/w/a")];
        let plain = resolve_roots(&folders, &CoreConfig::default(), &no_env).unwrap();
        assert!(walked_roots(&plain, ScanMode::OpenFiles).is_empty());
        assert_eq!(walked_roots(&plain, ScanMode::WorkspaceOnly), folders);
        let cfg = CoreConfig {
            root_folder: "/elsewhere".into(),
            ..Default::default()
        };
        let rf = resolve_roots(&folders, &cfg, &no_env).unwrap();
        assert_eq!(
            walked_roots(&rf, ScanMode::CurrentFile),
            vec![PathBuf::from("/elsewhere")]
        );
    }

    #[test]
    fn deepest_root_wins() {
        let roots = vec![PathBuf::from("/w"), PathBuf::from("/w/inner")];
        assert_eq!(
            deepest_root(Path::new("/w/inner/x.rs"), &roots),
            Some(Path::new("/w/inner"))
        );
        assert_eq!(
            deepest_root(Path::new("/w/y.rs"), &roots),
            Some(Path::new("/w"))
        );
        assert_eq!(deepest_root(Path::new("/other/y.rs"), &roots), None);
    }
}
