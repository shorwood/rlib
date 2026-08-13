extern crate rustc_errors;
extern crate rustc_hir;
extern crate rustc_span;

use std::borrow::Cow;

use rustc_errors::DiagDecorator;
use rustc_hir::{Item, ItemKind};
use rustc_lint::{LateContext, LateLintPass, LintContext};
use rustc_span::Span;
use rustc_span::def_id::LocalDefId;

use super::utils::contracts::{SerdeAttributes, SerdeContractCatalog, SerdeFlag};
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

dylint_linting::impl_late_lint! {
    #[doc = include_str!("README.md")]
    pub SERDE_DEFAULTS_HIDING_MISSING_DATA,
    Warn,
    "finds implicit Serde defaults that hide missing domain data",
    SerdeDefaultsHidingMissingData::default()
}

impl SerdeDefaultsHidingMissingData {
    /// Returns whether the type is the standard optional container.
    fn is_option(ty: &syn::Type) -> bool {
        matches!(ty, syn::Type::Path(path)
        if path.path.segments.last().is_some_and(|segment| segment.ident == "Option"))
    }
}
impl LateLintPass<'_> for SerdeDefaultsHidingMissingData {
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
            let Some(name) = field.ident.as_ref() else {
                continue;
            };

            if field
                .attrs
                .iter()
                .any(|attribute| attribute.path().is_ident("doc"))
                || Self::is_option(&field.ty)
                || !SerdeAttributes::from_attributes(&field.attrs).has(SerdeFlag::ImplicitDefault)
            {
                continue;
            }

            self.candidates.push(Candidate {
                definition: item.owner_id.def_id,
                span: item.span,
                field: name.to_string(),
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
