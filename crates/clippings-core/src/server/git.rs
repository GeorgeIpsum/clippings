//! `git rev-parse HEAD` for automatic git refresh (spec section 5.11): run
//! without a shell, one call per workspace folder.

use std::path::Path;
use std::process::Command;

/// The folder's HEAD commit, or `None` when it is not a git work tree.
pub fn head(folder: &Path) -> Option<String> {
    let out = Command::new("git")
        .args(["rev-parse", "HEAD"])
        .current_dir(folder)
        .output()
        .ok()?;
    out.status
        .success()
        .then(|| String::from_utf8_lossy(&out.stdout).trim().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn head_of_a_fresh_repo_and_a_plain_dir() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(head(dir.path()), None);
        let run = |args: &[&str]| {
            assert!(Command::new("git")
                .args(args)
                .current_dir(dir.path())
                .status()
                .unwrap()
                .success());
        };
        run(&["init", "-q"]);
        run(&[
            "-c",
            "user.name=t",
            "-c",
            "user.email=t@t",
            "commit",
            "-q",
            "--allow-empty",
            "-m",
            "x",
        ]);
        let h = head(dir.path()).unwrap();
        assert_eq!(h.len(), 40);
    }
}
