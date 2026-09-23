//! Core of Clippings: configuration, walking, scanning, tag extraction and the index.

pub mod admission;
pub mod colour_names;
pub mod colours;
pub mod comments;
pub mod config;
pub mod decorations;
pub mod documents;
pub mod error;
pub mod extract;
pub mod fs;
pub mod globs;
pub mod ignore_rules;
pub mod index;
pub mod labels;
pub mod model;
pub mod navigate;
pub mod pattern;
pub mod position;
pub mod protocol;
pub mod report;
pub mod roots;
pub mod scanner;
pub mod settings;
pub mod status;
pub mod styles;
pub mod uri;
pub mod view;
pub mod walker;

pub use error::CoreError;

/// Version of the client-server protocol, checked at `initialize` and by `clippings probe`.
pub const PROTOCOL_VERSION: u32 = 1;
