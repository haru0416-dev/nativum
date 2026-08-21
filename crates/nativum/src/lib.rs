//! Facade: the pieces an app crate needs, re-exported from one place.

#![forbid(unsafe_code)]

pub use nativum_core::{
    tokens_for, Appearance, Color, Command, Error, Message, Point, Rect, Result, Size, TokenSet,
    Value,
};
pub use nativum_engine::{
    load_app_dir, Frame, JsonCore, LoadedApp, Manifest, Session, SnapshotNode, Step, Surface,
    WindowSpec,
};
pub use nativum_markup::{check_document, parse_document, parse_expr, CheckReport, Document};
