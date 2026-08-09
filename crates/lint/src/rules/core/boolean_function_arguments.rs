extern crate rustc_errors;
extern crate rustc_hir;
extern crate rustc_span;

use std::borrow::Cow;

use rustc_errors::DiagDecorator;
use rustc_hir::intravisit::FnKind;
use rustc_hir::{Body, FnDecl, HirId, TraitItem};
use rustc_lint::{LateContext, LateLintPass};
use rustc_span::Span;
use rustc_span::def_id::LocalDefId;

use crate::utils::diagnostic::LateViolation;
use crate::utils::parameter_analysis::{Parameter, ParameterSignature};

// -----------------------------------------------------------------------------
// Violation: Boolean argument diagnostic
// -----------------------------------------------------------------------------

/// One unsupported boolean-parameter shape with the facts needed to explain it.
enum Violation<'analysis> {
    /// A single unnamed binary policy that should become a semantic enum.
    SingleFlag {
        /// Function-like declaration that owns the parameter.
        hir_id: HirId,
        /// Boolean parameter used as the primary diagnostic location.
        parameter: &'analysis Parameter,
    },
    /// Several positional policies that should become a named options type.
    MultipleFlags {
        /// Function-like declaration that owns the parameters.
        hir_id: HirId,
        /// Boolean parameters labelled by the diagnostic.
        parameters: Vec<&'analysis Parameter>,
    },
}

impl<'analysis> Violation<'analysis> {
    /// Classifies unsupported boolean parameters while preserving exact setter contracts.
    fn from_signature(signature: &'analysis ParameterSignature) -> Option<Self> {
        let parameters = signature.boolean_parameters();
        if parameters.is_empty() || signature.has_exact_boolean_setter() {
            return None;
        }
        if parameters.len() == 1 {
            return Some(Self::SingleFlag {
                hir_id: signature.hir_id,
                parameter: parameters[0],
            });
        }
        Some(Self::MultipleFlags {
            hir_id: signature.hir_id,
            parameters,
        })
    }

    /// Emits the single-flag form with enum-oriented remediation.
    fn emit_single(&self, cx: &LateContext<'_>, hir_id: HirId, parameter: &Parameter) {
        // Name the authored flag and prescribe vocabulary for its alternatives.
        cx.tcx.emit_node_span_lint(
            BOOLEAN_FUNCTION_ARGUMENTS,
            hir_id,
            parameter.span,
            DiagDecorator(|diag| {
                diag.primary_message(self.primary_message().into_owned());
                diag.note(self.rationale_message().into_owned());
                diag.help(self.remediation_message().into_owned());
            }),
        );
    }

    /// Emits the multiple-flag form with options-type remediation.
    fn emit_multiple(&self, cx: &LateContext<'_>, hir_id: HirId, parameters: &[&Parameter]) {
        // Expose every positional choice before prescribing a named aggregate.
        cx.tcx.emit_node_span_lint(
            BOOLEAN_FUNCTION_ARGUMENTS,
            hir_id,
            parameters[0].span,
            DiagDecorator(|diag| {
                diag.primary_message(self.primary_message().into_owned());
                for parameter in parameters {
                    diag.span_label(
                        parameter.span,
                        format!("`{}` is a positional policy choice", parameter.name),
                    );
                }
                diag.note(self.rationale_message().into_owned());
                diag.help(self.remediation_message().into_owned());
            }),
        );
    }
}

impl LateViolation for Violation<'_> {
    fn primary_message(&self) -> Cow<'_, str> {
        match self {
            Self::SingleFlag { parameter, .. } => Cow::Owned(format!(
                "boolean parameter `{}` hides its alternatives at call sites",
                parameter.name
            )),
            Self::MultipleFlags { parameters, .. } => Cow::Owned(format!(
                "{} boolean parameters create an unreadable positional flag combination",
                parameters.len()
            )),
        }
    }

    fn rationale_message(&self) -> Cow<'_, str> {
        match self {
            Self::SingleFlag { parameter, .. } => Cow::Owned(format!(
                "callers otherwise see only `true` or `false` instead of the behavior selected by `{}`",
                parameter.name
            )),
            Self::MultipleFlags { .. } => Cow::Borrowed(
                "positional boolean arguments can be swapped without a type error and do not reveal which choice each literal controls",
            ),
        }
    }

    fn remediation_message(&self) -> Cow<'_, str> {
        match self {
            Self::SingleFlag { parameter, .. } => Cow::Owned(format!(
                "replace `{}: bool` with an enum whose type and variants name the policy choices",
                parameter.name
            )),
            Self::MultipleFlags { .. } => Cow::Borrowed(
                "collect these choices in a named options type, using enums for choices that are not true predicates",
            ),
        }
    }

    fn emit(self, cx: &LateContext<'_>) {
        match &self {
            Self::SingleFlag { hir_id, parameter } => {
                self.emit_single(cx, *hir_id, parameter);
            }
            Self::MultipleFlags { hir_id, parameters } => {
                self.emit_multiple(cx, *hir_id, parameters);
            }
        }
    }
}

// -----------------------------------------------------------------------------
// BooleanFunctionArguments: Lint pass
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
    /// Classifies and emits one violation for an unsupported boolean signature.
    fn check_signature(cx: &LateContext<'_>, signature: &ParameterSignature) {
        let Some(violation) = Violation::from_signature(signature) else {
            return;
        };
        violation.emit(cx);
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
