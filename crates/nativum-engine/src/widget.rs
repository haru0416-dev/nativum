//! Flattened widget tree: what layout and paint see after `for`/`if` expansion.

use nativum_core::{Color, Edges, Rect};
use nativum_markup::Expr;

/// Flex axis.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Axis {
    /// `row`
    Horizontal,
    /// `column`
    Vertical,
}

/// Main/cross alignment.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Align {
    /// Pack toward the start.
    Start,
    /// Center on the axis.
    Center,
    /// Pack toward the end.
    End,
    /// Stretch on the cross axis (default for containers).
    Stretch,
}

impl Align {
    /// Parse `main` / `cross` attribute values.
    pub fn parse(raw: &str) -> Self {
        match raw {
            "center" => Self::Center,
            "end" => Self::End,
            "stretch" => Self::Stretch,
            _ => Self::Start,
        }
    }
}

/// A bound message. Payload is evaluated at dispatch against `scope`.
#[derive(Clone, Debug)]
pub struct Handler {
    /// Message kind.
    pub kind: String,
    /// Optional payload expression (path-only in the Native SDK dialect).
    pub payload: Option<Expr>,
    /// Extra bindings captured from `for` (`h`, ...).
    pub scope: nativum_core::Value,
}

/// Visual / interactive kind.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WidgetKind {
    /// Flex row.
    Row,
    /// Flex column.
    Column,
    /// Overlay stack.
    Stack,
    /// Card / panel surface.
    Surface,
    /// Scroll clip.
    Scroll,
    /// Flexible or fixed empty space.
    Spacer,
    /// Hairline.
    Separator,
    /// Text leaf.
    Text,
    /// Badge leaf.
    Badge,
    /// Status bar.
    StatusBar,
    /// Button.
    Button,
    /// Checkbox.
    Checkbox,
    /// Radio.
    Radio,
    /// Text field.
    TextField,
    /// Slider track.
    Slider,
    /// Progress bar.
    Progress,
    /// Alert well.
    Alert,
    /// Expansion wrapper. Layout splices children into the parent.
    Fragment,
}

/// One laid-out (or to-be-laid-out) widget.
#[derive(Clone, Debug)]
pub struct Widget {
    /// Structural identity, stable across rebuilds when `key` is set.
    pub id: u64,
    /// Kind.
    pub kind: WidgetKind,
    /// Accessible name.
    pub label: String,
    /// Resolved text content.
    pub text: String,
    /// Placeholder for empty fields.
    pub placeholder: String,
    /// Flex grow weight.
    pub grow: f32,
    /// Definite width.
    pub width: Option<f32>,
    /// Definite height.
    pub height: Option<f32>,
    /// Minimum width.
    pub min_width: Option<f32>,
    /// Padding.
    pub padding: Edges,
    /// Gap between children.
    pub gap: f32,
    /// Main-axis alignment.
    pub main: Align,
    /// Cross-axis alignment.
    pub cross: Align,
    /// Background fill.
    pub background: Option<Color>,
    /// Foreground (text / icon).
    pub foreground: Option<Color>,
    /// Border color.
    pub border: Option<Color>,
    /// Corner radius in px.
    pub radius: f32,
    /// Control variant (`primary`, `secondary`, `default`).
    pub variant: String,
    /// Size token (`sm`, `default`, `lg`, `heading`, `display`).
    pub size: String,
    /// Selected / checked.
    pub selected: bool,
    /// Disabled.
    pub disabled: bool,
    /// Numeric value (slider/progress).
    pub value: f32,
    /// Word wrap for text.
    pub wrap: bool,
    /// Text alignment.
    pub text_align: Align,
    /// Press handler.
    pub on_press: Option<Handler>,
    /// Toggle handler.
    pub on_toggle: Option<Handler>,
    /// Input handler (text fields).
    pub on_input: Option<Handler>,
    /// Submit handler.
    pub on_submit: Option<Handler>,
    /// Children.
    pub children: Vec<Widget>,
    /// Assigned frame after layout.
    pub frame: Rect,
    /// Font scale (1 = 8px, 2 = 16px body).
    pub font_scale: u32,
}

impl Widget {
    /// Construct a column/row/surface with defaults.
    pub fn new(kind: WidgetKind, id: u64) -> Self {
        Self {
            id,
            kind,
            label: String::new(),
            text: String::new(),
            placeholder: String::new(),
            grow: 0.0,
            width: None,
            height: None,
            min_width: None,
            padding: Edges::default(),
            gap: 0.0,
            main: Align::Start,
            cross: Align::Stretch,
            background: None,
            foreground: None,
            border: None,
            radius: 0.0,
            variant: String::new(),
            size: "default".to_string(),
            selected: false,
            disabled: false,
            value: 0.0,
            wrap: false,
            text_align: Align::Start,
            on_press: None,
            on_toggle: None,
            on_input: None,
            on_submit: None,
            children: Vec::new(),
            frame: Rect::default(),
            font_scale: 2,
        }
    }

    /// True if this widget claims pointer presses.
    pub fn is_hit_target(&self) -> bool {
        self.on_press.is_some()
            || self.on_toggle.is_some()
            || self.on_input.is_some()
            || matches!(
                self.kind,
                WidgetKind::Button
                    | WidgetKind::Checkbox
                    | WidgetKind::Radio
                    | WidgetKind::TextField
                    | WidgetKind::Slider
            )
    }
}
