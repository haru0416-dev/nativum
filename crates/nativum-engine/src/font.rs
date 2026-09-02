//! Text measurement against the bundled 8×8 bitmap, scaled.

use crate::font_data::FONT8X8;

/// Glyph cell size before scaling.
pub const GLYPH_W: f32 = 8.0;
/// Glyph cell height before scaling.
pub const GLYPH_H: f32 = 8.0;

/// Map a Unicode scalar to a glyph index. Extra punctuation used by the demos
/// falls back to a nearby ASCII lookalike so layouts stay deterministic.
pub fn glyph_index(ch: char) -> usize {
    match ch {
        '×' => b'x' as usize,
        '÷' => b'/' as usize,
        '−' | '–' | '—' => b'-' as usize,
        '±' => b'+' as usize,
        '·' | '•' => b'.' as usize,
        '…' => b'.' as usize,
        '’' | '‘' | '‛' => b'\'' as usize,
        '“' | '”' => b'"' as usize,
        c if (c as u32) < 128 => c as usize,
        _ => b'?' as usize,
    }
}

/// Bitmap rows for `ch`.
pub fn glyph(ch: char) -> &'static [u8; 8] {
    &FONT8X8[glyph_index(ch)]
}

/// Advance width of a string at `scale` (integer pixel scale ≥ 1).
pub fn measure_width(text: &str, scale: u32) -> f32 {
    let s = scale.max(1) as f32;
    text.chars().count() as f32 * GLYPH_W * s
}

/// Line height at `scale`.
pub fn measure_height(scale: u32) -> f32 {
    let s = scale.max(1) as f32;
    (GLYPH_H + 2.0) * s
}

/// Greedy word wrap. Returns lines and the bounding size.
pub fn wrap(text: &str, scale: u32, max_width: f32) -> (Vec<String>, f32, f32) {
    let s = scale.max(1) as f32;
    let cell = GLYPH_W * s;
    if max_width <= cell || text.is_empty() {
        let lines = if text.is_empty() {
            Vec::new()
        } else {
            vec![text.to_string()]
        };
        let w = measure_width(text, scale);
        let h = if lines.is_empty() {
            0.0
        } else {
            measure_height(scale)
        };
        return (lines, w, h);
    }
    let max_chars = (max_width / cell).floor().max(1.0) as usize;
    let mut lines = Vec::new();
    let mut current = String::new();
    for word in text.split_whitespace() {
        if current.is_empty() {
            push_broken(word, max_chars, &mut lines, &mut current);
            continue;
        }
        if current.chars().count() + 1 + word.chars().count() <= max_chars {
            current.push(' ');
            current.push_str(word);
        } else {
            lines.push(std::mem::take(&mut current));
            push_broken(word, max_chars, &mut lines, &mut current);
        }
    }
    if !current.is_empty() {
        lines.push(current);
    }
    let w = lines
        .iter()
        .map(|l| measure_width(l, scale))
        .fold(0.0_f32, f32::max)
        .min(max_width);
    let h = lines.len() as f32 * measure_height(scale);
    (lines, w, h)
}

fn push_broken(word: &str, max_chars: usize, lines: &mut Vec<String>, current: &mut String) {
    if word.chars().count() <= max_chars {
        *current = word.to_string();
        return;
    }
    let mut buf = String::new();
    for ch in word.chars() {
        if buf.chars().count() >= max_chars {
            lines.push(std::mem::take(&mut buf));
        }
        buf.push(ch);
    }
    *current = buf;
}
