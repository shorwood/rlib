extern crate rustc_errors;
extern crate rustc_hir;
extern crate rustc_span;

use rustc_errors::DiagDecorator;
use rustc_hir::intravisit::FnKind;
use rustc_hir::{Body, FnDecl, TraitItem};
use rustc_lint::{LateContext, LateLintPass};
use rustc_span::Span;
use rustc_span::def_id::LocalDefId;

use crate::utils::parameter_analysis::ParameterSignature;

// -----------------------------------------------------------------------------
// BooleanFunctionArguments
// -----------------------------------------------------------------------------

/// Late lint pass that rejects unnamed boolean policy in function signatures.
struct BooleanFunctionArguments;

dylint_linting::impl_late_lint! {
    /// ### What it does
    ///
    /// Finds direct boolean function and method parameters. One boolean is accepted only for an
    /// exact setter contract such as `set_enabled(enabled: bool)`; two or more are always rejected.
    /// Boolean returns, predicate callbacks, and wrapped state such as `Option<bool>` are outside
    /// the rule.
    ///
    /// ### Why is this bad?
    ///
    /// A boolean argument hides a choice at its call site. Literals such as `true` and `false` do
    /// not explain the policy they select, and multiple flags can be reordered without a type
    /// error. Adding another boolean is an easy compiling change for an agent but steadily erodes
    /// the API's vocabulary.
    ///
    /// This call makes neither decision reviewable:
    ///
    /// ```rust
    /// fn render(document: &Document, minify: bool, include_metadata: bool) {}
    ///
    /// render(&document, true, false);
    /// ```
    ///
    /// Name the policy through an enum or options type:
    ///
    /// ```rust
    /// struct RenderOptions {
    ///     minify: bool,
    ///     include_metadata: bool,
    /// }
    ///
    /// fn render(document: &Document, options: RenderOptions) {}
    /// ```
    pub BOOLEAN_FUNCTION_ARGUMENTS,
    Warn,
    "rejects boolean function parameters that hide policy at call sites",
    BooleanFunctionArguments
}

impl BooleanFunctionArguments {
    /// Emits the single-flag form with enum-oriented remediation.
    fn emit_single(cx: &LateContext<'_>, signature: &ParameterSignature) {
        // Point at the flag while prescribing vocabulary for its alternatives.
        let parameter = signature.boolean_parameters()[0];

        // Render enum-oriented guidance for the unnamed binary choice.
        cx.tcx.emit_node_span_lint(
            BOOLEAN_FUNCTION_ARGUMENTS,
            signature.hir_id,
            parameter.span,
            DiagDecorator(|diag| {
                diag.primary_message(format!(
                    "boolean parameter `{}` hides a policy choice at call sites",
                    parameter.name
                ));
                diag.help("replace the flag with a semantic enum that names both choices");
            }),
        );
    }

    /// Emits the multiple-flag form with options-type remediation.
    fn emit_multiple(cx: &LateContext<'_>, signature: &ParameterSignature) {
        // Label every independent choice hidden in the positional signature.
        let booleans = signature.boolean_parameters();

        // Render one label per independent choice before suggesting an options type.
        cx.tcx.emit_node_span_lint(
            BOOLEAN_FUNCTION_ARGUMENTS,
            signature.hir_id,
            booleans[0].span,
            DiagDecorator(|diag| {
                diag.primary_message("multiple boolean parameters hide independent policy choices");

                // Expose each hidden choice directly on its authored parameter.
                for parameter in &booleans {
                    diag.span_label(
                        parameter.span,
                        format!("boolean policy `{}`", parameter.name),
                    );
                }
                diag.help("collect these decisions in a named options type");
            }),
        );
    }

    /// Emits one signature-level finding for direct booleans not covered by a setter contract.
    fn check_signature(cx: &LateContext<'_>, signature: &ParameterSignature) {
        let booleans = signature.boolean_parameters();
        if booleans.is_empty() || signature.has_exact_boolean_setter() {
            return;
        }
        if booleans.len() == 1 {
            Self::emit_single(cx, signature);
            return;
        }
        Self::emit_multiple(cx, signature);
    }
}

impl<'tcx> LateLintPass<'tcx> for BooleanFunctionArguments {
    fn check_fn(
        &mut self,
        cx: &LateContext<'tcx>,
        kind: FnKind<'tcx>,
        _: &'tcx FnDecl<'tcx>,
        body: &'tcx Body<'tcx>,
        _: Span,
        def_id: LocalDefId,
    ) {
        let Some(signature) = ParameterSignature::from_body(cx, kind, body, def_id) else {
            return;
        };
        Self::check_signature(cx, &signature);
    }

    fn check_trait_item(&mut self, cx: &LateContext<'tcx>, item: &'tcx TraitItem<'tcx>) {
        let Some(signature) = ParameterSignature::from_required_trait(cx, item) else {
            return;
        };
        Self::check_signature(cx, &signature);
    }
}
