use std::process::Command;

fn bin() -> Command {
    Command::new(env!("CARGO_BIN_EXE_clippings"))
}

#[test]
fn probe_prints_versions() {
    let out = bin().arg("probe").output().unwrap();
    assert!(out.status.success());
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(v["protocolVersion"], 1);
    assert_eq!(v["version"], env!("CARGO_PKG_VERSION"));
}

#[test]
fn scan_json_lists_todos_with_relative_paths() {
    let t = tempfile::tempdir().unwrap();
    std::fs::create_dir(t.path().join("src")).unwrap();
    std::fs::write(t.path().join("src/a.ts"), "// TODO hello\n").unwrap();
    let out = bin()
        .args(["scan", "--json"])
        .arg(t.path())
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(v["files"][0]["path"], "src/a.ts");
    assert_eq!(v["files"][0]["todos"][0]["tag"], "TODO");
    assert_eq!(v["files"][0]["todos"][0]["after"], "hello");
}

#[test]
fn scan_text_output_and_flags() {
    let t = tempfile::tempdir().unwrap();
    std::fs::create_dir(t.path().join(".hidden")).unwrap();
    std::fs::write(t.path().join(".hidden/a.ts"), "// FIXME hidden\n").unwrap();
    let plain = bin().arg("scan").arg(t.path()).output().unwrap();
    assert_eq!(String::from_utf8_lossy(&plain.stdout), "");
    let hidden = bin()
        .args(["scan", "--hidden"])
        .arg(t.path())
        .output()
        .unwrap();
    assert_eq!(
        String::from_utf8_lossy(&hidden.stdout),
        ".hidden/a.ts:1:1: FIXME hidden\n"
    );
}

#[test]
fn bad_regex_config_fails_cleanly() {
    let t = tempfile::tempdir().unwrap();
    let cfg = t.path().join("c.json");
    std::fs::write(&cfg, r#"{"regex":"(unclosed"}"#).unwrap();
    let out = bin()
        .args(["scan", "--config"])
        .arg(&cfg)
        .arg(t.path())
        .output()
        .unwrap();
    assert!(!out.status.success());
    assert!(String::from_utf8_lossy(&out.stderr).contains("invalid regex"));
}

#[cfg(unix)]
#[test]
fn scan_through_a_symlinked_root_reports_relative_paths() {
    let t = tempfile::tempdir().unwrap();
    let real = t.path().join("real");
    std::fs::create_dir_all(real.join("src")).unwrap();
    std::fs::write(real.join("src/a.ts"), "// TODO via link\n").unwrap();
    let link = t.path().join("link");
    std::os::unix::fs::symlink(&real, &link).unwrap();
    let out = bin().args(["scan", "--json"]).arg(&link).output().unwrap();
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(v["files"][0]["path"], "src/a.ts");
}
