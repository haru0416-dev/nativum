//! Layout, software renderer, and the Model/Msg/update session.

#![forbid(unsafe_code)]

mod core_json;
mod expand;
mod font;
mod font_data;
mod hit;
mod layout;
mod manifest;
mod paint;
mod png;
mod session;
mod snapshot;
mod widget;

pub use core_json::JsonCore;
pub use manifest::{load_app_dir, LoadedApp, Manifest, WindowSpec};
pub use paint::Surface;
pub use session::{Frame, Session, Step};
pub use snapshot::SnapshotNode;
pub use widget::{Align, Axis, Handler, Widget, WidgetKind};

pub use nativum_core::{
    tokens_for, Appearance, Color, Command, Error, Message, Point, Rect, Result, Size, TokenSet,
    Value,
};
pub use nativum_markup::{check_document, parse_document, Document};
