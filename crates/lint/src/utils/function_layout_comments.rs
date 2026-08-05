extern crate rustc_lexer;
extern crate rustc_lint;
extern crate rustc_span;

use convert_case::{Case, Casing};
use rustc_lexer::{FrontmatterAllowed, TokenKind, tokenize};
use rustc_lint::{LateContext, LintContext};
use rustc_span::{BytePos, Span};
use std::ops::RangeInclusive;

use super::config::FunctionStructureConfig;
use super::identifier_case::sentence_case;

// -----------------------------------------------------------------------------
// FunctionLayout: Parse and validate explanatory comments
// -----------------------------------------------------------------------------

/// One function-layout problem found in authored source.
pub struct FunctionLayoutFinding {
    /// Source range occupied by the malformed comment or oversized phase.
    pub(crate) span: Span,
    /// Primary explanation of the layout problem.
    pub(crate) message: String,
    /// Guidance describing the configured canonical phase-comment form.
    pub(crate) help: String,
    /// Safe canonical replacement for a repairable comment header.
    pub(crate) replacement: Option<String>,
}

/// Authored ordinary line comment with source position metadata.
struct FunctionLayoutRawComment {
    /// Exact source range of the comment token.
    span: Span,
    /// One-based physical source line containing the comment.
    line: usize,
    /// Complete comment token text, including its marker.
    text: String,
}

/// Maps a byte position to its one-based physical source line.
fn function_layout_source_line(cx: &LateContext<'_>, position: BytePos) -> usize {
    cx.sess().source_map().lookup_char_pos(position).line
}

/// Converts lexer offsets into a source-mapped ordinary line comment.
fn function_layout_raw_comment(
    cx: &LateContext<'_>,
    source: &str,
    span: Span,
    start: usize,
    end: usize,
) -> FunctionLayoutRawComment {
    // Locate the authored comment within the enclosing gap span.
    let lo = u32::try_from(start).expect("comment offset should fit BytePos");
    let hi = u32::try_from(end).expect("comment offset should fit BytePos");
    let comment_span = span
        .with_lo(span.lo() + BytePos(lo))
        .with_hi(span.lo() + BytePos(hi));

    // Preserve the source text and line needed to group adjacent comments later.
    FunctionLayoutRawComment {
        span: comment_span,
        line: function_layout_source_line(cx, comment_span.lo()),
        text: source[start..end].to_owned(),
    }
}

/// Lexes all authored ordinary line comments from a source gap.
fn function_layout_raw_comments(cx: &LateContext<'_>, span: Span) -> Vec<FunctionLayoutRawComment> {
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
            comments.push(function_layout_raw_comment(cx, &source, span, offset, end));
        }
        offset = end;
    }
    comments
}

/// Accepts populated conventional labels whose uppercase spelling is intentional.
fn function_layout_is_known_label(content: &str) -> bool {
    ["SAFETY:", "TODO:", "FIXME:", "NOTE:"].iter().any(|label| {
        content
            .strip_prefix(label)
            .is_some_and(|rest| rest.starts_with(' ') && !rest.trim().is_empty())
    })
}

/// Detects multiword all-uppercase prose rather than identifiers or acronyms.
fn function_layout_is_shouting(content: &str, words: &[&str]) -> bool {
    words.len() > 1
        && content
            .chars()
            .any(|character| character.is_ascii_alphabetic())
        && !content
            .chars()
            .any(|character| character.is_ascii_lowercase())
}

/// Returns whether casing normalization could corrupt code or a proper technical term.
fn function_layout_is_protected_word(word: &str) -> bool {
    let bare = word.trim_matches(|character: char| !character.is_ascii_alphanumeric());
    word.contains('`')
        || bare.contains(['_', '-'])
        || matches!(bare, "Rust" | "Rustfix")
        || bare.chars().skip(1).any(char::is_uppercase)
}

/// Normalizes one prose word while preserving protected authored spelling.
fn function_layout_normalize_word(word: &str, starts_sentence: bool) -> String {
    if function_layout_is_protected_word(word) {
        word.to_owned()
    } else if starts_sentence {
        word.to_case(Case::Sentence)
    } else {
        word.to_case(Case::Lower)
    }
}

/// Returns whether an automatic whole-sentence case repair would be unsafe.
fn function_layout_has_protected_content(words: &[&str]) -> bool {
    words.iter().any(|word| {
        let bare = word.trim_matches(|character: char| !character.is_ascii_alphanumeric());
        word.contains(['`', '_'])
            || (bare.len() > 1 && bare.chars().all(|character| character.is_ascii_uppercase()))
    })
}

/// Validates natural sentence casing through protected word-by-word normalization.
fn function_layout_is_sentence_style(content: &str) -> bool {
    // Accept conventional labels before normalizing ordinary prose.
    if function_layout_is_known_label(content) {
        return true;
    }
    let words = content.split_whitespace().collect::<Vec<_>>();
    if words.is_empty() {
        return false;
    }
    if function_layout_is_shouting(content, &words) {
        return false;
    }

    // Normalize prose with the casing crate while preserving code and established proper terms.
    let mut starts_sentence = true;
    let normalized_words = words.iter().map(|word| {
        let normalized = function_layout_normalize_word(word, starts_sentence);
        starts_sentence = word.ends_with(['.', '!', '?']);
        normalized
    });

    // Compare authored prose with the casing crate's protected normalization.
    let normalized = normalized_words.collect::<Vec<_>>().join(" ");
    normalized == content
}

/// Produces a sentence-case repair only when no authored technical spelling is at risk.
fn function_layout_safe_sentence_replacement(content: &str) -> Option<String> {
    let words = content.split_whitespace().collect::<Vec<_>>();
    let is_shouting = function_layout_is_shouting(content, &words);
    let has_protected_content = !is_shouting && function_layout_has_protected_content(&words);
    (!has_protected_content).then(|| sentence_case(content))
}

/// Validates presence, spacing, and sentence style of phase-comment prose.
fn function_layout_content_is_canonical(content: Option<&str>) -> bool {
    content.is_some_and(|content| {
        content == content.trim() && function_layout_is_sentence_style(content)
    })
}

/// Builds a canonical first-line replacement for safely repairable prose.
fn function_layout_comment_replacement(content: Option<&str>, prefix: &str) -> Option<String> {
    let repairable = content.map(str::trim).filter(|content| !content.is_empty());
    repairable
        .and_then(function_layout_safe_sentence_replacement)
        .map(|content| format!("{prefix} {content}"))
}

/// Consecutive line comments headed by the configured phase prefix.
struct FunctionLayoutCommentBlock {
    /// Complete span from the first header through the final continuation.
    span: Span,
    /// Independently replaceable first-line span.
    first_span: Span,
    /// Inclusive physical line range occupied by the comment block.
    lines: RangeInclusive<usize>,
    /// Whether header prose and continuations use canonical syntax.
    is_canonical: bool,
    /// Safe repair for the first line, when available.
    replacement: Option<String>,
}

impl FunctionLayoutCommentBlock {
    /// Constructs a parsed block from its boundary comments and validation results.
    fn from_parts(
        first: &FunctionLayoutRawComment,
        last: &FunctionLayoutRawComment,
        is_canonical: bool,
        replacement: Option<String>,
    ) -> Self {
        Self {
            span: first.span.with_hi(last.span.hi()),
            first_span: first.span,
            lines: first.line..=last.line,
            is_canonical,
            replacement,
        }
    }

    /// Parses a consecutive comment group when its first line uses `prefix`.
    fn parse(comments: &[FunctionLayoutRawComment], prefix: &str) -> Option<Self> {
        // Locate the configured prefix at the start of the comment block.
        let first = comments.first()?;
        let remainder = first.text.strip_prefix(prefix)?;

        // Validate the header content and every natural continuation line.
        let content = remainder.strip_prefix(' ');
        let continuation_is_canonical = Self::continuations_are_canonical(&comments[1..]);
        let content_is_canonical = function_layout_content_is_canonical(content);

        // Suggest only transformations that cannot damage protected authored terms.
        let replacement = function_layout_comment_replacement(content, prefix);
        let last = comments.last().expect("comment blocks are nonempty");
        let is_canonical = content_is_canonical && continuation_is_canonical;

        // Retain both the complete block and its independently repairable first line.
        Some(Self::from_parts(first, last, is_canonical, replacement))
    }

    /// Validates nonempty natural continuation lines after a phase header.
    fn continuations_are_canonical(comments: &[FunctionLayoutRawComment]) -> bool {
        comments.iter().all(|comment| {
            comment
                .text
                .strip_prefix("// ")
                .is_some_and(|content| !content.trim().is_empty())
        })
    }
}

/// Groups adjacent comments and parses blocks headed by the configured prefix.
fn function_layout_comment_blocks(
    cx: &LateContext<'_>,
    span: Span,
    prefix: &str,
) -> Vec<FunctionLayoutCommentBlock> {
    let comments = function_layout_raw_comments(cx, span);
    let mut blocks = Vec::new();
    let mut index = 0;
    while index < comments.len() {
        let mut end = index + 1;
        while end < comments.len() && comments[end].line == comments[end - 1].line + 1 {
            end += 1;
        }

        // A configured prefix marks only the first line; following `//` lines wrap its prose.
        if let Some(block) = FunctionLayoutCommentBlock::parse(&comments[index..end], prefix) {
            blocks.push(block);
        }
        index = end;
    }
    blocks
}

/// Returns whether a comment header is separated from preceding code by a blank line.
fn function_layout_previous_line_is_blank(cx: &LateContext<'_>, gap: Span, comment: Span) -> bool {
    let before = gap.with_hi(comment.lo());
    let source_map = cx.sess().source_map();
    let snippet = source_map.span_to_snippet(before).ok();
    let previous_line = snippet
        .as_deref()
        .and_then(|source| source.lines().rev().nth(1));
    previous_line.is_some_and(|line| line.trim().is_empty())
}

/// Parsed comment state between two function-body entries.
pub(super) struct FunctionLayoutEntryGap {
    /// Whether the gap contains a canonical header attached to the next entry.
    pub(super) has_valid_header: bool,
    /// Malformed phase-comment findings discovered in the gap.
    pub(super) findings: Vec<FunctionLayoutFinding>,
}

impl FunctionLayoutEntryGap {
    /// Analyzes comments in one source gap.
    pub(super) fn analyze(
        cx: &LateContext<'_>,
        config: &FunctionStructureConfig,
        span: Span,
        previous: Option<Span>,
        next: Option<Span>,
    ) -> Self {
        // Resolve source positions needed to validate every candidate header.
        let blocks = function_layout_comment_blocks(cx, span, &config.phase_comment_prefix);
        let next_line = next.map(|span| function_layout_source_line(cx, span.lo()));
        let previous_line = previous.map(|span| function_layout_source_line(cx, span.hi()));
        let mut has_valid_header = false;
        let mut findings = Vec::new();

        // A canonical block must attach to the next phase and separate it from earlier code.
        for block in blocks {
            // Check that the header is visually separated and attached to its phase.
            let has_required_blank = previous.is_none_or(|_| {
                previous_line.is_some_and(|line| block.lines.start() > &(line + 1))
                    && function_layout_previous_line_is_blank(cx, span, block.first_span)
            });
            let immediately_precedes_code = next_line == Some(block.lines.end() + 1);
            let is_layout_valid = has_required_blank && immediately_precedes_code;
            if block.is_canonical && is_layout_valid {
                has_valid_header = true;
                continue;
            }

            // Select the most actionable syntax or placement failure.
            let (message, replacement) = if !block.is_canonical {
                (
                    "this code phase comment is not canonical",
                    block.replacement,
                )
            } else if !has_required_blank {
                (
                    "this code phase comment must be preceded by a blank line",
                    None,
                )
            } else {
                (
                    "this code phase comment must immediately precede its phase",
                    None,
                )
            };

            // Record a safe first-line repair only when placement is already valid.
            let finding_span = if replacement.is_some() {
                block.first_span
            } else {
                block.span
            };
            let help = format!(
                "use `{}` followed by concise sentence-style prose immediately before the phase",
                config.phase_comment_prefix
            );

            // Preserve the malformed block and its safest available repair.
            findings.push(FunctionLayoutFinding {
                span: finding_span,
                message: message.to_owned(),
                help,
                replacement: replacement.filter(|_| is_layout_valid),
            });
        }
        Self {
            has_valid_header,
            findings,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{
        function_layout_is_known_label, function_layout_is_sentence_style,
        function_layout_safe_sentence_replacement,
    };

    #[test]
    fn accepts_natural_sentence_style_and_protected_terms() {
        assert!(function_layout_is_sentence_style(
            "Prepare the value using `u8` and HIR context."
        ));
        assert!(function_layout_is_sentence_style(
            "Read the input. Preserve Rust semantics."
        ));
        assert!(!function_layout_is_sentence_style("Prepare The Value."));
        assert!(!function_layout_is_sentence_style("THIS IS LOUD"));
    }

    #[test]
    fn accepts_only_populated_known_labels() {
        for label in ["SAFETY:", "TODO:", "FIXME:", "NOTE:"] {
            assert!(function_layout_is_known_label(&format!(
                "{label} Explain the exceptional case."
            )));
            assert!(!function_layout_is_known_label(label));
        }
    }

    #[test]
    fn suggests_only_unambiguous_case_repairs() {
        assert_eq!(
            function_layout_safe_sentence_replacement("THIS IS LOUD"),
            Some("This is loud".to_owned())
        );
        assert_eq!(
            function_layout_safe_sentence_replacement("Preserve HIR"),
            None
        );
        assert_eq!(
            function_layout_safe_sentence_replacement("Preserve `u8`"),
            None
        );
    }
}
