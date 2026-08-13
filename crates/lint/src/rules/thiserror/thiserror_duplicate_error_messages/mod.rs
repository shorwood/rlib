extern crate rustc_errors;
extern crate rustc_hir;
extern crate rustc_span;

use std::borrow::Cow;
use std::collections::BTreeMap;

use rustc_errors::DiagDecorator;
use rustc_hir::{Item, ItemKind};
use rustc_lint::{LateContext, LateLintPass, LintContext};
use rustc_span::Span;
use rustc_span::def_id::LocalDefId;

use super::contracts::{ThiserrorContractCatalog, static_error_message};
use crate::utils::diagnostic::LateViolation;
use crate::utils::source_provenance::authored_item_source;

struct Candidate {
    definition: LocalDefId,
    span: Span,
    message: String,
    variants: Vec<String>,
}

struct Violation {
    span: Span,
    message: String,
    variants: Vec<String>,
}

impl LateViolation for Violation {
    fn primary_message(&self) -> Cow<'_, str> {
        Cow::Owned(format!(
            "thiserror variants render the same static message `{}`",
            self.message
        ))
    }

    fn rationale_message(&self) -> Cow<'_, str> {
        Cow::Owned(format!(
            "variants {} carry distinct typed meanings but become indistinguishable in error presentation",
            self.variants.join(", ")
        ))
    }

    fn remediation_message(&self) -> Cow<'_, str> {
        Cow::Borrowed(
            "add semantic context to each message or distinguish them with stable diagnostic codes",
        )
    }

    fn emit(self, cx: &LateContext<'_>) {
        cx.emit_span_lint(
            THISERROR_DUPLICATE_ERROR_MESSAGES,
            self.span,
            DiagDecorator(|diag| {
                diag.primary_message(self.primary_message().into_owned());
                diag.span_label(
                    self.span,
                    "these variants lose their distinction when displayed",
                );
                diag.note(self.rationale_message().into_owned());
                diag.help(self.remediation_message().into_owned());
            }),
        );
    }
}

#[derive(Default)]
struct ThiserrorDuplicateErrorMessages {
    catalog: ThiserrorContractCatalog,
    candidates: Vec<Candidate>,
}

dylint_linting::impl_late_lint! {
    #[doc = include_str!("README.md")]
    pub THISERROR_DUPLICATE_ERROR_MESSAGES,
    Warn,
    "finds duplicate static messages across thiserror variants",
    ThiserrorDuplicateErrorMessages::default()
}

impl LateLintPass<'_> for ThiserrorDuplicateErrorMessages {
    fn check_item(&mut self, cx: &LateContext<'_>, item: &Item<'_>) {
        self.catalog.check_item(cx, item);
        if item.span.from_expansion() || !matches!(item.kind, ItemKind::Enum(..)) {
            return;
        }
        let Some(source) = authored_item_source(cx, item) else {
            return;
        };
        let Ok(enumeration) = syn::parse_str::<syn::ItemEnum>(&source) else {
            return;
        };
        let mut messages = BTreeMap::<String, Vec<String>>::new();
        for variant in &enumeration.variants {
            let Some(message) = static_error_message(&variant.attrs) else {
                continue;
            };
            messages
                .entry(message)
                .or_default()
                .push(format!("`{}`", variant.ident));
        }
        for (message, variants) in messages {
            if variants.len() < 2 {
                continue;
            }
            self.candidates.push(Candidate {
                definition: item.owner_id.def_id,
                span: item.span,
                message,
                variants,
            });
        }
    }

    fn check_crate_post(&mut self, cx: &LateContext<'_>) {
        for candidate in self.candidates.drain(..) {
            if self.catalog.derived_type(candidate.definition).is_none() {
                continue;
            }
            Violation {
                span: candidate.span,
                message: candidate.message,
                variants: candidate.variants,
            }
            .emit(cx);
        }
    }
}
