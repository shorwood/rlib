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

impl StrumManualEnumMetadata {
    /// Recognizes method names conventionally used to expose enum metadata.
    fn is_metadata_name(name: &str) -> bool {
        let vocabulary = [
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
        ];
        name.trim_start_matches("r#")
            .split('_')
            .any(|component| vocabulary.contains(&component))
    }

    /// Returns whether documentation deliberately preserves authored policy or localization.
    fn has_authored_policy(cx: &LateContext<'_>, owner: rustc_hir::HirId) -> bool {
        cx.tcx.hir_attrs(owner).iter().any(|attribute| {
            attribute.doc_str().is_some_and(|documentation| {
                let documentation = documentation.as_str().to_ascii_lowercase();
                ["policy", "localized", "localization", "locale", "i18n"]
                    .into_iter()
                    .any(|term| documentation.contains(term))
            })
        })
    }
}
impl LateLintPass<'_> for StrumManualEnumMetadata {
    fn check_item(&mut self, cx: &LateContext<'_>, item: &Item<'_>) {
        self.catalog.check_item(cx, item);
    }

    fn check_impl_item(&mut self, cx: &LateContext<'_>, item: &ImplItem<'_>) {
        let Some(family) = VariantValueFamily::from_impl_item(cx, item) else {
            return;
        };

        if matches!(
            family.method_name.as_str(),
            "as_str" | "as_static_str" | "name"
        ) || !Self::is_metadata_name(family.method_name.as_str())
            || Self::has_authored_policy(cx, family.owner)
        {
            return;
        }

        let is_message = family
            .method_name
            .as_str()
            .trim_start_matches("r#")
            .split('_')
            .any(|component| component == "message");
        let provider = if is_message {
            "EnumMessage"
        } else {
            "EnumProperty"
        };
        if self.catalog.contracts().iter().any(|contract| {
            contract.def_id == family.enum_def
                && if is_message {
                    contract.has_authored_message_metadata()
                } else {
                    contract.has_authored_property(family.method_name.as_str())
                }
        }) {
            return;
        }

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
