//! Hit-testing: deepest press-claiming widget under a point.

use nativum_core::Point;

use crate::widget::Widget;

/// Find the innermost hit target containing `p`.
pub fn hit_test(root: &Widget, p: Point) -> Option<&Widget> {
    hit_rec(root, p)
}

fn hit_rec(w: &Widget, p: Point) -> Option<&Widget> {
    if !w.frame.contains(p) {
        return None;
    }
    for child in w.children.iter().rev() {
        if let Some(h) = hit_rec(child, p) {
            return Some(h);
        }
    }
    if w.is_hit_target() && !w.disabled {
        Some(w)
    } else {
        None
    }
}

/// Find the first widget whose press/toggle kind equals `kind`.
pub fn find_handler<'a>(root: &'a Widget, kind: &str) -> Option<&'a Widget> {
    if w_kind(root, kind) {
        return Some(root);
    }
    for child in &root.children {
        if let Some(w) = find_handler(child, kind) {
            return Some(w);
        }
    }
    None
}

fn w_kind(w: &Widget, kind: &str) -> bool {
    w.on_press.as_ref().is_some_and(|h| h.kind == kind)
        || w.on_toggle.as_ref().is_some_and(|h| h.kind == kind)
        || w.on_submit.as_ref().is_some_and(|h| h.kind == kind)
        || w.on_input.as_ref().is_some_and(|h| h.kind == kind)
}

/// First text field in tree order.
pub fn first_field(root: &Widget) -> Option<&Widget> {
    if matches!(root.kind, crate::widget::WidgetKind::TextField) {
        return Some(root);
    }
    for child in &root.children {
        if let Some(w) = first_field(child) {
            return Some(w);
        }
    }
    None
}

/// Mutable first text field.
#[allow(dead_code)]
pub fn first_field_mut(root: &mut Widget) -> Option<&mut Widget> {
    if matches!(root.kind, crate::widget::WidgetKind::TextField) {
        return Some(root);
    }
    for child in &mut root.children {
        if let Some(w) = first_field_mut(child) {
            return Some(w);
        }
    }
    None
}
