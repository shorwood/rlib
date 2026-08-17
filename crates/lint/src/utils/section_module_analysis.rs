extern crate rustc_hir;
extern crate rustc_lexer;
extern crate rustc_lint;
extern crate rustc_span;

use std::collections::HashMap;

use event_stream::{
    SectionEventCandidate, SectionEventStreamEntry, SectionEventStreamGroup,
    SectionEventStreamState,
};
use rustc_hir::{HirId, ItemKind, Mod, Node};
use rustc_lexer::{FrontmatterAllowed, TokenKind, tokenize};
use rustc_lint::{LateContext, LintContext};
use rustc_span::{BytePos, Span};

use super::template::ParsedContent;
use super::{
    SectionAnalysis, SectionAnalyzer, SectionAnalyzerRenderRequest, SectionFinding, SectionGroup,
};
use crate::utils::identifier_case;
use crate::utils::source_provenance::ItemProvenanceExt;

/// Tokens required to recognize `identifier!delimiter` at an item boundary.
const MACRO_INVOCATION_PREFIX_TOKENS: usize = 3;

// -----------------------------------------------------------------------------
// ModuleAnalysis: Complete section analysis
// -----------------------------------------------------------------------------
/// Authored and inferred names involved in one section-prefix mismatch.
#[derive(Clone, Copy)]
struct ModulePrefixMismatch<'name> {
    /// Prefix written in the divider.
    authored: &'name str,
    /// Shared prefix inferred from the declarations.
    expected: &'name str,
}
/// Validated section prefix passed through module-level checks.
#[derive(Clone, Copy)]
struct ModuleSectionPrefix<'name> {
    /// Authored `PascalCase` prefix.
    text: &'name str,
}

/// Semantic namespace supplied by a named containing module.
struct ModuleNamespace {
    /// Canonical `PascalCase` form of the module name.
    prefix: String,
    /// Current and parent module names that serve only as namespace qualifiers.
    prefixes: Vec<String>,
}

impl ModuleNamespace {
    /// Returns whether a shared type prefix merely repeats its module namespace.
    fn contains(&self, prefix: &str) -> bool {
        self.prefixes.iter().any(|namespace| namespace == prefix)
    }
}

#[path = "section_event_stream.rs"]
mod event_stream;

/// Builds and validates the ordered section event stream for one source module.
pub(super) struct ModuleAnalysis;

impl ModuleAnalysis {
    /// Collects authored items and dividers, then analyzes them in source order.
    pub(super) fn analyze(
        analyzer: &SectionAnalyzer,
        cx: &LateContext<'_>,
        module: &Mod<'_>,
        hir_id: HirId,
    ) -> SectionAnalysis {
        // Identify the physical source file represented by this HIR module.
        let source_map = cx.sess().source_map();
        let module_span = Self::source_span(cx, module, hir_id);
        let module_file = source_map.span_to_filename(module_span);

        // Retain only authored declarations from the module's physical file.
        let source_items = module
            .item_ids
            .iter()
            .map(|item_id| cx.tcx.hir_item(*item_id));

        // Ignore expansions and declarations sourced from child module files.
        let mut items = source_items
            .filter(|item| {
                !item.span.from_expansion()
                    && !item.is_framework_generated()
                    && source_map.span_to_filename(item.span) == module_file
            })
            .collect::<Vec<_>>();

        // Establish source order before constructing the section event stream.
        items.sort_unstable_by_key(|item| item.span.lo());

        // Interleave authored dividers with the declarations they introduce.
        let mut events = Vec::new();
        let mut previous = module_span.lo();
        for item in items {
            if previous <= item.span.lo() {
                let gap = module_span.with_lo(previous).with_hi(item.span.lo());
                Self::append_dividers(analyzer, cx, gap, &mut events);
            }
            if let Some(participant) = SectionEventCandidate::from_item(cx, item) {
                events.push(SectionEventStreamEntry::Candidate(participant));
            }
            previous = previous.max(item.span.hi());
        }
        if previous <= module_span.hi() {
            let gap = module_span.with_lo(previous).with_hi(module_span.hi());
            Self::append_dividers(analyzer, cx, gap, &mut events);
        }
        events.sort_unstable_by_key(SectionEventStreamEntry::position);
        let namespace = Self::namespace(cx, hir_id);
        Self::analyze_events(analyzer, events, namespace)
    }

    /// Appends every divider found in `span` to the module event stream.
    fn append_dividers(
        analyzer: &SectionAnalyzer,
        cx: &LateContext<'_>,
        span: Span,
        events: &mut Vec<SectionEventStreamEntry>,
    ) {
        let dividers = analyzer.dividers_in_span(cx, span);
        events.extend(dividers.into_iter().map(SectionEventStreamEntry::Divider));
        events.extend(
            Self::macro_invocations(cx, span)
                .into_iter()
                .map(SectionEventStreamEntry::Candidate),
        );
    }

    /// Finds authored item-position macro invocations omitted from the lowered module HIR.
    fn macro_invocations(cx: &LateContext<'_>, span: Span) -> Vec<SectionEventCandidate> {
        // Unavailable source text cannot contribute authored macro invocations.
        let Ok(source) = cx.sess().source_map().span_to_snippet(span) else {
            return Vec::new();
        };
        let mut tokens = Vec::new();
        let mut offset = 0_usize;
        for token in tokenize(&source, FrontmatterAllowed::No) {
            let length = usize::try_from(token.len).expect("token length should fit usize");
            let end = offset + length;
            if !matches!(
                token.kind,
                TokenKind::Whitespace
                    | TokenKind::LineComment { .. }
                    | TokenKind::BlockComment { .. }
            ) {
                tokens.push((token.kind, offset, end));
            }
            offset = end;
        }

        let mut invocations = Vec::new();
        let mut index = 0_usize;
        let mut depth = 0_usize;
        while index + MACRO_INVOCATION_PREFIX_TOKENS <= tokens.len() {
            let (kind, start, name_end) = tokens[index];
            if depth == 0
                && matches!(kind, TokenKind::Ident | TokenKind::RawIdent)
                && matches!(tokens[index + 1].0, TokenKind::Bang)
                && matches!(
                    tokens[index + 2].0,
                    TokenKind::OpenParen | TokenKind::OpenBrace | TokenKind::OpenBracket
                )
            {
                let opening = tokens[index + 2].0;
                let mut invocation_depth = 1_usize;
                let mut end_index = index + 3;
                while end_index < tokens.len() && invocation_depth > 0 {
                    let candidate = tokens[end_index].0;
                    if candidate == opening {
                        invocation_depth += 1;
                    } else if Self::is_matching_delimiter(opening, candidate) {
                        invocation_depth -= 1;
                    }
                    end_index += 1;
                }
                if invocation_depth == 0 {
                    let end = tokens[end_index - 1].2;
                    let lo = u32::try_from(start).expect("macro offset should fit BytePos");
                    let hi = u32::try_from(end).expect("macro offset should fit BytePos");
                    let invocation_span = span
                        .with_lo(span.lo() + BytePos(lo))
                        .with_hi(span.lo() + BytePos(hi));
                    let name = source[start..name_end].trim_start_matches("r#").to_owned();
                    invocations.push(SectionEventCandidate::from_macro_invocation(
                        name,
                        invocation_span,
                    ));
                    index = end_index;
                    continue;
                }
            }

            match kind {
                TokenKind::OpenParen | TokenKind::OpenBrace | TokenKind::OpenBracket => depth += 1,
                TokenKind::CloseParen | TokenKind::CloseBrace | TokenKind::CloseBracket => {
                    depth = depth.saturating_sub(1);
                }
                _ => {}
            }
            index += 1;
        }
        invocations
    }

    /// Returns whether `closing` terminates the given opening delimiter.
    const fn is_matching_delimiter(opening: TokenKind, closing: TokenKind) -> bool {
        matches!(
            (opening, closing),
            (TokenKind::OpenParen, TokenKind::CloseParen)
                | (TokenKind::OpenBrace, TokenKind::CloseBrace)
                | (TokenKind::OpenBracket, TokenKind::CloseBracket)
        )
    }

    /// Reduces a complete event stream into independently reportable findings.
    fn analyze_events(
        analyzer: &SectionAnalyzer,
        events: Vec<SectionEventStreamEntry>,
        namespace: Option<ModuleNamespace>,
    ) -> SectionAnalysis {
        let mut state = SectionEventStreamState {
            namespace,
            ..SectionEventStreamState::default()
        };

        // Close each group when the following divider starts a new one.
        for event in events {
            state.apply(analyzer, event);
        }
        state.finish(analyzer)
    }

    /// Resolves the semantic namespace of a named containing module.
    fn namespace(cx: &LateContext<'_>, hir_id: HirId) -> Option<ModuleNamespace> {
        let containing_nodes = [cx.tcx.hir_node(hir_id), cx.tcx.parent_hir_node(hir_id)];
        let item = containing_nodes.into_iter().find_map(|node| match node {
            Node::Item(item) if matches!(item.kind, ItemKind::Mod(..)) => Some(item),
            _ => None,
        })?;
        let ident = item.kind.ident()?;
        let prefix = identifier_case::to_pascal(ident.name.as_str());
        let mut prefixes = vec![prefix.clone()];
        if let Some(parent) = cx.tcx.opt_parent(item.owner_id.to_def_id()) {
            prefixes.push(identifier_case::to_pascal(
                cx.tcx.item_name(parent).as_str(),
            ));
        }
        Some(ModuleNamespace { prefix, prefixes })
    }

    /// Locates the authored byte immediately after an inline module's opening brace.
    fn opening_brace_offset(cx: &LateContext<'_>, span: Span) -> Option<BytePos> {
        // Missing source text prevents locating the authored delimiter.
        let Ok(source) = cx.sess().source_map().span_to_snippet(span) else {
            return None;
        };
        let offset = source.rfind('{')? + 1;

        // Offsets outside the compiler position range cannot form a valid span.
        let Ok(offset) = u32::try_from(offset) else {
            return None;
        };
        Some(BytePos(offset))
    }

    /// Locates the authored byte at an inline module's closing brace.
    fn closing_brace_offset(cx: &LateContext<'_>, span: Span) -> Option<BytePos> {
        // Missing source text prevents locating the authored delimiter.
        let Ok(source) = cx.sess().source_map().span_to_snippet(span) else {
            return None;
        };
        let offset = source.find('}')?;

        // Offsets outside the compiler position range cannot form a valid span.
        let Ok(offset) = u32::try_from(offset) else {
            return None;
        };
        Some(BytePos(offset))
    }

    /// Finds the physical source extent in which module-level dividers may appear.
    fn source_span(cx: &LateContext<'_>, module: &Mod<'_>, hir_id: HirId) -> Span {
        // Resolve the inline module item surrounding the HIR module body.
        let inner = module.spans.inner_span;
        let containing_nodes = [cx.tcx.hir_node(hir_id), cx.tcx.parent_hir_node(hir_id)];
        let item = containing_nodes.into_iter().find_map(|node| match node {
            Node::Item(item) if matches!(item.kind, ItemKind::Mod(..)) => Some(item),
            _ => None,
        });

        // File modules already expose the complete inner span from HIR.
        let Some(item) = item else {
            return inner;
        };

        // File-backed modules already use a complete span from their own source file.
        if cx.sess().source_map().span_to_filename(item.span)
            != cx.sess().source_map().span_to_filename(inner)
        {
            return inner;
        }

        // Expand inline-module bodies to include comments beside their braces.
        let opening_span = item.span.with_hi(inner.lo());
        let opening_offset = Self::opening_brace_offset(cx, opening_span);
        let lo = opening_offset.map_or_else(|| inner.lo(), |offset| item.span.lo() + offset);

        // Extend through the authored closing brace when its source is available.
        let closing_span = item.span.with_lo(inner.hi());
        let closing_offset = Self::closing_brace_offset(cx, closing_span);
        let hi = closing_offset.map_or_else(|| inner.hi(), |offset| inner.hi() + offset);
        inner.with_lo(lo).with_hi(hi)
    }

    /// Records placement, syntax, and width failures for one authored divider.
    fn record_malformed_section(
        analyzer: &SectionAnalyzer,
        section: &SectionEventStreamGroup,
        parsed: &ParsedContent,
        analysis: &mut SectionAnalysis,
    ) {
        // A test module is absent from non-test HIR even though its authored divider remains.
        let is_inactive_test_section = parsed
            .prefix
            .as_deref()
            .is_some_and(|prefix| prefix == "Tests");
        if section.participants.is_empty() && !is_inactive_test_section {
            analysis.malformed.push(SectionFinding {
                span: section.divider.span,
                message: "section divider does not contain any module declarations".to_owned(),
                help: "remove this divider or place it before the declarations it describes"
                    .to_owned(),
                replacement: None,
            });
        }

        // Prefer a safe canonical replacement for syntactic content errors.
        if let Some(message) = &parsed.error {
            // Render the normalized content only when the template can represent it safely.
            let replacement = parsed.normalized.as_ref().and_then(|content| {
                analyzer.render_replacement(SectionAnalyzerRenderRequest {
                    content,
                    indentation: &section.divider.indentation,
                })
            });

            // Explain the canonical grammar alongside any machine-applicable fix.
            analysis.malformed.push(SectionFinding {
                span: section.divider.span,
                message: message.clone(),
                help: "use `PascalCaseAbstraction: Sentence case responsibility`".to_owned(),
                replacement,
            });
        } else if !analyzer.rendered_lines_fit(&section.divider.raw_content) {
            // Describe the configured width failure independently from syntax errors.
            let message = format!(
                "section divider exceeds the configured {}-character line limit",
                analyzer.max_line_length
            );
            let help = "shorten the responsibility description or choose a more compact template"
                .to_owned();

            // Record the width failure without proposing an unsafe content rewrite.
            analysis.malformed.push(SectionFinding {
                span: section.divider.span,
                message,
                help,
                replacement: None,
            });
        }
    }

    /// Publishes a syntactically valid, nonempty section for companion analyses.
    fn record_valid_section(
        analyzer: &SectionAnalyzer,
        section: &SectionEventStreamGroup,
        parsed: &ParsedContent,
        prefix: ModuleSectionPrefix<'_>,
        namespace: Option<&ModuleNamespace>,
        analysis: &mut SectionAnalysis,
    ) {
        // Require canonical syntax, width, and at least one declaration participant.
        let is_valid = parsed.error.is_none()
            && analyzer.rendered_lines_fit(&section.divider.raw_content)
            && !section.participants.is_empty();

        // Invalid or empty sections are excluded from semantic companion analyses.
        if !is_valid {
            return;
        }

        // Collapse a nominal declaration and all of its implementation blocks into one concept.
        let participants = section.participants.distinct_declarations();
        if participants.len() > analyzer.max_declarations_per_section
            && section
                .participants
                .has_multiple_declaration_families(namespace)
        {
            // Use configured scale as corroborating evidence, not a reason to split one family.
            let message = format!(
                "section `{}` mixes {} declarations from several naming families, exceeding the configured maximum of {}",
                prefix.text,
                participants.len(),
                analyzer.max_declarations_per_section
            );

            // Steer remediation toward concepts and names before mechanical splitting.
            let help = "reconsider whether these declarations express smaller coherent concepts and rename them around those concepts; add another divider only for a genuinely independent family".to_owned();
            analysis.overloaded.push(SectionFinding {
                span: section.divider.span,
                message,
                help,
                replacement: None,
            });
        }

        // Publish the stable semantic section used by companion naming analyses.
        analysis.sections.push(SectionGroup {
            ordinal: analysis.sections.len() + 1,
            prefix: prefix.text.to_owned(),
            span: section.divider.span,
            participants,
        });
    }

    /// Records a finding when `prefix` has already appeared in the module.
    fn record_duplicate_section(
        section: &SectionEventStreamGroup,
        prefix: ModuleSectionPrefix<'_>,
        seen_prefixes: &mut HashMap<String, Span>,
        analysis: &mut SectionAnalysis,
    ) {
        // Register this prefix and stop when it has not appeared before.
        if seen_prefixes
            .insert(prefix.text.to_owned(), section.divider.span)
            .is_none()
        {
            return;
        }

        // Report the later occurrence while naming the family it should rejoin.
        let message = format!(
            "section prefix `{}` is used more than once in this module",
            prefix.text
        );
        let help = format!(
            "combine this family with the earlier `{}` section",
            prefix.text
        );

        // Attach the prepared diagnostic text to the later divider occurrence.
        analysis.duplicates.push(SectionFinding {
            span: section.divider.span,
            message,
            help,
            replacement: None,
        });
    }

    /// Builds a finding for a valid prefix that differs from the inferred family name.
    fn wrong_prefix_finding(
        section: &SectionEventStreamGroup,
        mismatch: ModulePrefixMismatch<'_>,
    ) -> SectionFinding {
        // Explain the mismatch and the inferred declaration-family prefix.
        let message = format!(
            "section prefix `{}` does not match its declaration family",
            mismatch.authored
        );
        let help = format!(
            "rename the declarations into one coherent family first; their longest shared PascalCase prefix is `{}`, and a new section is appropriate only for an independent concept",
            mismatch.expected
        );

        // Attach the naming-first guidance to the authored divider.
        SectionFinding {
            span: section.divider.span,
            message,
            help,
            replacement: None,
        }
    }

    /// Builds naming-first guidance for declarations that share no `PascalCase` prefix.
    fn unrelated_names_finding(
        section: &SectionEventStreamGroup,
        prefix: ModuleSectionPrefix<'_>,
    ) -> SectionFinding {
        // Explain both the absent shared prefix and the declarations that need renaming.
        let message = format!(
            "section `{}` contains declarations without a shared PascalCase prefix",
            prefix.text
        );
        let help = format!(
            "reconsider the names {} so closely related declarations share a visible prefix; split the section only when they represent independent concepts",
            section.participants.formatted_family_names()
        );

        // Attach naming-first guidance to the authored divider.
        SectionFinding {
            span: section.divider.span,
            message,
            help,
            replacement: None,
        }
    }

    /// Compares an authored prefix with the names grouped beneath it.
    fn mismatch_finding(
        section: &SectionEventStreamGroup,
        prefix: ModuleSectionPrefix<'_>,
        namespace: Option<&ModuleNamespace>,
    ) -> Option<SectionFinding> {
        // Infer a family only when the divider actually governs declarations.
        if section.participants.is_empty() {
            return None;
        }

        // Opaque macros expose no declaration names from which to infer a prefix.
        if section.participants.contains_only_opaque_macros() {
            return None;
        }

        // A divider matching its containing module already names the local namespace.
        if namespace.is_some_and(|namespace| namespace.prefix == prefix.text) {
            return None;
        }
        let names = section.participants.family_names();

        // `Violation` is a diagnostic role whose supporting evidence keeps domain-specific names.
        if prefix.text == "Violation" && names.contains(&"Violation") {
            return None;
        }

        // Report declarations with no shared naming root directly.
        let Some(expected) = identifier_case::longest_common_pascal_prefix(&names) else {
            return Some(Self::unrelated_names_finding(section, prefix));
        };

        // The authored divider already matches the inferred declaration family.
        if expected == prefix.text {
            return None;
        }

        // A declaration named for the divider may legitimately specialize its module namespace.
        if names.contains(&prefix.text)
            && namespace.is_some_and(|namespace| namespace.contains(&expected))
        {
            return None;
        }

        // Report a concrete mismatch using the inferred family prefix.
        Some(Self::wrong_prefix_finding(
            section,
            ModulePrefixMismatch {
                authored: prefix.text,
                expected: &expected,
            },
        ))
    }

    /// Runs every section-level check when a divider group closes.
    fn finish_section(
        analyzer: &SectionAnalyzer,
        section: &SectionEventStreamGroup,
        namespace: Option<&ModuleNamespace>,
        seen_prefixes: &mut HashMap<String, Span>,
        analysis: &mut SectionAnalysis,
    ) {
        // Parse the divider content to validate its syntax and extract the authored prefix.
        let parsed = ParsedContent::from(section.divider.raw_content.as_str());
        Self::record_malformed_section(analyzer, section, &parsed, analysis);

        // Malformed, oversized, or empty sections cannot enter semantic analyses.
        if parsed.error.is_some()
            || !analyzer.rendered_lines_fit(&section.divider.raw_content)
            || section.participants.is_empty()
        {
            return;
        }

        // A valid semantic section still requires an authored family prefix.
        let Some(prefix) = parsed.prefix.as_deref() else {
            return;
        };
        let prefix = ModuleSectionPrefix { text: prefix };

        // Record the section independently for each semantic companion lint.
        Self::record_valid_section(analyzer, section, &parsed, prefix, namespace, analysis);
        Self::record_duplicate_section(section, prefix, seen_prefixes, analysis);

        // Matching section names produce no mismatch diagnostic.
        let Some(finding) = Self::mismatch_finding(section, prefix, namespace) else {
            return;
        };
        analysis.mismatches.push(finding);
    }
}

// -----------------------------------------------------------------------------
