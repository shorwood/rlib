extern crate rustc_errors;
extern crate rustc_hir;
extern crate rustc_span;

use std::borrow::Cow;

use rustc_errors::DiagDecorator;
use rustc_hir::def_id::LocalDefId;
use rustc_hir::intravisit::FnKind;
use rustc_hir::{Body, FnDecl, Item};
use rustc_lint::{LateContext, LateLintPass};
use rustc_span::Span;

use crate::utils::diagnostic::LateViolation;
use crate::utils::iterator_analysis::{IteratorAnalysis, IteratorCandidate};

// -----------------------------------------------------------------------------
// Violation: Stateful traversal outside iterator
// -----------------------------------------------------------------------------

/// Unique cursor-like method with exact receiver, item, and state evidence.
struct Violation {
    /// Complete traversal and remediation context discovered by the analyzer.
    candidate: IteratorCandidate,
}

impl From<&IteratorCandidate> for Violation {
    fn from(candidate: &IteratorCandidate) -> Self {
        Self {
            candidate: candidate.clone(),
        }
    }
}

impl LateViolation for Violation {
    fn primary_message(&self) -> Cow<'_, str> {
        Cow::Owned(format!(
            "`{}` advances `{}` as an ad hoc iterator over `{}`",
            self.candidate.source.name,
            self.candidate.protocol.type_name,
            self.candidate.protocol.item_name
        ))
    }

    fn rationale_message(&self) -> Cow<'_, str> {
        Cow::Owned(format!(
            "the mutable receiver, persistent state advance, and `Option<{}>` exhaustion contract form Rust's standard `Iterator::next` protocol; keeping it named hides adapters, `for`, collection, and generic consumption",
            self.candidate.protocol.item_name
        ))
    }

    fn remediation_message(&self) -> Cow<'_, str> {
        Cow::Owned(format!(
            "implement `Iterator<Item = {}>` for `{}` and move this state transition into `next`; if `{}` must remain reusable, move the cursor into a dedicated iterator type instead",
            self.candidate.protocol.item_name,
            self.candidate.protocol.type_name,
            self.candidate.protocol.type_name
        ))
    }

    fn emit(self, cx: &LateContext<'_>) {
        cx.tcx.emit_node_span_lint(
            AD_HOC_ITERATORS,
            self.candidate.source.hir_id,
            self.candidate.source.name_span,
            DiagDecorator(|diag| {
                diag.primary_message(self.primary_message().into_owned());
                diag.span_label(
                    self.candidate.source.receiver_span,
                    "persistent traversal state is mutated here",
                );
                diag.span_label(
                    self.candidate.source.evidence_span,
                    "this operation advances that state",
                );
                diag.note(self.rationale_message().into_owned());
                diag.help(self.remediation_message().into_owned());
            }),
        );
    }
}

// -----------------------------------------------------------------------------
// AdHocIterators: Standard stateful traversal policy
// -----------------------------------------------------------------------------

/// Collects iterator occupancy and stateful traversal candidates crate-wide.
#[derive(Default)]
struct AdHocIterators {
    /// Stateful traversal analyzer and existing iterator occupancy.
    iterators: IteratorAnalysis,
}

dylint_linting::impl_late_lint! {
    #[doc = include_str!("README.md")]
    pub AD_HOC_ITERATORS,
    Warn,
    "requires canonical stateful traversals to implement Iterator",
    AdHocIterators::default()
}

impl<'tcx> LateLintPass<'tcx> for AdHocIterators {
    fn check_item(&mut self, cx: &LateContext<'tcx>, item: &'tcx Item<'tcx>) {
        self.iterators.record_item(cx, item);
    }

    fn check_fn(
        &mut self,
        cx: &LateContext<'tcx>,
        kind: FnKind<'tcx>,
        _: &'tcx FnDecl<'tcx>,
        body: &'tcx Body<'tcx>,
        _: Span,
        def_id: LocalDefId,
    ) {
        self.iterators.record_function(cx, kind, body, def_id);
    }

    fn check_crate_post(&mut self, cx: &LateContext<'tcx>) {
        for candidate in self.iterators.findings() {
            Violation::from(candidate).emit(cx);
        }
    }
}
