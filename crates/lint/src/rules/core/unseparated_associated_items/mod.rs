extern crate rustc_errors;
extern crate rustc_hir;

use std::borrow::Cow;

use rustc_errors::{Applicability, DiagDecorator};
use rustc_hir::{ImplItemId, Item, ItemKind, TraitItemId};
use rustc_lint::{LateContext, LateLintPass};

use crate::utils::diagnostic::LateViolation;
use crate::utils::item_separation::{Analyzer, Finding, Source};

// -----------------------------------------------------------------------------
// Violation: Unseparated associated item diagnostic
// -----------------------------------------------------------------------------

/// Adjacent associated items lacking a visually empty line between them.
struct Violation {
    /// Shared declaration-spacing evidence.
    finding: Finding,
}

impl LateViolation for Violation {
    fn primary_message(&self) -> Cow<'_, str> {
        Cow::Owned(format!(
            "associated items `{}` and `{}` are not separated by a blank line",
            self.finding.previous_name, self.finding.following_name
        ))
    }

    fn rationale_message(&self) -> Cow<'_, str> {
        Cow::Borrowed(
            "visually separating associated items makes implementation and trait boundaries easier for readers and tools to scan",
        )
    }

    fn remediation_message(&self) -> Cow<'_, str> {
        Cow::Owned(format!(
            "insert one blank line before `{}`",
            self.finding.following_name
        ))
    }

    fn emit(self, cx: &LateContext<'_>) {
        // Render diagnostic text before moving the optional insertion span.
        let primary = self.primary_message().into_owned();
        let remediation = self.remediation_message().into_owned();

        // Suggest a source edit only when no comment or other syntax occupies the boundary.
        cx.tcx.emit_node_span_lint(
            UNSEPARATED_ASSOCIATED_ITEMS,
            self.finding.hir_id,
            self.finding.span,
            DiagDecorator(|diag| {
                diag.primary_message(primary);
                diag.span_label(
                    self.finding.span,
                    "this associated item needs visual separation",
                );
                diag.note(self.rationale_message().into_owned());
                if let Some(insertion) = self.finding.insertion {
                    diag.span_suggestion(
                        insertion,
                        remediation,
                        "\n",
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
// UnseparatedAssociatedItems: Associated item boundary policy
// -----------------------------------------------------------------------------

/// Late lint pass that requires one visually empty line between associated items.
struct UnseparatedAssociatedItems;

dylint_linting::impl_late_lint! {
    #[doc = include_str!("README.md")]
    pub UNSEPARATED_ASSOCIATED_ITEMS,
    Warn,
    "rejects associated items without a separating blank line",
    UnseparatedAssociatedItems
}

impl UnseparatedAssociatedItems {
    /// Diagnoses every adjacent authored pair without an intervening empty line.
    fn check_items(cx: &LateContext<'_>, items: &[Source]) {
        for finding in Analyzer::findings(cx, items) {
            Violation { finding }.emit(cx);
        }
    }

    /// Resolves directly authored implementation items into comparable source identities.
    fn impl_items(cx: &LateContext<'_>, item_ids: &[ImplItemId]) -> Vec<Source> {
        let mut sources = Vec::new();
        for id in item_ids {
            let item = cx.tcx.hir_impl_item(*id);
            sources.push(Source::new(
                item.hir_id(),
                item.span,
                item.ident.name.to_string(),
            ));
        }
        sources
    }

    /// Resolves directly authored trait items into comparable source identities.
    fn trait_items(cx: &LateContext<'_>, item_ids: &[TraitItemId]) -> Vec<Source> {
        let mut sources = Vec::new();
        for id in item_ids {
            let item = cx.tcx.hir_trait_item(*id);
            sources.push(Source::new(
                item.hir_id(),
                item.span,
                item.ident.name.to_string(),
            ));
        }
        sources
    }
}

impl<'tcx> LateLintPass<'tcx> for UnseparatedAssociatedItems {
    fn check_item(&mut self, cx: &LateContext<'tcx>, item: &'tcx Item<'tcx>) {
        // Exclude outer declarations produced beyond the current source file.
        if item.span.from_expansion() {
            return;
        }

        // Resolve the associated-item family owned by the declaration.
        let items = match item.kind {
            ItemKind::Impl(implementation) => Self::impl_items(cx, implementation.items),
            ItemKind::Trait(.., item_ids) => Self::trait_items(cx, item_ids),
            // Other declarations do not own associated-item sequences.
            _ => return,
        };

        // Diagnose every missing visual boundary in source order.
        Self::check_items(cx, &items);
    }
}
