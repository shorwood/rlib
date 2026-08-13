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
    declaration: String,
    serialize: String,
    deserialize: String,
}

struct Violation {
    span: Span,
    declaration: String,
    serialize: String,
    deserialize: String,
}

impl LateViolation for Violation {
    fn primary_message(&self) -> Cow<'_, str> {
        Cow::Owned(format!(
            "Serde names for `{}` differ by direction",
            self.declaration
        ))
    }

    fn rationale_message(&self) -> Cow<'_, str> {
        Cow::Owned(format!(
            "serialization writes `{}` while deserialization accepts `{}` as the primary name, without an authored compatibility explanation",
            self.serialize, self.deserialize
        ))
    }

    fn remediation_message(&self) -> Cow<'_, str> {
        Cow::Borrowed(
            "use one stable name or document the intentional one-way schema migration at this declaration",
        )
    }

    fn emit(self, cx: &LateContext<'_>) {
        cx.emit_span_lint(
            SERDE_ASYMMETRIC_SERDE_CONTRACTS,
            self.span,
            DiagDecorator(|diag| {
                diag.primary_message(self.primary_message().into_owned());
                diag.span_label(self.span, "this declaration has directional wire names");
                diag.note(self.rationale_message().into_owned());
                diag.help(self.remediation_message().into_owned());
            }),
        );
    }
}

#[derive(Default)]
struct SerdeAsymmetricSerdeContracts {
    catalog: SerdeContractCatalog,
    candidates: Vec<Candidate>,
}

dylint_linting::impl_late_lint! {
    #[doc = include_str!("README.md")]
    pub SERDE_ASYMMETRIC_SERDE_CONTRACTS,
    Warn,
    "finds unexplained directional Serde naming contracts",
    SerdeAsymmetricSerdeContracts::default()
}

impl LateLintPass<'_> for SerdeAsymmetricSerdeContracts {
    fn check_item(&mut self, cx: &LateContext<'_>, item: &Item<'_>) {
        self.catalog.check_item(cx, item);
        if item.span.from_expansion() {
            return;
        }
        let Some(source) = authored_item_source(cx, item) else {
            return;
        };
        let members = match item.kind {
            ItemKind::Struct(..) => {
                let Ok(structure) = syn::parse_str::<syn::ItemStruct>(&source) else {
                    return;
                };
                structure
                    .fields
                    .iter()
                    .filter_map(|field| {
                        Some((field.ident.as_ref()?.to_string(), field.attrs.clone()))
                    })
                    .collect::<Vec<_>>()
            }
            ItemKind::Enum(..) => {
                let Ok(enumeration) = syn::parse_str::<syn::ItemEnum>(&source) else {
                    return;
                };
                enumeration
                    .variants
                    .iter()
                    .map(|variant| (variant.ident.to_string(), variant.attrs.clone()))
                    .collect::<Vec<_>>()
            }
            _ => return,
        };
        for (declaration, attributes) in members {
            if attributes
                .iter()
                .any(|attribute| attribute.path().is_ident("doc"))
            {
                continue;
            }
            let attributes = serde_attributes(&attributes);
            let (Some(serialize), Some(deserialize)) =
                (attributes.rename_serialize, attributes.rename_deserialize)
            else {
                continue;
            };
            if serialize == deserialize {
                continue;
            }
            self.candidates.push(Candidate {
                definition: item.owner_id.def_id,
                span: item.span,
                declaration,
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
                declaration: candidate.declaration,
                serialize: candidate.serialize,
                deserialize: candidate.deserialize,
            }
            .emit(cx);
        }
    }
}
