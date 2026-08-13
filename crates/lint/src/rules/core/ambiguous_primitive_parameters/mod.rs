extern crate rustc_errors;
extern crate rustc_hir;
extern crate rustc_span;

use std::borrow::Cow;
use std::collections::HashSet;

use rustc_errors::DiagDecorator;
use rustc_hir::intravisit::FnKind;
use rustc_hir::{Body, FieldDef, FnDecl, HirId, Item, TraitItem};
use rustc_lint::{LateContext, LateLintPass};
use rustc_span::def_id::LocalDefId;
use rustc_span::{Span, Symbol};

use crate::utils::diagnostic::LateViolation;
use crate::utils::parameter_analysis::{ParameterGroup, ParameterSignature};
use crate::utils::parameter_kind::ParameterKind;
use crate::utils::string_domain_analysis::DomainAnalyzer;

// -----------------------------------------------------------------------------
// Violation: Interchangeable primitive role diagnostic
// -----------------------------------------------------------------------------

/// One named parameter role retained as violation evidence.
struct ViolationParameter {
    /// Authored binding name.
    name: Symbol,
    /// Binding span used for a role-specific label.
    span: Span,
}

/// One primitive representation carrying several distinct parameter roles.
struct Violation {
    /// Function or method node used to anchor the lint level.
    hir_id: HirId,
    /// First ambiguous parameter span used as the primary diagnostic location.
    span: Span,
    /// Shared primitive representation hidden behind distinct authored roles.
    kind: ParameterKind,
    /// Parameter names and spans that remain type-interchangeable.
    parameters: Vec<ViolationParameter>,
}

impl Violation {
    /// Captures the complete diagnostic evidence from a borrowed analysis group.
    fn from_group(hir_id: HirId, group: &ParameterGroup<'_>) -> Self {
        // Own every parameter role before the borrowed analysis group is discarded.
        let parameters = group
            .parameters
            .iter()
            .map(|parameter| ViolationParameter {
                name: parameter.name,
                span: parameter.span,
            })
            .collect();

        // Preserve the shared representation beside its independently owned roles.
        Self {
            hir_id,
            span: group.parameters[0].span,
            kind: group.kind,
            parameters,
        }
    }

    /// Renders every authored role in the interchangeable parameter family.
    fn parameter_names(&self) -> String {
        let names = self
            .parameters
            .iter()
            .map(|parameter| format!("`{}`", parameter.name));
        names.collect::<Vec<_>>().join(", ")
    }
}

impl LateViolation for Violation {
    fn primary_message(&self) -> Cow<'_, str> {
        Cow::Owned(format!(
            "{} parameters {} carry distinct roles but remain interchangeable",
            self.kind.description(),
            self.parameter_names()
        ))
    }

    fn rationale_message(&self) -> Cow<'_, str> {
        Cow::Owned(format!(
            "the compiler cannot prevent callers from swapping {} because their domain roles exist only in parameter names",
            self.parameter_names()
        ))
    }

    fn remediation_message(&self) -> Cow<'_, str> {
        if self.kind == ParameterKind::Text {
            Cow::Borrowed(
                "introduce domain-specific string types, or group operation-specific text in a named request",
            )
        } else {
            Cow::Borrowed(
                "introduce role-specific newtypes, or group values belonging to one operation in a named struct",
            )
        }
    }

    fn emit(self, cx: &LateContext<'_>) {
        // Render the three stable diagnostic layers before moving label evidence.
        let primary_message = self.primary_message().into_owned();
        let rationale_message = self.rationale_message().into_owned();
        let remediation_message = self.remediation_message().into_owned();

        // Emit the summary before consuming the independently owned role labels.
        cx.tcx.emit_node_span_lint(
            AMBIGUOUS_PRIMITIVE_PARAMETERS,
            self.hir_id,
            self.span,
            DiagDecorator(|diag| {
                diag.primary_message(primary_message);
                for parameter in self.parameters {
                    diag.span_label(parameter.span, format!("role `{}`", parameter.name));
                }
                diag.note(rationale_message);
                diag.help(remediation_message);
            }),
        );
    }
}

// -----------------------------------------------------------------------------
// AmbiguousPrimitiveParameters: Type-safe parameter role policy
// -----------------------------------------------------------------------------

/// Collects authored signatures before applying string-domain diagnostic precedence.
#[derive(Default)]
struct AmbiguousPrimitiveParameters {
    /// Function and method signatures awaiting crate-wide evidence.
    signatures: Vec<ParameterSignature>,
    /// String-domain evidence that can supersede a local textual-parameter warning.
    strings: DomainAnalyzer,
}

dylint_linting::impl_late_lint! {
    #[doc = include_str!("README.md")]
    pub AMBIGUOUS_PRIMITIVE_PARAMETERS,
    Warn,
    "detects primitive parameters whose distinct domain roles are type-interchangeable",
    AmbiguousPrimitiveParameters::default()
}

impl AmbiguousPrimitiveParameters {
    /// Emits every nonsuperseded primitive finding for one signature.
    fn emit_signature(
        cx: &LateContext<'_>,
        signature: &ParameterSignature,
        superseded: &HashSet<LocalDefId>,
    ) {
        for group in signature.ambiguous_groups() {
            let is_superseded_text =
                group.kind == ParameterKind::Text && superseded.contains(&signature.def_id);
            if is_superseded_text {
                continue;
            }
            Violation::from_group(signature.hir_id, &group).emit(cx);
        }
    }
}

impl<'tcx> LateLintPass<'tcx> for AmbiguousPrimitiveParameters {
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
        self.strings.record_function(cx, &signature, body);
        self.signatures.push(signature);
    }

    fn check_trait_item(&mut self, cx: &LateContext<'tcx>, item: &'tcx TraitItem<'tcx>) {
        let Some(signature) = ParameterSignature::from_required_trait(cx, item) else {
            return;
        };
        self.signatures.push(signature);
    }

    fn check_field_def(&mut self, cx: &LateContext<'tcx>, field: &'tcx FieldDef<'tcx>) {
        self.strings.record_field(cx, field);
    }

    fn check_item(&mut self, cx: &LateContext<'tcx>, item: &'tcx Item<'tcx>) {
        self.strings.record_item(cx, item);
    }

    fn check_crate_post(&mut self, cx: &LateContext<'tcx>) {
        let superseded = self.strings.stronger_function_ids();
        for signature in &self.signatures {
            Self::emit_signature(cx, signature, &superseded);
        }
    }
}
