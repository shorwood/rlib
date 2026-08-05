extern crate rustc_errors;
extern crate rustc_hir;
extern crate rustc_span;

use rustc_errors::DiagDecorator;
use rustc_hir::intravisit::FnKind;
use rustc_hir::{Body, FnDecl};
use rustc_lint::{LateContext, LateLintPass, LintContext};
use rustc_span::Span;
use rustc_span::def_id::LocalDefId;

use crate::utils::function_structure::FunctionStructureAnalyzer;

// -----------------------------------------------------------------------------
// LongMethodChains
// -----------------------------------------------------------------------------

struct LongMethodChains {
    analyzer: FunctionStructureAnalyzer,
}

impl LongMethodChains {
    fn new() -> Self {
        Self {
            analyzer: FunctionStructureAnalyzer::from_config(),
        }
    }
}

dylint_linting::impl_late_lint! {
    /// ### What it does
    ///
    /// Finds maximal expressions containing more consecutive method calls than the configured
    /// limit. Associated-function calls do not count, and field access or `?` starts a new chain.
    ///
    /// ### Why is this bad?
    ///
    /// Long fluent expressions hide meaningful transformations behind punctuation and leave no
    /// vocabulary for intermediate states. Named bindings make the data flow inspectable and give
    /// later operations a concept to refer to.
    ///
    /// ```rust
    /// fn names(values: &[String]) -> Vec<String> {
    ///     values.iter().filter(|value| !value.is_empty()).cloned().collect()
    /// }
    /// ```
    ///
    /// Break the expression where an intermediate state has a useful name:
    ///
    /// ```rust
    /// fn names(values: &[String]) -> Vec<String> {
    ///     let populated = values.iter().filter(|value| !value.is_empty());
    ///     populated.cloned().collect()
    /// }
    /// ```
    pub LONG_METHOD_CHAINS,
    Warn,
    "rejects method chains beyond the configured call limit",
    LongMethodChains::new()
}

impl<'tcx> LateLintPass<'tcx> for LongMethodChains {
    fn check_fn(
        &mut self,
        cx: &LateContext<'tcx>,
        kind: FnKind<'tcx>,
        _: &'tcx FnDecl<'tcx>,
        body: &'tcx Body<'tcx>,
        _: Span,
        def_id: LocalDefId,
    ) {
        if matches!(kind, FnKind::Closure) {
            return;
        }
        let unit = FunctionStructureAnalyzer::function_returns_unit(cx, def_id);
        for finding in self
            .analyzer
            .analyze_control_flow(cx, body, unit)
            .long_method_chains
        {
            cx.emit_span_lint(
                LONG_METHOD_CHAINS,
                finding.span,
                DiagDecorator(|diag| {
                    diag.primary_message(finding.message);
                    diag.help(finding.help);
                }),
            );
        }
    }
}
