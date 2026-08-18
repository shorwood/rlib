extern crate rustc_errors;
extern crate rustc_hir;
extern crate rustc_middle;
extern crate rustc_span;

use std::borrow::Cow;
use std::collections::{HashMap, HashSet};
use std::iter::once;

use rustc_errors::{Applicability, DiagDecorator};
use rustc_hir::def_id::LocalDefId;
use rustc_hir::intravisit::FnKind;
use rustc_hir::{Body, Expr, FnDecl, HirId, Item, ItemKind, Node};
use rustc_lint::{LateContext, LateLintPass, LintContext};
use rustc_middle::ty;
use rustc_span::Span;

use crate::utils::collection_construction_analysis::CollectionConstructionAnalysis;
use crate::utils::construction_analysis::{
    ConstructionAnalysis, ConstructionAnalysisModuleItem, ConstructionCandidate, ConstructionOrigin,
};
use crate::utils::conversion_analysis::ConversionAnalysis;
use crate::utils::diagnostic::LateViolation;

// -----------------------------------------------------------------------------
// Migration: Guarded constructor relocation
// -----------------------------------------------------------------------------

/// One source replacement in a constructor relocation.
struct MigrationEdit {
    /// Authored source range replaced by the edit.
    span: Span,
    /// Complete replacement text.
    replacement: String,
}

/// Complete source edits for one safe adjacent constructor relocation.
struct Migration {
    /// Function definition wrapped in an inherent implementation.
    definition: MigrationEdit,
    /// Resolved function references rewritten to associated paths.
    references: Vec<MigrationEdit>,
}

// -----------------------------------------------------------------------------
// Violation: Externally owned construction contract
// -----------------------------------------------------------------------------

/// Constructor-like free function and its precise ownership remediation.
struct Violation {
    /// HIR owner used for lint-level configuration.
    hir_id: HirId,
    /// Authored constructor identifier.
    span: Span,
    /// Free-function name shown in the diagnostic.
    function_name: String,
    /// Same-module type that should own construction.
    target_name: String,
    /// Safe mechanical relocation when every source precondition holds.
    migration: Option<Migration>,
}

impl Violation {
    /// Captures the exact `candidate` context and its available remediation.
    fn from_candidate(
        cx: &LateContext<'_>,
        candidate: &ConstructionCandidate,
        migration: Option<Migration>,
    ) -> Self {
        Self {
            hir_id: cx.tcx.local_def_id_to_hir_id(candidate.function.def_id),
            span: candidate.function.name_span,
            function_name: candidate.function.name.to_string(),
            target_name: candidate.target.name.to_string(),
            migration,
        }
    }
}

impl LateViolation for Violation {
    fn primary_message(&self) -> Cow<'_, str> {
        Cow::Owned(format!(
            "constructor-like free function `{}` creates `{}` outside its inherent impl",
            self.function_name, self.target_name
        ))
    }

    fn rationale_message(&self) -> Cow<'_, str> {
        Cow::Owned(format!(
            "`{}`'s construction contract is hidden in the module namespace instead of being discoverable through `{}`",
            self.function_name, self.target_name
        ))
    }

    fn remediation_message(&self) -> Cow<'_, str> {
        Cow::Owned(format!(
            "move `{}` into an inherent `impl {}`; reconsider whether its name describes the invariant or source concept instead of merely saying `new`, `build`, or `create`",
            self.function_name, self.target_name
        ))
    }

    fn emit(self, cx: &LateContext<'_>) {
        // Render every diagnostic layer before moving the optional migration.
        let primary = self.primary_message().into_owned();
        let rationale = self.rationale_message().into_owned();
        let remediation = self.remediation_message().into_owned();

        // Emit either the complete safe relocation or contextual manual guidance.
        cx.tcx.emit_node_span_lint(
            CONSTRUCTOR_LIKE_FREE_FUNCTIONS,
            self.hir_id,
            self.span,
            DiagDecorator(|diag| {
                diag.primary_message(primary);
                diag.note(rationale);
                if let Some(migration) = self.migration {
                    let edits = once(migration.definition)
                        .chain(migration.references)
                        .map(|edit| (edit.span, edit.replacement))
                        .collect();
                    diag.multipart_suggestion(remediation, edits, Applicability::MachineApplicable);
                } else {
                    diag.help(remediation);
                }
            }),
        );
    }
}

// -----------------------------------------------------------------------------
// ConstructorLikeFreeFunctions: Construction ownership policy
// -----------------------------------------------------------------------------
/// Collects proven construction functions and the references needed for safe relocation.
#[derive(Default)]
struct ConstructorLikeFreeFunctions {
    /// Shared semantic and source analyzer.
    constructions: ConstructionAnalysis,
    /// Conversion families used to defer canonical pairs to the stronger trait policy.
    conversions: ConversionAnalysis,
    /// Sequence-ingestion families deferred to standard collection traits.
    collections: CollectionConstructionAnalysis,
}

crate::impl_late_lint! {
    #[doc = include_str!("README.md")]
    pub CONSTRUCTOR_LIKE_FREE_FUNCTIONS,
    Warn,
    "requires same-module constructor functions to be inherent associated functions",
    ConstructorLikeFreeFunctions::default()
}

impl<'tcx> LateLintPass<'tcx> for ConstructorLikeFreeFunctions {
    fn check_item(&mut self, cx: &LateContext<'tcx>, item: &'tcx Item<'tcx>) {
        self.constructions.record_item(cx, item);
        self.conversions.record_item(cx, item);
        self.collections.record_item(cx, item);
    }

    fn check_expr(&mut self, cx: &LateContext<'tcx>, expression: &'tcx Expr<'tcx>) {
        self.constructions.record_expression(cx, expression);
    }

    fn check_fn(
        &mut self,
        cx: &LateContext<'tcx>,
        kind: FnKind<'tcx>,
        _: &'tcx FnDecl<'tcx>,
        body: &'tcx Body<'tcx>,
        span: Span,
        def_id: LocalDefId,
    ) {
        self.constructions
            .record_function(cx, kind, body, span, def_id);
        self.collections.record_function(cx, kind, body, def_id);

        // Only recognized construction functions contribute conversion evidence.
        let Some(candidate) = self.constructions.candidate(def_id) else {
            return;
        };
        self.conversions.record_function(cx, kind, body, candidate);
    }

    fn check_crate_post(&mut self, cx: &LateContext<'tcx>) {
        let parser_families = self.constructions.parser_families();
        let reportable_conversions = self.conversions.reportable_candidates();
        let conversion_definitions = reportable_conversions
            .into_iter()
            .map(|candidate| candidate.identity.def_id)
            .collect::<HashSet<_>>();
        let collection_definitions = self.collections.reportable_definitions();
        for candidate in &self.constructions.candidates {
            // Leave receiver-shaped functions and canonical parsers to their stronger rules.
            if !candidate.ownership.is_target_same_module
                || candidate.ownership.origin != ConstructionOrigin::Free
                || candidate.ownership.is_first_input_target
                || conversion_definitions.contains(&candidate.function.def_id)
                || collection_definitions.contains(&candidate.function.def_id)
            {
                continue;
            }
            if Self::parser_has_precedence(&self.constructions, &parser_families, candidate) {
                continue;
            }

            // Emit ownership guidance with a migration only when every source check succeeds.
            let migration = Self::migration(cx, &self.constructions, candidate);
            let violation = Violation::from_candidate(cx, candidate, migration);
            violation.emit(cx);
        }
    }
}

impl ConstructorLikeFreeFunctions {
    /// Resolves a direct inherent impl item to its local nominal target.
    fn inherent_impl_target(cx: &LateContext<'_>, def_id: LocalDefId) -> Option<LocalDefId> {
        // Require a direct inherent implementation before inspecting its self type.
        let Node::Item(item) = cx.tcx.hir_node_by_def_id(def_id) else {
            return None;
        };

        // Non-implementation items cannot supply an inherent construction owner.
        let ItemKind::Impl(implementation) = item.kind else {
            return None;
        };

        // Trait implementations follow an external ownership contract and cannot receive the move.
        if implementation.of_trait.is_some() {
            return None;
        }

        // Resolve aliases to the implementation's local nominal target.
        let self_type = cx.tcx.type_of(def_id).instantiate_identity();

        // Non-ADT self types have no nominal declaration that can own the constructor.
        let ty::Adt(definition, _) = self_type.kind() else {
            return None;
        };
        definition.did().as_local()
    }

    /// Requires the target declaration followed only by its own inherent impl group.
    fn preceding_target_item(
        cx: &LateContext<'_>,
        analysis: &ConstructionAnalysis,
        candidate: &ConstructionCandidate,
    ) -> Option<ConstructionAnalysisModuleItem> {
        let items = analysis.module_items.get(&candidate.function.module)?;
        let function_index = items
            .iter()
            .position(|item| item.def_id == candidate.function.def_id)?;
        let mut index = function_index.checked_sub(1)?;

        let preceding = items[index];
        while Self::inherent_impl_target(cx, items[index].def_id) == Some(candidate.target.def_id) {
            index = index.checked_sub(1)?;
        }
        (items[index].def_id == candidate.target.def_id).then_some(preceding)
    }

    /// Rejects comments or generated text between the adjacent owner group and function.
    fn gap_is_whitespace(cx: &LateContext<'_>, previous: Span, function: Span) -> bool {
        let gap = previous.with_hi(function.lo()).with_lo(previous.hi());
        let source_map = cx.sess().source_map();
        source_map
            .span_to_snippet(gap)
            .is_ok_and(|source| source.trim().is_empty())
    }

    /// Prevents a move from creating a duplicate associated-function name.
    fn has_name_collision(cx: &LateContext<'_>, candidate: &ConstructionCandidate) -> bool {
        let implementations = cx.tcx.inherent_impls(candidate.target.def_id);
        let associated = implementations.iter().flat_map(|implementation| {
            cx.tcx
                .associated_items(*implementation)
                .in_definition_order()
        });
        associated
            .into_iter()
            .any(|item| item.name() == candidate.function.name)
    }

    /// Returns whether a stronger unique-parser diagnostic owns this `candidate`.
    fn parser_has_precedence(
        analysis: &ConstructionAnalysis,
        families: &HashMap<LocalDefId, Vec<&ConstructionCandidate>>,
        candidate: &ConstructionCandidate,
    ) -> bool {
        families
            .get(&candidate.target.def_id)
            .is_some_and(|family| {
                family.len() == 1
                    && family[0].function.def_id == candidate.function.def_id
                    && candidate.has_unqualified_parser_name()
                    && !analysis.from_str_targets.contains(&candidate.target.def_id)
            })
    }

    /// Checks declaration-level conditions required for an automatic move.
    fn is_migration_source_safe(
        cx: &LateContext<'_>,
        analysis: &ConstructionAnalysis,
        candidate: &ConstructionCandidate,
    ) -> bool {
        // Separate declaration safety from cross-reference and destination safety.
        let source_is_safe = candidate.migration.is_private
            && !candidate.migration.has_attributes
            && !candidate.function.item_span.from_expansion();

        // Require a target that can own a nongeneric adjacent impl.
        let target_is_nongeneric = cx
            .tcx
            .generics_of(candidate.target.def_id)
            .own_params
            .is_empty();

        // Reject references mediated through import aliases.
        let is_not_imported = !analysis
            .imported_functions
            .contains(&candidate.function.def_id);

        // Require every independent migration precondition.
        source_is_safe
            && target_is_nongeneric
            && is_not_imported
            && !Self::has_name_collision(cx, candidate)
    }

    /// Builds same-file reference replacements or rejects the whole migration.
    fn reference_edits(
        cx: &LateContext<'_>,
        analysis: &ConstructionAnalysis,
        candidate: &ConstructionCandidate,
    ) -> Option<Vec<MigrationEdit>> {
        let source_map = cx.sess().source_map();
        let candidate_file = source_map.span_to_filename(candidate.function.item_span);
        let mut edits = Vec::new();
        let references = analysis.function_uses.get(&candidate.function.def_id);

        // Every reference must remain disjoint, authored, and in the definition's source file.
        for span in references.into_iter().flatten() {
            // Reject overlaps, expansions, and cross-file paths before building an edit.
            let overlaps_definition = candidate.function.item_span.contains(*span);

            // One unsafe reference invalidates the migration's all-or-nothing edit set.
            if overlaps_definition
                || span.from_expansion()
                || source_map.span_to_filename(*span) != candidate_file
            {
                return None;
            }

            // Preserve the exact authored path for an atomic associated-path rewrite.
            edits.push(MigrationEdit {
                span: *span,
                replacement: candidate.qualified_associated_path(cx),
            });
        }
        Some(edits)
    }

    /// Indents authored function source for insertion inside an inherent impl.
    fn indented_function(
        cx: &LateContext<'_>,
        candidate: &ConstructionCandidate,
    ) -> Option<String> {
        let source_map = cx.sess().source_map();

        // Missing authored function text prevents a complete mechanical relocation.
        let Ok(function) = source_map.span_to_snippet(candidate.function.item_span) else {
            return None;
        };
        let lines = function.lines().map(|line| format!("    {line}"));
        Some(lines.collect::<Vec<_>>().join("\n"))
    }

    /// Builds a migration only when adjacency and every reference are mechanically safe.
    fn migration(
        cx: &LateContext<'_>,
        analysis: &ConstructionAnalysis,
        candidate: &ConstructionCandidate,
    ) -> Option<Migration> {
        // Require private ordinary source next to the complete target declaration group.
        if !Self::is_migration_source_safe(cx, analysis, candidate) {
            return None;
        }
        let preceding = Self::preceding_target_item(cx, analysis, candidate)?;

        // Comments or other syntax in the ownership gap make automatic wrapping unsafe.
        if !Self::gap_is_whitespace(cx, preceding.span, candidate.function.item_span) {
            return None;
        }

        // Construct the definition edit only after every external reference is editable.
        let references = Self::reference_edits(cx, analysis, candidate)?;
        let function = Self::indented_function(cx, candidate)?;
        let target = candidate.target.name;
        let definition = MigrationEdit {
            span: candidate.function.item_span,
            replacement: format!("impl {target} {{\n{function}\n}}"),
        };

        // Return the definition and all references as one atomic migration.
        Some(Migration {
            definition,
            references,
        })
    }
}
