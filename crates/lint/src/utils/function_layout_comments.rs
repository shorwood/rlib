extern crate rustc_lint;
extern crate rustc_span;

use std::ops::RangeInclusive;

use rustc_lint::{LateContext, LintContext};
use rustc_span::Span;

use super::function_layout_prose::FunctionLayoutProse;
use super::function_layout_source::{
    FunctionLayoutComment, FunctionLayoutPositionExt, FunctionLayoutSpanExt,
};
use super::function_structure_config::FunctionStructureConfig;

// -----------------------------------------------------------------------------
// FunctionLayoutFinding: Layout diagnostic
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

// -----------------------------------------------------------------------------
// Block: Parsed phase comment block
// -----------------------------------------------------------------------------

/// Consecutive line comments headed by the configured phase prefix.
struct Block {
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

impl Block {
    /// Parses a consecutive comment group when its first line uses `prefix`.
    fn parse(comments: &[FunctionLayoutComment], prefix: &str) -> Option<Self> {
        // Locate the configured prefix at the start of the comment block.
        let first = comments.first()?;
        let remainder = first.text.strip_prefix(prefix)?;

        // Validate the header content and every natural continuation line.
        let content = remainder.strip_prefix(' ');
        let is_canonical = FunctionLayoutProse::is_canonical(content)
            && Self::continuations_are_canonical(&comments[1..]);

        // Suggest only transformations that cannot damage protected authored terms.
        let replacement = FunctionLayoutProse::replacement(content, prefix);
        let last = comments.last().expect("comment blocks are nonempty");

        // Retain both the complete block and its independently repairable first line.
        Some(Self {
            span: first.span.with_hi(last.span.hi()),
            first_span: first.span,
            lines: first.line..=last.line,
            is_canonical,
            replacement,
        })
    }

    /// Groups adjacent comments and parses blocks headed by the configured prefix.
    fn collect(cx: &LateContext<'_>, span: Span, prefix: &str) -> Vec<Self> {
        let comments = span.comments(cx);
        let mut blocks = Vec::new();
        let mut index = 0;
        while index < comments.len() {
            let mut end = index + 1;
            while end < comments.len() && comments[end].line == comments[end - 1].line + 1 {
                end += 1;
            }

            // A configured prefix marks only the first line; following `//` lines wrap its prose.
            if let Some(block) = Self::parse(&comments[index..end], prefix) {
                blocks.push(block);
            }
            index = end;
        }
        blocks
    }

    /// Validates nonempty natural continuation lines after a phase header.
    fn continuations_are_canonical(comments: &[FunctionLayoutComment]) -> bool {
        comments.iter().all(|comment| {
            comment
                .text
                .strip_prefix("// ")
                .is_some_and(|content| !content.trim().is_empty())
        })
    }

    /// Returns whether a comment header is separated from preceding code by a blank line.
    fn previous_line_is_blank(cx: &LateContext<'_>, gap: Span, comment: Span) -> bool {
        let before = gap.with_hi(comment.lo());
        let source_map = cx.sess().source_map();
        let Ok(snippet) = source_map.span_to_snippet(before) else {
            return false;
        };
        let mut lines = snippet.lines().rev();
        let mut previous_line = lines.nth(1);
        while previous_line.is_some_and(|line| line.trim_start().starts_with("#[")) {
            previous_line = lines.next();
        }
        previous_line.is_some_and(|line| line.trim().is_empty())
    }
}

// -----------------------------------------------------------------------------
// FunctionLayoutEntryGap: Comments between direct entries
// -----------------------------------------------------------------------------

/// Parsed comment state between two function-body entries.
pub(super) struct FunctionLayoutEntryGap {
    /// Whether the gap contains a canonical header attached to the next entry.
    pub(super) has_valid_header: bool,
    /// Whether a blank authored line marks an otherwise unnamed boundary.
    pub(super) has_visual_boundary: bool,
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
        let blocks = Block::collect(cx, span, &config.phase_comment_prefix);
        let next_line = next.map(|span| span.lo().source_line(cx));
        let previous_line = previous.map(|span| span.hi().source_line(cx));
        let mut has_valid_header = false;
        let mut findings = Vec::new();
        let has_visual_boundary = previous.is_some() && Self::contains_blank_line(cx, span);

        // A canonical block must attach to the next phase and separate it from earlier code.
        for block in blocks {
            // Check that the header is visually separated and attached to its phase.
            let has_required_blank = previous.is_none_or(|_| {
                previous_line.is_some_and(|line| block.lines.start() > &(line + 1))
                    && Block::previous_line_is_blank(cx, span, block.first_span)
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
            has_visual_boundary,
            findings,
        }
    }

    /// Returns whether the gap contains a complete blank physical line.
    fn contains_blank_line(cx: &LateContext<'_>, span: Span) -> bool {
        let Ok(source) = cx.sess().source_map().span_to_snippet(span) else {
            return false;
        };
        let lines = source.split('\n').collect::<Vec<_>>();
        lines.len() > 2
            && lines[1..lines.len() - 1]
                .iter()
                .any(|line| line.trim().is_empty())
    }
}
