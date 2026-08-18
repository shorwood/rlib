extern crate rustc_hir;
extern crate rustc_lint;
extern crate rustc_span;

use rustc_hir::{Body, Expr, ExprKind};
use rustc_lint::LateContext;
use rustc_span::def_id::LocalDefId;

use super::control_flow_analysis::{
    ControlFlowAnalysis, ControlFlowAnalyzer, ControlFlowAnalyzerFunctionReturn,
};
use super::early_return_analysis::{EarlyReturnAnalyzer, EarlyReturnFinding};
use super::function_layout_analysis::{FunctionLayoutAnalysis, FunctionLayoutAnalyzer};
use crate::config::core::FunctionStructureConfig;
use crate::config::store::ConfigStore;

// -----------------------------------------------------------------------------
// FunctionStructureAnalyzer: Shared named function analysis
// -----------------------------------------------------------------------------

/// Runs the source-layout and semantic-control-flow analyzers with one configuration.
pub struct FunctionStructureAnalyzer {
    /// Validated limits and comment syntax shared by all sub-analyses.
    config: FunctionStructureConfig,
}

impl FunctionStructureAnalyzer {
    /// Loads and validates the shared function-structure configuration.
    pub(crate) fn from_config() -> Self {
        let config = ConfigStore::get().function_structure.clone();
        Self { config }
    }

    /// Finds explicit early returns whose guard boundary has no phase explanation.
    pub(crate) fn analyze_early_returns<'tcx>(
        cx: &LateContext<'tcx>,
        body: &'tcx Body<'tcx>,
    ) -> Vec<EarlyReturnFinding> {
        EarlyReturnAnalyzer::new(cx).analyze(Self::authored_body(cx, body))
    }

    /// Unwraps the compiler-created closure that represents an authored async function body.
    fn authored_body<'tcx>(cx: &LateContext<'tcx>, body: &'tcx Body<'tcx>) -> &'tcx Expr<'tcx> {
        if let ExprKind::Closure(closure) = body.value.kind {
            cx.tcx.hir_body(closure.body).value
        } else {
            body.value
        }
    }

    /// Finds phase-comment and linear-code layout problems in one named function.
    pub(crate) fn analyze_layout<'tcx>(
        &self,
        cx: &LateContext<'tcx>,
        body: &'tcx Body<'tcx>,
    ) -> FunctionLayoutAnalysis {
        FunctionLayoutAnalyzer::new(cx, &self.config).analyze(Self::authored_body(cx, body))
    }

    /// Finds nesting, match-arm, and method-chain problems in one named function.
    pub(crate) fn analyze_control_flow<'tcx>(
        &self,
        cx: &LateContext<'tcx>,
        body: &'tcx Body<'tcx>,
        def_id: LocalDefId,
    ) -> ControlFlowAnalysis {
        let signature = cx.tcx.fn_sig(def_id).instantiate_identity();
        let output = signature.output().skip_binder();
        let function_return = ControlFlowAnalyzerFunctionReturn::from_output(output);
        ControlFlowAnalyzer::new(cx, &self.config, function_return)
            .analyze(Self::authored_body(cx, body))
    }
}
