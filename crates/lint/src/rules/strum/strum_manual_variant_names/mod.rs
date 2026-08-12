extern crate rustc_errors;
extern crate rustc_hir;
extern crate rustc_span;

use std::borrow::Cow;

use rustc_errors::DiagDecorator;
use rustc_hir::{ImplItem, Item};
use rustc_lint::{LateContext, LateLintPass};
use rustc_span::{Span, Symbol};

use super::utils::authored_contracts::StringTableCandidate;
use super::utils::contracts::{ContractCatalog, EnumContract};
use crate::utils::diagnostic::LateViolation;

struct Violation {
    owner: rustc_hir::HirId,
    span: Span,
    enum_name: Symbol,
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
struct StrumManualVariantNames {
    catalog: ContractCatalog,
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
        if let Some(table) = StringTableCandidate::from_item(cx, item) {
            self.tables.push(table);
        }
    }

    fn check_impl_item(&mut self, cx: &LateContext<'_>, item: &ImplItem<'_>) {
        if let Some(table) = StringTableCandidate::from_impl_item(cx, item) {
            self.tables.push(table);
        }
    }

    fn check_crate_post(&mut self, cx: &LateContext<'_>) {
        let contracts = self.catalog.contracts();
        for table in self.tables.drain(..) {
            let Some(contract) = matching_contract(&contracts, &table) else {
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

fn matching_contract<'a>(
    contracts: &'a [EnumContract],
    table: &StringTableCandidate,
) -> Option<&'a EnumContract> {
    let mut matches = contracts.iter().filter(|contract| {
        table
            .enum_def
            .is_none_or(|enum_def| enum_def == contract.def_id)
            && table.values
                == contract
                    .variants
                    .iter()
                    .map(|variant| variant.preferred_name.clone())
                    .collect::<Vec<_>>()
            && (table.enum_def.is_some()
                || table
                    .name
                    .as_str()
                    .to_ascii_lowercase()
                    .contains(&contract.name.as_str().to_ascii_lowercase()))
    });
    let first = matches.next()?;
    matches.next().is_none().then_some(first)
}
