//! Comment leaders by file extension, a port of the parts of the
//! `comment-patterns` table todo-tree uses: single-line leaders for extra
//! lines and block markers for the primary text.

use std::path::Path;

pub struct CommentSyntax {
    pub line: &'static [&'static str],
    pub block: &'static [(&'static str, &'static str)],
}

const C_LIKE: CommentSyntax = CommentSyntax {
    line: &["//"],
    block: &[("/*", "*/")],
};
const HASH: CommentSyntax = CommentSyntax {
    line: &["#"],
    block: &[],
};
const HTML: CommentSyntax = CommentSyntax {
    line: &[],
    block: &[("<!--", "-->")],
};
const DASH: CommentSyntax = CommentSyntax {
    line: &["--"],
    block: &[],
};
const SEMI: CommentSyntax = CommentSyntax {
    line: &[";"],
    block: &[],
};
const PERCENT: CommentSyntax = CommentSyntax {
    line: &["%"],
    block: &[],
};
const QUOTE: CommentSyntax = CommentSyntax {
    line: &["\""],
    block: &[],
};
const HASKELL: CommentSyntax = CommentSyntax {
    line: &["--"],
    block: &[("{-", "-}")],
};
const PYTHON: CommentSyntax = CommentSyntax {
    line: &["#"],
    block: &[("\"\"\"", "\"\"\""), ("'''", "'''")],
};
const LUA: CommentSyntax = CommentSyntax {
    line: &["--"],
    block: &[("--[[", "]]")],
};
const NONE: CommentSyntax = CommentSyntax {
    line: &[],
    block: &[],
};

/// The comment syntax for `path`, from its file name's extension, or for a
/// few well-known extensionless names such as `Dockerfile`.
pub fn syntax_for(path: &str) -> &'static CommentSyntax {
    let path = Path::new(path);
    let ext = match path.extension() {
        Some(e) => e.to_string_lossy().to_ascii_lowercase(),
        None => {
            let name = path.file_name().unwrap_or_default().to_string_lossy();
            return match name.to_ascii_lowercase().as_str() {
                "dockerfile" | "makefile" | "rakefile" | "gemfile" => &HASH,
                _ => &NONE,
            };
        }
    };
    match ext.as_str() {
        "c" | "h" | "cc" | "cpp" | "cxx" | "hpp" | "cs" | "java" | "js" | "jsx" | "mjs" | "cjs"
        | "ts" | "tsx" | "mts" | "cts" | "jsonc" | "go" | "rs" | "swift" | "kt" | "kts"
        | "scala" | "dart" | "php" | "css" | "scss" | "less" | "groovy" | "gradle" | "proto"
        | "zig" | "sol" | "prisma" => &C_LIKE,
        "py" | "pyw" => &PYTHON,
        "sh" | "bash" | "zsh" | "fish" | "rb" | "pl" | "pm" | "r" | "yaml" | "yml" | "toml"
        | "ini" | "cfg" | "conf" | "dockerfile" | "mk" | "cmake" | "nix" | "ps1" | "tf" | "ex"
        | "exs" | "jl" => &HASH,
        "html" | "htm" | "xml" | "svg" | "vue" | "md" | "markdown" | "svelte" => &HTML,
        "sql" | "ada" | "elm" => &DASH,
        "lua" => &LUA,
        "hs" => &HASKELL,
        "lisp" | "clj" | "cljs" | "el" | "scm" | "asm" => &SEMI,
        "tex" | "erl" | "m" => &PERCENT,
        "vim" => &QUOTE,
        _ => &NONE,
    }
}

/// Trims `text` and strips one leading single-line comment leader.
pub fn strip_line_comment(text: &str, path: &str) -> String {
    let t = text.trim();
    for leader in syntax_for(path).line {
        if let Some(rest) = t.strip_prefix(leader) {
            return rest.trim().to_string();
        }
    }
    t.to_string()
}

/// Strips a leading block-comment opener and trailing closer from `text`.
pub fn strip_block_comment(text: &str, path: &str) -> String {
    let t = text.trim();
    for (open, close) in syntax_for(path).block {
        if let Some(rest) = t.strip_prefix(open) {
            let rest = rest.trim_end();
            return rest.strip_suffix(close).unwrap_or(rest).trim().to_string();
        }
    }
    t.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strips_line_leaders_by_extension() {
        assert_eq!(strip_line_comment("   // more text ", "a.ts"), "more text");
        assert_eq!(strip_line_comment("# more", "a.py"), "more");
        assert_eq!(strip_line_comment("-- more", "q.sql"), "more");
        assert_eq!(strip_line_comment("// kept", "a.unknown"), "// kept");
    }

    #[test]
    fn syntax_comes_from_the_file_name_only() {
        assert_eq!(syntax_for("a/b.c/Dockerfile").line, &["#"]);
        assert_eq!(syntax_for("Makefile").line, &["#"]);
        assert_eq!(syntax_for("x/Rakefile").line, &["#"]);
        assert_eq!(syntax_for("x/Gemfile").line, &["#"]);
        assert!(syntax_for("x.dir/README").line.is_empty());
        assert!(syntax_for("CMakeLists.txt").line.is_empty());
        assert_eq!(syntax_for("a/b.RS").line, &["//"]);
    }

    #[test]
    fn strips_block_markers() {
        assert_eq!(strip_block_comment("/* TODO x */", "a.c"), "TODO x");
        assert_eq!(strip_block_comment("<!-- TODO y -->", "a.md"), "TODO y");
        assert_eq!(strip_block_comment("{- TODO z -}", "a.hs"), "TODO z");
    }
}
