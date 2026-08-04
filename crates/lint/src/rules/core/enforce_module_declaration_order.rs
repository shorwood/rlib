extern crate rustc_errors;
extern crate rustc_hir;

use std::collections::HashSet;

use rustc_errors::{Applicability, DiagDecorator};
use rustc_hir::def_id::LocalDefId;
use rustc_hir::{HirId, Item, ItemKind, Mod};
use rustc_lint::{LateContext, LateLintPass, LintContext};

use crate::utils::declaration_node::{DeclarationNode, DeclarationNodeList};
use crate::utils::item_dependencies::item_dependencies;
use crate::utils::reorder_declarations::reorder_declarations;

struct EnforceModuleDeclarationOrder;

dylint_linting::impl_late_lint! {
    /// ### What it does
    ///
    /// Orders declarations in each module before local declarations which use them. Imports and
    /// macro output are excluded, while types and their immediately adjacent impls move as a unit.
    pub ENFORCE_MODULE_DECLARATION_ORDER,
    Warn,
    "enforces dependency-first ordering of module declarations",
    EnforceModuleDeclarationOrder
}

impl EnforceModuleDeclarationOrder {
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

    fn is_direct_impl_of(cx: &LateContext<'_>, item: &Item<'_>, expected: LocalDefId) -> bool {
        if !matches!(item.kind, ItemKind::Impl(_)) {
            return false;
        }
        cx.tcx
            .type_of(item.owner_id)
            .instantiate_identity()
            .ty_adt_def()
            .is_some_and(|definition| definition.did().as_local() == Some(expected))
    }

    fn contained_definitions(cx: &LateContext<'_>, item: &Item<'_>) -> HashSet<LocalDefId> {
        let mut definitions = HashSet::from([item.owner_id.def_id]);
        match item.kind {
            ItemKind::Impl(implementation) => {
                definitions.extend(implementation.items.iter().map(|id| id.owner_id.def_id));
            }
            ItemKind::Trait(.., items) => {
                definitions.extend(items.iter().map(|id| id.owner_id.def_id));
            }
            ItemKind::ForeignMod { items, .. } => {
                definitions.extend(items.iter().map(|id| id.owner_id.def_id));
            }
            ItemKind::Mod(_, module) => {
                for id in module.item_ids {
                    definitions.extend(Self::contained_definitions(cx, cx.tcx.hir_item(*id)));
                }
            }
            _ => {}
        }
        definitions
    }

    fn canonical_name(cx: &LateContext<'_>, item: &Item<'_>) -> String {
        match item.kind {
            ItemKind::ForeignMod { .. } => "extern block".to_owned(),
            ItemKind::Impl(_) => format!(
                "impl {}",
                cx.tcx.type_of(item.owner_id).instantiate_identity()
            ),
            _ => item
                .kind
                .ident()
                .map_or_else(|| "declaration".to_owned(), |ident| ident.name.to_string()),
        }
    }

    fn category(item: &Item<'_>) -> u8 {
        match item.kind {
            ItemKind::Const(..) | ItemKind::Static(..) => 1,
            ItemKind::Fn { .. } => 2,
            _ => 0,
        }
    }

    fn is_outward_visible_value(item: &Item<'_>) -> bool {
        matches!(
            item.kind,
            ItemKind::Const(..) | ItemKind::Static(..) | ItemKind::Fn { .. }
        ) && !item.vis_span.is_empty()
    }

    fn emit_if_needed(cx: &LateContext<'_>, hir_id: HirId, nodes: &DeclarationNodeList) {
        let ordering = nodes.declaration_order();
        if ordering.iter().copied().eq(0..nodes.len()) {
            return;
        }
        let mismatch = ordering
            .iter()
            .enumerate()
            .find_map(|(position, expected)| (position != *expected).then_some(position))
            .expect("different orders have a mismatch");
        let names = ordering
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
            ENFORCE_MODULE_DECLARATION_ORDER,
            hir_id,
            nodes[mismatch].span,
            DiagDecorator(|diag| {
                diag.primary_message("module declarations are not in dependency-first order");
                diag.span_label(nodes[mismatch].span, "first declaration out of order");
                diag.note(format!("expected declaration order: {names}"));
                if let Some(edits) = edits {
                    diag.multipart_suggestion(
                        "reorder these declarations",
                        edits,
                        Applicability::MachineApplicable,
                    );
                } else {
                    diag.help(format!(
                        "move complete declaration groups into this order: {names}"
                    ));
                }
            }),
        );
    }
}

impl<'tcx> LateLintPass<'tcx> for EnforceModuleDeclarationOrder {
    fn check_mod(&mut self, cx: &LateContext<'tcx>, module: &'tcx Mod<'tcx>, hir_id: HirId) {
        let source_map = cx.sess().source_map();
        let items = module
            .item_ids
            .iter()
            .map(|id| cx.tcx.hir_item(*id))
            .filter(|item| !item.span.in_external_macro(source_map))
            .collect::<Vec<_>>();
        let mut nodes = Vec::new();
        let mut index = 0;
        while index < items.len() {
            let item = items[index];
            if matches!(
                item.kind,
                ItemKind::ExternCrate(..) | ItemKind::Use(..) | ItemKind::Macro(..)
            ) {
                index += 1;
                continue;
            }
            let mut end = index;
            let mut definitions = Self::contained_definitions(cx, item);
            let mut dependencies = item_dependencies(cx.tcx, item);
            if Self::is_type(item) {
                while let Some(next) = items.get(end + 1)
                    && Self::is_direct_impl_of(cx, next, item.owner_id.def_id)
                {
                    end += 1;
                    definitions.extend(Self::contained_definitions(cx, items[end]));
                    dependencies.extend(item_dependencies(cx.tcx, items[end]));
                }
            }
            nodes.push(DeclarationNode {
                defs: definitions.into_iter().collect(),
                name: Self::canonical_name(cx, item),
                span: item.span.with_hi(items[end].span.hi()),
                category: Self::category(item),
                is_outward_visible: Self::is_outward_visible_value(item),
                dependencies,
            });
            index = end + 1;
        }
        Self::emit_if_needed(cx, hir_id, &DeclarationNodeList::new(nodes));
    }
}
