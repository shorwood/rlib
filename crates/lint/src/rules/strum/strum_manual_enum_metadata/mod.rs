extern crate rustc_errors;
extern crate rustc_hir;
extern crate rustc_span;

use std::borrow::Cow;

use rustc_errors::DiagDecorator;
use rustc_hir::{ImplItem, Item};
use rustc_lint::{LateContext, LateLintPass};
use rustc_span::{Span, Symbol};

use super::utils::authored_contracts::VariantValueFamily;
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
    /// Stores the `provider` value used by this analysis.
    provider: &'static str,
    /// Stores the `is_public` value used by this analysis.
    is_public: bool,
}

impl LateViolation for Violation {
    fn primary_message(&self) -> Cow<'_, str> {
        Cow::Owned(format!(
            "`{}` metadata is maintained manually",
            self.enum_name
        ))
    }

    fn rationale_message(&self) -> Cow<'_, str> {
        Cow::Borrowed("the exhaustive match selects only static metadata by variant")
    }

    fn remediation_message(&self) -> Cow<'_, str> {
        Cow::Owned(format!(
            "derive `strum::{}` and move the values onto their variants",
            self.provider
        ))
    }

    fn emit(self, cx: &LateContext<'_>) {
        cx.tcx.emit_node_span_lint(
            STRUM_MANUAL_ENUM_METADATA,
            self.owner,
            self.span,
            DiagDecorator(|diag| {
                diag.primary_message(self.primary_message().into_owned());
                diag.note(self.rationale_message().into_owned());
                if self.is_public {
                    diag.note("this method is public; generated trait methods may require a compatibility shim");
                }
                diag.help(self.remediation_message().into_owned());
            }),
        );
    }
}

/// Performs the `is_metadata_name` step of the lint analysis.
fn is_metadata_name(name: &str) -> bool {
    // Perform the next step of the analysis.
    [
        "message",
        "description",
        "label",
        "code",
        "color",
        "icon",
        "category",
        "level",
        "severity",
        "kind",
        "property",
    ]
    .into_iter()
    .any(|token| name.contains(token))
}

#[derive(Default)]
/// Carries the `StrumManualEnumMetadata` state used by this analysis.
struct StrumManualEnumMetadata {
    /// Stores the `catalog` value used by this analysis.
    catalog: ContractCatalog,
}

dylint_linting::impl_late_lint! {
    #[doc = include_str!("README.md")]
    pub STRUM_MANUAL_ENUM_METADATA,
    Warn,
    "finds manual enum metadata matches reproducible by Strum",
    StrumManualEnumMetadata::default()
}

impl LateLintPass<'_> for StrumManualEnumMetadata {
    fn check_item(&mut self, cx: &LateContext<'_>, item: &Item<'_>) {
        self.catalog.check_item(cx, item);
    }

    fn check_impl_item(&mut self, cx: &LateContext<'_>, item: &ImplItem<'_>) {
        // Prepare the values used by this stage.
        let Some(family) = VariantValueFamily::from_impl_item(cx, item) else {
            return;
        };

        // Reject inputs that do not satisfy this stage.
        if self
            .catalog
            .contracts()
            .iter()
            .any(|contract| contract.def_id == family.enum_def && contract.has_authored_metadata())
        // Perform the next step of the analysis.
        {
            return;
        }
        if matches!(
            family.method_name.as_str(),
            "as_str" | "as_static_str" | "name"
        ) || !is_metadata_name(family.method_name.as_str())
        // Perform the next step of the analysis.
        {
            return;
        }

        // Prepare the values used by this stage.
        let provider = if matches!(family.method_name.as_str(), "message" | "detailed_message") {
            "EnumMessage"
        } else {
            "EnumProperty"
        };

        // Perform the next step of the analysis.
        Violation {
            owner: family.owner,
            span: family.span,
            enum_name: cx.tcx.item_name(family.enum_def.to_def_id()),
            provider,
            is_public: family.is_public,
        }
        .emit(cx);
    }
}
