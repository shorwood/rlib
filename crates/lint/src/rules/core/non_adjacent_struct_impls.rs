extern crate rustc_errors;
extern crate rustc_hir;
extern crate rustc_span;

use std::collections::HashMap;

use rustc_errors::{Applicability, DiagDecorator};
use rustc_hir::{HirId, Item, ItemKind, Mod, def_id::LocalDefId};
use rustc_lint::{LateContext, LateLintPass, LintContext};
use rustc_span::{Span, Symbol};

use crate::utils::direct_impl_struct::direct_impl_struct;

// -----------------------------------------------------------------------------
// ImplGroup: Collected implementation groups
// -----------------------------------------------------------------------------

/// An item paired with its position after uneditable external macro output has been removed.
#[derive(Clone, Copy)]
struct ImplGroupItem<'hir> {
    /// Position in the filtered module-item sequence.
    index: usize,
    /// Authored implementation item at that position.
    item: &'hir Item<'hir>,
}

/// One struct and every direct impl block for it in the same module.
struct ImplGroup<'hir> {
    /// Position of the struct in the filtered module-item sequence.
    struct_index: usize,
    /// Struct declaration that owns the implementation group.
    struct_item: &'hir Item<'hir>,
    /// Struct name shown in diagnostic guidance.
    struct_name: Symbol,
    /// Direct inherent implementations in source order.
    impls: Vec<ImplGroupItem<'hir>>,
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
    fn first_misplaced(&self) -> ImplGroupItem<'_> {
        let mut indexed = self.impls.iter().enumerate();
        let misplaced = indexed.find_map(|(offset, impl_)| {
            (impl_.index != self.struct_index + offset + 1).then_some(*impl_)
        });
        misplaced.expect("a reported impl group must contain a misplaced block")
    }
}

// -----------------------------------------------------------------------------
// Migration: Conservative automatic migration
// -----------------------------------------------------------------------------

/// One deletion or insertion in an atomic implementation-group move.
struct MigrationEdit {
    /// Source range replaced by the edit.
    span: Span,
    /// Replacement text, empty for deletion edits.
    replacement: String,
}

/// Builds a source move for an entire impl group when comments, macros, and file boundaries are
/// known not to change its meaning.
struct Migration<'lint, 'hir> {
    /// Compiler context used to inspect source ownership and snippets.
    cx: &'lint LateContext<'hir>,
    /// Authored module items after external macro output is removed.
    items: &'lint [&'hir Item<'hir>],
    /// Misplaced struct and implementation group being migrated.
    group: &'lint ImplGroup<'hir>,
}

impl<'lint, 'hir> Migration<'lint, 'hir> {
    /// Starts a possible migration for one misplaced impl group.
    const fn new(
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
            let Some(next) = self.items.get(impl_.index + 1) else {
                continue;
            };
            boundaries.push((impl_.item.span.hi(), next.span.lo()));
        }

        for (lo, hi) in boundaries {
            let gap = source_map
                .span_to_snippet(Span::with_root_ctxt(lo, hi))
                .ok()?;
            if !gap.contains("//") && !gap.contains("/*") {
                continue;
            }
            return None;
        }
        Some(())
    }

    /// Returns whether moving an impl would cross a textually scoped macro definition.
    fn crosses_macro_definition(&self, impl_: ImplGroupItem<'_>) -> bool {
        let target = self.group.struct_index;
        let start = target.min(impl_.index);
        let end = target.max(impl_.index);
        self.items[start + 1..end]
            .iter()
            .any(|item| matches!(item.kind, ItemKind::Macro(..)))
    }

    /// Rejects moves across `macro_rules!` definitions because those names follow textual order.
    fn check_no_macro_definition_is_crossed(&self) -> Option<()> {
        let crosses_macro = self
            .group
            .impls
            .iter()
            .any(|impl_| self.crosses_macro_definition(*impl_));

        // Permit the move only when textual macro scope cannot change.
        (!crosses_macro).then_some(())
    }

    /// Returns all deletions and the matching insertion, or declines if any source ownership is
    /// uncertain.
    fn build(&self) -> Option<Vec<MigrationEdit>> {
        // Validate source ownership, comments, and textual macro scope before editing.
        self.check_plain_same_file_source()?;
        self.check_comments_are_unattached()?;
        self.check_no_macro_definition_is_crossed()?;

        // Capture every misplaced implementation in its authored module order.
        let source_map = self.cx.sess().source_map();
        let snippets = self
            .group
            .impls
            .iter()
            .map(|impl_| source_map.span_to_snippet(impl_.item.span).ok())
            .collect::<Option<Vec<_>>>()?;

        // Join the authored implementations for one insertion after the struct.
        let insertion = format!("\n\n{}", snippets.join("\n\n"));

        // Delete every old impl location before inserting their combined source.
        let mut edits = self
            .group
            .impls
            .iter()
            .map(|impl_| MigrationEdit {
                span: impl_.item.span,
                replacement: String::new(),
            })
            .collect::<Vec<_>>();

        // Insert the complete group immediately after its struct definition.
        edits.push(MigrationEdit {
            span: self.group.struct_item.span.shrink_to_hi(),
            replacement: insertion,
        });
        Some(edits)
    }
}

// -----------------------------------------------------------------------------
// NonAdjacentStructImpls: Lint pass
// -----------------------------------------------------------------------------

/// Late lint pass that keeps direct inherent impl groups beside their struct.
struct NonAdjacentStructImpls;

dylint_linting::impl_late_lint! {
    /// ### What it does
    ///
    /// Checks that all impl blocks for a struct form one group immediately after the struct.
    ///
    /// ### Why is this bad?
    ///
    /// Keeping a struct and its behavior together makes the type easier to understand without
    /// searching through the rest of the module.
    ///
    /// For example, this helper splits the struct from its implementation:
    ///
    /// ```rust
    /// struct Cache;
    /// fn cache_directory() -> &'static str { "/tmp" }
    /// impl Cache {
    ///     fn clear(&mut self) {}
    /// }
    /// ```
    ///
    /// Keep the complete impl group directly after the definition:
    ///
    /// ```rust
    /// struct Cache;
    /// impl Cache {
    ///     fn clear(&mut self) {}
    /// }
    /// fn cache_directory() -> &'static str { "/tmp" }
    /// ```
    pub NON_ADJACENT_STRUCT_IMPLS,
    Warn,
    "enforces impl blocks immediately after their struct definition",
    NonAdjacentStructImpls
}

impl NonAdjacentStructImpls {
    /// Returns module items whose ordering is controlled by the current crate.
    ///
    /// External macros may generate hidden helper items or derived impls. Ignoring that output keeps
    /// the rule focused on definitions an agent can actually rearrange. Local macro output remains.
    fn editable_module_items<'tcx>(
        cx: &LateContext<'tcx>,
        module: &'tcx Mod<'tcx>,
    ) -> Vec<&'tcx Item<'tcx>> {
        // Resolve direct module items under the current source map.
        let source_map = cx.sess().source_map();
        let resolved = module
            .item_ids
            .iter()
            .map(|item_id| cx.tcx.hir_item(*item_id));

        // Exclude only items whose source belongs to an external macro.
        resolved
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
        let mut impls_by_struct = HashMap::<LocalDefId, Vec<ImplGroupItem<'tcx>>>::new();
        for (index, item) in items.iter().enumerate() {
            let Some(struct_def_id) = direct_impl_struct(cx, item) else {
                continue;
            };
            impls_by_struct
                .entry(struct_def_id)
                .or_default()
                .push(ImplGroupItem { index, item });
        }

        let indexed = items.iter().enumerate();

        // Construct groups only for structs that own at least one direct impl.
        let groups = indexed.filter_map(|(struct_index, item)| {
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
        });

        // Preserve module order when returning the discovered struct groups.
        groups.collect()
    }

    /// Emits one warning for the complete misplaced group and offers one atomic move when safe.
    fn emit_group<'tcx>(
        cx: &LateContext<'tcx>,
        items: &[&'tcx Item<'tcx>],
        group: &ImplGroup<'tcx>,
    ) {
        // Resolve the first misplaced block and any complete atomic move.
        let first_misplaced = group.first_misplaced();
        let suggestion = Migration::new(cx, items, group).build();
        let impl_count = group.impls.len();

        // Tailor movement guidance to one implementation or the complete group.
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

        // Report the group and attach the atomic move only when every edit is safe.
        cx.tcx.emit_node_span_lint(
            NON_ADJACENT_STRUCT_IMPLS,
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
                        edits
                            .into_iter()
                            .map(|edit| (edit.span, edit.replacement))
                            .collect(),
                        Applicability::MachineApplicable,
                    );
                } else {
                    diag.help(move_message);
                }
            }),
        );
    }
}

impl<'tcx> LateLintPass<'tcx> for NonAdjacentStructImpls {
    /// Checks the complete item order of one module.
    ///
    /// Looking at a whole module at once lets the rule compare every impl for a struct and emit one
    /// useful warning rather than several competing warnings.
    fn check_mod(&mut self, cx: &LateContext<'tcx>, module: &'tcx Mod<'tcx>, _: HirId) {
        let items = Self::editable_module_items(cx, module);
        for group in Self::discover_impl_groups(cx, &items) {
            if group.is_immediately_after_struct() {
                continue;
            }
            Self::emit_group(cx, &items, &group);
        }
    }
}
