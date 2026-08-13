extern crate rustc_errors;
extern crate rustc_hir;
extern crate rustc_span;

use std::borrow::Cow;

use rustc_errors::DiagDecorator;
use rustc_hir::{Item, ItemKind};
use rustc_lint::{LateContext, LateLintPass, LintContext};
use rustc_span::Span;
use rustc_span::def_id::LocalDefId;

use super::contracts::{SerdeAttributes, SerdeContractCatalog, SerdeFlag};
use crate::utils::diagnostic::LateViolation;
use crate::utils::source_provenance::AuthoredItemSource;

/// Carries the `Candidate` state used by this analysis.
struct Candidate {
    /// Stores the `definition` value used by this analysis.
    definition: LocalDefId,
    /// Stores the `span` value used by this analysis.
    span: Span,
    /// Stores the `field` value used by this analysis.
    field: String,
    /// Stores the `predicate` value used by this analysis.
    predicate: String,
}

/// Carries the `Violation` state used by this analysis.
struct Violation {
    /// Stores the `span` value used by this analysis.
    span: Span,
    /// Stores the `field` value used by this analysis.
    field: String,
    /// Stores the `predicate` value used by this analysis.
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

/// Performs the `is_option` step of the lint analysis.
fn is_option(ty: &syn::Type) -> bool {
    matches!(ty, syn::Type::Path(path)
        if path.path.segments.last().is_some_and(|segment| segment.ident == "Option"))
}

#[derive(Default)]
/// Carries the `SerdeLossyConditionalSerialization` state used by this analysis.
struct SerdeLossyConditionalSerialization {
    /// Stores the `catalog` value used by this analysis.
    catalog: SerdeContractCatalog,
    /// Stores the `candidates` value used by this analysis.
    candidates: Vec<Candidate>,
}

dylint_linting::impl_late_lint! {
    #[doc = include_str!("README.md")]
    pub SERDE_LOSSY_CONDITIONAL_SERIALIZATION,
    Warn,
    "finds Serde skip predicates whose omissions cannot deserialize",
    SerdeLossyConditionalSerialization::default()
}

impl LateLintPass<'_> for SerdeLossyConditionalSerialization {
    fn check_item(&mut self, cx: &LateContext<'_>, item: &Item<'_>) {
        self.catalog.check_item(cx, item);
        if item.span.from_expansion() || !matches!(item.kind, ItemKind::Struct(..)) {
            return;
        }
        let Some(source) = AuthoredItemSource::for_item(cx, item) else {
            return;
        };
        let Ok(structure) = syn::parse_str::<syn::ItemStruct>(&source) else {
            return;
        };
        for field in &structure.fields {
            // Prepare the values used by this stage.
            let Some(name) = field.ident.as_ref() else {
                continue;
            };
            let attributes = SerdeAttributes::analyze_serde_attributes(&field.attrs);
            let has_deserialization_fallback =
                attributes.has(SerdeFlag::HasDefault) || attributes.has(SerdeFlag::SkipDeserialize);

            // Prepare the values used by this stage.
            let Some(predicate) = attributes.skip_serializing_if else {
                continue;
            };

            // Reject inputs that do not satisfy this stage.
            if has_deserialization_fallback
                || is_option(&field.ty)
                || field
                    .attrs
                    .iter()
                    .any(|attribute| attribute.path().is_ident("doc"))
            {
                continue;
            }

            // Update the accumulated analysis state.
            self.candidates.push(Candidate {
                definition: item.owner_id.def_id,
                span: item.span,
                field: name.to_string(),
                predicate,
            });
        }
    }

    fn check_crate_post(&mut self, cx: &LateContext<'_>) {
        for analyze_candidate in self.candidates.drain(..) {
            // Reject inputs that do not satisfy this stage.
            if self
                .catalog
                .derived_type(analyze_candidate.definition, "Serialize")
                .is_none()
                || self
                    .catalog
                    .derived_type(analyze_candidate.definition, "Deserialize")
                    .is_none()
            // Perform the next step of the analysis.
            {
                continue;
            }

            // Perform the next step of the analysis.
            Violation {
                span: analyze_candidate.span,
                field: analyze_candidate.field,
                predicate: analyze_candidate.predicate,
            }
            .emit(cx);
        }
    }
}
