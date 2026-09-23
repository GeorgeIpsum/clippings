mod support;

use clippings_core::admission::Admission;
use clippings_core::config::CoreConfig;
use clippings_core::fs::NativeFs;
use clippings_core::pattern;
use clippings_core::report::scan_report;
use clippings_core::walker::walk_and_scan;
use std::path::PathBuf;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;

fn fixture() -> (tempfile::TempDir, PathBuf) {
    let t = tempfile::tempdir().unwrap();
    let root = dunce::canonicalize(t.path()).unwrap();
    support::build_fixture(&root);
    (t, root)
}

#[test]
fn default_scan_matches_golden() {
    let (_t, root) = fixture();
    let report = scan_report(&[root], &CoreConfig::default(), Arc::new(NativeFs)).unwrap();
    insta::assert_json_snapshot!("default_scan", report);
}

#[test]
fn sub_tag_and_multi_line_scan_matches_golden() {
    let (_t, root) = fixture();
    let cfg = CoreConfig {
        sub_tag_regex: r"^\s*\((.*?)\)".into(),
        regex: r"(//|#|<!--|;|/\*|^|^[ \t]*(-|\d+.))\s*($TAGS).*(\n\s*//\s{2,}.*)*".into(),
        ..Default::default()
    };
    let report = scan_report(&[root], &cfg, Arc::new(NativeFs)).unwrap();
    insta::assert_json_snapshot!("sub_tag_multi_line_scan", report);
}

#[test]
fn walk_and_admission_agree_on_every_file() {
    let (_t, root) = fixture();
    for cfg in [
        CoreConfig::default(),
        CoreConfig {
            include_hidden_files: true,
            ..Default::default()
        },
        CoreConfig {
            ignore_git_submodules: true,
            ..Default::default()
        },
        CoreConfig {
            built_in_excludes: vec![],
            include_hidden_files: true,
            ..Default::default()
        },
    ] {
        let p = pattern::build(&cfg).unwrap();
        let fs = Arc::new(NativeFs);
        let walk = walk_and_scan(
            &cfg,
            std::slice::from_ref(&root),
            &p,
            fs.clone(),
            &AtomicBool::new(false),
        )
        .unwrap();
        let admission = Admission::new(&cfg, vec![root.clone()], fs).unwrap();
        for entry in walkdir(&root) {
            let binary = entry.extension().is_some_and(|e| e == "bin" || e == "dat");
            let admitted = admission.admits_disk(&entry);
            let walked = walk.seen.binary_search(&entry).is_ok();
            if !binary {
                assert_eq!(
                    admitted,
                    walked,
                    "{} under {:?}",
                    entry.display(),
                    cfg.built_in_excludes.len()
                );
            }
        }
    }
}

fn walkdir(root: &std::path::Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(d) = stack.pop() {
        for e in std::fs::read_dir(&d).unwrap() {
            let p = e.unwrap().path();
            if p.file_name().is_some_and(|n| n == ".git") {
                continue;
            }
            if p.is_dir() {
                stack.push(p)
            } else {
                out.push(p)
            }
        }
    }
    out
}
