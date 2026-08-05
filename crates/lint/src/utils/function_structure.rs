extern crate rustc_hir;
extern crate rustc_lint;
extern crate rustc_span;

use rustc_hir::{Body, Expr, ExprKind};
use rustc_lint::LateContext;
use rustc_span::def_id::LocalDefId;

use super::config::{FunctionStructureConfig, LibraryConfig};
use super::control_flow_analysis::{ControlFlowAnalysis, ControlFlowAnalyzer};
use super::function_layout_analysis::{FunctionLayoutAnalysis, FunctionLayoutAnalyzer};

// -----------------------------------------------------------------------------
// FunctionStructureAnalyzer: Shared named function analysis
// -----------------------------------------------------------------------------

/// Runs the source-layout and semantic-control-flow analyzers with one configuration.
pub(crate) struct FunctionStructureAnalyzer {
    config: FunctionStructureConfig,
}

impl FunctionStructureAnalyzer {
    /// Loads and validates the shared function-structure configuration.
    pub(crate) fn from_config() -> Self {
        let config = LibraryConfig::load().function_structure;
        config.validate().unwrap_or_else(|message| {
            panic!("invalid function structure configuration: {message}")
        });
        Self { config }
    }

    /// Returns whether an authored function completes with the unit type.
    pub(crate) fn function_returns_unit(cx: &LateContext<'_>, def_id: LocalDefId) -> bool {
        let signature = cx.tcx.fn_sig(def_id).instantiate_identity();
        signature.output().skip_binder().is_unit()
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
        is_function_returning_unit: bool,
    ) -> ControlFlowAnalysis {
        ControlFlowAnalyzer::new(cx, &self.config, is_function_returning_unit)
            .analyze(Self::authored_body(cx, body))
    }
}
