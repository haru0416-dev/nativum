//! Toolkit error type. Markup errors carry `file:line:column` when a span is known.

use std::fmt;

/// nativum error.
#[derive(Clone, Debug)]
pub enum Error {
    /// Parse or check diagnostic with source location.
    Located {
        /// 1-based line.
        line: usize,
        /// 1-based column.
        column: usize,
        /// Human-readable message. Teaching, not cryptic.
        message: String,
        /// Optional path.
        path: Option<String>,
    },
    /// Type or evaluation error without a span.
    Type(String),
    /// I/O.
    Io(String),
    /// App manifest or core.json.
    Config(String),
}

impl Error {
    /// Construct a located diagnostic.
    pub fn at(line: usize, column: usize, message: impl AsRef<str>) -> Self {
        Self::Located {
            line,
            column,
            message: message.as_ref().to_string(),
            path: None,
        }
    }

    /// Attach a file path to a located error.
    pub fn with_path(mut self, path: impl Into<String>) -> Self {
        if let Self::Located { path: slot, .. } = &mut self {
            *slot = Some(path.into());
        }
        self
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Located {
                line,
                column,
                message,
                path,
            } => {
                if let Some(p) = path {
                    write!(f, "{p}:{line}:{column}: {message}")
                } else {
                    write!(f, "{line}:{column}: {message}")
                }
            }
            Self::Type(m) | Self::Io(m) | Self::Config(m) => f.write_str(m),
        }
    }
}

impl std::error::Error for Error {}

impl From<std::io::Error> for Error {
    fn from(e: std::io::Error) -> Self {
        Self::Io(e.to_string())
    }
}

impl From<serde_json::Error> for Error {
    fn from(e: serde_json::Error) -> Self {
        Self::Config(e.to_string())
    }
}

/// Result alias.
pub type Result<T> = std::result::Result<T, Error>;
