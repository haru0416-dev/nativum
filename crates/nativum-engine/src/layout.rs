//! Flex layout. Column/row, grow, gap, padding, main/cross alignment.

use nativum_core::{Rect, Size};

use crate::font;
use crate::widget::{Align, Widget, WidgetKind};

/// Layout `root` into `viewport`.
pub fn layout(root: &mut Widget, viewport: Size) {
    let w = root.width.unwrap_or(viewport.width);
    let h = root.height.unwrap_or(viewport.height);
    layout_widget(root, Rect::new(0.0, 0.0, w, h));
}

fn layout_widget(w: &mut Widget, allotted: Rect) {
    let size = intrinsic(w, allotted.size());
    let frame = Rect::new(allotted.x, allotted.y, size.width, size.height);
    w.frame = match (w.width, w.height) {
        (Some(wd), Some(ht)) => Rect::new(allotted.x, allotted.y, wd, ht),
        (Some(wd), None) => Rect::new(allotted.x, allotted.y, wd, size.height),
        (None, Some(ht)) => Rect::new(allotted.x, allotted.y, size.width, ht),
        (None, None) => {
            // Fill the allotted box for stretching containers and grow children.
            if matches!(
                w.kind,
                WidgetKind::Row
                    | WidgetKind::Column
                    | WidgetKind::Stack
                    | WidgetKind::Surface
                    | WidgetKind::Scroll
            ) {
                allotted
            } else {
                frame
            }
        }
    };
    if let Some(wd) = w.width {
        w.frame.width = wd;
    }
    if let Some(ht) = w.height {
        w.frame.height = ht;
    }
    if let Some(min) = w.min_width {
        w.frame.width = w.frame.width.max(min);
    }
    place_children(w);
}

fn intrinsic(w: &Widget, allotted: Size) -> Size {
    if let (Some(wd), Some(ht)) = (w.width, w.height) {
        return Size::new(wd, ht);
    }
    let inner_max = Size::new(
        (allotted.width - w.padding.horizontal()).max(0.0),
        (allotted.height - w.padding.vertical()).max(0.0),
    );
    let content = match w.kind {
        WidgetKind::Text | WidgetKind::Badge | WidgetKind::StatusBar | WidgetKind::Alert => {
            text_size(w, inner_max.width)
        }
        WidgetKind::Button => {
            let t = text_size(w, inner_max.width);
            let pad = button_padding(w);
            Size::new(t.width + pad.horizontal(), t.height + pad.vertical()).max(button_min(w))
        }
        WidgetKind::Checkbox | WidgetKind::Radio => {
            let t = text_size(w, inner_max.width);
            Size::new(18.0 + 8.0 + t.width, t.height.max(18.0))
        }
        WidgetKind::TextField => Size::new(inner_max.width.max(80.0), 36.0),
        WidgetKind::Slider | WidgetKind::Progress => Size::new(inner_max.width.max(80.0), 16.0),
        WidgetKind::Separator => {
            if matches_parent_row(w) {
                Size::new(1.0, inner_max.height.max(8.0))
            } else {
                Size::new(inner_max.width.max(1.0), 1.0)
            }
        }
        WidgetKind::Spacer => Size::new(w.width.unwrap_or(0.0), w.height.unwrap_or(0.0)),
        WidgetKind::Row => measure_row(w, inner_max),
        WidgetKind::Column | WidgetKind::Scroll | WidgetKind::Surface => {
            measure_column(w, inner_max)
        }
        WidgetKind::Stack => measure_stack(w, inner_max),
        WidgetKind::Fragment => measure_column(w, inner_max),
    };
    let mut size = Size::new(
        content.width + w.padding.horizontal(),
        content.height + w.padding.vertical(),
    );
    if let Some(wd) = w.width {
        size.width = wd;
    }
    if let Some(ht) = w.height {
        size.height = ht;
    }
    if let Some(min) = w.min_width {
        size.width = size.width.max(min);
    }
    size
}

fn matches_parent_row(_w: &Widget) -> bool {
    false
}

fn text_size(w: &Widget, max_width: f32) -> Size {
    if w.wrap {
        let (_, width, height) = font::wrap(&w.text, w.font_scale, max_width.max(8.0));
        Size::new(width, height.max(font::measure_height(w.font_scale)))
    } else {
        Size::new(
            font::measure_width(&w.text, w.font_scale),
            font::measure_height(w.font_scale),
        )
    }
}

fn button_padding(w: &Widget) -> nativum_core::Edges {
    match w.size.as_str() {
        "sm" => nativum_core::Edges::vh(6.0, 10.0),
        "lg" => nativum_core::Edges::vh(12.0, 18.0),
        "icon" => nativum_core::Edges::all(8.0),
        _ => nativum_core::Edges::vh(8.0, 14.0),
    }
}

fn button_min(w: &Widget) -> Size {
    match w.size.as_str() {
        "sm" => Size::new(28.0, 28.0),
        "lg" => Size::new(44.0, 44.0),
        "icon" => Size::new(32.0, 32.0),
        _ => Size::new(36.0, 36.0),
    }
}

fn measure_row(w: &Widget, inner: Size) -> Size {
    let mut width = 0.0_f32;
    let mut height = 0.0_f32;
    for (i, child) in w.children.iter().enumerate() {
        if i > 0 {
            width += w.gap;
        }
        let s = intrinsic(child, Size::new(f32::INFINITY, inner.height));
        width += s.width;
        height = height.max(s.height);
    }
    Size::new(width, height)
}

fn measure_column(w: &Widget, inner: Size) -> Size {
    let mut width = 0.0_f32;
    let mut height = 0.0_f32;
    let child_max = Size::new(inner.width, f32::INFINITY);
    for (i, child) in w.children.iter().enumerate() {
        if i > 0 {
            height += w.gap;
        }
        let s = intrinsic(child, child_max);
        height += s.height;
        width = width.max(s.width);
    }
    Size::new(width.min(inner.width.max(width)), height)
}

fn measure_stack(w: &Widget, inner: Size) -> Size {
    let mut size = Size::default();
    for child in &w.children {
        size = size.max(intrinsic(child, inner));
    }
    size
}

fn place_children(w: &mut Widget) {
    let content = w.frame.inset(w.padding);
    match w.kind {
        WidgetKind::Row => place_flex(w, content, true),
        WidgetKind::Column | WidgetKind::Scroll | WidgetKind::Surface => {
            place_flex(w, content, false)
        }
        WidgetKind::Stack => {
            let kids = std::mem::take(&mut w.children);
            let mut laid = Vec::with_capacity(kids.len());
            for mut child in kids {
                layout_widget(&mut child, content);
                laid.push(child);
            }
            w.children = laid;
        }
        _ => {}
    }
}

fn place_flex(w: &mut Widget, content: Rect, horizontal: bool) {
    let n = w.children.len();
    if n == 0 {
        return;
    }
    let main_total = if horizontal {
        content.width
    } else {
        content.height
    };
    let cross_total = if horizontal {
        content.height
    } else {
        content.width
    };
    let gap_sum = w.gap * (n.saturating_sub(1) as f32);
    let mut sizes: Vec<Size> = Vec::with_capacity(n);
    let mut grow_sum = 0.0_f32;
    let allotted_for_measure = if horizontal {
        Size::new(f32::INFINITY, content.height)
    } else {
        Size::new(content.width, f32::INFINITY)
    };
    for child in &w.children {
        sizes.push(intrinsic(child, allotted_for_measure));
        grow_sum += child.grow.max(0.0);
    }
    let used: f32 = sizes
        .iter()
        .map(|s| if horizontal { s.width } else { s.height })
        .sum::<f32>()
        + gap_sum;
    let extra = (main_total - used).max(0.0);
    if grow_sum > 0.0 {
        for (i, child) in w.children.iter().enumerate() {
            if child.grow > 0.0 {
                let add = extra * (child.grow / grow_sum);
                if horizontal {
                    sizes[i].width += add;
                } else {
                    sizes[i].height += add;
                }
            }
        }
    }
    let packed: f32 = sizes
        .iter()
        .map(|s| if horizontal { s.width } else { s.height })
        .sum::<f32>()
        + gap_sum;
    let free = (main_total - packed).max(0.0);
    let mut cursor = if horizontal { content.x } else { content.y };
    cursor += match w.main {
        Align::Center => free / 2.0,
        Align::End => free,
        _ => 0.0,
    };
    let kids = std::mem::take(&mut w.children);
    let mut laid = Vec::with_capacity(kids.len());
    for (i, mut child) in kids.into_iter().enumerate() {
        let size = sizes[i];
        let cross = if horizontal { size.height } else { size.width };
        let leading = match w.cross {
            Align::Center => (cross_total - cross) / 2.0,
            Align::End => cross_total - cross,
            Align::Start | Align::Stretch => 0.0,
        };
        let (x, y, ww, hh) = if horizontal {
            let hh = if w.cross == Align::Stretch {
                cross_total
            } else {
                size.height
            };
            (cursor, content.y + leading.max(0.0), size.width, hh)
        } else {
            let ww = if w.cross == Align::Stretch {
                cross_total
            } else {
                size.width
            };
            (content.x + leading.max(0.0), cursor, ww, size.height)
        };
        layout_widget(&mut child, Rect::new(x, y, ww, hh));
        // Force the allocated box so grow/stretch actually fill.
        child.frame = Rect::new(x, y, ww, hh);
        place_children(&mut child);
        cursor += if horizontal { ww } else { hh };
        cursor += w.gap;
        laid.push(child);
    }
    w.children = laid;
}

/// Walk all widgets.
/// Walk all widgets mutably.
#[allow(dead_code)]
pub fn visit_mut<F: FnMut(&mut Widget)>(w: &mut Widget, f: &mut F) {
    f(w);
    for c in &mut w.children {
        visit_mut(c, f);
    }
}

#[allow(dead_code)]
pub fn visit<F: FnMut(&Widget)>(w: &Widget, f: &mut F) {
    f(w);
    for c in &w.children {
        visit(c, f);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::widget::WidgetKind;

    #[test]
    fn row_centers_two_buttons() {
        let mut root = Widget::new(WidgetKind::Row, 1);
        root.main = Align::Center;
        root.cross = Align::Center;
        root.gap = 8.0;
        let mut a = Widget::new(WidgetKind::Button, 2);
        a.text = "-".to_string();
        let mut b = Widget::new(WidgetKind::Button, 3);
        b.text = "+".to_string();
        root.children = vec![a, b];
        layout(&mut root, Size::new(200.0, 80.0));
        assert!(root.children[0].frame.x > 0.0);
        assert!(root.children[1].frame.x > root.children[0].frame.x);
    }
}
