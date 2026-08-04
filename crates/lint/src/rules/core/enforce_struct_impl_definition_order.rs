extern crate rustc_errors;
extern crate rustc_hir;
extern crate rustc_span;

use std::collections::HashMap;

use rustc_errors::{Applicability, DiagDecorator};
use rustc_hir::{HirId, Item, ItemKind, Mod, def_id::LocalDefId};
use rustc_lint::{LateContext, LateLintPass, LintContext};
use rustc_span::{Span, Symbol};

use crate::utils::direct_impl_struct::direct_impl_struct;

// Collected model
// -------------------------------------------------------------------------------------------------

/// An item paired with its position after uneditable external macro output has been removed.
#[derive(Clone, Copy)]
struct IndexedItem<'hir> {
    index: usize,
    item: &'hir Item<'hir>,
}

/// One struct and every direct impl block for it in the same module.
struct ImplGroup<'hir> {
    struct_index: usize,
    struct_item: &'hir Item<'hir>,
    struct_name: Symbol,
    impls: Vec<IndexedItem<'hir>>,
}

impl ImplGroup<'_> {
    /// Returns whether every impl occupies the positions directly following the struct.
    ///
    /// Comments and attributes do not appear as module items, so they may still separate the raw
    /// lines without weakening the definition order.
    fn is_immediately_after_struct(&self) -> bool {
        self.impls
            .iter()
            .enumerate()
            .all(|(offset, impl_)| impl_.index == self.struct_index + offset + 1)
    }

    /// Returns the first impl that is not in its required position.
    fn first_misplaced(&self) -> IndexedItem<'_> {
        self.impls
            .iter()
            .enumerate()
            .find_map(|(offset, impl_)| {
                (impl_.index != self.struct_index + offset + 1).then_some(*impl_)
            })
            .expect("a reported impl group must contain a misplaced block")
    }
}

// Conservative automatic migration
// -------------------------------------------------------------------------------------------------

/// Builds a source move for an entire impl group when comments, macros, and file boundaries are
/// known not to change its meaning.
struct Migration<'lint, 'hir> {
    cx: &'lint LateContext<'hir>,
    items: &'lint [&'hir Item<'hir>],
    group: &'lint ImplGroup<'hir>,
}

impl<'lint, 'hir> Migration<'lint, 'hir> {
    /// Starts a possible migration for one misplaced impl group.
    fn new(
        cx: &'lint LateContext<'hir>,
        items: &'lint [&'hir Item<'hir>],
        group: &'lint ImplGroup<'hir>,
    ) -> Self {
        Self { cx, items, group }
    }

    /// Requires ordinary, unattributed source in one physical file.
    ///
    /// Attributes may control compilation or formatting, while expanded and cross-file text cannot
    /// be safely owned by one editor suggestion.
    fn check_plain_same_file_source(&self) -> Option<()> {
        let source_map = self.cx.sess().source_map();
        let struct_file = source_map.span_to_filename(self.group.struct_item.span);
        if self.group.struct_item.span.from_expansion()
            || self.group.impls.iter().any(|impl_| {
                impl_.item.span.from_expansion()
                    || !self.cx.tcx.hir_attrs(impl_.item.hir_id()).is_empty()
                    || source_map.span_to_filename(impl_.item.span) != struct_file
            })
        {
            return None;
        }
        Some(())
    }

    /// Rejects a move when nearby comments could be intended to travel with an impl block.
    ///
    /// Whitespace is safe to leave behind. A comment is authored structure, so the lint asks the
    /// agent to move that case manually instead of guessing who owns the comment.
    fn check_comments_are_unattached(&self) -> Option<()> {
        let source_map = self.cx.sess().source_map();
        let mut boundaries = Vec::new();
        boundaries.push((
            self.group.struct_item.span.hi(),
            self.items.get(self.group.struct_index + 1)?.span.lo(),
        ));
        for impl_ in &self.group.impls {
            if let Some(previous) = impl_
                .index
                .checked_sub(1)
                .and_then(|index| self.items.get(index))
            {
                boundaries.push((previous.span.hi(), impl_.item.span.lo()));
            }
            if let Some(next) = self.items.get(impl_.index + 1) {
                boundaries.push((impl_.item.span.hi(), next.span.lo()));
            }
        }

        for (lo, hi) in boundaries {
            let gap = source_map
                .span_to_snippet(Span::with_root_ctxt(lo, hi))
                .ok()?;
            if gap.contains("//") || gap.contains("/*") {
                return None;
            }
        }
        Some(())
    }

    /// Rejects moves across `macro_rules!` definitions because those names follow textual order.
    fn check_no_macro_definition_is_crossed(&self) -> Option<()> {
        let target = self.group.struct_index;
        let crosses_macro = self.group.impls.iter().any(|impl_| {
            let start = target.min(impl_.index);
            let end = target.max(impl_.index);
            self.items[start + 1..end]
                .iter()
                .any(|item| matches!(item.kind, ItemKind::Macro(..)))
        });
        (!crosses_macro).then_some(())
    }

    /// Returns all deletions and the matching insertion, or declines if any source ownership is
    /// uncertain.
    fn build(&self) -> Option<Vec<(Span, String)>> {
        self.check_plain_same_file_source()?;
        self.check_comments_are_unattached()?;
        self.check_no_macro_definition_is_crossed()?;

        let source_map = self.cx.sess().source_map();
        let snippets = self
            .group
            .impls
            .iter()
            .map(|impl_| source_map.span_to_snippet(impl_.item.span).ok())
            .collect::<Option<Vec<_>>>()?;
        let insertion = format!("\n\n{}", snippets.join("\n\n"));

        let mut edits = self
            .group
            .impls
            .iter()
            .map(|impl_| (impl_.item.span, String::new()))
            .collect::<Vec<_>>();
        edits.push((self.group.struct_item.span.shrink_to_hi(), insertion));
        Some(edits)
    }
}

// Lint pass
// -------------------------------------------------------------------------------------------------

struct EnforceStructImplDefinitionOrder;

dylint_linting::impl_late_lint! {
    /// ### What it does
    ///
    /// Checks that all impl blocks for a struct form one group immediately after the struct.
    ///
    /// ### Why is this bad?
    ///
    /// Keeping a struct and its behavior together makes the type easier to understand without
    /// searching through the rest of the module.
    pub ENFORCE_STRUCT_IMPL_DEFINITION_ORDER,
    Warn,
    "enforces impl blocks immediately after their struct definition",
    EnforceStructImplDefinitionOrder
}

impl EnforceStructImplDefinitionOrder {
    /// Returns module items whose ordering is controlled by the current crate.
    ///
    /// External macros may generate hidden helper items or derived impls. Ignoring that output keeps
    /// the rule focused on definitions an agent can actually rearrange. Local macro output remains.
    fn editable_module_items<'tcx>(
        cx: &LateContext<'tcx>,
        module: &'tcx Mod<'tcx>,
    ) -> Vec<&'tcx Item<'tcx>> {
        let source_map = cx.sess().source_map();
        module
            .item_ids
            .iter()
            .map(|item_id| cx.tcx.hir_item(*item_id))
            .filter(|item| !item.span.in_external_macro(source_map))
            .collect()
    }

    /// Associates each struct with direct inherent and trait impls from the same module.
    ///
    /// For example, both blocks below belong to `Widget`, while `impl Trait for &Widget` does not.
    ///
    /// ```rust
    /// struct Widget;
    /// trait Draw {}
    ///
    /// impl Widget {}
    /// impl Draw for Widget {}
    /// ```
    fn discover_impl_groups<'tcx>(
        cx: &LateContext<'tcx>,
        items: &[&'tcx Item<'tcx>],
    ) -> Vec<ImplGroup<'tcx>> {
        let mut impls_by_struct = HashMap::<LocalDefId, Vec<IndexedItem<'tcx>>>::new();
        for (index, item) in items.iter().enumerate() {
            if let Some(struct_def_id) = direct_impl_struct(cx, item) {
                impls_by_struct
                    .entry(struct_def_id)
                    .or_default()
                    .push(IndexedItem { index, item });
            }
        }

        items
            .iter()
            .enumerate()
            .filter_map(|(struct_index, item)| {
                let ItemKind::Struct(name, ..) = item.kind else {
                    return None;
                };
                let impls = impls_by_struct.remove(&item.owner_id.def_id)?;
                Some(ImplGroup {
                    struct_index,
                    struct_item: item,
                    struct_name: name.name,
                    impls,
                })
            })
            .collect()
    }

    /// Emits one warning for the complete misplaced group and offers one atomic move when safe.
    fn emit_group<'tcx>(
        cx: &LateContext<'tcx>,
        items: &[&'tcx Item<'tcx>],
        group: &ImplGroup<'tcx>,
    ) {
        let first_misplaced = group.first_misplaced();
        let suggestion = Migration::new(cx, items, group).build();
        let impl_count = group.impls.len();
        let move_message = if impl_count == 1 {
            format!(
                "move this impl block immediately after `{}`",
                group.struct_name
            )
        } else {
            format!(
                "move all {impl_count} impl blocks immediately after `{}`",
                group.struct_name
            )
        };

        cx.tcx.emit_node_span_lint(
            ENFORCE_STRUCT_IMPL_DEFINITION_ORDER,
            group.struct_item.hir_id(),
            group.struct_item.span,
            DiagDecorator(|diag| {
                diag.primary_message(format!(
                    "impl blocks for `{}` should immediately follow its definition",
                    group.struct_name
                ));
                diag.span_label(
                    first_misplaced.item.span,
                    format!("this impl is outside `{}`'s impl group", group.struct_name),
                );
                if let Some(edits) = suggestion {
                    diag.multipart_suggestion(
                        move_message,
                        edits,
                        Applicability::MachineApplicable,
                    );
                } else {
                    diag.help(move_message);
                }
            }),
        );
    }
}

impl<'tcx> LateLintPass<'tcx> for EnforceStructImplDefinitionOrder {
    /// Checks the complete item order of one module.
    ///
    /// Looking at a whole module at once lets the rule compare every impl for a struct and emit one
    /// useful warning rather than several competing warnings.
    fn check_mod(&mut self, cx: &LateContext<'tcx>, module: &'tcx Mod<'tcx>, _: HirId) {
        let items = Self::editable_module_items(cx, module);
        for group in Self::discover_impl_groups(cx, &items) {
            if !group.is_immediately_after_struct() {
                Self::emit_group(cx, &items, &group);
            }
        }
    }
}
