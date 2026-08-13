extern crate rustc_errors;
extern crate rustc_hir;
extern crate rustc_span;

use std::borrow::Cow;

use rustc_errors::DiagDecorator;
use rustc_hir::{Item, ItemKind};
use rustc_lint::{LateContext, LateLintPass, LintContext};
use rustc_span::Span;
use rustc_span::def_id::LocalDefId;

use super::contracts::{SerdeContractCatalog, SerdeFlag, serde_attributes};
use crate::utils::diagnostic::LateViolation;
use crate::utils::source_provenance::authored_item_source;

struct Candidate {
    definition: LocalDefId,
    span: Span,
    fields: Vec<String>,
}

struct Violation {
    span: Span,
    fields: Vec<String>,
}

impl LateViolation for Violation {
    fn primary_message(&self) -> Cow<'_, str> {
        Cow::Borrowed("Serde flattening conflicts with the container's unknown-field policy")
    }

    fn rationale_message(&self) -> Cow<'_, str> {
        Cow::Owned(format!(
            "flattened {} {} {} an open key namespace into a container that promises to reject unknown keys",
            if self.fields.len() == 1 {
                "field"
            } else {
                "fields"
            },
            self.fields.join(", "),
            if self.fields.len() == 1 {
                "merges"
            } else {
                "merge"
            }
        ))
    }

    fn remediation_message(&self) -> Cow<'_, str> {
        Cow::Borrowed(
            "remove `deny_unknown_fields` to accept flattened extensions, or remove flattening to keep a closed schema",
        )
    }

    fn emit(self, cx: &LateContext<'_>) {
        cx.emit_span_lint(
            SERDE_FLATTENED_UNKNOWN_FIELD_POLICIES,
            self.span,
            DiagDecorator(|diag| {
                diag.primary_message(self.primary_message().into_owned());
                diag.span_label(
                    self.span,
                    "these Serde policies cannot both be enforced coherently",
                );
                diag.note(self.rationale_message().into_owned());
                diag.help(self.remediation_message().into_owned());
            }),
        );
    }
}

#[derive(Default)]
struct SerdeFlattenedUnknownFieldPolicies {
    catalog: SerdeContractCatalog,
    candidates: Vec<Candidate>,
}

dylint_linting::impl_late_lint! {
    #[doc = include_str!("README.md")]
    pub SERDE_FLATTENED_UNKNOWN_FIELD_POLICIES,
    Warn,
    "finds incoherent Serde flattening and unknown-field policies",
    SerdeFlattenedUnknownFieldPolicies::default()
}

impl LateLintPass<'_> for SerdeFlattenedUnknownFieldPolicies {
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
        if !serde_attributes(&structure.attrs).has(SerdeFlag::DenyUnknownFields) {
            return;
        }
        let fields = structure
            .fields
            .iter()
            .filter(|field| serde_attributes(&field.attrs).has(SerdeFlag::Flatten))
            .map(|field| {
                field
                    .ident
                    .as_ref()
                    .map_or_else(|| "`<positional>`".to_owned(), |name| format!("`{name}`"))
            })
            .collect::<Vec<_>>();
        if fields.is_empty() {
            return;
        }
        self.candidates.push(Candidate {
            definition: item.owner_id.def_id,
            span: item.span,
            fields,
        });
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
                fields: candidate.fields,
            }
            .emit(cx);
        }
    }
}
