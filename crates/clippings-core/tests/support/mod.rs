//! Builds the fixture workspace from spec section 12.2 in a temporary directory.
//!
//! `root` must be a subdirectory of a scratch directory: the fixture also
//! writes a stray `.gitignore` into `root`'s parent, above the repository.

use std::fs;
use std::path::Path;
use std::process::Command;

fn write(root: &Path, rel: &str, contents: impl AsRef<[u8]>) {
    let p = root.join(rel);
    fs::create_dir_all(p.parent().unwrap()).unwrap();
    fs::write(p, contents).unwrap();
}

fn git_init(dir: &Path) {
    let ok = Command::new("git")
        .args(["init", "-q"])
        .current_dir(dir)
        .status()
        .unwrap()
        .success();
    assert!(ok, "git init failed in {}", dir.display());
}

pub fn build_fixture(root: &Path) {
    git_init(root);
    write(root, ".gitignore", "dist/\n*.log\n");
    write(
        root,
        "src/main.ts",
        "export const x = 1; // TODO rename x\n/* FIXME block comment */\n",
    );
    write(
        root,
        "src/lib.rs",
        "fn a() {} // TODO(alice) sub tag\n// HACK: two // XXX on one line\n",
    );
    write(
        root,
        "src/app.py",
        "# BUG crashes on empty input\nprint('no tag')\n",
    );
    write(
        root,
        "src/unicode.ts",
        "const é = 'ééé'; // TODO after non-ascii\n",
    );
    write(
        root,
        "src/crlf.ts",
        "// TODO windows line\r\nconst y = 2;\r\n",
    );
    write(
        root,
        "src/long.ts",
        format!("// TODO {}\n", "long ".repeat(400)),
    );
    write(root, "src/multi.ts", "// TODO first line\n//   second line\n//   third line\n// FIXME next\n//   next continued\n");
    write(
        root,
        "docs/notes.md",
        "# Notes\n- [ ] write docs\n- [x] done item\n<!-- TODO html comment -->\n",
    );
    write(root, "app/[slug]/page.tsx", "// TODO dynamic route\n");
    write(root, "dist/bundle.js", "// TODO ignored by gitignore\n");
    write(root, "debug.log", "TODO ignored log\n");
    write(
        root,
        "node_modules/pkg/index.js",
        "// TODO in node_modules\n",
    );
    write(root, ".claude/notes.md", "- [ ] hidden and built-in\n");
    write(
        root,
        ".github/workflows/ci.yml",
        "# TODO hidden directory\n",
    );
    let mut bin = b"\x00\x01binary // TODO in binary".to_vec();
    bin.extend_from_slice(b"\n");
    write(root, "assets/early-nul.bin", bin);
    let mut late = b"// TODO before late nul\n".to_vec();
    late.extend(std::iter::repeat_n(b'a', 70_000));
    late.extend_from_slice(b"\n\x00\n// TODO after late nul\n");
    write(root, "assets/late-nul.dat", late);
    let nested = root.join("vendor/nested");
    fs::create_dir_all(&nested).unwrap();
    git_init(&nested);
    write(root, "vendor/nested/lib.c", "/* TODO nested repo */\n");
    // Ignore-rule precedence cases the walker and admission must agree on.
    // (a) The outer `*.log` rule stops at the nested repository's root.
    write(root, "vendor/nested/debug.log", "TODO log in nested repo\n");
    // (b) A `.gitignore` above the repository root does not apply.
    write(root.parent().unwrap(), ".gitignore", "stray.txt\n");
    write(
        root,
        "stray.txt",
        "// TODO above-repo gitignore does not apply\n",
    );
    // (c) A `.ignore` whitelist beats a deeper `.gitignore` rule.
    write(root, ".ignore", "!keep.txt\n!.whitelisted/\n");
    write(root, "sub/.gitignore", "keep.txt\n");
    write(root, "sub/keep.txt", "// TODO whitelisted by .ignore\n");
    // An ignore-file whitelist beats the hidden rule.
    write(
        root,
        ".whitelisted/notes.md",
        "- [ ] whitelisted hidden directory\n",
    );
    // (d) `.rgignore` applies only while ignore files are respected.
    write(root, ".rgignore", "scratch/\n");
    write(root, "scratch/notes.txt", "// TODO ignored by rgignore\n");
}
