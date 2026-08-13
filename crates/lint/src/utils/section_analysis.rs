extern crate rustc_hir;
extern crate rustc_lint;
extern crate rustc_span;

use rustc_hir::def_id::LocalDefId;
use rustc_hir::{HirId, Mod};
use rustc_lint::{LateContext, LintContext};
use rustc_span::{BytePos, Span};

use super::config::LibraryConfig;

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
#[derive(Default)]
/// Findings split by lint identity so each rule remains independently configurable.
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
#[derive(Clone, Copy)]
/// Authored content and indentation used to render one divider replacement.
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
}

impl SectionAnalyzer {
    /// Loads and validates the shared library configuration.
    pub(crate) fn from_config() -> Self {
        // Load the divider-specific project configuration.
        let config = LibraryConfig::load().section_dividers;
        config
            .validate()
            .unwrap_or_else(|message| panic!("invalid section divider configuration: {message}"));

        // Parse the source template before retaining the complete semantic policy.
        // Fail during lint construction when the project template is invalid.
        let parsed_template = Template::parse(&config.template, config.max_line_length);
        let template = parsed_template
            .unwrap_or_else(|message| panic!("invalid section divider template: {message}"));

        // Retain the parsed template beside both configured limits.
        Self {
            template,
            max_line_length: config.max_line_length,
            max_declarations_per_section: config.max_declarations_per_section,
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

    /// Finds divider templates in an item-free source range.
    fn dividers_in_span(&self, cx: &LateContext<'_>, span: Span) -> Vec<SectionEventDivider> {
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
    fn rendered_lines_fit(&self, content: &str) -> bool {
        self.template
            .render(content)
            .lines()
            .all(|line| line.chars().count() <= self.max_line_length)
    }

    /// Renders an indented canonical divider when it satisfies the width policy.
    fn render_replacement(&self, request: SectionAnalyzerRenderRequest<'_>) -> Option<String> {
        self.rendered_lines_fit(request.content).then(|| {
            let rendered = self.template.render(request.content);
            rendered
                .lines()
                .collect::<Vec<_>>()
                .join(&format!("\n{}", request.indentation))
        })
    }
}

#[cfg(test)]
mod config_tests {
    use crate::utils::config::LibraryConfig;
    use crate::utils::section_divider_config::SectionDividerConfig;

    #[test]
    fn parses_custom_section_declaration_limit() {
        let config = toml::from_str::<LibraryConfig>(
            r"
                [section_dividers]
                max_declarations_per_section = 12
            ",
        )
        .expect("custom section declaration limit should parse");
        assert_eq!(config.section_dividers.max_declarations_per_section, 12);
        assert!(config.section_dividers.validate().is_ok());
    }

    #[test]
    fn rejects_zero_section_declaration_limit() {
        let config = SectionDividerConfig {
            max_declarations_per_section: 0,
            ..SectionDividerConfig::default()
        };
        assert!(config.validate().is_err());
    }
}
