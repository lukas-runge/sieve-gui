//! Source positions and diagnostics.

use std::fmt;

use serde::Serialize;

/// A half-open byte range `start..end` into the source text.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize)]
pub struct Span {
    pub start: usize,
    pub end: usize,
}

impl Span {
    pub const fn new(start: usize, end: usize) -> Self {
        Self { start, end }
    }

    /// The smallest span covering both `self` and `other`.
    pub fn to(self, other: Span) -> Span {
        Span::new(self.start.min(other.start), self.end.max(other.end))
    }

    pub fn len(self) -> usize {
        self.end - self.start
    }

    pub fn is_empty(self) -> bool {
        self.start == self.end
    }
}

/// An error found while lexing or parsing a script.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ParseError {
    pub message: String,
    pub span: Span,
}

impl ParseError {
    pub(crate) fn new(message: impl Into<String>, span: Span) -> Self {
        Self { message: message.into(), span }
    }
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} at {}..{}", self.message, self.span.start, self.span.end)
    }
}

impl std::error::Error for ParseError {}

/// Maps byte offsets to line/column and UTF-16 positions (the latter are what
/// CodeMirror and other JavaScript editors use).
#[derive(Debug, Clone)]
pub struct LineIndex<'a> {
    src: &'a str,
    line_starts: Vec<usize>,
}

/// A 1-based line and column; the column counts Unicode scalar values.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct LineCol {
    pub line: usize,
    pub col: usize,
}

impl<'a> LineIndex<'a> {
    pub fn new(src: &'a str) -> Self {
        let line_starts = std::iter::once(0)
            .chain(src.match_indices('\n').map(|(i, _)| i + 1))
            .collect();
        Self { src, line_starts }
    }

    /// Line and column of a byte offset. Offsets past the end clamp to the end.
    pub fn line_col(&self, offset: usize) -> LineCol {
        let offset = self.clamp(offset);
        let line = self.line_starts.partition_point(|&s| s <= offset) - 1;
        let col = self.src[self.line_starts[line]..offset].chars().count() + 1;
        LineCol { line: line + 1, col }
    }

    /// Byte offset of the start of a 1-based line, if it exists.
    pub fn line_start(&self, line: usize) -> Option<usize> {
        self.line_starts.get(line.checked_sub(1)?).copied()
    }

    /// Converts a byte offset to a UTF-16 code unit offset.
    pub fn utf16_offset(&self, offset: usize) -> usize {
        let offset = self.clamp(offset);
        self.src[..offset].encode_utf16().count()
    }

    fn clamp(&self, offset: usize) -> usize {
        let mut offset = offset.min(self.src.len());
        while !self.src.is_char_boundary(offset) {
            offset -= 1;
        }
        offset
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn line_col_and_utf16() {
        let src = "ab\näö𝄞x\n";
        let idx = LineIndex::new(src);
        assert_eq!(idx.line_col(0), LineCol { line: 1, col: 1 });
        assert_eq!(idx.line_col(3), LineCol { line: 2, col: 1 });
        let x = src.find('x').unwrap();
        assert_eq!(idx.line_col(x), LineCol { line: 2, col: 4 });
        // "ab\n" = 3, "äö" = 2, "𝄞" = 2 UTF-16 units
        assert_eq!(idx.utf16_offset(x), 7);
        assert_eq!(idx.line_col(usize::MAX), LineCol { line: 3, col: 1 });
        assert_eq!(idx.line_start(2), Some(3));
        assert_eq!(idx.line_start(0), None);
    }
}
