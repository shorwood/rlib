extern crate rustc_errors;
extern crate rustc_hir;

use rustc_errors::{Applicability, DiagDecorator};
use rustc_hir::{ImplItem, ImplItemKind, Item, ItemKind};
use rustc_lint::{LateContext, LateLintPass, LintContext};

use crate::utils::declaration_node::{DeclarationNode, DeclarationNodeList};
use crate::utils::impl_item_dependencies::impl_item_dependencies;
use crate::utils::reorder_declarations::reorder_declarations;

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
    fn return_mentions_self(cx: &LateContext<'_>, output: rustc_hir::FnRetTy<'_>) -> bool {
        let rustc_hir::FnRetTy::Return(ty) = output else {
            return false;
        };
        cx.sess()
            .source_map()
            .span_to_snippet(ty.span)
            .is_ok_and(|snippet| {
                snippet
                    .split(|character: char| !character.is_alphanumeric() && character != '_')
                    .any(|word| word == "Self")
            })
    }

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

    fn emit_if_needed(
        cx: &LateContext<'_>,
        implementation: &Item<'_>,
        nodes: &DeclarationNodeList,
    ) {
        let ordering = nodes.declaration_order();
        if ordering.iter().copied().eq(0..nodes.len()) {
            return;
        }
        let mismatch = ordering
            .iter()
            .enumerate()
            .find_map(|(position, expected)| (position != *expected).then_some(position))
            .expect("different orders have a mismatch");
        let expected_names = ordering
            .iter()
            .map(|index| format!("`{}`", nodes[*index].name))
            .collect::<Vec<_>>()
            .join(", ");
        let editable = nodes.iter().all(|node| {
            node.defs.iter().all(|definition| {
                cx.tcx
                    .hir_attrs(rustc_hir::HirId::make_owner(*definition))
                    .is_empty()
            })
        });
        let edits = editable
            .then(|| reorder_declarations(cx, nodes, &ordering))
            .flatten();
        cx.tcx.emit_node_span_lint(
            MISORDERED_INHERENT_IMPL_ITEMS,
            implementation.hir_id(),
            nodes[mismatch].span,
            DiagDecorator(|diag| {
                diag.primary_message("inherent impl items are not in dependency-first order");
                diag.span_label(nodes[mismatch].span, "first item out of order");
                diag.note(format!("expected item order: {expected_names}"));
                if let Some(edits) = edits {
                    diag.multipart_suggestion(
                        "reorder these associated items",
                        edits,
                        Applicability::MachineApplicable,
                    );
                } else {
                    diag.help(format!(
                        "move associated items into this order: {expected_names}"
                    ));
                }
            }),
        );
    }
}

impl<'tcx> LateLintPass<'tcx> for MisorderedInherentImplItems {
    fn check_item(&mut self, cx: &LateContext<'tcx>, item: &'tcx Item<'tcx>) {
        let ItemKind::Impl(implementation) = item.kind else {
            return;
        };
        if implementation.of_trait.is_some() || item.span.in_external_macro(cx.sess().source_map())
        {
            return;
        }
        let items = implementation
            .items
            .iter()
            .map(|id| cx.tcx.hir_impl_item(*id))
            .filter(|item| !item.span.in_external_macro(cx.sess().source_map()))
            .collect::<Vec<_>>();
        let nodes = items
            .iter()
            .map(|item| DeclarationNode {
                defs: vec![item.owner_id.def_id],
                name: item.ident.name.to_string(),
                span: item.span,
                category: Self::category(cx, item),
                is_outward_visible: item.vis_span().is_some_and(|span| !span.is_empty()),
                dependencies: impl_item_dependencies(cx.tcx, item),
            })
            .collect::<Vec<_>>();
        Self::emit_if_needed(cx, item, &DeclarationNodeList::new(nodes));
    }
}
