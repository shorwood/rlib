extern crate rustc_hir;
extern crate rustc_span;

use rustc_span::Span;

pub(crate) use super::family_name_analysis::FamilyNameAnalyzer;
pub(crate) use super::section_analysis::SectionAnalyzer;

// -----------------------------------------------------------------------------
// Section: Shared organization analysis data
// -----------------------------------------------------------------------------

/// One source-level problem found by the shared analyzer.
pub(crate) struct SectionFinding {
    pub(crate) span: Span,
    pub(crate) message: String,
    pub(crate) help: String,
    pub(crate) replacement: Option<String>,
}

/// One distinct declaration covered by a valid section.
pub(crate) struct SectionParticipant {
    pub(crate) def_id: rustc_hir::def_id::LocalDefId,
    pub(crate) name: String,
    pub(crate) span: Span,
    pub(crate) is_nominal: bool,
}

/// One valid authored section available to semantic companion lints.
pub(crate) struct SectionGroup {
    pub(crate) ordinal: usize,
    pub(crate) prefix: String,
    pub(crate) span: Span,
    pub(crate) participants: Vec<SectionParticipant>,
}

/// Findings split by lint identity so each rule remains independently configurable.
#[derive(Default)]
pub(crate) struct SectionAnalysis {
    pub(crate) missing: Vec<SectionFinding>,
    pub(crate) malformed: Vec<SectionFinding>,
    pub(crate) duplicates: Vec<SectionFinding>,
    pub(crate) mismatches: Vec<SectionFinding>,
    pub(crate) sections: Vec<SectionGroup>,
}

impl SectionAnalysis {
    /// Returns the authored section containing an item at the given source span.
    pub(crate) fn section_ordinal_for_span(&self, span: Span) -> Option<usize> {
        self.sections
            .iter()
            .rev()
            .find(|section| section.span.lo() <= span.lo())
            .map(|section| section.ordinal)
    }
}
