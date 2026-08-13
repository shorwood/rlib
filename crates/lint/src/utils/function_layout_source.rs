extern crate rustc_lexer;
extern crate rustc_lint;
extern crate rustc_span;

use rustc_lexer::{FrontmatterAllowed, TokenKind, tokenize};
use rustc_lint::{LateContext, LintContext};
use rustc_span::{BytePos, Span};

/// Authored ordinary line comment with source position metadata.
pub(super) struct Comment {
    /// Exact source range of the comment token.
    pub(super) span: Span,
    /// One-based physical source line containing the comment.
    pub(super) line: usize,
    /// Complete comment token text, including its marker.
    pub(super) text: String,
}

impl Comment {
    /// Converts lexer offsets into a source-mapped ordinary line comment.
    fn from_token(
        cx: &LateContext<'_>,
        source: &str,
        span: Span,
        start: usize,
        end: usize,
    ) -> Self {
        // Locate the authored comment within the enclosing gap span.
        let lo = u32::try_from(start).expect("comment offset should fit BytePos");
        let hi = u32::try_from(end).expect("comment offset should fit BytePos");
        let comment_span = span
            .with_lo(span.lo() + BytePos(lo))
            .with_hi(span.lo() + BytePos(hi));

        // Preserve the source text and line needed to group adjacent comments later.
        let source_map = cx.sess().source_map();
        let line = source_map.lookup_char_pos(comment_span.lo()).line;
        Self {
            span: comment_span,
            line,
            text: source[start..end].to_owned(),
        }
    }
}

/// Source-position queries colocated with compiler byte positions.
pub(super) trait SourcePositionExt {
    /// Maps this byte position to its one-based physical source line.
    fn source_line(self, cx: &LateContext<'_>) -> usize;
}

impl SourcePositionExt for BytePos {
    fn source_line(self, cx: &LateContext<'_>) -> usize {
        cx.sess().source_map().lookup_char_pos(self).line
    }
}

/// Authored-source queries colocated with compiler spans.
pub(super) trait SourceSpanExt {
    /// Lexes all authored ordinary line comments from this source gap.
    fn comments(self, cx: &LateContext<'_>) -> Vec<Comment>;
}

impl SourceSpanExt for Span {
    fn comments(self, cx: &LateContext<'_>) -> Vec<Comment> {
        if self.is_empty() || self.from_expansion() {
            return Vec::new();
        }
        let Ok(source) = cx.sess().source_map().span_to_snippet(self) else {
            return Vec::new();
        };

        // Retain only authored ordinary line comments and their physical positions.
        let mut comments = Vec::new();
        let mut offset = 0_usize;
        for token in tokenize(&source, FrontmatterAllowed::No) {
            let length = usize::try_from(token.len).expect("token length should fit usize");
            let end = offset + length;
            if matches!(token.kind, TokenKind::LineComment { doc_style: None }) {
                comments.push(Comment::from_token(cx, &source, self, offset, end));
            }
            offset = end;
        }
        comments
    }
}
