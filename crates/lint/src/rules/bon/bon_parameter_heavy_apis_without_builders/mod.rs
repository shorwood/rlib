extern crate rustc_errors;
extern crate rustc_hir;
extern crate rustc_span;

use std::borrow::Cow;

use rustc_errors::DiagDecorator;
use rustc_hir::intravisit::FnKind;
use rustc_hir::{Body, FnDecl};
use rustc_lint::{LateContext, LateLintPass};
use rustc_span::Span;
use rustc_span::def_id::LocalDefId;

use crate::utils::diagnostic::LateViolation;
use crate::utils::parameter_analysis::ParameterSignature;

struct Violation {
    owner: rustc_hir::HirId,
    span: Span,
    parameter_count: usize,
    boolean_count: usize,
    ambiguous_groups: usize,
}

impl LateViolation for Violation {
    fn primary_message(&self) -> Cow<'_, str> {
        Cow::Owned(format!(
            "public API has {} positional parameters with builder-worthy ambiguity",
            self.parameter_count
        ))
    }

    fn rationale_message(&self) -> Cow<'_, str> {
        Cow::Owned(format!(
            "{} boolean choices and {} interchangeable type groups make call sites difficult to verify",
            self.boolean_count, self.ambiguous_groups
        ))
    }

    fn remediation_message(&self) -> Cow<'_, str> {
        Cow::Borrowed(
            "add `#[bon::builder]` for independent named choices, or introduce a domain options type when these values form one concept",
        )
    }

    fn emit(self, cx: &LateContext<'_>) {
        cx.tcx.emit_node_span_lint(
            BON_PARAMETER_HEAVY_APIS_WITHOUT_BUILDERS,
            self.owner,
            self.span,
            DiagDecorator(|diag| {
                diag.primary_message(self.primary_message().into_owned());
                diag.note(self.rationale_message().into_owned());
                diag.help(self.remediation_message().into_owned());
            }),
        );
    }
}

struct BonParameterHeavyApisWithoutBuilders;

dylint_linting::impl_late_lint! {
    #[doc = include_str!("README.md")]
    pub BON_PARAMETER_HEAVY_APIS_WITHOUT_BUILDERS,
    Warn,
    "recommends Bon for public parameter-heavy ambiguous APIs",
    BonParameterHeavyApisWithoutBuilders
}

impl LateLintPass<'_> for BonParameterHeavyApisWithoutBuilders {
    fn check_fn(
        &mut self,
        cx: &LateContext<'_>,
        kind: FnKind<'_>,
        _: &FnDecl<'_>,
        body: &Body<'_>,
        _: Span,
        def_id: LocalDefId,
    ) {
        if !cx.tcx.visibility(def_id).is_public() {
            return;
        }
        let Some(signature) = ParameterSignature::from_body(cx, kind, body, def_id) else {
            return;
        };
        let parameter_count = signature.parameter_count();
        let boolean_count = signature.boolean_parameters().len();
        let ambiguous_groups = signature.ambiguous_groups().len();
        if parameter_count < 5
            || (parameter_count < 7 && boolean_count < 2 && ambiguous_groups == 0)
        {
            return;
        }
        Violation {
            owner: signature.hir_id,
            span: signature.name_span(),
            parameter_count,
            boolean_count,
            ambiguous_groups,
        }
        .emit(cx);
    }
}
