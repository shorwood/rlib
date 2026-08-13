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

// -----------------------------------------------------------------------------
// Violation: Hand-maintained variant metadata table
// -----------------------------------------------------------------------------

/// Complete variant-to-value table reproducible by Strum properties.
struct Violation {
    /// Declaration whose lint level governs this finding.
    owner: rustc_hir::HirId,
    /// Authored declaration or expression range used as the diagnostic anchor.
    span: Span,
    /// Enum name quoted in the diagnostic.
    enum_name: Symbol,
    /// Explicitly resolved framework provider, when one is available.
    provider: &'static str,
    /// Whether the declaration is visible outside its defining module.
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

/// Recognizes method names conventionally used to expose enum metadata.
fn is_metadata_name(name: &str) -> bool {
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

// -----------------------------------------------------------------------------
// StrumManualEnumMetadata: Declarative variant metadata policy
// -----------------------------------------------------------------------------

/// Finds complete authored metadata tables that `EnumProperty` can generate.
#[derive(Default)]
struct StrumManualEnumMetadata {
    /// Effective Strum contracts consulted after generated items are associated.
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
        let Some(family) = VariantValueFamily::from_impl_item(cx, item) else {
            return;
        };

        if self
            .catalog
            .contracts()
            .iter()
            .any(|contract| contract.def_id == family.enum_def && contract.has_authored_metadata())
        {
            return;
        }
        if matches!(
            family.method_name.as_str(),
            "as_str" | "as_static_str" | "name"
        ) || !is_metadata_name(family.method_name.as_str())
        {
            return;
        }

        let provider = if matches!(family.method_name.as_str(), "message" | "detailed_message") {
            "EnumMessage"
        } else {
            "EnumProperty"
        };

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
