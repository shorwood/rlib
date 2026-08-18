extern crate rustc_errors;
extern crate rustc_hir;
extern crate rustc_span;

use std::borrow::Cow;
use std::collections::HashSet;

use rustc_errors::DiagDecorator;
use rustc_hir::def_id::LocalDefId;
use rustc_hir::intravisit::FnKind;
use rustc_hir::{Body, Expr, FnDecl, Item};
use rustc_lint::{LateContext, LateLintPass, LintContext};
use rustc_span::Span;

use super::utils::contracts::{SerdeAttributes, SerdeContractCatalog};
use crate::utils::construction_analysis::{ConstructionAnalysis, ConstructionOrigin};
use crate::utils::diagnostic::LateViolation;
use crate::utils::source_provenance::AuthoredItemSource;

// -----------------------------------------------------------------------------
// Violation: Deserialization bypassing invariants
// -----------------------------------------------------------------------------

/// Derived deserializer that can construct a restricted-field type directly.
struct Violation {
    /// Authored type declaration whose invariants deserialization can bypass.
    type_span: Span,
    /// Constructor whose validation establishes the invariant boundary.
    constructor_span: Span,
    /// Authored type or member name involved in the wire contract.
    name: String,
}

impl LateViolation for Violation {
    fn primary_message(&self) -> Cow<'_, str> {
        Cow::Owned(format!(
            "derived `Deserialize` bypasses `{}` invariants",
            self.name
        ))
    }

    fn rationale_message(&self) -> Cow<'_, str> {
        Cow::Borrowed(
            "Serde constructs restricted fields directly instead of passing the decoded value through the type's fallible invariant boundary",
        )
    }

    fn remediation_message(&self) -> Cow<'_, str> {
        Cow::Borrowed(
            "deserialize through `#[serde(try_from = ...)]`, a validating adapter, or a separate wire type",
        )
    }

    fn emit(self, cx: &LateContext<'_>) {
        cx.emit_span_lint(
            SERDE_DESERIALIZATION_BYPASSING_INVARIANTS,
            self.type_span,
            DiagDecorator(|diag| {
                diag.primary_message(self.primary_message().into_owned());
                diag.span_label(
                    self.type_span,
                    "direct field deserialization is derived here",
                );
                diag.span_label(
                    self.constructor_span,
                    "this constructor establishes fallible validation",
                );
                diag.note(self.rationale_message().into_owned());
                diag.help(self.remediation_message().into_owned());
            }),
        );
    }
}

// -----------------------------------------------------------------------------
// SerdeDeserializationBypassingInvariants: Validated construction policy
// -----------------------------------------------------------------------------

/// Rejects derived deserialization that can construct types with restricted fields directly.
#[derive(Default)]
struct SerdeDeserializationBypassingInvariants {
    /// Effective Serde contracts consulted after all local declarations are known.
    catalog: SerdeContractCatalog,
    /// Construction sites correlated with the owning type.
    constructions: ConstructionAnalysis,
    /// Types whose Serde contract already routes decoded data through a fallible conversion.
    validated_deserialization: HashSet<LocalDefId>,
}

crate::impl_late_lint! {
    #[doc = include_str!("README.md")]
    pub SERDE_DESERIALIZATION_BYPASSING_INVARIANTS,
    Warn,
    "rejects Serde deserialization that bypasses checked construction",
    SerdeDeserializationBypassingInvariants::default()
}

impl<'tcx> LateLintPass<'tcx> for SerdeDeserializationBypassingInvariants {
    fn check_item(&mut self, cx: &LateContext<'tcx>, item: &'tcx Item<'tcx>) {
        self.catalog.check_item(cx, item);
        let parsed = AuthoredItemSource::for_item(cx, item).and_then(|source| {
            // Unparseable authored source cannot expose a `try_from` container attribute.
            let Ok(input) = syn::parse_str::<syn::DeriveInput>(&source) else {
                return None;
            };
            Some(input)
        });
        if !item.span.from_expansion()
            && parsed.is_some_and(|input| {
                SerdeAttributes::from_attributes(&input.attrs)
                    .try_from
                    .is_some()
            })
        {
            self.validated_deserialization.insert(item.owner_id.def_id);
        }
        self.constructions.record_item(cx, item);
    }

    fn check_expr(&mut self, cx: &LateContext<'tcx>, expression: &'tcx Expr<'tcx>) {
        self.constructions.record_expression(cx, expression);
    }

    fn check_fn(
        &mut self,
        cx: &LateContext<'tcx>,
        kind: FnKind<'tcx>,
        _: &'tcx FnDecl<'tcx>,
        body: &'tcx Body<'tcx>,
        span: Span,
        def_id: LocalDefId,
    ) {
        self.constructions
            .record_function(cx, kind, body, span, def_id);
    }

    fn check_crate_post(&mut self, cx: &LateContext<'tcx>) {
        let mut reported = HashSet::new();
        for constructor in &self.constructions.candidates {
            if constructor.ownership.origin != ConstructionOrigin::Inherent
                || !constructor.is_fallible_direct()
                || self
                    .validated_deserialization
                    .contains(&constructor.target.def_id)
                || !reported.insert(constructor.target.def_id)
            {
                continue;
            }

            let Some(contract) = self
                .catalog
                .derived_type(constructor.target.def_id, "Deserialize")
            else {
                continue;
            };

            if !contract.has_restricted_fields {
                continue;
            }

            Violation {
                type_span: contract.span,
                constructor_span: constructor.function.name_span,
                name: contract.name.to_string(),
            }
            .emit(cx);
        }
    }
}
