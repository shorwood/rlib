extern crate rustc_errors;
extern crate rustc_hir;
extern crate rustc_span;

use std::borrow::Cow;

use rustc_errors::DiagDecorator;
use rustc_hir::{HirId, Item, Mod};
use rustc_lint::{LateContext, LateLintPass};
use rustc_span::Span;

use crate::utils::diagnostic::LateViolation;
use crate::utils::source_provenance::ItemProvenanceExt;
use crate::utils::test_module::CanonicalTestExt;

// -----------------------------------------------------------------------------
// ViolationKind: Test-module topology failure
// -----------------------------------------------------------------------------

/// Structural reason one in-source test module violates the canonical layout.
enum ViolationKind {
    /// Another canonical test module already owns the containing module's tests.
    Duplicate(
        /// Span of the first canonical test module.
        Span,
    ),
    /// An authored declaration appears after the sole test module.
    Nonterminal(
        /// Span of the first authored declaration after the test module.
        Span,
    ),
}

// -----------------------------------------------------------------------------
// Violation: Noncanonical test-module layout diagnostic
// -----------------------------------------------------------------------------

/// In-source test module that is duplicated or not terminal.
struct Violation {
    /// Test module used to honor local lint levels.
    hir_id: HirId,
    /// Complete test-module declaration receiving the diagnostic.
    span: Span,
    /// Concrete topology failure and related declaration.
    kind: ViolationKind,
}

impl LateViolation for Violation {
    fn primary_message(&self) -> Cow<'_, str> {
        match self.kind {
            ViolationKind::Duplicate(_) => {
                Cow::Borrowed("this module contains more than one in-source test module")
            }
            ViolationKind::Nonterminal(_) => {
                Cow::Borrowed("this in-source test module is not the final declaration")
            }
        }
    }

    fn rationale_message(&self) -> Cow<'_, str> {
        Cow::Borrowed(
            "one terminal test module gives production declarations and their tests a predictable source boundary",
        )
    }

    fn remediation_message(&self) -> Cow<'_, str> {
        match self.kind {
            ViolationKind::Duplicate(_) => Cow::Borrowed(
                "merge the test modules into one `mod test` or `mod tests` block and place it last",
            ),
            ViolationKind::Nonterminal(_) => {
                Cow::Borrowed("move the complete test module and its Tests divider to the end")
            }
        }
    }

    fn emit(self, cx: &LateContext<'_>) {
        cx.tcx.emit_node_span_lint(
            MISORDERED_TEST_DECLARATIONS,
            self.hir_id,
            self.span,
            DiagDecorator(|diag| {
                diag.primary_message(self.primary_message().into_owned());
                match self.kind {
                    ViolationKind::Duplicate(first) => {
                        diag.span_label(first, "the first test module is here");
                        diag.span_label(self.span, "merge this test module into the first");
                    }
                    ViolationKind::Nonterminal(following) => {
                        diag.span_label(self.span, "tests begin here");
                        diag.span_label(following, "this declaration appears after the tests");
                    }
                }
                diag.note(self.rationale_message().into_owned());
                diag.help(self.remediation_message().into_owned());
            }),
        );
    }
}

// -----------------------------------------------------------------------------
// MisorderedTestDeclarations: Single terminal test-module policy
// -----------------------------------------------------------------------------

/// Late lint pass enforcing one terminal canonical in-source test module.
struct MisorderedTestDeclarations;

crate::impl_late_lint! {
    #[doc = include_str!("README.md")]
    pub MISORDERED_TEST_DECLARATIONS,
    Warn,
    "requires one terminal in-source test module per authored module",
    MisorderedTestDeclarations
}

impl MisorderedTestDeclarations {
    /// Returns authored direct declarations in stable source order.
    fn authored_items<'tcx>(
        cx: &LateContext<'tcx>,
        module: &'tcx Mod<'tcx>,
    ) -> Vec<&'tcx Item<'tcx>> {
        module
            .item_ids
            .iter()
            .map(|item| cx.tcx.hir_item(*item))
            .filter(|item| !item.span.from_expansion() && !item.is_framework_generated())
            .collect()
    }
}

impl<'tcx> LateLintPass<'tcx> for MisorderedTestDeclarations {
    fn check_mod(&mut self, cx: &LateContext<'tcx>, module: &'tcx Mod<'tcx>, _: HirId) {
        let items = Self::authored_items(cx, module);
        let tests = items
            .iter()
            .enumerate()
            .filter(|(_, item)| item.is_canonical_in_source_test_module(cx))
            .collect::<Vec<_>>();

        // Modules without test blocks have no test-placement contract to enforce.
        let Some((first_index, first)) = tests.first().copied() else {
            return;
        };

        // Multiple test blocks violate the single terminal test-module contract directly.
        if tests.len() > 1 {
            for (_, duplicate) in tests.into_iter().skip(1) {
                Violation {
                    hir_id: duplicate.hir_id(),
                    span: duplicate.span,
                    kind: ViolationKind::Duplicate(first.span),
                }
                .emit(cx);
            }
            return;
        }

        // A sole test block at the final item position is already canonical.
        let Some(following) = items.get(first_index + 1) else {
            return;
        };
        Violation {
            hir_id: first.hir_id(),
            span: first.span,
            kind: ViolationKind::Nonterminal(following.span),
        }
        .emit(cx);
    }
}
