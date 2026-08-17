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

use super::utils::contracts::{ErrorMessage, ThiserrorContractCatalog};
use crate::utils::diagnostic::LateViolation;
use crate::utils::source_provenance::AuthoredItemSource;

/// Smallest variant count that establishes a duplicate message.
const MINIMUM_DUPLICATE_MESSAGE_VARIANTS: usize = 2;

// -----------------------------------------------------------------------------
// Violation: Duplicate static variant messages
// -----------------------------------------------------------------------------

/// Derived variants that lose their distinction when displayed.
struct Violation {
    /// Error enum receiving the diagnostic.
    span: Span,
    /// Static message shared by multiple variants.
    message: String,
    /// Distinct variants rendering that message.
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

// -----------------------------------------------------------------------------
// Candidate: Duplicate message evidence
// -----------------------------------------------------------------------------

/// Duplicate group retained until the enum's thiserror derive is confirmed.
struct Candidate {
    /// Candidate enum definition.
    definition: LocalDefId,
    /// Enum declaration receiving a later diagnostic.
    span: Span,
    /// Static message shared by the variants.
    message: String,
    /// Variants that render the shared message.
    variants: Vec<String>,
}

// -----------------------------------------------------------------------------
// ThiserrorDuplicateErrorMessages: Distinct presentation policy
// -----------------------------------------------------------------------------

/// Groups static messages and validates their enums against thiserror contracts.
#[derive(Default)]
struct ThiserrorDuplicateErrorMessages {
    /// Local derived error contracts.
    catalog: ThiserrorContractCatalog,
    /// Duplicate message groups awaiting derive confirmation.
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

        // Generated and non-enum items cannot define authored variant-message collisions.
        if item.span.from_expansion() || !matches!(item.kind, ItemKind::Enum(..)) {
            return;
        }

        // Missing authored source prevents reliable recovery of error attributes.
        let Some(source) = AuthoredItemSource::for_item(cx, item) else {
            return;
        };

        // Source that cannot be parsed as its enum cannot provide authored variant messages.
        let Ok(enumeration) = syn::parse_str::<syn::ItemEnum>(&source) else {
            return;
        };
        let mut messages = BTreeMap::<String, Vec<String>>::new();
        for variant in &enumeration.variants {
            let Some(message) = ErrorMessage::static_from(&variant.attrs) else {
                continue;
            };
            messages
                .entry(message)
                .or_default()
                .push(format!("`{}`", variant.ident));
        }
        for (message, variants) in messages {
            if variants.len() < MINIMUM_DUPLICATE_MESSAGE_VARIANTS {
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
