//! File content access. The scanner reads bytes only through [`Fs`], so a
//! later web build can serve reads from the client. The walker module uses
//! the `ignore` crate, which touches `std::fs` itself, and is native-only.

use std::io;
use std::path::Path;

pub trait Fs: Send + Sync {
    /// Reads a whole file.
    fn read(&self, path: &Path) -> io::Result<Vec<u8>>;
    /// File size in bytes, without reading it.
    fn len(&self, path: &Path) -> io::Result<u64>;
    /// Whether a path exists (file, directory or symlink).
    fn exists(&self, path: &Path) -> bool;
    /// Whether a path is a directory.
    fn is_dir(&self, path: &Path) -> bool;
}

#[derive(Clone, Copy, Debug, Default)]
pub struct NativeFs;

impl Fs for NativeFs {
    fn read(&self, path: &Path) -> io::Result<Vec<u8>> {
        std::fs::read(path)
    }
    fn len(&self, path: &Path) -> io::Result<u64> {
        Ok(std::fs::metadata(path)?.len())
    }
    fn exists(&self, path: &Path) -> bool {
        std::fs::symlink_metadata(path).is_ok()
    }
    fn is_dir(&self, path: &Path) -> bool {
        std::fs::metadata(path).map(|m| m.is_dir()).unwrap_or(false)
    }
}
