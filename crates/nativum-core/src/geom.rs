//! Axis-aligned geometry in CSS-style logical pixels.

use serde::{Deserialize, Serialize};

/// A point in view space. Origin is the top-left of the window.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Point {
    /// Horizontal offset from the left edge.
    pub x: f32,
    /// Vertical offset from the top edge.
    pub y: f32,
}

impl Point {
    /// Construct a point.
    pub fn new(x: f32, y: f32) -> Self {
        Self { x, y }
    }
}

/// Width and height.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Size {
    /// Extent on the x axis.
    pub width: f32,
    /// Extent on the y axis.
    pub height: f32,
}

impl Size {
    /// Construct a size.
    pub fn new(width: f32, height: f32) -> Self {
        Self { width, height }
    }

    /// Component-wise maximum.
    pub fn max(self, other: Self) -> Self {
        Self {
            width: self.width.max(other.width),
            height: self.height.max(other.height),
        }
    }
}

/// Inclusive-origin rectangle `[x, x+width) × [y, y+height)`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Rect {
    /// Left edge.
    pub x: f32,
    /// Top edge.
    pub y: f32,
    /// Width.
    pub width: f32,
    /// Height.
    pub height: f32,
}

impl Rect {
    /// Construct a rect from origin and size.
    pub fn new(x: f32, y: f32, width: f32, height: f32) -> Self {
        Self {
            x,
            y,
            width,
            height,
        }
    }

    /// Origin.
    pub fn origin(self) -> Point {
        Point::new(self.x, self.y)
    }

    /// Size.
    pub fn size(self) -> Size {
        Size::new(self.width, self.height)
    }

    /// Right edge.
    pub fn right(self) -> f32 {
        self.x + self.width
    }

    /// Bottom edge.
    pub fn bottom(self) -> f32 {
        self.y + self.height
    }

    /// Whether `p` is inside the half-open rectangle.
    pub fn contains(self, p: Point) -> bool {
        p.x >= self.x && p.y >= self.y && p.x < self.right() && p.y < self.bottom()
    }

    /// Intersection. Empty if the rectangles do not overlap.
    pub fn intersect(self, other: Self) -> Self {
        let x = self.x.max(other.x);
        let y = self.y.max(other.y);
        let r = self.right().min(other.right());
        let b = self.bottom().min(other.bottom());
        Self {
            x,
            y,
            width: (r - x).max(0.0),
            height: (b - y).max(0.0),
        }
    }

    /// Inset by `edges`. Clamped so size never goes negative.
    pub fn inset(self, edges: Edges) -> Self {
        let x = self.x + edges.left;
        let y = self.y + edges.top;
        let width = (self.width - edges.left - edges.right).max(0.0);
        let height = (self.height - edges.top - edges.bottom).max(0.0);
        Self {
            x,
            y,
            width,
            height,
        }
    }

    /// True when either axis has no area.
    pub fn is_empty(self) -> bool {
        self.width <= 0.0 || self.height <= 0.0
    }
}

/// Four-sided spacing.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Edges {
    /// Top inset.
    pub top: f32,
    /// Right inset.
    pub right: f32,
    /// Bottom inset.
    pub bottom: f32,
    /// Left inset.
    pub left: f32,
}

impl Edges {
    /// Uniform inset on every side.
    pub fn all(v: f32) -> Self {
        Self {
            top: v,
            right: v,
            bottom: v,
            left: v,
        }
    }

    /// Horizontal + vertical pair (CSS shorthand `padding="12 8"` → 12 vert, 8 horiz).
    pub fn vh(vertical: f32, horizontal: f32) -> Self {
        Self {
            top: vertical,
            right: horizontal,
            bottom: vertical,
            left: horizontal,
        }
    }

    /// Sum of left and right.
    pub fn horizontal(self) -> f32 {
        self.left + self.right
    }

    /// Sum of top and bottom.
    pub fn vertical(self) -> f32 {
        self.top + self.bottom
    }
}
