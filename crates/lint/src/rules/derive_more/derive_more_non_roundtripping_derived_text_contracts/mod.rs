extern crate rustc_errors;
extern crate rustc_hir;
extern crate rustc_middle;
extern crate rustc_span;

use std::borrow::Cow;
use std::collections::HashMap;

use rustc_errors::DiagDecorator;
use rustc_hir::{Item, ItemKind};
use rustc_lint::{LateContext, LateLintPass, LintContext};
use rustc_middle::ty;
use rustc_span::def_id::LocalDefId;
use rustc_span::{Span, Symbol};

use super::utils::contracts::DeriveMoreContractCatalog;
use crate::utils::diagnostic::LateViolation;
use crate::utils::source_provenance::AuthoredItemSource;

// -----------------------------------------------------------------------------
// Violation: Generated text contract that cannot round-trip
// -----------------------------------------------------------------------------

/// Derived display grammar awaiting comparison with the type's parser.
struct Candidate {
    /// Authored declaration or expression range used as the diagnostic anchor.
    span: Span,
    /// Local type name used to identify the affected derive contract.
    name: Symbol,
    /// Explicit display grammar that the transparent parser cannot consume.
    format: String,
}

/// Derived display and parsing pair proven unable to round-trip.
struct Violation(
    /// Derived text contract that cannot round-trip.
    Candidate,
);

impl LateViolation for Violation {
    fn primary_message(&self) -> Cow<'_, str> {
        Cow::Owned(format!(
            "derived text contracts for `{}` cannot round trip",
            self.0.name
        ))
    }

    fn rationale_message(&self) -> Cow<'_, str> {
        Cow::Owned(format!(
            "`Display` uses `{}`, but `FromStr` forwards the complete output to the numeric field parser",
            self.0.format
        ))
    }

    fn remediation_message(&self) -> Cow<'_, str> {
        Cow::Borrowed(
            "make `Display` transparent or replace the derived parser with an authored `FromStr` implementation for the same grammar",
        )
    }

    fn emit(self, cx: &LateContext<'_>) {
        cx.emit_span_lint(
            DERIVE_MORE_NON_ROUNDTRIPPING_DERIVED_TEXT_CONTRACTS,
            self.0.span,
            DiagDecorator(|diag| {
                diag.primary_message(self.primary_message().into_owned());
                diag.span_label(self.0.span, "incompatible text traits are derived here");
                diag.note(self.rationale_message().into_owned());
                diag.help(self.remediation_message().into_owned());
            }),
        );
    }
}

// -----------------------------------------------------------------------------
// DeriveMoreNonRoundtrippingDerivedTextContracts: Round-trip text policy
// -----------------------------------------------------------------------------

/// Correlates generated display grammars with transparent parsing behavior.
#[derive(Default)]
struct DeriveMoreNonRoundtrippingDerivedTextContracts {
    /// Authored type contracts and `derive_more` expansions consulted by this rule.
    catalog: DeriveMoreContractCatalog,
    /// Authored declarations awaiting association with `derive_more` expansions.
    candidates: HashMap<LocalDefId, Candidate>,
}

dylint_linting::impl_late_lint! {
    #[doc = include_str!("README.md")]
    pub DERIVE_MORE_NON_ROUNDTRIPPING_DERIVED_TEXT_CONTRACTS,
    Warn,
    "rejects provably incompatible derive_more Display and FromStr contracts",
    DeriveMoreNonRoundtrippingDerivedTextContracts::default()
}

impl DeriveMoreNonRoundtrippingDerivedTextContracts {
    /// Recovers an authored `derive_more` display format that adds text around a field.
    fn explicit_nontransparent_format(cx: &LateContext<'_>, item: &Item<'_>) -> Option<String> {
        let source = AuthoredItemSource::for_item(cx, item)?;
        let item = match syn::parse_str::<syn::ItemStruct>(&source) {
            Ok(item) => item,
            Err(_error) => return None,
        };

        let attribute = item
            .attrs
            .iter()
            .find(|attribute| attribute.path().is_ident("display"))?;
        let format = match attribute.parse_args::<syn::LitStr>() {
            Ok(format) => format,
            Err(_error) => return None,
        };

        let format = format.value();
        (!matches!(format.as_str(), "{}" | "{_0}" | "{0}")).then_some(format)
    }

    /// Returns whether the newtype field uses a primitive numeric parser.
    fn has_numeric_field(cx: &LateContext<'_>, definition: LocalDefId) -> bool {
        let definition = cx.tcx.adt_def(definition);
        let Some(field) = definition.non_enum_variant().fields.iter().next() else {
            return false;
        };
        matches!(
            field.ty(cx.tcx, ty::GenericArgs::empty()).kind(),
            ty::Int(_) | ty::Uint(_) | ty::Float(_)
        )
    }
}
impl<'tcx> LateLintPass<'tcx> for DeriveMoreNonRoundtrippingDerivedTextContracts {
    fn check_item(&mut self, cx: &LateContext<'tcx>, item: &'tcx Item<'tcx>) {
        self.catalog.check_item(cx, item);
        if item.span.from_expansion() {
            return;
        }
        let ItemKind::Struct(identifier, _, data) = item.kind else {
            return;
        };

        if data.fields().len() != 1 {
            return;
        }
        let Some(format) = Self::explicit_nontransparent_format(cx, item) else {
            return;
        };

        self.candidates.insert(
            item.owner_id.def_id,
            Candidate {
                span: identifier.span,
                name: identifier.name,
                format,
            },
        );
    }

    fn check_crate_post(&mut self, cx: &LateContext<'tcx>) {
        for (definition, candidate) in self.candidates.drain() {
            if self.catalog.derived_type(definition, "Display").is_none()
                || self.catalog.derived_type(definition, "FromStr").is_none()
                || !Self::has_numeric_field(cx, definition)
            {
                continue;
            }
            Violation(candidate).emit(cx);
        }
    }
}
