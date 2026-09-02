//! CPU rasterizer: rounded rects, text, controls. Deterministic.

use nativum_core::{Color, Point, Rect, Size, TokenSet};

use crate::font;
use crate::widget::{Align, Widget, WidgetKind};

/// Packed RGBA framebuffer.
#[derive(Clone, Debug)]
pub struct Surface {
    /// Width in pixels.
    pub width: u32,
    /// Height in pixels.
    pub height: u32,
    /// Row-major RGBA.
    pub pixels: Vec<u8>,
}

impl Surface {
    /// Opaque fill.
    pub fn new(size: Size, color: Color) -> Self {
        let width = size.width.max(1.0).round() as u32;
        let height = size.height.max(1.0).round() as u32;
        let mut pixels = vec![0u8; (width * height * 4) as usize];
        for px in pixels.chunks_exact_mut(4) {
            px[0] = color.r;
            px[1] = color.g;
            px[2] = color.b;
            px[3] = 255;
        }
        Self {
            width,
            height,
            pixels,
        }
    }

    fn idx(&self, x: i32, y: i32) -> Option<usize> {
        if x < 0 || y < 0 {
            return None;
        }
        let x = x as u32;
        let y = y as u32;
        if x >= self.width || y >= self.height {
            return None;
        }
        Some(((y * self.width + x) * 4) as usize)
    }

    fn put(&mut self, x: i32, y: i32, color: Color, coverage: f32) {
        let Some(i) = self.idx(x, y) else { return };
        let dst = Color {
            r: self.pixels[i],
            g: self.pixels[i + 1],
            b: self.pixels[i + 2],
            a: self.pixels[i + 3],
        };
        let out = color.blend_over(dst, coverage);
        self.pixels[i] = out.r;
        self.pixels[i + 1] = out.g;
        self.pixels[i + 2] = out.b;
        self.pixels[i + 3] = out.a;
    }

    /// Fill a rectangle, clipped to `clip`.
    pub fn fill_rect(&mut self, rect: Rect, color: Color, clip: Rect) {
        self.fill_rounded(rect, 0.0, color, clip);
    }

    /// Fill a rounded rectangle using a cheap distance field.
    pub fn fill_rounded(&mut self, rect: Rect, radius: f32, color: Color, clip: Rect) {
        let r = rect.intersect(clip);
        if r.is_empty() {
            return;
        }
        let rad = radius.min(rect.width / 2.0).min(rect.height / 2.0).max(0.0);
        let x0 = r.x.floor() as i32;
        let y0 = r.y.floor() as i32;
        let x1 = r.right().ceil() as i32;
        let y1 = r.bottom().ceil() as i32;
        for y in y0..y1 {
            for x in x0..x1 {
                let cx = x as f32 + 0.5;
                let cy = y as f32 + 0.5;
                let cov = coverage_rounded(cx, cy, rect, rad);
                if cov > 0.0 {
                    self.put(x, y, color, cov);
                }
            }
        }
    }

    /// 1px border following the rounded rect edge.
    pub fn stroke_rounded(&mut self, rect: Rect, radius: f32, color: Color, clip: Rect) {
        if rect.is_empty() {
            return;
        }
        let inner = Rect::new(
            rect.x + 1.0,
            rect.y + 1.0,
            (rect.width - 2.0).max(0.0),
            (rect.height - 2.0).max(0.0),
        );
        let r = rect.intersect(clip);
        let rad = radius.min(rect.width / 2.0).min(rect.height / 2.0).max(0.0);
        let x0 = r.x.floor() as i32;
        let y0 = r.y.floor() as i32;
        let x1 = r.right().ceil() as i32;
        let y1 = r.bottom().ceil() as i32;
        for y in y0..y1 {
            for x in x0..x1 {
                let cx = x as f32 + 0.5;
                let cy = y as f32 + 0.5;
                let outer = coverage_rounded(cx, cy, rect, rad);
                let inner_c = coverage_rounded(cx, cy, inner, (rad - 1.0).max(0.0));
                let cov = (outer - inner_c).clamp(0.0, 1.0);
                if cov > 0.0 {
                    self.put(x, y, color, cov);
                }
            }
        }
    }

    /// Draw bitmap text. `align` is horizontal inside `rect`.
    pub fn draw_text(
        &mut self,
        text: &str,
        rect: Rect,
        scale: u32,
        color: Color,
        align: Align,
        clip: Rect,
    ) {
        if text.is_empty() {
            return;
        }
        let scale = scale.max(1);
        let (lines, _, _) = if rect.width > 8.0 {
            font::wrap(text, scale, rect.width)
        } else {
            (vec![text.to_string()], 0.0, 0.0)
        };
        let line_h = font::measure_height(scale);
        let mut y = rect.y + ((rect.height - line_h * lines.len() as f32) / 2.0).max(0.0);
        for line in &lines {
            let w = font::measure_width(line, scale);
            let x = match align {
                Align::Center => rect.x + (rect.width - w) / 2.0,
                Align::End => rect.x + rect.width - w,
                _ => rect.x,
            };
            self.blit_line(line, Point::new(x, y), scale, color, clip);
            y += line_h;
        }
    }

    fn blit_line(&mut self, text: &str, origin: Point, scale: u32, color: Color, clip: Rect) {
        let s = scale as i32;
        let mut x = origin.x.round() as i32;
        let y = origin.y.round() as i32;
        for ch in text.chars() {
            let g = font::glyph(ch);
            for (row, bits) in g.iter().enumerate() {
                for col in 0..8 {
                    if (bits & (1 << col)) != 0 {
                        for dy in 0..s {
                            for dx in 0..s {
                                let px = x + col * s + dx;
                                let py = y + row as i32 * s + dy;
                                if clip.contains(Point::new(px as f32, py as f32)) {
                                    self.put(px, py, color, 1.0);
                                }
                            }
                        }
                    }
                }
            }
            x += 8 * s;
        }
    }
}

fn coverage_rounded(px: f32, py: f32, rect: Rect, radius: f32) -> f32 {
    if rect.is_empty() {
        return 0.0;
    }
    if radius <= 0.5 {
        if px >= rect.x && px < rect.right() && py >= rect.y && py < rect.bottom() {
            return 1.0;
        }
        return 0.0;
    }
    let cx = rect.x + radius;
    let cy = rect.y + radius;
    let c2x = rect.right() - radius;
    let c2y = rect.bottom() - radius;
    let qx = px.clamp(cx, c2x);
    let qy = py.clamp(cy, c2y);
    // Inside the inner box (not in a corner arc):
    if px >= cx && px <= c2x && py >= cy && py <= c2y {
        return 1.0;
    }
    if px >= cx && px <= c2x && py >= rect.y && py < rect.bottom() {
        return 1.0;
    }
    if py >= cy && py <= c2y && px >= rect.x && px < rect.right() {
        return 1.0;
    }
    // Corner: distance to nearest corner center.
    let dx = px - qx;
    let dy = py - qy;
    let dist = (dx * dx + dy * dy).sqrt();
    (radius - dist + 0.5).clamp(0.0, 1.0)
}

/// Paint the widget tree into a new surface.
pub fn paint(root: &Widget, tokens: &TokenSet, focused: Option<u64>) -> Surface {
    let mut surface = Surface::new(root.frame.size(), tokens.background);
    paint_widget(&mut surface, root, tokens, focused, root.frame);
    surface
}

fn paint_widget(
    surface: &mut Surface,
    w: &Widget,
    tokens: &TokenSet,
    focused: Option<u64>,
    clip: Rect,
) {
    let clip = clip.intersect(w.frame);
    if clip.is_empty() {
        return;
    }
    match w.kind {
        WidgetKind::Button => paint_button(surface, w, tokens, focused, clip),
        WidgetKind::Checkbox => paint_checkbox(surface, w, tokens, focused, clip, false),
        WidgetKind::Radio => paint_checkbox(surface, w, tokens, focused, clip, true),
        WidgetKind::TextField => paint_field(surface, w, tokens, focused, clip),
        WidgetKind::Separator => {
            surface.fill_rect(w.frame, w.background.unwrap_or(tokens.border), clip);
        }
        WidgetKind::Badge => {
            let bg = w.background.unwrap_or(tokens.surface_subtle);
            surface.fill_rounded(w.frame, w.radius.max(8.0), bg, clip);
            let fg = w.foreground.unwrap_or(tokens.text_muted);
            surface.draw_text(
                &w.text,
                w.frame.inset(nativum_core::Edges::vh(2.0, 6.0)),
                w.font_scale,
                fg,
                Align::Center,
                clip,
            );
        }
        WidgetKind::StatusBar => {
            let bg = w.background.unwrap_or(tokens.surface);
            surface.fill_rect(w.frame, bg, clip);
            surface.fill_rect(
                Rect::new(w.frame.x, w.frame.y, w.frame.width, 1.0),
                tokens.border,
                clip,
            );
            let fg = w.foreground.unwrap_or(tokens.text_muted);
            surface.draw_text(
                &w.text,
                w.frame.inset(nativum_core::Edges::vh(0.0, 12.0)),
                w.font_scale,
                fg,
                Align::Start,
                clip,
            );
        }
        WidgetKind::Alert => {
            let bg = w.background.unwrap_or(tokens.surface_subtle);
            surface.fill_rounded(w.frame, w.radius.max(8.0), bg, clip);
            let fg = w.foreground.unwrap_or(tokens.text_muted);
            surface.draw_text(
                &w.text,
                w.frame.inset(nativum_core::Edges::all(12.0)),
                w.font_scale,
                fg,
                Align::Center,
                clip,
            );
        }
        WidgetKind::Slider | WidgetKind::Progress => {
            let track = Rect::new(
                w.frame.x,
                w.frame.y + (w.frame.height - 6.0) / 2.0,
                w.frame.width,
                6.0,
            );
            surface.fill_rounded(track, 3.0, tokens.surface_subtle, clip);
            let fill_w = (track.width * w.value.clamp(0.0, 1.0)).max(0.0);
            surface.fill_rounded(
                Rect::new(track.x, track.y, fill_w, track.height),
                3.0,
                tokens.accent,
                clip,
            );
        }
        WidgetKind::Text => {
            if let Some(bg) = w.background {
                surface.fill_rounded(w.frame, w.radius, bg, clip);
            }
            let fg = w.foreground.unwrap_or(tokens.text);
            surface.draw_text(&w.text, w.frame, w.font_scale, fg, w.text_align, clip);
        }
        _ => {
            if let Some(bg) = w.background {
                surface.fill_rounded(w.frame, w.radius, bg, clip);
            }
            if let Some(border) = w.border {
                surface.stroke_rounded(w.frame, w.radius, border, clip);
            }
        }
    }
    let child_clip = if w.kind == WidgetKind::Scroll {
        clip.intersect(w.frame)
    } else {
        clip
    };
    for child in &w.children {
        paint_widget(surface, child, tokens, focused, child_clip);
    }
    if focused == Some(w.id) && w.is_hit_target() {
        let ring = Rect::new(
            w.frame.x - 2.0,
            w.frame.y - 2.0,
            w.frame.width + 4.0,
            w.frame.height + 4.0,
        );
        surface.stroke_rounded(ring, (w.radius + 2.0).max(4.0), tokens.focus_ring, clip);
    }
}

fn paint_button(
    surface: &mut Surface,
    w: &Widget,
    tokens: &TokenSet,
    _focused: Option<u64>,
    clip: Rect,
) {
    let (bg, fg, border) = if w.disabled {
        (tokens.surface_subtle, tokens.disabled, tokens.border)
    } else if w.variant == "primary" || w.selected {
        (tokens.accent, tokens.accent_text, tokens.accent)
    } else if w.variant == "secondary" {
        (tokens.surface_subtle, tokens.text, tokens.border)
    } else {
        (
            w.background.unwrap_or(tokens.surface),
            tokens.text,
            tokens.border,
        )
    };
    let radius = if w.radius > 0.0 { w.radius } else { 8.0 };
    surface.fill_rounded(w.frame, radius, bg, clip);
    surface.stroke_rounded(w.frame, radius, border, clip);
    let fg = w.foreground.unwrap_or(fg);
    surface.draw_text(&w.text, w.frame, w.font_scale, fg, Align::Center, clip);
}

fn paint_checkbox(
    surface: &mut Surface,
    w: &Widget,
    tokens: &TokenSet,
    _focused: Option<u64>,
    clip: Rect,
    radio: bool,
) {
    let box_r = Rect::new(
        w.frame.x,
        w.frame.y + (w.frame.height - 18.0) / 2.0,
        18.0,
        18.0,
    );
    let radius = if radio { 9.0 } else { 4.0 };
    surface.fill_rounded(box_r, radius, tokens.surface, clip);
    surface.stroke_rounded(box_r, radius, tokens.border, clip);
    if w.selected {
        let inner = box_r.inset(nativum_core::Edges::all(if radio { 5.0 } else { 4.0 }));
        surface.fill_rounded(inner, if radio { 4.0 } else { 2.0 }, tokens.accent, clip);
    }
    let text_r = Rect::new(
        box_r.right() + 8.0,
        w.frame.y,
        (w.frame.right() - box_r.right() - 8.0).max(0.0),
        w.frame.height,
    );
    surface.draw_text(
        &w.text,
        text_r,
        w.font_scale,
        tokens.text,
        Align::Start,
        clip,
    );
}

fn paint_field(
    surface: &mut Surface,
    w: &Widget,
    tokens: &TokenSet,
    focused: Option<u64>,
    clip: Rect,
) {
    let radius = if w.radius > 0.0 { w.radius } else { 8.0 };
    surface.fill_rounded(w.frame, radius, tokens.surface, clip);
    let border = if focused == Some(w.id) {
        tokens.accent
    } else {
        tokens.border
    };
    surface.stroke_rounded(w.frame, radius, border, clip);
    let inner = w.frame.inset(nativum_core::Edges::vh(0.0, 10.0));
    if w.text.is_empty() {
        surface.draw_text(
            &w.placeholder,
            inner,
            w.font_scale,
            tokens.text_muted,
            Align::Start,
            clip,
        );
    } else {
        surface.draw_text(
            &w.text,
            inner,
            w.font_scale,
            tokens.text,
            Align::Start,
            clip,
        );
    }
}
