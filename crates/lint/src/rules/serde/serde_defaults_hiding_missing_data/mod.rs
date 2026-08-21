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
// Violation: Default hiding required missing data
// -----------------------------------------------------------------------------

/// Defaulted field awaiting classification of its domain significance.
struct Candidate {
    /// Local declaration identity used to associate evidence collected in separate passes.
    definition: LocalDefId,
    /// Authored declaration or expression range used as the diagnostic anchor.
    span: Span,
    /// Field name quoted in the diagnostic.
    field: String,
}

/// Required domain value silently synthesized when wire data is absent.
struct Violation {
    /// Authored declaration or expression range used as the diagnostic anchor.
    span: Span,
    /// Field name quoted in the diagnostic.
    field: String,
}

impl LateViolation for Violation {
    fn primary_message(&self) -> Cow<'_, str> {
        Cow::Owned(format!(
            "missing `{}` data is replaced with an implicit default",
            self.field
        ))
    }

    fn rationale_message(&self) -> Cow<'_, str> {
        Cow::Borrowed(
            "bare `#[serde(default)]` turns absence into a potentially meaningful domain value without naming the compatibility policy",
        )
    }

    fn remediation_message(&self) -> Cow<'_, str> {
        Cow::Borrowed(
            "use `Option`, a named default function, or document why the generic default represents missing input",
        )
    }

    fn emit(self, cx: &LateContext<'_>) {
        cx.emit_span_lint(
            SERDE_DEFAULTS_HIDING_MISSING_DATA,
            self.span,
            DiagDecorator(|diag| {
                diag.primary_message(self.primary_message().into_owned());
                diag.span_label(self.span, "missing input is silently defaulted here");
                diag.note(self.rationale_message().into_owned());
                diag.help(self.remediation_message().into_owned());
            }),
        );
    }
}

// -----------------------------------------------------------------------------
// SerdeDefaultsHidingMissingData: Explicit missing-data policy
// -----------------------------------------------------------------------------

/// Rejects defaults that turn absent required domain data into plausible values.
#[derive(Default)]
struct SerdeDefaultsHidingMissingData {
    /// Effective Serde contracts consulted after all local declarations are known.
    catalog: SerdeContractCatalog,
    /// Authored Serde declarations awaiting crate-wide contract comparison.
    candidates: Vec<Candidate>,
}

crate::impl_late_lint! {
    #[doc = include_str!("README.md")]
    pub SERDE_DEFAULTS_HIDING_MISSING_DATA,
    Warn,
    "finds implicit Serde defaults that hide missing domain data",
    SerdeDefaultsHidingMissingData::default()
}

impl SerdeDefaultsHidingMissingData {
    /// Returns whether documentation actually explains the missing-data policy.
    fn has_documented_default(attributes: &[syn::Attribute]) -> bool {
        attributes
            .iter()
            .filter(|attribute| attribute.path().is_ident("doc"))
            .filter_map(|attribute| {
                // Non-name-value doc attributes contribute no missing-data policy prose.
                let Ok(value) = attribute.meta.require_name_value() else {
                    return None;
                };
                Some(value)
            })
            .filter_map(|value| match &value.value {
                syn::Expr::Lit(expression) => match &expression.lit {
                    syn::Lit::Str(value) => Some(value.value().to_ascii_lowercase()),
                    _ => None,
                },
                _ => None,
            })
            .any(|documentation| {
                ["absent", "default", "legacy", "missing"]
                    .iter()
                    .any(|term| documentation.contains(term))
            })
    }

    /// Returns whether the resolved field type is the standard optional container.
    fn is_option(cx: &LateContext<'_>, field: LocalDefId) -> bool {
        cx.tcx
            .type_of(field)
            .instantiate_identity()
            .ty_adt_def()
            .is_some_and(|definition| cx.tcx.is_diagnostic_item(sym::Option, definition.did()))
    }
}

impl LateLintPass<'_> for SerdeDefaultsHidingMissingData {
    fn check_item(&mut self, cx: &LateContext<'_>, item: &Item<'_>) {
        self.catalog.check_item(cx, item);

        // Generated and non-data items cannot define authored field-default policy.
        if item.span.from_expansion()
            || !matches!(item.kind, ItemKind::Struct(..) | ItemKind::Enum(..))
        {
            return;
        }

        // Missing authored source prevents recovery of default attributes and documentation.
        let Some(source) = AuthoredItemSource::for_item(cx, item) else {
            return;
        };

        // Items without an authored field model expose no defaults to evaluate.
        let Some(SerdeAuthoredFieldSet { fields, .. }) =
            SerdeAuthoredFieldSet::for_item(item, &source)
        else {
            return;
        };

        for SerdeAuthoredField {
            name: field,
            attributes,
            definition: field_definition,
        } in fields
        {
            let serde = SerdeAttributes::from_attributes(&attributes);
            if Self::has_documented_default(&attributes)
                || Self::is_option(cx, field_definition)
                || serde.has_flag(SerdeFlag::SkipDeserialize)
                || !serde.has_flag(SerdeFlag::ImplicitDefault)
            {
                continue;
            }

            self.candidates.push(Candidate {
                definition: item.owner_id.def_id,
                span: item.span,
                field,
            });
        }
    }

    fn check_crate_post(&mut self, cx: &LateContext<'_>) {
        for candidate in self.candidates.drain(..) {
            if self
                .catalog
                .derived_type(candidate.definition, "Deserialize")
                .is_none()
            {
                continue;
            }

            Violation {
                span: candidate.span,
                field: candidate.field,
            }
            .emit(cx);
        }
    }
}
