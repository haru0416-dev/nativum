//! Byte offsets with converted line/column for diagnostics.

/// A half-open byte range into the source, plus a start line/column.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Span {
    /// Byte offset of the start.
    pub start: usize,
    /// Byte offset of the end.
    pub end: usize,
    /// 1-based line of `start`.
    pub line: usize,
    /// 1-based column of `start` (bytes, not graphemes).
    pub column: usize,
}

impl Span {
    /// Dummy span for generated nodes.
    pub fn dummy() -> Self {
        Self {
            start: 0,
            end: 0,
            line: 1,
            column: 1,
        }
    }

    /// Point span at `offset` given the full source (used by the parser).
    pub fn at(source: &str, offset: usize) -> Self {
        let (line, column) = line_col(source, offset);
        Self {
            start: offset,
            end: offset,
            line,
            column,
        }
    }
}

/// Convert a byte offset into 1-based line/column.
pub fn line_col(source: &str, offset: usize) -> (usize, usize) {
    let offset = offset.min(source.len());
    let mut line = 1usize;
    let mut col = 1usize;
    for (i, ch) in source.char_indices() {
        if i >= offset {
            break;
        }
        if ch == '\n' {
            line += 1;
            col = 1;
        } else {
            col += ch.len_utf8();
        }
    }
    (line, col)
}
