//! Effects returned from `update`. The runtime applies them after the model changes.

use serde::{Deserialize, Serialize};

/// A side effect. `update` stays pure by returning these instead of doing I/O inline.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Command {
    /// No-op, useful as an empty list stand-in in tests.
    None,
    /// Write UTF-8 bytes to a path relative to the app directory.
    WriteFile {
        /// Destination path.
        path: String,
        /// File contents.
        contents: String,
    },
    /// Read a file; the result comes back as a message `{kind, payload: string}`.
    ReadFile {
        /// Source path.
        path: String,
        /// Message kind that receives the file text.
        reply: String,
    },
    /// Arm a one-shot timer. Fires `reply` after `ms` milliseconds.
    Timeout {
        /// Delay in milliseconds.
        ms: u64,
        /// Message kind to dispatch.
        reply: String,
    },
    /// Replace the model with this value. Used by restore-on-boot paths.
    ReplaceModel {
        /// Next model.
        model: serde_json::Value,
    },
}

impl Command {
    /// Convenience constructor.
    pub fn write_file(path: impl Into<String>, contents: impl Into<String>) -> Self {
        Self::WriteFile {
            path: path.into(),
            contents: contents.into(),
        }
    }
}
