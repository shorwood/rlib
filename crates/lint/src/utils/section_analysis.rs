extern crate rustc_hir;
extern crate rustc_lint;
extern crate rustc_span;

use rustc_hir::def_id::LocalDefId;
use rustc_hir::{HirId, Mod};
use rustc_lint::{LateContext, LintContext};
use rustc_span::{BytePos, Span};

use crate::config::core::{BooleanPredicateConfig, SectionDividerConfig};
use crate::config::store::ConfigStore;
use crate::utils::identifier_case::longest_common_pascal_prefix;

#[path = "section_template.rs"]
mod template;
#[path = "section_module_analysis.rs"]
mod module_analysis;

use module_analysis::ModuleAnalysis;
use template::{Template, TemplateMatch};

// -----------------------------------------------------------------------------
// Section: Shared organization analysis data
// -----------------------------------------------------------------------------

/// Authored divider data owned by section analysis and consumed by its event stream.
struct SectionEventDivider {
    /// Complete source range occupied by the divider template.
    span: Span,
    /// Unparsed content captured from the template placeholder.
    raw_content: String,
    /// Whitespace that prefixes each rendered divider line.
    indentation: String,
}

/// One source-level problem found by the shared analyzer.
pub struct SectionFinding {
    /// Authored source range to underline or replace.
    pub(crate) span: Span,
    /// Primary explanation of the organization problem.
    pub(crate) message: String,
    /// Naming- or organization-first remediation guidance.
    pub(crate) help: String,
    /// Canonical divider text when a safe replacement can be synthesized.
    pub(crate) replacement: Option<String>,
}

/// One distinct declaration covered by a valid section.
pub struct SectionParticipant {
    /// Local definition represented by this declaration or implementation.
    pub(super) def_id: LocalDefId,
    /// Authored declaration name used for family inference and diagnostics.
    pub(super) name: String,
    /// Source range occupied by the declaration.
    pub(super) span: Span,
    /// Whether this participant defines a nominal type rather than supporting it.
    pub(super) is_nominal: bool,
}

/// One valid authored section available to semantic companion lints.
pub struct SectionGroup {
    /// One-based position of the section in its source module.
    pub(super) ordinal: usize,
    /// `PascalCase` family prefix declared by the divider.
    pub(super) prefix: String,
    /// Complete source range of the divider template.
    pub(super) span: Span,
    /// Distinct declarations governed by the divider.
    pub(super) participants: Vec<SectionParticipant>,
}

/// Findings split by lint identity so each rule remains independently configurable.
#[derive(Default)]
pub struct SectionAnalysis {
    /// Declaration groups that have no preceding divider.
    pub(crate) missing: Vec<SectionFinding>,
    /// Dividers whose syntax, width, or placement is invalid.
    pub(crate) malformed: Vec<SectionFinding>,
    /// Repeated section family prefixes within one module.
    pub(crate) duplicates: Vec<SectionFinding>,
    /// Dividers whose prefix disagrees with the declarations they contain.
    pub(crate) mismatches: Vec<SectionFinding>,
    /// Dividers governing more distinct declarations than the configured limit.
    pub(crate) overloaded: Vec<SectionFinding>,
    /// Valid sections available to companion semantic lints.
    pub(super) sections: Vec<SectionGroup>,
}

impl SectionAnalysis {
    /// Returns the authored section containing an item at the given source span.
    pub(crate) fn section_ordinal_for_span(&self, span: Span) -> Option<usize> {
        let preceding_section = self
            .sections
            .iter()
            .rev()
            .find(|section| section.span.lo() <= span.lo());
        preceding_section.map(|section| section.ordinal)
    }
}

// -----------------------------------------------------------------------------
// SectionAnalyzer: Source analysis
// -----------------------------------------------------------------------------
/// Authored content and indentation used to render one divider replacement.
#[derive(Clone, Copy)]
struct SectionAnalyzerRenderRequest<'source> {
    /// Canonical placeholder content.
    content: &'source str,
    /// Whitespace preceding the divider in its module.
    indentation: &'source str,
}

/// Parses configured divider blocks and relates them to module-level declaration groups.
pub struct SectionAnalyzer {
    /// Parsed divider template used for both recognition and rendering.
    template: Template,
    /// Maximum permitted width of every rendered divider line.
    max_line_length: usize,
    /// Maximum number of distinct declarations governed by one divider.
    max_declarations_per_section: usize,
    /// Shared callable grammar removed before declaration-family inference.
    boolean_predicates: BooleanPredicateConfig,
}

impl SectionAnalyzer {
    /// Loads and validates the shared library configuration.
    pub(crate) fn from_config() -> Self {
        // Load the divider-specific project configuration.
        let config = ConfigStore::get();

        // Parse the source template before retaining the complete semantic policy.
        // Fail during lint construction when the project template is invalid.
        let parsed_template = Template::parse(
            SectionDividerConfig::TEMPLATE,
            SectionDividerConfig::MAX_LINE_LENGTH,
        );
        let template = parsed_template
            .unwrap_or_else(|message| panic!("invalid section divider template: {message}"));

        // Retain the parsed template beside both configured limits.
        Self {
            template,
            max_line_length: SectionDividerConfig::MAX_LINE_LENGTH,
            max_declarations_per_section: config.section_dividers.max_declarations_per_section,
            boolean_predicates: config.boolean_predicates.clone(),
        }
    }

    /// Converts a snippet-relative template match into an absolute source event.
    fn divider_from_match(span: Span, matched: TemplateMatch) -> SectionEventDivider {
        // Convert snippet offsets into source-map byte positions.
        let start = u32::try_from(matched.start).expect("source span offset should fit in BytePos");
        let end = u32::try_from(matched.end).expect("source span offset should fit in BytePos");

        // Preserve the template's exact authored extent and indentation.
        let divider_span = span
            .with_lo(span.lo() + BytePos(start))
            .with_hi(span.lo() + BytePos(end));

        // Carry authored content forward for semantic validation and fixes.
        SectionEventDivider {
            span: divider_span,
            raw_content: matched.content,
            indentation: matched.indentation,
        }
    }

    /// Analyzes one source module without inspecting comments inside item bodies.
    pub(crate) fn analyze(
        &self,
        cx: &LateContext<'_>,
        module: &Mod<'_>,
        hir_id: HirId,
    ) -> SectionAnalysis {
        ModuleAnalysis::analyze(self, cx, module, hir_id)
    }

    /// Infers the ownership prefix after removing configured predicate and query grammar.
    fn predicate_subject_family_prefix(&self, names: &[&str]) -> Option<String> {
        let semantic_names = names
            .iter()
            .map(|name| self.boolean_predicates.semantic_callable_name(name))
            .collect::<Vec<_>>();
        longest_common_pascal_prefix(&semantic_names)
    }

    /// Infers an ownership prefix, using predicate subjects when grammar hides the raw family.
    fn semantic_family_prefix(&self, names: &[&str]) -> Option<String> {
        longest_common_pascal_prefix(names).or_else(|| self.predicate_subject_family_prefix(names))
    }

    /// Finds divider templates in an item-free source range.
    fn dividers_in_span(&self, cx: &LateContext<'_>, span: Span) -> Vec<SectionEventDivider> {
        // Unavailable source cannot yield authored divider positions.
        let Ok(source) = cx.sess().source_map().span_to_snippet(span) else {
            return Vec::new();
        };

        // Translate template-relative offsets back into source-map spans.
        let matches = self.template.find_matches(&source);
        matches
            .into_iter()
            .map(|matched| Self::divider_from_match(span, matched))
            .collect()
    }

    /// Returns whether rendering `content` stays within the configured line width.
    fn has_fitting_rendered_lines(&self, content: &str) -> bool {
        self.template
            .render(content)
            .lines()
            .all(|line| line.chars().count() <= self.max_line_length)
    }

    /// Renders an indented canonical divider when it satisfies the width policy.
    fn render_replacement(&self, request: SectionAnalyzerRenderRequest<'_>) -> Option<String> {
        self.has_fitting_rendered_lines(request.content).then(|| {
            let rendered = self.template.render(request.content);
            rendered
                .lines()
                .collect::<Vec<_>>()
                .join(&format!("\n{}", request.indentation))
        })
    }
}
