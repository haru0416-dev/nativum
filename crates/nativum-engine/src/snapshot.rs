//! Accessibility-style snapshot of the laid-out tree. Agents drive apps from this.

use serde::Serialize;

use crate::widget::{Widget, WidgetKind};

/// One node in the snapshot.
#[derive(Clone, Debug, Serialize)]
pub struct SnapshotNode {
    /// Structural id.
    pub id: u64,
    /// Role string.
    pub role: String,
    /// Accessible name.
    pub label: String,
    /// Displayed value / text.
    pub value: String,
    /// Frame `[x, y, width, height]`.
    pub frame: [f32; 4],
    /// Bound message kinds.
    pub actions: Vec<String>,
    /// Children.
    pub children: Vec<SnapshotNode>,
}

impl SnapshotNode {
    /// Build from a widget tree.
    pub fn from_widget(w: &Widget) -> Self {
        let mut actions = Vec::new();
        if let Some(h) = &w.on_press {
            actions.push(format!("press:{}", h.kind));
        }
        if let Some(h) = &w.on_toggle {
            actions.push(format!("toggle:{}", h.kind));
        }
        if let Some(h) = &w.on_input {
            actions.push(format!("input:{}", h.kind));
        }
        if let Some(h) = &w.on_submit {
            actions.push(format!("submit:{}", h.kind));
        }
        Self {
            id: w.id,
            role: role_of(w.kind).into(),
            label: if w.label.is_empty() {
                w.text.clone()
            } else {
                w.label.clone()
            },
            value: w.text.clone(),
            frame: [w.frame.x, w.frame.y, w.frame.width, w.frame.height],
            actions,
            children: w.children.iter().map(Self::from_widget).collect(),
        }
    }
}

fn role_of(kind: WidgetKind) -> &'static str {
    match kind {
        WidgetKind::Button => "button",
        WidgetKind::Checkbox => "checkbox",
        WidgetKind::Radio => "radio",
        WidgetKind::TextField => "textbox",
        WidgetKind::Text => "text",
        WidgetKind::Slider => "slider",
        WidgetKind::Progress => "progressbar",
        WidgetKind::Scroll => "scrollview",
        WidgetKind::StatusBar => "status",
        WidgetKind::Alert => "alert",
        WidgetKind::Separator => "separator",
        WidgetKind::Badge => "badge",
        WidgetKind::Row | WidgetKind::Column | WidgetKind::Stack | WidgetKind::Surface => "group",
        WidgetKind::Spacer => "none",
        WidgetKind::Fragment => "group",
    }
}
