extern crate rustc_errors;
extern crate rustc_hir;
extern crate rustc_span;

use std::borrow::Cow;
use std::collections::HashMap;

use rustc_errors::{Applicability, DiagDecorator};
use rustc_hir::def_id::LocalDefId;
use rustc_hir::{HirId, Item, ItemKind, Mod};
use rustc_lint::{LateContext, LateLintPass, LintContext};
use rustc_span::{Span, Symbol};

use crate::utils::diagnostic::LateViolation;
use crate::utils::impl_target::ImplTargetExt;

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

/// Source condition that prevents a trustworthy automatic implementation-group move.
#[derive(Clone, Copy)]
enum MigrationBarrier {
    /// Expanded source cannot be owned by one authored edit.
    ExpandedSource,
    /// An impl attribute may need to travel with more context than its item span.
    AttributedImplementation,
    /// Struct and impl source belong to different physical files.
    CrossFileSource,
    /// A nearby comment may semantically belong to the implementation block.
    NearbyComment,
    /// Moving the impl would cross a textually scoped macro definition.
    MacroDefinition,
    /// Required source text or item boundaries could not be recovered safely.
    UnavailableSource,
}

impl MigrationBarrier {
    /// Explains why the diagnostic provides guidance instead of a machine-applicable edit.
    const fn message(self) -> &'static str {
        match self {
            Self::ExpandedSource => {
                "automatic migration is unavailable because expanded source has no single authored edit location"
            }
            Self::AttributedImplementation => {
                "automatic migration is withheld because an impl attribute may carry compilation or formatting semantics"
            }
            Self::CrossFileSource => {
                "automatic migration is unavailable because the struct and impl do not belong to one physical source file"
            }
            Self::NearbyComment => {
                "automatic migration is withheld because a nearby comment may belong to the impl block"
            }
            Self::MacroDefinition => {
                "automatic migration is withheld because moving the impl across a macro definition could change name resolution"
            }
            Self::UnavailableSource => {
                "automatic migration is unavailable because the complete authored source could not be recovered safely"
            }
        }
    }
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
    fn check_plain_same_file_source(&self) -> Result<(), MigrationBarrier> {
        // Resolve the common source file expected to own every migration edit.
        let source_map = self.cx.sess().source_map();
        let struct_file = source_map.span_to_filename(self.group.struct_item.span);
        let impls = &self.group.impls;

        // Reject generated declarations or implementations without one authored location.
        if self.group.struct_item.span.from_expansion()
            || impls.iter().any(|impl_| impl_.item.span.from_expansion())
        {
            return Err(MigrationBarrier::ExpandedSource);
        }

        // Preserve implementation attributes that may affect compilation or formatting.
        if impls
            .iter()
            .any(|impl_| !self.cx.tcx.hir_attrs(impl_.item.hir_id()).is_empty())
        {
            return Err(MigrationBarrier::AttributedImplementation);
        }

        // Keep a machine edit within one physical source file.
        if impls
            .iter()
            .any(|impl_| source_map.span_to_filename(impl_.item.span) != struct_file)
        {
            return Err(MigrationBarrier::CrossFileSource);
        }
        Ok(())
    }

    /// Rejects a move when nearby comments could be intended to travel with an impl block.
    ///
    /// Whitespace is safe to leave behind. A comment is authored structure, so the lint asks the
    /// agent to move that case manually instead of guessing who owns the comment.
    fn check_comments_are_unattached(&self) -> Result<(), MigrationBarrier> {
        // Start with the gap immediately following the struct declaration.
        let source_map = self.cx.sess().source_map();
        let mut boundaries = Vec::new();
        let following_struct = self
            .items
            .get(self.group.struct_index + 1)
            .ok_or(MigrationBarrier::UnavailableSource)?;
        boundaries.push((self.group.struct_item.span.hi(), following_struct.span.lo()));

        // Collect both neighboring gaps for every implementation block.
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

        // Reject the migration when any boundary contains authored commentary.
        for (lo, hi) in boundaries {
            let gap = source_map
                .span_to_snippet(Span::with_root_ctxt(lo, hi))
                .map_err(|_| MigrationBarrier::UnavailableSource)?;
            if !gap.contains("//") && !gap.contains("/*") {
                continue;
            }
            return Err(MigrationBarrier::NearbyComment);
        }
        Ok(())
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
    fn check_no_macro_definition_is_crossed(&self) -> Result<(), MigrationBarrier> {
        let crosses_macro = self
            .group
            .impls
            .iter()
            .any(|impl_| self.crosses_macro_definition(*impl_));

        // Permit the move only when textual macro scope cannot change.
        if crosses_macro {
            return Err(MigrationBarrier::MacroDefinition);
        }
        Ok(())
    }

    /// Returns all deletions and the matching insertion, or declines if any source ownership is
    /// uncertain.
    fn build(&self) -> Result<Vec<MigrationEdit>, MigrationBarrier> {
        // Validate source ownership and textual macro scope before assessing comment attachment.
        self.check_plain_same_file_source()?;
        self.check_no_macro_definition_is_crossed()?;
        self.check_comments_are_unattached()?;

        // Capture every misplaced implementation in its authored module order.
        let source_map = self.cx.sess().source_map();
        let impls = &self.group.impls;
        let snippets = impls
            .iter()
            .map(|impl_| {
                source_map
                    .span_to_snippet(impl_.item.span)
                    .map_err(|_| MigrationBarrier::UnavailableSource)
            })
            .collect::<Result<Vec<_>, _>>()?;

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
        Ok(edits)
    }
}

// -----------------------------------------------------------------------------
// Remediation: Implementation group migration
// -----------------------------------------------------------------------------

/// Safest available response to a misplaced implementation group.
enum Remediation {
    /// Manual movement guidance when source ownership is uncertain.
    Guidance {
        /// Exact source condition that prevented an automatic move.
        barrier: MigrationBarrier,
    },
    /// Atomic source edits that move the complete implementation group.
    Migration {
        /// Deletions and insertion comprising one machine-applicable move.
        edits: Vec<MigrationEdit>,
    },
}

// -----------------------------------------------------------------------------
// Violation: Nonadjacent implementation group
// -----------------------------------------------------------------------------

/// One misplaced implementation group with enough context to explain and repair it.
struct Violation<'hir> {
    /// Struct declaration that owns and receives the diagnostic.
    struct_item: &'hir Item<'hir>,
    /// First implementation block outside its required position.
    first_misplaced_span: Span,
    /// Struct name used throughout the diagnostic narrative.
    struct_name: Symbol,
    /// Number of direct implementation blocks that form the complete group.
    impl_count: usize,
    /// Manual guidance or a verified atomic source migration.
    remediation: Remediation,
}

impl Violation<'_> {
    /// Classifies one misplaced group and retains an atomic migration only when it is safe.
    fn from_group<'tcx>(
        cx: &LateContext<'tcx>,
        items: &[&'tcx Item<'tcx>],
        group: &ImplGroup<'tcx>,
    ) -> Violation<'tcx> {
        // Resolve the first misplaced block and conservatively prepare the complete group move.
        let first_misplaced_span = group.first_misplaced().item.span;
        let remediation = Migration::new(cx, items, group).build().map_or_else(
            |barrier| Remediation::Guidance { barrier },
            |edits| Remediation::Migration { edits },
        );

        // Preserve the group facts needed to explain the structural problem without reanalysis.
        Violation {
            struct_item: group.struct_item,
            first_misplaced_span,
            struct_name: group.struct_name,
            impl_count: group.impls.len(),
            remediation,
        }
    }

    /// Describes the misplaced block that demonstrates the broken group boundary.
    fn label_message(&self) -> String {
        format!("this impl is outside `{}`'s impl group", self.struct_name)
    }
}

impl LateViolation for Violation<'_> {
    fn primary_message(&self) -> Cow<'_, str> {
        Cow::Owned(format!(
            "impl blocks for `{}` should immediately follow its definition",
            self.struct_name
        ))
    }

    fn rationale_message(&self) -> Cow<'_, str> {
        if self.impl_count == 1 {
            return Cow::Owned(format!(
                "keeping this direct impl block adjacent lets readers and tools understand `{}` without searching the module",
                self.struct_name
            ));
        }
        Cow::Owned(format!(
            "keeping all {} direct impl blocks adjacent lets readers and tools understand `{}` without searching the module",
            self.impl_count, self.struct_name
        ))
    }

    fn remediation_message(&self) -> Cow<'_, str> {
        if self.impl_count == 1 {
            return Cow::Owned(format!(
                "move this impl block immediately after `{}`",
                self.struct_name
            ));
        }
        Cow::Owned(format!(
            "move all {} impl blocks immediately after `{}`",
            self.impl_count, self.struct_name
        ))
    }

    fn emit(self, cx: &LateContext<'_>) {
        // Materialize the complete diagnostic narrative before consuming source edits.
        let primary_message = self.primary_message().into_owned();
        let rationale_message = self.rationale_message().into_owned();
        let remediation_message = self.remediation_message().into_owned();
        let label_message = self.label_message();

        // Explain the broken group and attach an atomic migration only when it is safe.
        cx.tcx.emit_node_span_lint(
            NON_ADJACENT_STRUCT_IMPLS,
            self.struct_item.hir_id(),
            self.struct_item.span,
            DiagDecorator(|diag| {
                diag.primary_message(primary_message);
                diag.span_label(self.first_misplaced_span, label_message);
                diag.note(rationale_message);
                match self.remediation {
                    Remediation::Guidance { barrier } => {
                        diag.note(barrier.message());
                        diag.help(remediation_message);
                    }
                    Remediation::Migration { edits } => {
                        diag.multipart_suggestion(
                            remediation_message,
                            edits
                                .into_iter()
                                .map(|edit| (edit.span, edit.replacement))
                                .collect(),
                            Applicability::MachineApplicable,
                        );
                    }
                }
            }),
        );
    }
}

// -----------------------------------------------------------------------------
// NonAdjacentStructImpls: Lint pass
// -----------------------------------------------------------------------------

/// Late lint pass that keeps direct inherent impl groups beside their struct.
struct NonAdjacentStructImpls;

dylint_linting::impl_late_lint! {
    #[doc = include_str!("README.md")]
    pub NON_ADJACENT_STRUCT_IMPLS,
    Warn,
    "enforces impl blocks immediately after their struct definition",
    NonAdjacentStructImpls
}

impl NonAdjacentStructImpls {
    /// Returns module items whose ordering is controlled by the current crate.
    ///
    /// External macros may generate hidden helper items or derived impls. Ignoring that output
    /// keeps the rule focused on definitions an agent can actually rearrange. Local macro output
    /// remains.
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
            let Some(struct_def_id) = item.direct_struct(cx) else {
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
            Violation::from_group(cx, &items, &group).emit(cx);
        }
    }
}
