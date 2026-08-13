extern crate rustc_errors;
extern crate rustc_hir;
extern crate rustc_span;

use std::borrow::Cow;

use rustc_errors::DiagDecorator;
use rustc_hir::{Item, ItemKind};
use rustc_lint::{LateContext, LateLintPass, LintContext};
use rustc_span::Span;
use rustc_span::def_id::LocalDefId;

use super::contracts::{ThiserrorContractCatalog, thiserror_attributes};
use crate::utils::diagnostic::LateViolation;
use crate::utils::source_provenance::authored_item_source;

struct FieldCandidate {
    name: String,
    target: LocalDefId,
    selected: bool,
}

struct Candidate {
    definition: LocalDefId,
    span: Span,
    fields: Vec<FieldCandidate>,
}

struct Violation {
    span: Span,
    fields: Vec<String>,
    selected: Vec<String>,
}

impl LateViolation for Violation {
    fn primary_message(&self) -> Cow<'_, str> {
        Cow::Borrowed("thiserror type stores several plausible primary sources")
    }

    fn rationale_message(&self) -> Cow<'_, str> {
        let policy = if self.selected.is_empty() {
            "none is selected for the standard source chain".to_owned()
        } else {
            format!(
                "only {} enters the standard source chain",
                self.selected.join(", ")
            )
        };
        Cow::Owned(format!(
            "fields {} all contain local thiserror types, but {policy}",
            self.fields.join(", ")
        ))
    }

    fn remediation_message(&self) -> Cow<'_, str> {
        Cow::Borrowed(
            "select and name one primary source, then classify other failures as related, suppressed, or aggregate errors",
        )
    }

    fn emit(self, cx: &LateContext<'_>) {
        cx.emit_span_lint(
            THISERROR_AMBIGUOUS_ERROR_SOURCES,
            self.span,
            DiagDecorator(|diag| {
                diag.primary_message(self.primary_message().into_owned());
                diag.span_label(
                    self.span,
                    "the causal policy among these fields is ambiguous",
                );
                diag.note(self.rationale_message().into_owned());
                diag.help(self.remediation_message().into_owned());
            }),
        );
    }
}

#[derive(Default)]
struct ThiserrorAmbiguousErrorSources {
    catalog: ThiserrorContractCatalog,
    candidates: Vec<Candidate>,
}

dylint_linting::impl_late_lint! {
    #[doc = include_str!("README.md")]
    pub THISERROR_AMBIGUOUS_ERROR_SOURCES,
    Warn,
    "finds derived errors with multiple plausible causal sources",
    ThiserrorAmbiguousErrorSources::default()
}

impl LateLintPass<'_> for ThiserrorAmbiguousErrorSources {
    fn check_item(&mut self, cx: &LateContext<'_>, item: &Item<'_>) {
        self.catalog.check_item(cx, item);
        if item.span.from_expansion() {
            return;
        }
        let hir_fields = match item.kind {
            ItemKind::Struct(_, _, data) => data.fields().iter().collect::<Vec<_>>(),
            ItemKind::Enum(_, _, definition) => definition
                .variants
                .iter()
                .flat_map(|variant| variant.data.fields())
                .collect(),
            _ => return,
        };
        let Some(source) = authored_item_source(cx, item) else {
            return;
        };
        let syn_fields = if let Ok(structure) = syn::parse_str::<syn::ItemStruct>(&source) {
            structure.fields.into_iter().collect::<Vec<_>>()
        } else if let Ok(enumeration) = syn::parse_str::<syn::ItemEnum>(&source) {
            enumeration
                .variants
                .into_iter()
                .flat_map(|variant| variant.fields)
                .collect()
        } else {
            return;
        };
        let fields = syn_fields
            .iter()
            .zip(hir_fields)
            .filter_map(|(field, hir_field)| {
                let name = field.ident.as_ref()?.to_string();
                if !causal_name(&name) {
                    return None;
                }
                let target = cx
                    .tcx
                    .type_of(hir_field.def_id)
                    .instantiate_identity()
                    .ty_adt_def()?
                    .did()
                    .as_local()?;
                let attributes = thiserror_attributes(&field.attrs);
                Some(FieldCandidate {
                    selected: attributes.source || name == "source",
                    name,
                    target,
                })
            })
            .collect();
        self.candidates.push(Candidate {
            definition: item.owner_id.def_id,
            span: item.span,
            fields,
        });
    }

    fn check_crate_post(&mut self, cx: &LateContext<'_>) {
        for candidate in self.candidates.drain(..) {
            if self.catalog.derived_type(candidate.definition).is_none() {
                continue;
            }
            let fields = candidate
                .fields
                .into_iter()
                .filter(|field| self.catalog.derived_type(field.target).is_some())
                .collect::<Vec<_>>();
            if fields.len() < 2 {
                continue;
            }
            Violation {
                span: candidate.span,
                selected: fields
                    .iter()
                    .filter(|field| field.selected)
                    .map(|field| format!("`{}`", field.name))
                    .collect(),
                fields: fields
                    .into_iter()
                    .map(|field| format!("`{}`", field.name))
                    .collect(),
            }
            .emit(cx);
        }
    }
}

fn causal_name(name: &str) -> bool {
    if ["related", "suppressed", "fallback", "retry"]
        .iter()
        .any(|role| name.contains(role))
    {
        return false;
    }
    matches!(name, "cause" | "error" | "source") || name.ends_with("_error")
}
