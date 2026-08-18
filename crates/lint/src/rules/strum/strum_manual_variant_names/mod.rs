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

// -----------------------------------------------------------------------------
// Violation: Hand-maintained variant-name table
// -----------------------------------------------------------------------------

/// Complete variant-name mapping reproducible by `VariantNames`.
struct Violation {
    /// Declaration whose lint level governs this finding.
    owner: rustc_hir::HirId,
    /// Authored declaration or expression range used as the diagnostic anchor.
    span: Span,
    /// Enum name quoted in the diagnostic.
    enum_name: Symbol,
    /// Whether the declaration is visible outside its defining module.
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

// -----------------------------------------------------------------------------
// StrumManualVariantNames: Generated variant-name policy
// -----------------------------------------------------------------------------

/// Finds authored variant-name tables equivalent to Strum's generated names.
#[derive(Default)]
struct StrumManualVariantNames {
    /// Effective Strum contracts consulted after generated items are associated.
    catalog: ContractCatalog,
    /// Authored name tables awaiting comparison with their enum contracts.
    tables: Vec<StringTableCandidate>,
}

crate::impl_late_lint! {
    #[doc = include_str!("README.md")]
    pub STRUM_MANUAL_VARIANT_NAMES,
    Warn,
    "finds manually maintained enum variant-name tables",
    StrumManualVariantNames::default()
}

impl LateLintPass<'_> for StrumManualVariantNames {
    fn check_item(&mut self, cx: &LateContext<'_>, item: &Item<'_>) {
        self.catalog.check_item(cx, item);

        // Items without a complete string table expose no manual variant-name contract.
        let Some(table) = StringTableCandidate::from_item(cx, item) else {
            return;
        };
        self.tables.push(table);
    }

    fn check_impl_item(&mut self, cx: &LateContext<'_>, item: &ImplItem<'_>) {
        // Implementation items without a complete string table are unrelated.
        let Some(table) = StringTableCandidate::from_impl_item(cx, item) else {
            return;
        };
        self.tables.push(table);
    }

    fn check_crate_post(&mut self, cx: &LateContext<'_>) {
        for table in self.tables.drain(..) {
            let Some(contract) = self.catalog.matching_string_table(&table) else {
                continue;
            };

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
