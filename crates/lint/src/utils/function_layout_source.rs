extern crate rustc_lexer;
extern crate rustc_lint;
extern crate rustc_span;

use rustc_lexer::{FrontmatterAllowed, TokenKind, tokenize};
use rustc_lint::{LateContext, LintContext};
use rustc_span::{BytePos, Span};

// -----------------------------------------------------------------------------
// FunctionLayoutSource: Authored comment extraction
// -----------------------------------------------------------------------------

/// Authored ordinary line comment with source position metadata.
pub(super) struct Comment {
    /// Exact source range of the comment token.
    pub(super) span: Span,
    /// One-based physical source line containing the comment.
    pub(super) line: usize,
    /// Complete comment token text, including its marker.
    pub(super) text: String,
}

/// Maps a byte position to its one-based physical source line.
pub(super) fn line(cx: &LateContext<'_>, position: BytePos) -> usize {
    cx.sess().source_map().lookup_char_pos(position).line
}

/// Converts lexer offsets into a source-mapped ordinary line comment.
fn comment(cx: &LateContext<'_>, source: &str, span: Span, start: usize, end: usize) -> Comment {
    // Locate the authored comment within the enclosing gap span.
    let lo = u32::try_from(start).expect("comment offset should fit BytePos");
    let hi = u32::try_from(end).expect("comment offset should fit BytePos");
    let comment_span = span
        .with_lo(span.lo() + BytePos(lo))
        .with_hi(span.lo() + BytePos(hi));

    // Preserve the source text and line needed to group adjacent comments later.
    Comment {
        span: comment_span,
        line: line(cx, comment_span.lo()),
        text: source[start..end].to_owned(),
    }
}

/// Lexes all authored ordinary line comments from a source gap.
pub(super) fn comments(cx: &LateContext<'_>, span: Span) -> Vec<Comment> {
    if span.is_empty() || span.from_expansion() {
        return Vec::new();
    }
    let Ok(source) = cx.sess().source_map().span_to_snippet(span) else {
        return Vec::new();
    };

    // Retain only authored ordinary line comments and their physical positions.
    let mut comments = Vec::new();
    let mut offset = 0_usize;
    for token in tokenize(&source, FrontmatterAllowed::No) {
        let length = usize::try_from(token.len).expect("token length should fit usize");
        let end = offset + length;
        if matches!(token.kind, TokenKind::LineComment { doc_style: None }) {
            comments.push(comment(cx, &source, span, offset, end));
        }
        offset = end;
    }
    comments
}
