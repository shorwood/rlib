extern crate rustc_errors;
extern crate rustc_hir;
extern crate rustc_span;

use std::borrow::Cow;

use rustc_errors::DiagDecorator;
use rustc_hir::{Item, ItemKind};
use rustc_lint::{LateContext, LateLintPass, LintContext};
use rustc_span::Span;
use rustc_span::def_id::LocalDefId;

use super::contracts::{SerdeContractCatalog, serde_attributes};
use crate::utils::diagnostic::LateViolation;
use crate::utils::source_provenance::authored_item_source;

struct Candidate {
    definition: LocalDefId,
    span: Span,
    field: String,
    serialize: String,
    deserialize: String,
}

struct Violation {
    span: Span,
    field: String,
    serialize: String,
    deserialize: String,
}

impl LateViolation for Violation {
    fn primary_message(&self) -> Cow<'_, str> {
        Cow::Owned(format!(
            "Serde adapters for `{}` do not round-trip",
            self.field
        ))
    }

    fn rationale_message(&self) -> Cow<'_, str> {
        Cow::Owned(format!(
            "serialization uses `{}` while deserialization uses the incompatible `{}` contract",
            self.serialize, self.deserialize
        ))
    }

    fn remediation_message(&self) -> Cow<'_, str> {
        Cow::Borrowed("pair helpers with the same unit or encoding, or use one `with` module")
    }

    fn emit(self, cx: &LateContext<'_>) {
        cx.emit_span_lint(
            SERDE_NON_ROUNDTRIPPING_SERDE_ADAPTERS,
            self.span,
            DiagDecorator(|diag| {
                diag.primary_message(self.primary_message().into_owned());
                diag.span_label(self.span, "these directional adapters disagree");
                diag.note(self.rationale_message().into_owned());
                diag.help(self.remediation_message().into_owned());
            }),
        );
    }
}

#[derive(Default)]
struct SerdeNonRoundtrippingSerdeAdapters {
    catalog: SerdeContractCatalog,
    candidates: Vec<Candidate>,
}

dylint_linting::impl_late_lint! {
    #[doc = include_str!("README.md")]
    pub SERDE_NON_ROUNDTRIPPING_SERDE_ADAPTERS,
    Warn,
    "finds provably incompatible directional Serde adapters",
    SerdeNonRoundtrippingSerdeAdapters::default()
}

impl LateLintPass<'_> for SerdeNonRoundtrippingSerdeAdapters {
    fn check_item(&mut self, cx: &LateContext<'_>, item: &Item<'_>) {
        self.catalog.check_item(cx, item);
        if item.span.from_expansion() || !matches!(item.kind, ItemKind::Struct(..)) {
            return;
        }
        let Some(source) = authored_item_source(cx, item) else {
            return;
        };
        let Ok(structure) = syn::parse_str::<syn::ItemStruct>(&source) else {
            return;
        };
        for field in &structure.fields {
            let Some(name) = field.ident.as_ref() else {
                continue;
            };
            let attributes = serde_attributes(&field.attrs);
            let (Some(serialize), Some(deserialize)) =
                (attributes.serialize_with, attributes.deserialize_with)
            else {
                continue;
            };
            if !provably_incompatible(&serialize, &deserialize) {
                continue;
            }
            self.candidates.push(Candidate {
                definition: item.owner_id.def_id,
                span: item.span,
                field: name.to_string(),
                serialize,
                deserialize,
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
                serialize: candidate.serialize,
                deserialize: candidate.deserialize,
            }
            .emit(cx);
        }
    }
}

fn provably_incompatible(serialize: &str, deserialize: &str) -> bool {
    let Some((serialize_family, serialize_contract)) = named_contract(serialize) else {
        return false;
    };
    let Some((deserialize_family, deserialize_contract)) = named_contract(deserialize) else {
        return false;
    };
    serialize_family == deserialize_family && serialize_contract != deserialize_contract
}

fn named_contract(path: &str) -> Option<(String, &'static str)> {
    const CONTRACTS: &[&str] = &[
        "milliseconds",
        "microseconds",
        "nanoseconds",
        "seconds",
        "base64",
        "hex",
    ];
    let name = path.rsplit("::").next()?;
    CONTRACTS.iter().find_map(|&contract| {
        name.strip_suffix(contract)
            .map(|family| (family.trim_end_matches('_').to_owned(), contract))
    })
}
