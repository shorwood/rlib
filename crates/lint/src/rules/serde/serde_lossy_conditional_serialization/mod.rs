extern crate rustc_errors;
extern crate rustc_hir;
extern crate rustc_span;

use std::borrow::Cow;

use rustc_errors::DiagDecorator;
use rustc_hir::{Item, ItemKind};
use rustc_lint::{LateContext, LateLintPass, LintContext};
use rustc_span::def_id::LocalDefId;
use rustc_span::{Span, sym};

use super::utils::contracts::{
    SerdeAttributes, SerdeAuthoredField, SerdeAuthoredFieldSet, SerdeContractCatalog, SerdeFlag,
};
use crate::utils::diagnostic::LateViolation;
use crate::utils::source_provenance::AuthoredItemSource;

// -----------------------------------------------------------------------------
// Violation: Conditional serialization that loses information
// -----------------------------------------------------------------------------

/// Conditionally omitted field awaiting domain-type classification.
struct Candidate {
    /// Local declaration identity used to associate evidence collected in separate passes.
    definition: LocalDefId,
    /// Authored declaration or expression range used as the diagnostic anchor.
    span: Span,
    /// Field name quoted in the diagnostic.
    field: String,
    /// Authored omission predicate that can discard a meaningful value.
    predicate: String,
}

/// Omission predicate that discards a value deserialization cannot recover.
struct Violation {
    /// Authored declaration or expression range used as the diagnostic anchor.
    span: Span,
    /// Field name quoted in the diagnostic.
    field: String,
    /// Omission predicate responsible for the lossy wire representation.
    predicate: String,
}

impl LateViolation for Violation {
    fn primary_message(&self) -> Cow<'_, str> {
        Cow::Owned(format!(
            "conditional serialization of `{}` is not reversible",
            self.field
        ))
    }

    fn rationale_message(&self) -> Cow<'_, str> {
        Cow::Owned(format!(
            "`{}` can omit this required field, but deserialization has no optional or default value for the missing key",
            self.predicate
        ))
    }

    fn remediation_message(&self) -> Cow<'_, str> {
        Cow::Borrowed(
            "add a matching deserialization default, make the field optional, or remove the skip predicate",
        )
    }

    fn emit(self, cx: &LateContext<'_>) {
        cx.emit_span_lint(
            SERDE_LOSSY_CONDITIONAL_SERIALIZATION,
            self.span,
            DiagDecorator(|diag| {
                diag.primary_message(self.primary_message().into_owned());
                diag.span_label(self.span, "this omission produces input the type rejects");
                diag.note(self.rationale_message().into_owned());
                diag.help(self.remediation_message().into_owned());
            }),
        );
    }
}

// -----------------------------------------------------------------------------
// SerdeLossyConditionalSerialization: Round-trip preservation policy
// -----------------------------------------------------------------------------

/// Rejects conditional serialization that cannot reconstruct the omitted domain value.
#[derive(Default)]
struct SerdeLossyConditionalSerialization {
    /// Effective Serde contracts consulted after all local declarations are known.
    catalog: SerdeContractCatalog,
    /// Authored Serde declarations awaiting crate-wide contract comparison.
    candidates: Vec<Candidate>,
}

crate::impl_late_lint! {
    #[doc = include_str!("README.md")]
    pub SERDE_LOSSY_CONDITIONAL_SERIALIZATION,
    Warn,
    "finds Serde skip predicates whose omissions cannot deserialize",
    SerdeLossyConditionalSerialization::default()
}

impl SerdeLossyConditionalSerialization {
    /// Returns whether the type is the standard optional container.
    fn is_option(cx: &LateContext<'_>, field: LocalDefId) -> bool {
        cx.tcx
            .type_of(field)
            .instantiate_identity()
            .ty_adt_def()
            .is_some_and(|definition| cx.tcx.is_diagnostic_item(sym::Option, definition.did()))
    }
}

impl LateLintPass<'_> for SerdeLossyConditionalSerialization {
    fn check_item(&mut self, cx: &LateContext<'_>, item: &Item<'_>) {
        self.catalog.check_item(cx, item);

        // Generated and non-data items cannot define authored field serialization policy.
        if item.span.from_expansion()
            || !matches!(item.kind, ItemKind::Struct(..) | ItemKind::Enum(..))
        {
            return;
        }

        // Missing authored source prevents recovery of conditional serialization attributes.
        let Some(source) = AuthoredItemSource::for_item(cx, item) else {
            return;
        };

        // Items without an authored field model expose no serialization conditions to inspect.
        let Some(SerdeAuthoredFieldSet { fields, .. }) =
            SerdeAuthoredFieldSet::for_item(item, &source)
        else {
            return;
        };
        for SerdeAuthoredField {
            name: field,
            attributes: authored_attributes,
            definition: field_definition,
        } in fields
        {
            let attributes = SerdeAttributes::from_attributes(&authored_attributes);
            let has_deserialization_fallback = attributes.has_flag(SerdeFlag::HasDefault)
                || attributes.has_flag(SerdeFlag::SkipDeserialize);

            let Some(predicate) = attributes.skip_serializing_if else {
                continue;
            };

            if has_deserialization_fallback || Self::is_option(cx, field_definition) {
                continue;
            }

            self.candidates.push(Candidate {
                definition: item.owner_id.def_id,
                span: item.span,
                field,
                predicate,
            });
        }
    }

    fn check_crate_post(&mut self, cx: &LateContext<'_>) {
        for candidate in self.candidates.drain(..) {
            if self
                .catalog
                .derived_type(candidate.definition, "Serialize")
                .is_none()
                || self
                    .catalog
                    .derived_type(candidate.definition, "Deserialize")
                    .is_none()
            {
                continue;
            }

            Violation {
                span: candidate.span,
                field: candidate.field,
                predicate: candidate.predicate,
            }
            .emit(cx);
        }
    }
}
