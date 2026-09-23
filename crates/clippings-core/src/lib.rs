//! Core of Clippings: configuration, walking, scanning, tag extraction and the index.

pub mod admission;
pub mod comments;
pub mod config;
pub mod error;
pub mod fs;
pub mod globs;
pub mod ignore_rules;
pub mod pattern;
pub mod position;
pub mod roots;

pub use error::CoreError;

/// Version of the client-server protocol, checked at `initialize` and by `clippings probe`.
pub const PROTOCOL_VERSION: u32 = 1;
