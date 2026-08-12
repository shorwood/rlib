extern crate rustc_errors;
extern crate rustc_hir;
extern crate rustc_span;

use std::borrow::Cow;

use rustc_errors::DiagDecorator;
use rustc_hir::def_id::LocalDefId;
use rustc_hir::{Expr, ImplItem, Item, Node};
use rustc_lint::{LateContext, LateLintPass};
use rustc_span::{Span, Symbol};

use super::utils::contracts::{ContractCatalog, StrumDerive, strum_associated_enum};
use crate::utils::diagnostic::LateViolation;

struct Violation {
    owner: rustc_hir::HirId,
    span: Span,
    enum_name: Symbol,
}

impl LateViolation for Violation {
    fn primary_message(&self) -> Cow<'_, str> {
        Cow::Owned(format!(
            "`{}::COUNT` is a total, not a filtered count",
            self.enum_name
        ))
    }
    fn rationale_message(&self) -> Cow<'_, str> {
        Cow::Borrowed(
            "the surrounding API promises a subset while `EnumCount` includes every declared variant",
        )
    }
    fn remediation_message(&self) -> Cow<'_, str> {
        Cow::Borrowed("count the declared subset explicitly or rename the API as a total count")
    }
    fn emit(self, cx: &LateContext<'_>) {
        cx.tcx.emit_node_span_lint(
            STRUM_FILTERED_ENUM_COUNT_CONTRACTS,
            self.owner,
            self.span,
            DiagDecorator(|diag| {
                diag.primary_message(self.primary_message().into_owned());
                diag.note(self.rationale_message().into_owned());
                diag.help(self.remediation_message().into_owned());
            }),
        );
    }
}

struct CountUse {
    owner: rustc_hir::HirId,
    span: Span,
    enum_def: LocalDefId,
}

#[derive(Default)]
struct StrumFilteredEnumCountContracts {
    catalog: ContractCatalog,
    uses: Vec<CountUse>,
}

dylint_linting::impl_late_lint! {
    #[doc = include_str!("README.md")]
    pub STRUM_FILTERED_ENUM_COUNT_CONTRACTS,
    Warn,
    "finds total Strum enum counts exposed as filtered subsets",
    StrumFilteredEnumCountContracts::default()
}

impl LateLintPass<'_> for StrumFilteredEnumCountContracts {
    fn check_item(&mut self, cx: &LateContext<'_>, item: &Item<'_>) {
        self.catalog.check_item(cx, item);
    }

    fn check_expr(&mut self, cx: &LateContext<'_>, expression: &Expr<'_>) {
        let Some(enum_def) = strum_associated_enum(cx, expression, "EnumCount", "COUNT") else {
            return;
        };
        if !enclosing_name(cx, expression.hir_id).is_some_and(is_subset_name) {
            return;
        }
        self.uses.push(CountUse {
            owner: expression.hir_id,
            span: expression.span.source_callsite(),
            enum_def,
        });
    }

    fn check_crate_post(&mut self, cx: &LateContext<'_>) {
        let contracts = self.catalog.contracts();
        for count_use in self.uses.drain(..) {
            let Some(contract) = contracts.iter().find(|contract| {
                contract.def_id == count_use.enum_def
                    && contract.derives(StrumDerive::EnumCount)
                    && contract.variants.iter().any(|variant| {
                        variant.disabled
                            || variant.deprecated
                            || variant.has_payload
                            || matches!(
                                variant.name.as_str(),
                                "Disabled" | "Unknown" | "Other" | "Unspecified"
                            )
                    })
            }) else {
                continue;
            };
            Violation {
                owner: count_use.owner,
                span: count_use.span,
                enum_name: contract.name,
            }
            .emit(cx);
        }
    }
}

fn enclosing_name(cx: &LateContext<'_>, hir_id: rustc_hir::HirId) -> Option<Symbol> {
    cx.tcx
        .hir_parent_iter(hir_id)
        .find_map(|(_, node)| match node {
            Node::Item(item) => item.kind.ident().map(|ident| ident.name),
            Node::ImplItem(ImplItem { ident, .. }) => Some(ident.name),
            _ => None,
        })
}

fn is_subset_name(name: Symbol) -> bool {
    [
        "enabled",
        "visible",
        "supported",
        "actionable",
        "available",
        "selectable",
    ]
    .into_iter()
    .any(|token| name.as_str().contains(token))
}
