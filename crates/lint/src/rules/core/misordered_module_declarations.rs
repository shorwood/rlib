extern crate rustc_errors;
extern crate rustc_hir;

use std::collections::HashSet;

use rustc_errors::{Applicability, DiagDecorator};
use rustc_hir::def_id::LocalDefId;
use rustc_hir::{HirId, Item, ItemKind, Mod};
use rustc_lint::{LateContext, LateLintPass, LintContext};

use crate::utils::declaration_node::{
    DeclarationConstraints, DeclarationNode, DeclarationNodeList, DeclarationSource,
};
use crate::utils::item_dependencies::DependenciesExt;
use crate::utils::reorder_declarations::DeclarationOrder;
use crate::utils::source_organization::SectionAnalyzer;

// -----------------------------------------------------------------------------
// MisorderedModule
// -----------------------------------------------------------------------------

/// Mutable declaration-group state extended through adjacent direct implementations.
struct MisorderedModuleDeclarationGroupExtension<'group> {
    /// Final source index currently included in the group.
    end: &'group mut usize,
    /// Definitions that must move with the declaration.
    definitions: &'group mut HashSet<LocalDefId>,
    /// Dependencies accumulated across the complete declaration group.
    dependencies: &'group mut HashSet<LocalDefId>,
}

/// Late lint pass that applies dependency-first ordering within authored sections.
struct MisorderedModuleDeclarations {
    /// Shared source-section analyzer used to preserve authored boundaries.
    sections: SectionAnalyzer,
}

impl MisorderedModuleDeclarations {
    /// Builds the pass from the configured section-divider policy.
    fn new() -> Self {
        Self {
            sections: SectionAnalyzer::from_config(),
        }
    }
}

dylint_linting::impl_late_lint! {
    /// ### What it does
    ///
    /// Orders declarations in each module before local declarations which use them. Imports and
    /// macro output are excluded, while types and their immediately adjacent impls move as a unit.
    ///
    /// ### Why is this bad?
    ///
    /// Reading dependency-first code does not require jumping forward to discover what a local
    /// name means. It also gives modules a deterministic order that can be maintained
    /// automatically as declarations are added.
    ///
    /// For example, this function appears before the local type in its signature:
    ///
    /// ```rust
    /// fn open() -> Connection { Connection }
    /// struct Connection;
    /// ```
    ///
    /// Declare the dependency before the code that uses it:
    ///
    /// ```rust
    /// struct Connection;
    /// fn open() -> Connection { Connection }
    /// ```
    pub MISORDERED_MODULE_DECLARATIONS,
    Warn,
    "enforces dependency-first ordering of module declarations",
    MisorderedModuleDeclarations::new()
}

impl MisorderedModuleDeclarations {
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

    /// Returns whether an item directly implements the expected local nominal type.
    fn is_direct_impl_of(cx: &LateContext<'_>, item: &Item<'_>, expected: LocalDefId) -> bool {
        if !matches!(item.kind, ItemKind::Impl(_)) {
            return false;
        }
        let item_type = cx.tcx.type_of(item.owner_id).instantiate_identity();
        item_type
            .ty_adt_def()
            .is_some_and(|definition| definition.did().as_local() == Some(expected))
    }

    /// Extends a nominal declaration node through its adjacent direct impl blocks.
    fn extend_direct_impl_group<'tcx>(
        cx: &LateContext<'tcx>,
        items: &[&'tcx Item<'tcx>],
        owner: LocalDefId,
        extension: &mut MisorderedModuleDeclarationGroupExtension<'_>,
    ) {
        while let Some(next) = items.get(*extension.end + 1)
            && Self::is_direct_impl_of(cx, next, owner)
        {
            *extension.end += 1;
            extension
                .definitions
                .extend(Self::contained_definitions(cx, items[*extension.end]));
            extension
                .dependencies
                .extend(items[*extension.end].dependencies(cx.tcx));
        }
    }

    /// Collects definitions whose identities move with a declaration node.
    fn contained_definitions(cx: &LateContext<'_>, item: &Item<'_>) -> HashSet<LocalDefId> {
        // Seed the declaration group with the outer item's own identity.
        let mut definitions = HashSet::from([item.owner_id.def_id]);

        // Extend through every definition semantically owned by the declaration.
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

        // Return every definition accumulated across the complete declaration group.
        definitions
    }

    /// Produces a stable human-readable name for diagnostic ordering output.
    fn canonical_name(cx: &LateContext<'_>, item: &Item<'_>) -> String {
        if matches!(item.kind, ItemKind::ForeignMod { .. }) {
            return "extern block".to_owned();
        }
        if matches!(item.kind, ItemKind::Impl(_)) {
            return format!(
                "impl {}",
                cx.tcx.type_of(item.owner_id).instantiate_identity()
            );
        }
        item.kind
            .ident()
            .map_or_else(|| "declaration".to_owned(), |ident| ident.name.to_string())
    }

    /// Assigns the declaration-kind tie-break rank used after dependencies.
    const fn category(item: &Item<'_>) -> u8 {
        match item.kind {
            ItemKind::Const(..) | ItemKind::Static(..) => 1,
            ItemKind::Fn { .. } => 2,
            _ => 0,
        }
    }

    /// Returns whether a value-level declaration is visible outside its module.
    fn is_outward_visible_value(item: &Item<'_>) -> bool {
        matches!(
            item.kind,
            ItemKind::Const(..) | ItemKind::Static(..) | ItemKind::Fn { .. }
        ) && !item.vis_span.is_empty()
    }

    /// Compares source and dependency order, then emits an atomic reorder when safe.
    fn emit_if_needed(cx: &LateContext<'_>, hir_id: HirId, nodes: &DeclarationNodeList) {
        // Stop when the authored order already matches dependency order.
        let ordering = nodes.declaration_order();
        if ordering.iter().copied().eq(0..nodes.len()) {
            return;
        }

        // Describe the first mismatch and the complete expected order.
        let source = (0..nodes.len()).collect::<Vec<_>>();
        let mismatch = DeclarationNodeList::first_mismatch(&source, &ordering);
        let names = nodes.formatted_names(&ordering);

        // Offer an atomic edit only when no declaration in a group carries attributes.
        let editable = nodes.has_only_unattributed_definitions(cx);
        let edits = editable
            .then(|| DeclarationOrder::edits(cx, nodes, &ordering))
            .flatten();

        // Keep the diagnostic useful even when comments or macros block a safe rewrite.
        cx.tcx.emit_node_span_lint(
            MISORDERED_MODULE_DECLARATIONS,
            hir_id,
            nodes[mismatch].source.span,
            DiagDecorator(|diag| {
                diag.primary_message("module declarations are not in dependency-first order");
                diag.span_label(
                    nodes[mismatch].source.span,
                    "first declaration out of order",
                );
                diag.note(format!("expected declaration order: {names}"));
                if let Some(edits) = edits {
                    diag.multipart_suggestion(
                        "reorder these declarations",
                        edits
                            .into_iter()
                            .map(|edit| (edit.span, edit.replacement))
                            .collect(),
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

impl<'tcx> LateLintPass<'tcx> for MisorderedModuleDeclarations {
    fn check_mod(&mut self, cx: &LateContext<'tcx>, module: &'tcx Mod<'tcx>, hir_id: HirId) {
        // Resolve authored module items and their optional section boundaries.
        let source_map = cx.sess().source_map();
        let sections = self.sections.analyze(cx, module, hir_id);
        let resolved = module.item_ids.iter().map(|id| cx.tcx.hir_item(*id));
        let items = resolved
            .filter(|item| !item.span.in_external_macro(source_map))
            .collect::<Vec<_>>();

        // Combine each nominal declaration with directly following inherent impl blocks.
        let mut nodes = Vec::new();
        let mut index = 0;
        while index < items.len() {
            // Skip imports and macros that do not participate in declaration ordering.
            let item = items[index];
            if matches!(
                item.kind,
                ItemKind::ExternCrate(..) | ItemKind::Use(..) | ItemKind::Macro(..)
            ) {
                index += 1;
                continue;
            }

            // Collect the declaration and dependencies owned by its movable source group.
            let mut end = index;
            let mut definitions = Self::contained_definitions(cx, item);
            let mut dependencies = item.dependencies(cx.tcx);
            if Self::is_type(item) {
                // Extend nominal types through their adjacent direct implementation group.
                let mut extension = MisorderedModuleDeclarationGroupExtension {
                    end: &mut end,
                    definitions: &mut definitions,
                    dependencies: &mut dependencies,
                };
                Self::extend_direct_impl_group(cx, &items, item.owner_id.def_id, &mut extension);
            }

            // Materialize the complete source-ordering node for this declaration group.
            let source = DeclarationSource {
                defs: definitions.into_iter().collect(),
                name: Self::canonical_name(cx, item),
                span: item.span.with_hi(items[end].span.hi()),
                section: sections.section_ordinal_for_span(item.span).unwrap_or(0),
            };

            // Attach dependency and tie-break constraints to the source identity.
            let constraints = DeclarationConstraints {
                category: Self::category(item),
                is_outward_visible: Self::is_outward_visible_value(item),
                dependencies,
            };

            // Combine the source identity and ordering constraints as one movable node.
            let node = DeclarationNode {
                source,
                constraints,
            };

            // Preserve the declaration group as one source-ordering unit.
            nodes.push(node);
            index = end + 1;
        }
        Self::emit_if_needed(cx, hir_id, &DeclarationNodeList::new(nodes));
    }
}
