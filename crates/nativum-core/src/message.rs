//! Runtime messages dispatched from markup `on-*` attributes.

use serde::{Deserialize, Serialize};

use crate::value::Value;

/// A tagged message. Markup writes `on-press="increment"` or `on-press="done:{h.id}"`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Message {
    /// Variant tag, matching a `core.json` handler name or a Rust `Msg` kind.
    pub kind: String,
    /// Optional payload evaluated from the `{binding}` after the colon.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub payload: Option<Value>,
}

impl Message {
    /// Tag with no payload.
    pub fn plain(kind: impl Into<String>) -> Self {
        Self {
            kind: kind.into(),
            payload: None,
        }
    }

    /// Tag with a payload.
    pub fn with_payload(kind: impl Into<String>, payload: Value) -> Self {
        Self {
            kind: kind.into(),
            payload: Some(payload),
        }
    }
}
