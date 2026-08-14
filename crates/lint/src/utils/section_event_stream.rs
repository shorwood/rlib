extern crate rustc_hir;
extern crate rustc_lint;
extern crate rustc_middle;
extern crate rustc_span;

use std::collections::HashMap;

use rustc_hir::def_id::LocalDefId;
use rustc_hir::{Item, ItemKind};
use rustc_lint::{LateContext, LintContext};
use rustc_middle::ty;
use rustc_span::{BytePos, Span};

use super::{ModuleAnalysis, ModuleNamespace};
use crate::utils::identifier_case;
use crate::utils::section_analysis::{
    SectionAnalysis, SectionAnalyzer, SectionEventDivider, SectionFinding, SectionParticipant,
};
use crate::utils::source_provenance::ItemProvenanceExt;

/// Smallest declaration group that can contain more than one responsibility.
const MINIMUM_MULTIPLE_FAMILY_SIZE: usize = 2;

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
    /// An in-source test module that always needs an explicit boundary.
    TestModule,
}

/// Module declaration that may participate in the current section.
pub(super) struct SectionEventCandidate {
    /// Local definition represented by the declaration.
    def_id: Option<LocalDefId>,
    /// Authored declaration name used for family inference.
    name: String,
    /// Complete declaration source range.
    span: Span,
    /// Whether this `candidate` introduces a nominal type.
    is_nominal_declaration: bool,
    /// Whether this declaration requires a divider even when it stands alone.
    is_standalone_divider_required: bool,
    /// Whether authored macro syntax hides the declarations produced by expansion.
    is_opaque_macro: bool,
}

impl SectionEventCandidate {
    /// Converts a section-relevant module item into a source `candidate`.
    pub(super) fn from_item(cx: &LateContext<'_>, item: &Item<'_>) -> Option<Self> {
        // Ignore declarations whose source was synthesized by expansion.
        if item.span.from_expansion() || item.is_framework_generated() {
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

        // Give every active in-source test module an explicit source boundary.
        if cx.sess().opts.test
            && matches!(item.kind, ItemKind::Mod(..))
            && item
                .kind
                .ident()
                .is_some_and(|ident| matches!(ident.name.as_str(), "test" | "tests"))
        {
            return Some(Self::from_named_item(
                cx,
                item,
                SectionEventCandidateKind::TestModule,
            ));
        }

        // Classify value declarations and supporting inherent implementations.
        match item.kind {
            ItemKind::Const(..)
            | ItemKind::Static(..)
            | ItemKind::Fn { .. }
            | ItemKind::Macro(..) => Some(Self::from_named_item(
                cx,
                item,
                SectionEventCandidateKind::Supporting,
            )),
            ItemKind::Impl(_) => Self::from_impl(cx, item),
            _ => None,
        }
    }

    /// Represents one authored item-position macro whose expansion is absent from HIR.
    pub(super) const fn from_macro_invocation(name: String, span: Span) -> Self {
        Self {
            def_id: None,
            name,
            span,
            is_nominal_declaration: false,
            is_standalone_divider_required: false,
            is_opaque_macro: true,
        }
    }

    /// Builds a `candidate` from an item whose definition and name are direct.
    fn from_named_item(
        cx: &LateContext<'_>,
        item: &Item<'_>,
        kind: SectionEventCandidateKind,
    ) -> Self {
        Self {
            def_id: Some(item.owner_id.def_id),
            name: cx.tcx.item_name(item.owner_id.to_def_id()).to_string(),
            span: item.span,
            is_nominal_declaration: matches!(kind, SectionEventCandidateKind::Nominal),
            is_standalone_divider_required: matches!(kind, SectionEventCandidateKind::TestModule),
            is_opaque_macro: false,
        }
    }

    /// Converts a direct inherent implementation into a `candidate` for its self type.
    fn from_impl(cx: &LateContext<'_>, item: &Item<'_>) -> Option<Self> {
        // Resolve the implementation's nominal self type and local definition.
        let self_type = cx.tcx.type_of(item.owner_id).instantiate_identity();
        let ty::Adt(definition, _) = self_type.kind() else {
            return None;
        };
        let definition = definition.did().as_local()?;

        // Represent the supporting implementation under its nominal type's family name.
        Some(Self {
            def_id: Some(definition),
            name: cx.tcx.item_name(definition.to_def_id()).to_string(),
            span: item.span,
            is_nominal_declaration: false,
            is_standalone_divider_required: false,
            is_opaque_macro: false,
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
/// Declarations accumulated before the next divider boundary.
#[derive(Default)]
pub(super) struct SectionEventStreamCandidates(
    /// Authored candidates retained in source order.
    Vec<SectionEventCandidate>,
);

impl SectionEventStreamCandidates {
    /// Collapses a declaration and its impls into one nominal participant.
    fn record_distinct_declaration(
        positions: &mut HashMap<LocalDefId, usize>,
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

    /// Collapses supporting impls and returns distinct declarations in source order.
    pub(super) fn distinct_declarations(&self) -> Vec<SectionParticipant> {
        let mut positions = HashMap::<LocalDefId, usize>::new();
        let mut declarations = Vec::<SectionParticipant>::new();
        for participant in &self.0 {
            let Some(def_id) = participant.def_id else {
                continue;
            };
            let candidate = SectionParticipant {
                def_id,
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

    /// Returns nominal declaration names, falling back to supporting declarations for helpers.
    pub(super) fn family_names(&self) -> Vec<&str> {
        let nominal = self
            .0
            .iter()
            .filter(|participant| !participant.is_opaque_macro)
            .filter(|participant| participant.is_nominal_declaration)
            .map(|participant| participant.name.as_str())
            .collect::<Vec<_>>();
        if nominal.is_empty() {
            self.0
                .iter()
                .filter(|participant| !participant.is_opaque_macro)
                .map(|participant| participant.name.as_str())
                .collect()
        } else {
            nominal
        }
    }

    /// Returns whether every participant is opaque authored macro syntax.
    pub(super) fn contains_only_opaque_macros(&self) -> bool {
        !self.0.is_empty() && self.0.iter().all(|participant| participant.is_opaque_macro)
    }

    /// Formats the nominal concepts that determine section responsibility.
    pub(super) fn formatted_family_names(&self) -> String {
        let mut names = self.family_names();
        names.sort_unstable();
        names.dedup();
        names
            .into_iter()
            .map(|name| format!("`{name}`"))
            .collect::<Vec<_>>()
            .join(", ")
    }

    /// Returns whether the declarations expose several independent naming families.
    pub(super) fn has_multiple_conceptual_families(
        &self,
        namespace: Option<&ModuleNamespace>,
    ) -> bool {
        let names = self.family_names();
        if names.len() < MINIMUM_MULTIPLE_FAMILY_SIZE {
            return false;
        }
        let Some(prefix) = identifier_case::longest_common_pascal_prefix(&names) else {
            return true;
        };
        namespace.is_some_and(|namespace| namespace.contains(&prefix))
    }

    /// Returns whether all authored declarations reveal several naming families.
    ///
    /// Prefix diagnostics privilege nominal declarations because values often support a nearby
    /// type. Capacity analysis cannot do that: otherwise one nominal type could hide an
    /// arbitrarily unrelated value-level API in an oversized section.
    pub(super) fn has_multiple_declaration_families(
        &self,
        namespace: Option<&ModuleNamespace>,
    ) -> bool {
        let names = self
            .0
            .iter()
            .filter(|participant| !participant.is_opaque_macro)
            .map(|participant| participant.name.as_str())
            .collect::<Vec<_>>();
        if names.len() < MINIMUM_MULTIPLE_FAMILY_SIZE {
            return false;
        }
        let Some(prefix) = identifier_case::longest_common_pascal_prefix(&names) else {
            return true;
        };
        namespace.is_some_and(|namespace| namespace.contains(&prefix))
    }

    /// Returns `candidate` names in their authored order.
    fn names(&self) -> Vec<&str> {
        self.0
            .iter()
            .map(|participant| participant.name.as_str())
            .collect()
    }

    /// Formats a stable, deduplicated set of `candidate` names for diagnostics.
    fn formatted_names(&self) -> String {
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

    /// Removes all accumulated declarations after their group has been handled.
    fn clear(&mut self) {
        self.0.clear();
    }

    /// Appends one declaration in source order.
    fn push(&mut self, participant: SectionEventCandidate) {
        self.0.push(participant);
    }

    /// Returns whether this group needs explicit conceptual navigation.
    fn requires_divider(
        &self,
        max_declarations_per_section: usize,
        namespace: Option<&ModuleNamespace>,
    ) -> bool {
        self.distinct_declarations().len() > max_declarations_per_section
            || self.has_multiple_conceptual_families(namespace)
            || self
                .0
                .iter()
                .any(|participant| participant.is_standalone_divider_required)
    }

    /// Produces naming-first guidance for a declaration group without a divider.
    fn missing_guidance(&self, namespace: Option<&ModuleNamespace>) -> String {
        if self.has_multiple_conceptual_families(namespace) {
            return format!(
                "add responsibility-based sections for the independently named concepts {}",
                self.formatted_family_names()
            );
        }
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
    fn missing_finding(&self, namespace: Option<&ModuleNamespace>) -> SectionFinding {
        // Prefer an inferred family prefix while keeping naming guidance actionable.
        let guidance = self.missing_guidance(namespace);
        let message = if self.has_multiple_conceptual_families(namespace) {
            "independently named module concepts are not separated by section dividers"
        } else {
            "module declarations are not covered by a section divider"
        };
        SectionFinding {
            span: self.0[0].span,
            message: message.to_owned(),
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
/// Mutable reducer state for a module's section event stream.
#[derive(Default)]
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
    fn record_uncovered(&mut self, analyzer: &SectionAnalyzer) {
        if self.uncovered.requires_divider(
            analyzer.max_declarations_per_section,
            self.namespace.as_ref(),
        ) {
            self.analysis
                .missing
                .push(self.uncovered.missing_finding(self.namespace.as_ref()));
        }
        self.uncovered.clear();
    }

    /// Closes the final groups and returns the completed analysis.
    pub(super) fn finish(mut self, analyzer: &SectionAnalyzer) -> SectionAnalysis {
        // Preserve declarations that appeared outside any authored section.
        self.record_uncovered(analyzer);
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
        self.record_uncovered(analyzer);

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
