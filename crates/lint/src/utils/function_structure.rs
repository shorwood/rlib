extern crate rustc_hir;
extern crate rustc_lint;
extern crate rustc_span;

use rustc_hir::{Body, Expr, ExprKind};
use rustc_lint::LateContext;
use rustc_span::def_id::LocalDefId;

use super::config::LibraryConfig;
use super::control_flow_analysis::{
    ControlFlowAnalysis, ControlFlowAnalyzer, ControlFlowAnalyzerFunctionReturn,
};
use super::function_layout_analysis::{FunctionLayoutAnalysis, FunctionLayoutAnalyzer};
use super::function_structure_config::FunctionStructureConfig;

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
        let config = LibraryConfig::load().function_structure;
        config.validate().unwrap_or_else(|message| {
            panic!("invalid function structure configuration: {message}")
        });
        Self { config }
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

#[cfg(test)]
mod tests {
    use crate::utils::config::LibraryConfig;
    use crate::utils::function_structure_config::FunctionStructureConfig;

    #[test]
    fn parses_custom_function_structure_limits() {
        let config = toml::from_str::<LibraryConfig>(
            r#"
                [function_structure]
                max_phase_lines = 11
                phase_comment_prefix = "// Phase:"
                max_control_flow_depth = 3
                max_match_arm_lines = 13
                max_method_chain_calls = 5
            "#,
        )
        .expect("custom function structure should parse");
        assert_eq!(config.function_structure.max_phase_lines, 11);
        assert_eq!(config.function_structure.phase_comment_prefix, "// Phase:");
        assert!(config.function_structure.validate().is_ok());
    }

    #[test]
    fn rejects_zero_limits_and_non_comment_prefixes() {
        let zero = FunctionStructureConfig {
            max_phase_lines: 0,
            ..FunctionStructureConfig::default()
        };
        let malformed = FunctionStructureConfig {
            phase_comment_prefix: "---".to_owned(),
            ..FunctionStructureConfig::default()
        };
        assert!(zero.validate().is_err());
        assert!(malformed.validate().is_err());
    }
}
