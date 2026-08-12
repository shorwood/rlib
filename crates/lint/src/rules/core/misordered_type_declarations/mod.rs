extern crate rustc_errors;
extern crate rustc_hir;
extern crate rustc_span;

use std::borrow::Cow;

use rustc_errors::{Applicability, DiagDecorator};
use rustc_hir::{HirId, Item, ItemKind, Mod};
use rustc_lint::{LateContext, LateLintPass, LintContext};
use rustc_span::Span;

use crate::utils::declaration_node::{
    DeclarationConstraints, DeclarationNode, DeclarationNodeList, DeclarationSource,
};
use crate::utils::diagnostic::LateViolation;
use crate::utils::item_dependencies::DependenciesExt;
use crate::utils::reorder_declarations::{DeclarationOrder, DeclarationOrderEdit};
use crate::utils::section_analysis::SectionAnalyzer;
use crate::utils::source_provenance::ItemProvenanceExt;

// -----------------------------------------------------------------------------
// Violation: Misordered type declaration diagnostic
// -----------------------------------------------------------------------------

/// Type declarations with a resolved dependency-first permutation.
struct Violation {
    /// Module node used to anchor the lint level.
    hir_id: HirId,
    /// First type group that differs from dependency-first order.
    span: Span,
    /// Complete resolved type-group order.
    expected_names: String,
    /// Atomic reorder edits when all source ranges are safely owned.
    edits: Option<Vec<DeclarationOrderEdit>>,
}

impl LateViolation for Violation {
    fn primary_message(&self) -> Cow<'_, str> {
        Cow::Borrowed("local types should be declared before their use")
    }

    fn rationale_message(&self) -> Cow<'_, str> {
        Cow::Owned(format!(
            "the first misplaced type requires forward navigation; the resolved dependency-first order is {}",
            self.expected_names
        ))
    }

    fn remediation_message(&self) -> Cow<'_, str> {
        Cow::Owned(format!(
            "move declarations into this order: {}",
            self.expected_names
        ))
    }

    fn emit(self, cx: &LateContext<'_>) {
        // Render the stable explanation before moving optional source edits.
        let rationale = self.rationale_message().into_owned();
        let remediation = self.remediation_message().into_owned();

        // Emit after resolving whether the exact order can be applied atomically.
        cx.tcx.emit_node_span_lint(
            MISORDERED_TYPE_DECLARATIONS,
            self.hir_id,
            self.span,
            DiagDecorator(|diag| {
                diag.primary_message(self.primary_message().into_owned());
                diag.span_label(self.span, "this declaration is the first out of order");
                diag.note(rationale);
                if let Some(edits) = self.edits {
                    diag.multipart_suggestion(
                        remediation,
                        edits
                            .into_iter()
                            .map(|edit| (edit.span, edit.replacement))
                            .collect(),
                        Applicability::MachineApplicable,
                    );
                } else {
                    diag.help(remediation);
                }
            }),
        );
    }
}

// -----------------------------------------------------------------------------
// MisorderedTypeDeclarations
// -----------------------------------------------------------------------------

/// Late lint pass that dependency-orders nominal types within authored sections.
struct MisorderedTypeDeclarations {
    /// Shared source-section analyzer used to preserve authored boundaries.
    sections: SectionAnalyzer,
}

impl MisorderedTypeDeclarations {
    /// Builds the pass from the configured section-divider policy.
    fn new() -> Self {
        Self {
            sections: SectionAnalyzer::from_config(),
        }
    }
}

dylint_linting::impl_late_lint! {
    #[doc = include_str!("README.md")]
    pub MISORDERED_TYPE_DECLARATIONS,
    Warn,
    "enforces dependency-first ordering of local type declarations",
    MisorderedTypeDeclarations::new()
}

impl MisorderedTypeDeclarations {
    /// Returns whether an item is a nominal type declaration.
    const fn is_type(item: &Item<'_>) -> bool {
        // Classify concrete nominal declarations.
        let is_concrete = matches!(
            item.kind,
            ItemKind::Struct(..) | ItemKind::Enum(..) | ItemKind::Union(..)
        );

        // Classify abstract nominal declarations before combining both groups.
        let is_abstract = matches!(
            item.kind,
            ItemKind::TyAlias(..) | ItemKind::Trait(..) | ItemKind::TraitAlias(..)
        );
        is_concrete || is_abstract
    }

    /// Returns whether an item directly implements the expected local type.
    fn is_direct_impl_of(
        cx: &LateContext<'_>,
        item: &Item<'_>,
        expected: rustc_hir::def_id::LocalDefId,
    ) -> bool {
        if !matches!(item.kind, ItemKind::Impl(_)) {
            return false;
        }
        let ty = cx.tcx.type_of(item.owner_id).instantiate_identity();
        ty.ty_adt_def()
            .is_some_and(|definition| definition.did().as_local() == Some(expected))
    }

    /// Compares source and dependency order, then emits an atomic reorder when safe.
    fn emit_if_needed(cx: &LateContext<'_>, nodes: &DeclarationNodeList, hir_id: HirId) {
        // Stop when the authored order already matches dependency order.
        let ordering = nodes.declaration_order();
        let source = (0..nodes.len()).collect::<Vec<_>>();
        if ordering == source {
            return;
        }

        // Describe the first mismatch and the complete expected order.
        let mismatch = DeclarationNodeList::first_mismatch(&source, &ordering);
        let expected_names = nodes.formatted_names(&ordering);

        // Offer an atomic edit only when no declaration in a group carries attributes.
        let editable = nodes.has_only_unattributed_definitions(cx);
        let edits = editable
            .then(|| DeclarationOrder::edits(cx, nodes, &ordering))
            .flatten();

        // Keep the diagnostic useful even when comments or macros block a safe rewrite.
        Violation {
            hir_id,
            span: nodes[mismatch].source.span,
            expected_names,
            edits,
        }
        .emit(cx);
    }
}

impl<'tcx> LateLintPass<'tcx> for MisorderedTypeDeclarations {
    fn check_mod(&mut self, cx: &LateContext<'tcx>, module: &'tcx Mod<'tcx>, hir_id: HirId) {
        // Resolve authored items and section membership before selecting local types.
        let source_map = cx.sess().source_map();
        let sections = self.sections.analyze(cx, module, hir_id);
        let resolved = module.item_ids.iter().map(|id| cx.tcx.hir_item(*id));
        let items = resolved
            .filter(|item| {
                !item.span.in_external_macro(source_map) && !item.is_framework_generated()
            })
            .collect::<Vec<_>>();

        // Treat each type and its directly following impls as one movable declaration.
        let mut nodes = Vec::new();
        for (index, item) in items.iter().enumerate() {
            // Retain nominal declarations and initialize their movable source group.
            if !Self::is_type(item) {
                continue;
            }
            let mut span = item.span;
            let mut defs = vec![item.owner_id.def_id];

            // Extend the group through directly following inherent implementations.
            let following_impls = items[index + 1..].iter().take_while(|following| {
                Self::is_direct_impl_of(cx, following, item.owner_id.def_id)
            });
            for following in following_impls {
                span = span.with_hi(following.span.hi());
                defs.push(following.owner_id.def_id);
            }

            // Materialize one dependency node for the complete type declaration group.
            let source = DeclarationSource {
                defs,
                name: cx.tcx.item_name(item.owner_id.to_def_id()).to_string(),
                span,
                section: sections.section_ordinal_for_span(item.span).unwrap_or(0),
            };

            // Attach neutral tie-breaks and semantic dependencies to the source identity.
            let constraints = DeclarationConstraints {
                category: 0,
                // Visibility is intentionally neutral: independent types retain authored order.
                is_outward_visible: false,
                dependencies: item.dependencies(cx.tcx),
            };

            // Combine the source identity and ordering constraints as one movable node.
            nodes.push(DeclarationNode {
                source,
                constraints,
            });
        }
        Self::emit_if_needed(cx, &DeclarationNodeList::new(nodes), hir_id);
    }
}
