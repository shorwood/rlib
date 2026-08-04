extern crate rustc_errors;
extern crate rustc_hir;

use rustc_errors::{Applicability, DiagDecorator};
use rustc_hir::{HirId, Item, ItemKind, Mod};
use rustc_lint::{LateContext, LateLintPass, LintContext};

use crate::utils::declaration_node::{DeclarationNode, DeclarationNodeList};
use crate::utils::item_dependencies::item_dependencies;
use crate::utils::reorder_declarations::reorder_declarations;

struct MisorderedTypeDeclarations;

dylint_linting::impl_late_lint! {
    /// ### What it does
    ///
    /// Requires local type declarations to appear before declarations that reference them.
    /// Recursive type groups are kept contiguous in their existing internal order.
    ///
    /// ### Why is this bad?
    ///
    /// Dependency-first type declarations can be read from top to bottom without searching ahead
    /// for each field or variant's definition. Preserving recursive groups avoids inventing an
    /// impossible order for types that depend on one another.
    ///
    /// For example, `Request` refers to a type declared later:
    ///
    /// ```rust
    /// struct Request(Headers);
    /// struct Headers;
    /// ```
    ///
    /// Put the dependency before the type that consumes it:
    ///
    /// ```rust
    /// struct Headers;
    /// struct Request(Headers);
    /// ```
    pub MISORDERED_TYPE_DECLARATIONS,
    Warn,
    "enforces dependency-first ordering of local type declarations",
    MisorderedTypeDeclarations
}

impl MisorderedTypeDeclarations {
    fn is_type(item: &Item<'_>) -> bool {
        matches!(
            item.kind,
            ItemKind::Struct(..)
                | ItemKind::Enum(..)
                | ItemKind::Union(..)
                | ItemKind::TyAlias(..)
                | ItemKind::Trait(..)
                | ItemKind::TraitAlias(..)
        )
    }

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

    fn emit_if_needed(cx: &LateContext<'_>, nodes: &DeclarationNodeList, hir_id: HirId) {
        let ordering = nodes.declaration_order();
        let source = (0..nodes.len()).collect::<Vec<_>>();
        if ordering == source {
            return;
        }
        let mismatch = source
            .iter()
            .zip(&ordering)
            .position(|(actual, expected)| actual != expected)
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
            MISORDERED_TYPE_DECLARATIONS,
            hir_id,
            nodes[mismatch].span,
            DiagDecorator(|diag| {
                diag.primary_message("local types should be declared before their use");
                diag.span_label(
                    nodes[mismatch].span,
                    "this declaration is the first out of order",
                );
                diag.note(format!("expected declaration order: {expected_names}"));
                if let Some(edits) = edits {
                    diag.multipart_suggestion(
                        "reorder these declarations",
                        edits,
                        Applicability::MachineApplicable,
                    );
                } else {
                    diag.help(format!(
                        "move declarations into this order: {expected_names}"
                    ));
                }
            }),
        );
    }
}

impl<'tcx> LateLintPass<'tcx> for MisorderedTypeDeclarations {
    fn check_mod(&mut self, cx: &LateContext<'tcx>, module: &'tcx Mod<'tcx>, hir_id: HirId) {
        let source_map = cx.sess().source_map();
        let items = module
            .item_ids
            .iter()
            .map(|id| cx.tcx.hir_item(*id))
            .filter(|item| !item.span.in_external_macro(source_map))
            .collect::<Vec<_>>();
        let mut nodes = Vec::new();
        for (index, item) in items.iter().enumerate() {
            if !Self::is_type(item) {
                continue;
            }
            let mut span = item.span;
            let mut defs = vec![item.owner_id.def_id];
            for following in &items[index + 1..] {
                if !Self::is_direct_impl_of(cx, following, item.owner_id.def_id) {
                    break;
                }
                span = span.with_hi(following.span.hi());
                defs.push(following.owner_id.def_id);
            }
            nodes.push(DeclarationNode {
                defs,
                name: cx.tcx.item_name(item.owner_id.to_def_id()).to_string(),
                span,
                category: 0,
                // Visibility is intentionally neutral: independent types retain authored order.
                is_outward_visible: false,
                dependencies: item_dependencies(cx.tcx, item),
            });
        }
        Self::emit_if_needed(cx, &DeclarationNodeList::new(nodes), hir_id);
    }
}
