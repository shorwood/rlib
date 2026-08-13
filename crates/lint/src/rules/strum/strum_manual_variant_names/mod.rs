extern crate rustc_errors;
extern crate rustc_hir;
extern crate rustc_span;

use std::borrow::Cow;

use rustc_errors::DiagDecorator;
use rustc_hir::{ImplItem, Item};
use rustc_lint::{LateContext, LateLintPass};
use rustc_span::{Span, Symbol};

use super::utils::authored_contracts::StringTableCandidate;
use super::utils::contracts::ContractCatalog;
use crate::utils::diagnostic::LateViolation;

/// Carries the `Violation` state used by this analysis.
struct Violation {
    /// Stores the `owner` value used by this analysis.
    owner: rustc_hir::HirId,
    /// Stores the `span` value used by this analysis.
    span: Span,
    /// Stores the `enum_name` value used by this analysis.
    enum_name: Symbol,
    /// Stores the `is_public` value used by this analysis.
    is_public: bool,
}

impl LateViolation for Violation {
    fn primary_message(&self) -> Cow<'_, str> {
        Cow::Owned(format!(
            "`{}` variant names are maintained manually",
            self.enum_name
        ))
    }

    fn rationale_message(&self) -> Cow<'_, str> {
        Cow::Borrowed("the static table duplicates canonical names and declaration order")
    }

    fn remediation_message(&self) -> Cow<'_, str> {
        Cow::Borrowed("derive `strum::VariantNames` and use its `VARIANTS` constant")
    }

    fn emit(self, cx: &LateContext<'_>) {
        cx.tcx.emit_node_span_lint(
            STRUM_MANUAL_VARIANT_NAMES,
            self.owner,
            self.span,
            DiagDecorator(|diag| {
                diag.primary_message(self.primary_message().into_owned());
                diag.note(self.rationale_message().into_owned());
                if self.is_public {
                    diag.note("this table is public; preserve its type and symbol compatibility deliberately");
                }
                diag.help(self.remediation_message().into_owned());
            }),
        );
    }
}

#[derive(Default)]
/// Carries the `StrumManualVariantNames` state used by this analysis.
struct StrumManualVariantNames {
    /// Stores the `catalog` value used by this analysis.
    catalog: ContractCatalog,
    /// Stores the `tables` value used by this analysis.
    tables: Vec<StringTableCandidate>,
}

dylint_linting::impl_late_lint! {
    #[doc = include_str!("README.md")]
    pub STRUM_MANUAL_VARIANT_NAMES,
    Warn,
    "finds manually maintained enum variant-name tables",
    StrumManualVariantNames::default()
}

impl LateLintPass<'_> for StrumManualVariantNames {
    fn check_item(&mut self, cx: &LateContext<'_>, item: &Item<'_>) {
        self.catalog.check_item(cx, item);
        let Some(table) = StringTableCandidate::from_item(cx, item) else {
            return;
        };
        self.tables.push(table);
    }

    fn check_impl_item(&mut self, cx: &LateContext<'_>, item: &ImplItem<'_>) {
        let Some(table) = StringTableCandidate::from_impl_item(cx, item) else {
            return;
        };
        self.tables.push(table);
    }

    fn check_crate_post(&mut self, cx: &LateContext<'_>) {
        for table in self.tables.drain(..) {
            // Prepare the values used by this stage.
            let Some(contract) = self.catalog.matching_string_table(&table) else {
                continue;
            };

            // Perform the next step of the analysis.
            Violation {
                owner: table.owner,
                span: table.span,
                enum_name: contract.name,
                is_public: table.is_public,
            }
            .emit(cx);
        }
    }
}
