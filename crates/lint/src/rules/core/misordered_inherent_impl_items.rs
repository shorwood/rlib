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
        // Render the stable explanation before moving optional source edits.
        let rationale = self.rationale_message().into_owned();
        let remediation = self.remediation_message().into_owned();

        // Emit after resolving whether the exact order can be applied atomically.
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
// MisorderedInherentImplItems
// -----------------------------------------------------------------------------

/// Late lint pass that dependency-orders items inside direct inherent impls.
struct MisorderedInherentImplItems;

dylint_linting::impl_late_lint! {
    /// ### What it does
    ///
    /// Orders each inherent impl dependency-first, then by associated item kind and visibility.
    /// Trait impls and other inherent impl blocks are independent.
    ///
    /// ### Why is this bad?
    ///
    /// A stable order makes an impl predictable to scan: supporting types and constants come
    /// first, followed by constructors, other associated functions, and methods. Dependency-first
    /// ordering also prevents a declaration from relying on details introduced later in the block.
    ///
    /// For example, this method appears before the constructor it uses:
    ///
    /// ```rust
    /// struct Session;
    ///
    /// impl Session {
    ///     fn reset(&mut self) { *self = Self::new(); }
    ///     fn new() -> Self { Self }
    /// }
    /// ```
    ///
    /// Put the constructor before the dependent method:
    ///
    /// ```rust
    /// struct Session;
    ///
    /// impl Session {
    ///     fn new() -> Self { Self }
    ///     fn reset(&mut self) { *self = Self::new(); }
    /// }
    /// ```
    pub MISORDERED_INHERENT_IMPL_ITEMS,
    Warn,
    "enforces dependency-first ordering within inherent impl blocks",
    MisorderedInherentImplItems
}

impl MisorderedInherentImplItems {
    /// Returns whether authored return syntax explicitly names `Self`.
    fn return_mentions_self(cx: &LateContext<'_>, output: rustc_hir::FnRetTy<'_>) -> bool {
        // Require an explicit authored return type before inspecting its source.
        let rustc_hir::FnRetTy::Return(ty) = output else {
            return false;
        };

        // Inspect identifier-like words in the authored return type.
        let source_map = cx.sess().source_map();
        let Ok(snippet) = source_map.span_to_snippet(ty.span) else {
            return false;
        };
        snippet
            .split(|character: char| !character.is_alphanumeric() && character != '_')
            .any(|word| word == "Self")
    }

    /// Assigns the associated-item tie-break rank used after dependencies.
    fn category(cx: &LateContext<'_>, item: &ImplItem<'_>) -> u8 {
        match item.kind {
            ImplItemKind::Type(_) => 0,
            ImplItemKind::Const(..) => 1,
            ImplItemKind::Fn(signature, _) => {
                if signature.decl.implicit_self.has_implicit_self() {
                    4
                } else if Self::return_mentions_self(cx, signature.decl.output) {
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
