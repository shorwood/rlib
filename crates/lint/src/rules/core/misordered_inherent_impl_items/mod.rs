extern crate rustc_errors;
extern crate rustc_hir;
extern crate rustc_span;

use std::borrow::Cow;

use rustc_errors::{Applicability, DiagDecorator};
use rustc_hir::{HirId, ImplItem, ImplItemKind, Item, ItemKind};
use rustc_lint::{LateContext, LateLintPass, LintContext};
use rustc_span::Span;

use crate::utils::declaration_node::{
    DeclarationConstraints, DeclarationNode, DeclarationNodeList, DeclarationSource,
};
use crate::utils::diagnostic::LateViolation;
use crate::utils::impl_item_dependencies::DependenciesExt;
use crate::utils::reorder_declarations::{DeclarationOrder, DeclarationOrderEdit};

// -----------------------------------------------------------------------------
// Violation: Misordered inherent implementation diagnostic
// -----------------------------------------------------------------------------

/// Inherent impl whose complete dependency-first order has been resolved.
struct Violation {
    /// Implementation node used to anchor the lint level.
    hir_id: HirId,
    /// First associated item that differs from dependency-first order.
    span: Span,
    /// Complete resolved associated-item order.
    expected_names: String,
    /// Atomic reorder edits when all source ranges are safely owned.
    edits: Option<Vec<DeclarationOrderEdit>>,
}

impl LateViolation for Violation {
    fn primary_message(&self) -> Cow<'_, str> {
        Cow::Borrowed("inherent impl items are not in dependency-first order")
    }

    fn rationale_message(&self) -> Cow<'_, str> {
        Cow::Owned(format!(
            "the first misplaced item forces readers to search forward; the resolved dependency-first order is {}",
            self.expected_names
        ))
    }

    fn remediation_message(&self) -> Cow<'_, str> {
        Cow::Owned(format!(
            "move associated items into this order: {}",
            self.expected_names
        ))
    }

    fn emit(self, cx: &LateContext<'_>) {
        let rationale = self.rationale_message().into_owned();
        let remediation = self.remediation_message().into_owned();
        cx.tcx.emit_node_span_lint(
            MISORDERED_INHERENT_IMPL_ITEMS,
            self.hir_id,
            self.span,
            DiagDecorator(|diag| {
                diag.primary_message(self.primary_message().into_owned());
                diag.span_label(self.span, "first item out of order");
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
// MisorderedInherentImplItems: Dependency-first associated API layout
// -----------------------------------------------------------------------------

/// Late lint pass that dependency-orders items inside direct inherent impls.
struct MisorderedInherentImplItems;

crate::impl_late_lint! {
    #[doc = include_str!("README.md")]
    pub MISORDERED_INHERENT_IMPL_ITEMS,
    Warn,
    "enforces dependency-first ordering within inherent impl blocks",
    MisorderedInherentImplItems
}

impl MisorderedInherentImplItems {
    /// Returns whether the resolved return type contains the inherent impl's self type.
    fn returns_self_type(cx: &LateContext<'_>, item: &ImplItem<'_>) -> bool {
        // Non-function associated items have no return type to classify.
        let ImplItemKind::Fn(signature, _) = item.kind else {
            return false;
        };

        // An implicit unit return cannot contain the implementation's self type.
        if matches!(signature.decl.output, rustc_hir::FnRetTy::DefaultReturn(_)) {
            return false;
        }

        // Resolve aliases and authored `Self` syntax to the same semantic type identity.
        let impl_def_id = cx.tcx.local_parent(item.owner_id.def_id);
        let self_type = cx.tcx.type_of(impl_def_id).instantiate_identity();
        let output = cx
            .tcx
            .fn_sig(item.owner_id.def_id)
            .instantiate_identity()
            .skip_binder()
            .output();
        output
            .walk()
            .any(|argument| argument.as_type() == Some(self_type))
    }

    /// Assigns the associated-item tie-break rank used after dependencies.
    fn category(cx: &LateContext<'_>, item: &ImplItem<'_>) -> u8 {
        match item.kind {
            ImplItemKind::Type(_) => 0,
            ImplItemKind::Const(..) => 1,
            ImplItemKind::Fn(signature, _) => {
                if signature.decl.implicit_self.has_implicit_self() {
                    4
                } else if Self::returns_self_type(cx, item) {
                    2
                } else {
                    3
                }
            }
        }
    }

    /// Compares source and dependency order, then emits an atomic reorder when safe.
    fn emit_if_needed(
        cx: &LateContext<'_>,
        implementation: &Item<'_>,
        nodes: &DeclarationNodeList,
    ) {
        // Stop when the authored order already matches dependency order.
        let ordering = nodes.declaration_order();

        // An identity permutation needs no diagnostic or associated-item movement.
        if ordering.iter().copied().eq(0..nodes.len()) {
            return;
        }

        // Describe the first mismatch and the complete expected order.
        let source = (0..nodes.len()).collect::<Vec<_>>();
        let mismatch = DeclarationNodeList::first_mismatch(&source, &ordering);
        let expected_names = nodes.formatted_names(&ordering);

        // Offer an atomic edit only when no associated item carries attributes.
        let editable = nodes.has_only_unattributed_definitions(cx);
        let edits = editable
            .then(|| DeclarationOrder::edits(cx, nodes, &ordering))
            .flatten();

        // Keep the diagnostic useful even when source ownership blocks a safe rewrite.
        Violation {
            hir_id: implementation.hir_id(),
            span: nodes[mismatch].source.span,
            expected_names,
            edits,
        }
        .emit(cx);
    }
}

impl<'tcx> LateLintPass<'tcx> for MisorderedInherentImplItems {
    fn check_item(&mut self, cx: &LateContext<'tcx>, item: &'tcx Item<'tcx>) {
        // Retain authored inherent implementation blocks only.
        let ItemKind::Impl(implementation) = item.kind else {
            return;
        };

        // Trait implementations and generated blocks follow contracts outside inherent ordering.
        if implementation.of_trait.is_some() || item.span.in_external_macro(cx.sess().source_map())
        {
            return;
        }

        // Resolve associated items that remain under direct source control.
        let resolved = implementation
            .items
            .iter()
            .map(|id| cx.tcx.hir_impl_item(*id));
        let items = resolved
            .filter(|item| !item.span.in_external_macro(cx.sess().source_map()))
            .collect::<Vec<_>>();

        // Model every associated item with its ordering category and dependencies.
        let nodes = items
            .iter()
            .map(|item| DeclarationNode {
                source: DeclarationSource {
                    defs: vec![item.owner_id.def_id],
                    name: item.ident.name.to_string(),
                    span: item.span,
                    section: 0,
                },
                constraints: DeclarationConstraints {
                    category: Self::category(cx, item),
                    is_outward_visible: item.vis_span().is_some_and(|span| !span.is_empty()),
                    dependencies: item.dependencies(cx.tcx),
                },
            })
            .collect::<Vec<_>>();

        // Compare the authored implementation against its dependency-first order.
        Self::emit_if_needed(cx, item, &DeclarationNodeList::new(nodes));
    }
}
