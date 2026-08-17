extern crate rustc_errors;
extern crate rustc_hir;
extern crate rustc_span;

use std::borrow::Cow;

use rustc_errors::DiagDecorator;
use rustc_hir::def::DefKind;
use rustc_hir::intravisit::FnKind;
use rustc_hir::{Body, FnDecl};
use rustc_lint::{LateContext, LateLintPass};
use rustc_span::Span;
use rustc_span::def_id::LocalDefId;

use crate::utils::diagnostic::LateViolation;
use crate::utils::parameter_analysis::ParameterSignature;

// -----------------------------------------------------------------------------
// Violation: Ambiguous parameter-heavy public API
// -----------------------------------------------------------------------------

/// Public callable whose positional inputs are difficult to verify at call sites.
struct Violation {
    /// Callable used to honor local lint attributes.
    owner: rustc_hir::HirId,
    /// Callable name receiving the diagnostic.
    span: Span,
    /// Number of positional parameters in the signature.
    parameter_count: usize,
    /// Boolean choices whose meaning is hidden at call sites.
    boolean_count: usize,
    /// Same-typed parameter groups that can be accidentally reordered.
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

// -----------------------------------------------------------------------------
// BonParameterHeavyApisWithoutBuilders: Public signature policy
// -----------------------------------------------------------------------------

/// Recommends named construction for large or ambiguous public signatures.
struct BonParameterHeavyApisWithoutBuilders;

dylint_linting::impl_late_lint! {
    #[doc = include_str!("README.md")]
    pub BON_PARAMETER_HEAVY_APIS_WITHOUT_BUILDERS,
    Warn,
    "recommends Bon for public parameter-heavy ambiguous APIs",
    BonParameterHeavyApisWithoutBuilders
}

impl BonParameterHeavyApisWithoutBuilders {
    /// Parameter count at which ambiguity warrants a builder unconditionally.
    const UNCONDITIONAL_PARAMETER_THRESHOLD: usize = 7;

    /// Parameter count at which additional ambiguity warrants a builder.
    const AMBIGUOUS_PARAMETER_THRESHOLD: usize = 5;

    /// Boolean choices sufficient to make a medium-sized signature ambiguous.
    const BOOLEAN_CHOICE_THRESHOLD: usize = 2;

    /// Returns whether this declaration is an exported API that Bon can decorate.
    fn is_eligible(cx: &LateContext<'_>, def_id: LocalDefId) -> bool {
        cx.tcx.effective_visibilities(()).is_exported(def_id)
            && !cx.tcx.opt_local_parent(def_id).is_some_and(|parent| {
                matches!(
                    cx.tcx.def_kind(parent),
                    DefKind::Trait | DefKind::TraitAlias
                )
            })
    }
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
        // Ineligible callables are outside the public authored API policy.
        if !Self::is_eligible(cx, def_id) {
            return;
        }

        // Bodies without an eligible signature cannot establish API parameter pressure.
        let Some(signature) = ParameterSignature::from_body(cx, kind, body, def_id) else {
            return;
        };
        let parameter_count = signature.parameter_count();

        let boolean_count = signature.boolean_parameters().len();
        let ambiguous_groups = signature.ambiguous_groups().len();

        // APIs below every pressure threshold do not warrant a builder abstraction.
        if parameter_count < Self::AMBIGUOUS_PARAMETER_THRESHOLD
            || (parameter_count < Self::UNCONDITIONAL_PARAMETER_THRESHOLD
                && boolean_count < Self::BOOLEAN_CHOICE_THRESHOLD
                && ambiguous_groups == 0)
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
