extern crate rustc_errors;
extern crate rustc_hir;
extern crate rustc_span;

use std::borrow::Cow;

use rustc_errors::DiagDecorator;
use rustc_hir::def_id::LocalDefId;
use rustc_hir::intravisit::FnKind;
use rustc_hir::{Body, Expr, FnDecl, Item};
use rustc_lint::{LateContext, LateLintPass};
use rustc_span::Span;

use crate::utils::construction_analysis::{ConstructionAnalysis, ConstructionCandidate};
use crate::utils::diagnostic::LateViolation;

// -----------------------------------------------------------------------------
// Violation: Nonstandard canonical parsing contract
// -----------------------------------------------------------------------------

/// A unique string parser that should expose Rust's standard parsing protocol.
struct Violation {
    /// HIR owner used for lint-level configuration.
    hir_id: rustc_hir::HirId,
    /// Authored parser identifier.
    span: Span,
    /// Current function name shown in contextual guidance.
    function_name: String,
    /// Parsed local type that should own the contract.
    target_name: String,
}

impl Violation {
    /// Retains the precise parser and target context needed for remediation.
    fn from_candidate(cx: &LateContext<'_>, analyze_candidate: &ConstructionCandidate) -> Self {
        Self {
            hir_id: cx
                .tcx
                .local_def_id_to_hir_id(analyze_candidate.function.def_id),
            span: analyze_candidate.function.name_span,
            function_name: analyze_candidate.function.name.to_string(),
            target_name: analyze_candidate.target.name.to_string(),
        }
    }
}

impl LateViolation for Violation {
    fn primary_message(&self) -> Cow<'_, str> {
        Cow::Owned(format!(
            "`{}` is the canonical string parser for `{}` but does not implement `FromStr`",
            self.function_name, self.target_name
        ))
    }

    fn rationale_message(&self) -> Cow<'_, str> {
        Cow::Owned(format!(
            "a unique `&str -> Result<{}, _>` conversion is a type-level parsing contract; leaving it as an ad hoc function hides it from `.parse()`, generic bounds, and standard tooling",
            self.target_name
        ))
    }

    fn remediation_message(&self) -> Cow<'_, str> {
        Cow::Owned(format!(
            "implement `std::str::FromStr` for `{}` and move the parsing body into `from_str`; keep a wrapper only when its name adds a distinct policy or format",
            self.target_name
        ))
    }

    fn emit(self, cx: &LateContext<'_>) {
        cx.tcx.emit_node_span_lint(
            AD_HOC_STRING_PARSERS,
            self.hir_id,
            self.span,
            DiagDecorator(|diag| {
                diag.primary_message(self.primary_message().into_owned());
                diag.note(self.rationale_message().into_owned());
                diag.help(self.remediation_message().into_owned());
            }),
        );
    }
}

// -----------------------------------------------------------------------------
// AdHocStringParsers: Canonical textual conversion policy
// -----------------------------------------------------------------------------
#[derive(Default)]
/// Collects structural parsing evidence before selecting unique canonical parsers.
struct AdHocStringParsers {
    /// Shared semantic analyzer used to group parsers by their constructed type.
    constructions: ConstructionAnalysis,
}

dylint_linting::impl_late_lint! {
    #[doc = include_str!("README.md")]
    pub AD_HOC_STRING_PARSERS,
    Warn,
    "requires unique canonical string parsers to implement FromStr",
    AdHocStringParsers::default()
}

impl<'tcx> LateLintPass<'tcx> for AdHocStringParsers {
    fn check_item(&mut self, cx: &LateContext<'tcx>, item: &'tcx Item<'tcx>) {
        self.constructions.record_item(cx, item);
    }

    fn check_expr(&mut self, cx: &LateContext<'tcx>, expression: &'tcx Expr<'tcx>) {
        self.constructions.record_expression(cx, expression);
    }

    fn check_fn(
        &mut self,
        cx: &LateContext<'tcx>,
        kind: FnKind<'tcx>,
        _: &'tcx FnDecl<'tcx>,
        body: &'tcx Body<'tcx>,
        span: Span,
        def_id: LocalDefId,
    ) {
        self.constructions
            .record_function(cx, kind, body, span, def_id);
    }

    fn check_crate_post(&mut self, cx: &LateContext<'tcx>) {
        let parser_families = self.constructions.parser_families();
        let mut candidates = Vec::new();
        for family in parser_families.values() {
            // Retain only one unqualified parser for a target without the standard contract.
            let [analyze_candidate] = family.as_slice() else {
                continue;
            };
            if !analyze_candidate.has_unqualified_parser_name()
                || self
                    .constructions
                    .from_str_targets
                    .contains(&analyze_candidate.target.def_id)
            {
                continue;
            }

            // Defer emission until source order can be restored across hash-map families.
            candidates.push(*analyze_candidate);
        }
        candidates
            .sort_unstable_by_key(|analyze_candidate| analyze_candidate.function.name_span.lo());
        for analyze_candidate in candidates {
            Violation::from_candidate(cx, analyze_candidate).emit(cx);
        }
    }
}
