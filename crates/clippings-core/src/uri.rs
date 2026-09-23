//! File URIs in the exact form VS Code produces (`URI.file(path).toString()`),
//! and the reverse (spec section 5.7).

use std::path::{Path, PathBuf};

fn unreserved(b: u8) -> bool {
    b.is_ascii_alphanumeric() || matches!(b, b'-' | b'.' | b'_' | b'~')
}

fn encode_segment(out: &mut String, s: &str) {
    for &b in s.as_bytes() {
        if unreserved(b) {
            out.push(b as char);
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
}

/// `file:` URI for an absolute path, encoded as VS Code encodes it: every
/// byte outside `A-Za-z0-9-._~` is percent-encoded except `/`, and a Windows
/// drive letter is lower-cased with its colon encoded as `%3A`.
pub fn file_uri(path: &Path) -> String {
    let s = path.to_string_lossy().replace('\\', "/");
    let mut rest = s.as_str();
    let mut out = String::from("file://");
    let bytes = rest.as_bytes();
    if bytes.len() >= 2 && bytes[0].is_ascii_alphabetic() && bytes[1] == b':' {
        out.push('/');
        out.push((bytes[0] as char).to_ascii_lowercase());
        out.push_str("%3A");
        rest = &rest[2..];
    }
    let mut first = true;
    for segment in rest.split('/') {
        if !first {
            out.push('/');
        }
        first = false;
        encode_segment(&mut out, segment);
    }
    out
}

fn hex(b: u8) -> Option<u8> {
    (b as char).to_digit(16).map(|d| d as u8)
}

/// Percent-decodes a string, leaving malformed escapes as they are.
pub fn percent_decode(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%' && i + 2 < b.len() {
            if let (Some(h), Some(l)) = (hex(b[i + 1]), hex(b[i + 2])) {
                out.push(h * 16 + l);
                i += 3;
                continue;
            }
        }
        out.push(b[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// The path part of a URI: everything after `scheme://authority`, decoded,
/// without query or fragment.
pub fn uri_path(uri: &str) -> Option<String> {
    let (_, rest) = uri.split_once(':')?;
    let rest = rest
        .strip_prefix("//")
        .map(|r| r.find('/').map_or("", |i| &r[i..]))
        .unwrap_or(rest);
    let rest = rest.split(['?', '#']).next().unwrap_or("");
    Some(percent_decode(rest))
}

/// The URI's scheme, lower-cased.
pub fn scheme(uri: &str) -> Option<String> {
    uri.split_once(':').map(|(s, _)| s.to_ascii_lowercase())
}

/// Filesystem path for a `file:` URI, or for a `vscode-notebook-cell:` URI
/// whose path is the notebook's file path. Other schemes have no path.
pub fn uri_to_path(uri: &str) -> Option<PathBuf> {
    let s = scheme(uri)?;
    if s != "file" && s != "vscode-notebook-cell" {
        return None;
    }
    let p = uri_path(uri)?;
    let b = p.as_bytes();
    // `/c:/x` is a Windows drive path.
    if b.len() >= 3 && b[0] == b'/' && b[1].is_ascii_alphabetic() && b[2] == b':' {
        let drive = (b[1] as char).to_ascii_uppercase();
        return Some(PathBuf::from(format!(
            "{drive}:{}",
            p[3..].replace('/', "\\")
        )));
    }
    Some(PathBuf::from(p))
}

/// Last path segment of a URI, decoded.
pub fn uri_basename(uri: &str) -> String {
    let p = uri_path(uri).unwrap_or_default();
    p.rsplit('/').next().unwrap_or("").to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encodes_like_vscode() {
        assert_eq!(
            file_uri(Path::new("/Users/g1n/a b/c.ts")),
            "file:///Users/g1n/a%20b/c.ts"
        );
        assert_eq!(
            file_uri(Path::new("/r/app/[slug]/page.tsx")),
            "file:///r/app/%5Bslug%5D/page.tsx"
        );
        assert_eq!(file_uri(Path::new("/r/é.ts")), "file:///r/%C3%A9.ts");
        assert_eq!(
            file_uri(Path::new("C:\\Users\\x\\a.ts")),
            "file:///c%3A/Users/x/a.ts"
        );
    }

    #[test]
    fn decodes_file_uris() {
        assert_eq!(
            uri_to_path("file:///r/app/%5Bslug%5D/page.tsx"),
            Some(PathBuf::from("/r/app/[slug]/page.tsx"))
        );
        assert_eq!(
            uri_to_path("file:///c%3A/Users/x/a.ts"),
            Some(PathBuf::from("C:\\Users\\x\\a.ts"))
        );
        assert_eq!(
            uri_to_path("vscode-notebook-cell:/r/n.ipynb#W0sZmlsZQ%3D%3D"),
            Some(PathBuf::from("/r/n.ipynb"))
        );
        assert_eq!(uri_to_path("untitled:Untitled-1"), None);
    }

    #[test]
    fn round_trips() {
        for p in ["/a/b c/d.rs", "/x/%/y", "/q/[a]/(b)/c.ts"] {
            assert_eq!(uri_to_path(&file_uri(Path::new(p))), Some(PathBuf::from(p)));
        }
    }

    #[test]
    fn basenames() {
        assert_eq!(uri_basename("untitled:Untitled-1"), "Untitled-1");
        assert_eq!(uri_basename("file:///r/a%20b.ts"), "a b.ts");
    }
}
