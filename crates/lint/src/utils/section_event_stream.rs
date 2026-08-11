extern crate rustc_hir;
extern crate rustc_lint;
extern crate rustc_middle;
extern crate rustc_span;

use std::collections::HashMap;

use rustc_hir::{Item, ItemKind};
use rustc_lint::LateContext;
use rustc_middle::ty;
use rustc_span::{BytePos, Span};

use super::{ModuleAnalysis, ModuleNamespace};
use crate::utils::identifier_case;
use crate::utils::section_analysis::{
    SectionAnalysis, SectionAnalyzer, SectionEventDivider, SectionFinding, SectionParticipant,
};
use crate::utils::source_provenance::is_framework_generated_item;

// -----------------------------------------------------------------------------
// SectionEventCandidate: Section events and participants
// -----------------------------------------------------------------------------

/// Semantic role of a named declaration within a source section.
#[derive(Clone, Copy)]
enum SectionEventCandidateKind {
    /// A type-like declaration that establishes a nominal concept.
    Nominal,
    /// A value declaration that supports a surrounding concept.
    Supporting,
}

/// Module declaration that may participate in the current section.
pub(super) struct SectionEventCandidate {
    /// Local definition represented by the declaration.
    def_id: rustc_hir::def_id::LocalDefId,
    /// Authored declaration name used for family inference.
    name: String,
    /// Complete declaration source range.
    span: Span,
    /// Whether this candidate introduces a nominal type.
    is_nominal_declaration: bool,
}

impl SectionEventCandidate {
    /// Converts a section-relevant module item into a source candidate.
    pub(super) fn from_item(cx: &LateContext<'_>, item: &Item<'_>) -> Option<Self> {
        // Ignore declarations whose source was synthesized by expansion.
        if item.span.from_expansion() || is_framework_generated_item(item) {
            return None;
        }

        // Classify concrete nominal declarations directly.
        if matches!(
            item.kind,
            ItemKind::Struct(..) | ItemKind::Enum(..) | ItemKind::Union(..)
        ) {
            return Some(Self::from_named_item(
                cx,
                item,
                SectionEventCandidateKind::Nominal,
            ));
        }

        // Classify abstract nominal declarations directly.
        if matches!(
            item.kind,
            ItemKind::TyAlias(..) | ItemKind::Trait(..) | ItemKind::TraitAlias(..)
        ) {
            return Some(Self::from_named_item(
                cx,
                item,
                SectionEventCandidateKind::Nominal,
            ));
        }

        // Classify value declarations and supporting inherent implementations.
        match item.kind {
            ItemKind::Const(..) | ItemKind::Static(..) | ItemKind::Fn { .. } => Some(
                Self::from_named_item(cx, item, SectionEventCandidateKind::Supporting),
            ),
            ItemKind::Impl(_) => Self::from_impl(cx, item),
            _ => None,
        }
    }

    /// Builds a candidate from an item whose definition and name are direct.
    fn from_named_item(
        cx: &LateContext<'_>,
        item: &Item<'_>,
        kind: SectionEventCandidateKind,
    ) -> Self {
        Self {
            def_id: item.owner_id.def_id,
            name: cx.tcx.item_name(item.owner_id.to_def_id()).to_string(),
            span: item.span,
            is_nominal_declaration: matches!(kind, SectionEventCandidateKind::Nominal),
        }
    }

    /// Converts a direct inherent implementation into a candidate for its self type.
    fn from_impl(cx: &LateContext<'_>, item: &Item<'_>) -> Option<Self> {
        // Resolve the implementation's nominal self type and local definition.
        let self_type = cx.tcx.type_of(item.owner_id).instantiate_identity();
        let ty::Adt(definition, _) = self_type.kind() else {
            return None;
        };
        let definition = definition.did().as_local()?;

        // Represent the supporting implementation under its nominal type's family name.
        Some(Self {
            def_id: definition,
            name: cx.tcx.item_name(definition.to_def_id()).to_string(),
            span: item.span,
            is_nominal_declaration: false,
        })
    }
}

// -----------------------------------------------------------------------------
// SectionEventStream: Ordered section reduction
// -----------------------------------------------------------------------------

/// One divider or declaration in the module's source-ordered event stream.
pub(super) enum SectionEventStreamEntry {
    /// A recognized divider template starts a new section.
    Divider(
        /// Divider source and captured content that open the section.
        SectionEventDivider,
    ),
    /// A declaration belongs to the nearest preceding divider, if any.
    Candidate(
        /// Authored declaration associated with the active section.
        SectionEventCandidate,
    ),
}

impl SectionEventStreamEntry {
    /// Returns the event's source position for stable ordering.
    pub(super) fn position(&self) -> BytePos {
        match self {
            Self::Divider(divider) => divider.span.lo(),
            Self::Candidate(participant) => participant.span.lo(),
        }
    }
}

#[derive(Default)]
/// Declarations accumulated before the next divider boundary.
pub(super) struct SectionEventStreamCandidates(
    /// Authored candidates retained in source order.
    Vec<SectionEventCandidate>,
);

impl SectionEventStreamCandidates {
    /// Collapses a declaration and its impls into one nominal participant.
    fn record_distinct_declaration(
        positions: &mut HashMap<rustc_hir::def_id::LocalDefId, usize>,
        declarations: &mut Vec<SectionParticipant>,
        candidate: SectionParticipant,
    ) {
        let Some(index) = positions.get(&candidate.def_id).copied() else {
            positions.insert(candidate.def_id, declarations.len());
            declarations.push(candidate);
            return;
        };
        if !candidate.is_nominal || declarations[index].is_nominal {
            return;
        }
        declarations[index] = candidate;
    }

    /// Returns whether the group contains no declarations.
    pub(super) const fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// Returns candidate names in their authored order.
    pub(super) fn names(&self) -> Vec<&str> {
        self.0
            .iter()
            .map(|participant| participant.name.as_str())
            .collect()
    }

    /// Formats a stable, deduplicated set of candidate names for diagnostics.
    pub(super) fn formatted_names(&self) -> String {
        // Stabilize and deduplicate authored participant names.
        let mut names = self.names();
        names.sort_unstable();
        names.dedup();

        // Quote and join the stable set for diagnostic prose.
        let quoted_names = names
            .into_iter()
            .map(|name| format!("`{name}`"))
            .collect::<Vec<_>>();
        quoted_names.join(", ")
    }

    /// Collapses supporting impls and returns distinct declarations in source order.
    pub(super) fn distinct_declarations(&self) -> Vec<SectionParticipant> {
        let mut positions = HashMap::<rustc_hir::def_id::LocalDefId, usize>::new();
        let mut declarations = Vec::<SectionParticipant>::new();
        for participant in &self.0 {
            let candidate = SectionParticipant {
                def_id: participant.def_id,
                name: participant.name.clone(),
                span: participant.span,
                is_nominal: participant.is_nominal_declaration,
            };
            Self::record_distinct_declaration(&mut positions, &mut declarations, candidate);
        }

        // Return declarations in authored order after collapsing their impls.
        declarations.sort_unstable_by_key(|participant| participant.span.lo());
        declarations
    }

    /// Removes all accumulated declarations after their group has been handled.
    fn clear(&mut self) {
        self.0.clear();
    }

    /// Appends one declaration in source order.
    fn push(&mut self, participant: SectionEventCandidate) {
        self.0.push(participant);
    }

    /// Returns whether the accumulated declarations require an authored section.
    fn requires_divider(&self) -> bool {
        self.0.len() > 1
            || self
                .0
                .iter()
                .any(|participant| participant.is_nominal_declaration)
    }

    /// Produces naming-first guidance for a declaration group without a divider.
    fn missing_guidance(&self) -> String {
        let names = self.names();

        // Turn the inferred prefix, or its absence, into naming-first guidance.
        identifier_case::longest_common_pascal_prefix(&names).map_or_else(
            || format!(
                "reconsider the names {} so related declarations share a visible prefix, then add a divider; create separate sections only for independent concepts",
                self.formatted_names()
            ),
            |prefix| format!(
                "add a divider for `{prefix}`, after first checking whether any outlier should be renamed into that family"
            ),
        )
    }

    /// Builds the missing-divider finding for this nonempty declaration group.
    fn missing_finding(&self) -> SectionFinding {
        // Prefer an inferred family prefix while keeping naming guidance actionable.
        let guidance = self.missing_guidance();
        SectionFinding {
            span: self.0[0].span,
            message: "module declarations are not covered by a section divider".to_owned(),
            help: guidance,
            replacement: None,
        }
    }
}

/// Divider and declarations accumulated beneath it.
pub(super) struct SectionEventStreamGroup {
    /// Divider that opened the section.
    pub(super) divider: SectionEventDivider,
    /// Declarations governed by the divider.
    pub(super) participants: SectionEventStreamCandidates,
}

#[derive(Default)]
/// Mutable reducer state for a module's section event stream.
pub(super) struct SectionEventStreamState {
    /// Findings and valid sections accumulated so far.
    pub(super) analysis: SectionAnalysis,
    /// Declarations encountered before any active divider.
    pub(super) uncovered: SectionEventStreamCandidates,
    /// Divider group currently accepting declarations.
    pub(super) current: Option<SectionEventStreamGroup>,
    /// First source span associated with each previously used prefix.
    pub(super) seen_prefixes: HashMap<String, Span>,
    /// Semantic namespace inherited from the containing module.
    pub(super) namespace: Option<ModuleNamespace>,
}

impl SectionEventStreamState {
    /// Routes a declaration into the active section or the uncovered group.
    fn record_candidate(&mut self, participant: SectionEventCandidate) {
        let Some(section) = &mut self.current else {
            self.uncovered.push(participant);
            return;
        };
        section.participants.push(participant);
    }

    /// Reports and clears declarations accumulated outside a section.
    fn record_uncovered(&mut self) {
        if self.uncovered.requires_divider() {
            self.analysis.missing.push(self.uncovered.missing_finding());
        }
        self.uncovered.clear();
    }

    /// Closes the final groups and returns the completed analysis.
    pub(super) fn finish(mut self, analyzer: &SectionAnalyzer) -> SectionAnalysis {
        // Preserve declarations that appeared outside any authored section.
        self.record_uncovered();
        let Some(section) = self.current.take() else {
            return self.analysis;
        };

        // Close the final authored section after the event stream ends.
        ModuleAnalysis::finish_section(
            analyzer,
            &section,
            self.namespace.as_ref(),
            &mut self.seen_prefixes,
            &mut self.analysis,
        );

        // Return every finding and validated section accumulated by the reducer.
        self.analysis
    }

    /// Closes preceding groups and starts the section introduced by `divider`.
    fn start_section(&mut self, analyzer: &SectionAnalyzer, divider: SectionEventDivider) {
        self.record_uncovered();

        // Finish the preceding section before installing its successor.
        if let Some(section) = self.current.take() {
            ModuleAnalysis::finish_section(
                analyzer,
                &section,
                self.namespace.as_ref(),
                &mut self.seen_prefixes,
                &mut self.analysis,
            );
        }
        self.current = Some(SectionEventStreamGroup {
            divider,
            participants: SectionEventStreamCandidates::default(),
        });
    }

    /// Applies one source-ordered event to the reducer state.
    pub(super) fn apply(&mut self, analyzer: &SectionAnalyzer, event: SectionEventStreamEntry) {
        match event {
            SectionEventStreamEntry::Candidate(participant) => self.record_candidate(participant),
            SectionEventStreamEntry::Divider(divider) => self.start_section(analyzer, divider),
        }
    }
}
