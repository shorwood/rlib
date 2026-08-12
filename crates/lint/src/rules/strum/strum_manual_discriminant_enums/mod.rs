extern crate rustc_errors;
extern crate rustc_hir;
extern crate rustc_span;

use std::borrow::Cow;

use rustc_errors::DiagDecorator;
use rustc_hir::ImplItem;
use rustc_lint::{LateContext, LateLintPass};
use rustc_span::{Span, Symbol};

use super::utils::authored_contracts::DiscriminantMirrorCandidate;
use crate::utils::diagnostic::LateViolation;

struct Violation {
    owner: rustc_hir::HirId,
    span: Span,
    source: Symbol,
    mirror: Symbol,
}

impl LateViolation for Violation {
    fn primary_message(&self) -> Cow<'_, str> {
        Cow::Owned(format!(
            "`{}` manually mirrors `{}`",
            self.mirror, self.source
        ))
    }
    fn rationale_message(&self) -> Cow<'_, str> {
        Cow::Borrowed(
            "the private unit enum and conversion duplicate every source variant one-for-one",
        )
    }
    fn remediation_message(&self) -> Cow<'_, str> {
        Cow::Owned(format!(
            "derive `strum::EnumDiscriminants` and configure its generated name as `{}`",
            self.mirror
        ))
    }
    fn emit(self, cx: &LateContext<'_>) {
        cx.tcx.emit_node_span_lint(
            STRUM_MANUAL_DISCRIMINANT_ENUMS,
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

struct StrumManualDiscriminantEnums;

dylint_linting::impl_late_lint! {
    #[doc = include_str!("README.md")]
    pub STRUM_MANUAL_DISCRIMINANT_ENUMS,
    Warn,
    "finds manually mirrored enum discriminants reproducible by Strum",
    StrumManualDiscriminantEnums
}

impl LateLintPass<'_> for StrumManualDiscriminantEnums {
    fn check_impl_item(&mut self, cx: &LateContext<'_>, item: &ImplItem<'_>) {
        let Some(candidate) = DiscriminantMirrorCandidate::from_impl_item(cx, item) else {
            return;
        };
        Violation {
            owner: candidate.owner,
            span: candidate.span,
            source: cx.tcx.item_name(candidate.source_enum.to_def_id()),
            mirror: cx.tcx.item_name(candidate.mirror_enum.to_def_id()),
        }
        .emit(cx);
    }
}
