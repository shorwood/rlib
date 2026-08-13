extern crate rustc_errors;
extern crate rustc_hir;
extern crate rustc_span;

use std::borrow::Cow;

use rustc_errors::DiagDecorator;
use rustc_hir::def_id::LocalDefId;
use rustc_hir::{Expr, ImplItem, Item, Node};
use rustc_lint::{LateContext, LateLintPass};
use rustc_span::{Span, Symbol};

use super::utils::contracts::{ContractCatalog, StrumAssociatedItem, StrumDerive};
use crate::utils::diagnostic::LateViolation;

// -----------------------------------------------------------------------------
// Violation: Enum count disagrees with filtered iteration
// -----------------------------------------------------------------------------

/// Count contract that includes variants omitted by its companion iterator.
struct Violation {
    /// Declaration whose lint level governs this finding.
    owner: rustc_hir::HirId,
    /// Authored declaration or expression range used as the diagnostic anchor.
    span: Span,
    /// Enum name quoted in the diagnostic.
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

/// One filtered API that exposes an enum's total count.
struct ViolationUse {
    /// Declaration whose lint level governs this finding.
    owner: rustc_hir::HirId,
    /// Authored declaration or expression range used as the diagnostic anchor.
    span: Span,
    /// Local enum definition that owns the analyzed contract.
    enum_def: LocalDefId,
}

/// Returns the nearest authored item name surrounding a count expression.
fn enclosing_name(cx: &LateContext<'_>, hir_id: rustc_hir::HirId) -> Option<Symbol> {
    cx.tcx
        .hir_parent_iter(hir_id)
        .find_map(|(_, node)| match node {
            Node::Item(item) => item.kind.ident().map(|ident| ident.name),
            Node::ImplItem(ImplItem { ident, .. }) => Some(ident.name),
            _ => None,
        })
}

/// Recognizes names that conventionally describe a filtered enum subset.
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

// -----------------------------------------------------------------------------
// StrumFilteredEnumCountContracts: Matching count and iteration policy
// -----------------------------------------------------------------------------

/// Distinguishes full enum counts from deliberately filtered domain subsets.
#[derive(Default)]
struct StrumFilteredEnumCountContracts {
    /// Effective Strum contracts consulted after generated items are associated.
    catalog: ContractCatalog,
    /// Count call sites whose surrounding names imply a filtered subset.
    uses: Vec<ViolationUse>,
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
        let Some(enum_def) = (StrumAssociatedItem {
            trait_name: "EnumCount",
            item_name: "COUNT",
        })
        .enum_definition(cx, expression) else {
            return;
        };
        if !enclosing_name(cx, expression.hir_id).is_some_and(is_subset_name) {
            return;
        }

        self.uses.push(ViolationUse {
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
                        variant.is_disabled
                            || variant.is_deprecated
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
