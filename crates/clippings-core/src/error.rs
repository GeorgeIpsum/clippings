use thiserror::Error;

#[derive(Debug, Error)]
pub enum CoreError {
    #[error("invalid regex: {0}")]
    InvalidRegex(String),
    #[error("invalid glob {glob:?}: {message}")]
    InvalidGlob { glob: String, message: String },
    #[error("io error on {path}: {source}")]
    Io {
        path: String,
        #[source]
        source: std::io::Error,
    },
}
