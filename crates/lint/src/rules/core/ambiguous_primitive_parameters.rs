extern crate rustc_errors;
extern crate rustc_hir;
extern crate rustc_span;

use rustc_errors::DiagDecorator;
use rustc_hir::intravisit::FnKind;
use rustc_hir::{Body, FieldDef, FnDecl, Item, TraitItem};
use rustc_lint::{LateContext, LateLintPass};
use rustc_span::Span;
use rustc_span::def_id::LocalDefId;

use crate::utils::parameter_analysis::{ParameterGroup, ParameterKind, ParameterSignature};
use crate::utils::string_domain_analysis::DomainAnalyzer;

// -----------------------------------------------------------------------------
// AmbiguousPrimitiveParameters
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
    /// ### What it does
    ///
    /// Finds function and method signatures where parameters with the same primitive
    /// representation carry distinct domain roles. Scalar aliases are resolved semantically, and
    /// owned and borrowed UTF-8 strings form one textual family. Conventional coordinates,
    /// bounds, operands, ranges, dimensions, and generic text operations remain accepted.
    ///
    /// ### Why is this bad?
    ///
    /// Primitive values communicate representation but not meaning. Swapping two identifiers,
    /// limits, or credentials remains valid Rust and may survive review because the call contains
    /// no type-level explanation of either position. Generated code is especially prone to
    /// extending such signatures instead of discovering the domain concepts they represent.
    ///
    /// For example, every argument here is interchangeable:
    ///
    /// ```rust
    /// fn schedule(user: u64, project: u64, delay: u64) {}
    /// ```
    ///
    /// Give reusable identities their own types and group operation-specific values in a named
    /// request:
    ///
    /// ```rust
    /// struct UserId(u64);
    /// struct ProjectId(u64);
    /// struct ScheduleOptions {
    ///     delay: Duration,
    /// }
    ///
    /// fn schedule(user: UserId, project: ProjectId, options: ScheduleOptions) {}
    /// ```
    pub AMBIGUOUS_PRIMITIVE_PARAMETERS,
    Warn,
    "detects primitive parameters whose distinct domain roles are type-interchangeable",
    AmbiguousPrimitiveParameters::default()
}

impl AmbiguousPrimitiveParameters {
    /// Renders the authored roles in one interchangeable parameter family.
    fn group_names(group: &ParameterGroup<'_>) -> String {
        let names = group
            .parameters
            .iter()
            .map(|parameter| format!("`{}`", parameter.name));
        names.collect::<Vec<_>>().join(", ")
    }

    /// Emits one focused diagnostic for an interchangeable parameter family.
    fn emit_group(
        cx: &LateContext<'_>,
        signature: &ParameterSignature,
        group: &ParameterGroup<'_>,
    ) {
        // Name the shared representation and every role hidden behind it.
        let names = Self::group_names(group);
        let message = format!(
            "{} parameters {names} carry distinct roles but remain interchangeable",
            group.kind.description()
        );

        // Attach role-specific labels and remediation for the shared representation.
        cx.tcx.emit_node_span_lint(
            AMBIGUOUS_PRIMITIVE_PARAMETERS,
            signature.hir_id,
            group.parameters[0].span,
            DiagDecorator(|diag| {
                diag.primary_message(message);

                // Make every interchangeable role visible at its authored binding.
                for parameter in &group.parameters {
                    diag.span_label(parameter.span, format!("role `{}`", parameter.name));
                }

                // Prefer domain types for text and role types for scalar representations.
                let help = if group.kind == ParameterKind::Text {
                    "introduce domain-specific string types, or group operation-specific text in a named request"
                } else {
                    "introduce role-specific newtypes, or group values belonging to one operation in a named struct"
                };
                diag.help(help);
            }),
        );
    }

    /// Emits every nonsuperseded primitive finding for one signature.
    fn emit_signature(
        cx: &LateContext<'_>,
        signature: &ParameterSignature,
        superseded: &std::collections::HashSet<LocalDefId>,
    ) {
        for group in signature.ambiguous_groups() {
            let is_superseded_text =
                group.kind == ParameterKind::Text && superseded.contains(&signature.def_id);
            if is_superseded_text {
                continue;
            }
            Self::emit_group(cx, signature, &group);
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
