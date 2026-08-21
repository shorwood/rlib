extern crate rustc_errors;
extern crate rustc_hir;
extern crate rustc_span;

use std::borrow::Cow;

use rustc_errors::DiagDecorator;
use rustc_hir::{ImplItem, Item};
use rustc_lint::{LateContext, LateLintPass};
use rustc_span::{Span, Symbol};

use super::utils::authored_contracts::{
    GeneratedStringTrait, GeneratedStringTraitWrapperCandidate, StaticValue, VariantValueFamily,
};
use super::utils::contracts::{ContractCatalog, StrumDerive};
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
    /// Whether the method delegates to an already-generated metadata contract.
    is_wrapper: bool,
}

impl LateViolation for Violation {
    fn primary_message(&self) -> Cow<'_, str> {
        Cow::Owned(format!(
            "`{}` metadata is maintained manually",
            self.enum_name
        ))
    }

    fn rationale_message(&self) -> Cow<'_, str> {
        if self.is_wrapper {
            Cow::Borrowed("the method only forwards to the enum's complete generated metadata")
        } else {
            Cow::Borrowed("the exhaustive match selects only static metadata by variant")
        }
    }

    fn remediation_message(&self) -> Cow<'_, str> {
        if self.is_wrapper {
            Cow::Borrowed("remove this shim and migrate callers to `strum::EnumMessage`")
        } else {
            Cow::Owned(format!(
                "derive `strum::{}` and move the values onto their variants",
                self.provider
            ))
        }
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
                    diag.note("this method is public API; removing it requires callers to use the generated trait directly");
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
    /// Complete manual variant-to-value families awaiting contract classification.
    families: Vec<VariantValueFamily>,
    /// Inherent methods hiding a complete `EnumMessage` contract.
    wrappers: Vec<GeneratedStringTraitWrapperCandidate>,
}

crate::impl_late_lint! {
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
            "name",
        ];
        name.trim_start_matches("r#")
            .split('_')
            .any(|component| vocabulary.contains(&component))
    }
}

impl LateLintPass<'_> for StrumManualEnumMetadata {
    fn check_item(&mut self, cx: &LateContext<'_>, item: &Item<'_>) {
        self.catalog.check_item(cx, item);
    }

    fn check_impl_item(&mut self, cx: &LateContext<'_>, item: &ImplItem<'_>) {
        // A generated-trait wrapper cannot also be a manual metadata family.
        if let Some(wrapper) = GeneratedStringTraitWrapperCandidate::from_impl_item(cx, item)
            .filter(|wrapper| wrapper.generated_trait == GeneratedStringTrait::EnumMessage)
        {
            self.wrappers.push(wrapper);
            return;
        }

        // Implementation items without a complete variant-value family expose no metadata table.
        let Some(family) = VariantValueFamily::from_impl_item(cx, item) else {
            return;
        };

        // String conversions, unrelated names, and explicitly governed methods are not metadata.
        if matches!(
            family.method_name.as_str(),
            "as_str" | "as_static_str" | "suffix"
        ) || !Self::is_metadata_name(family.method_name.as_str())
        {
            return;
        }
        self.families.push(family);
    }

    fn check_crate_post(&mut self, cx: &LateContext<'_>) {
        let contracts = self.catalog.contracts();
        for family in self.families.drain(..) {
            let contract = contracts
                .iter()
                .find(|contract| contract.def_id == family.enum_def);

            // Canonical names belong to AsRefStr rather than custom metadata.
            if contract.is_some_and(|contract| {
                contract.variants.iter().all(|variant| {
                    family.values.get(&variant.def_id)
                        == Some(&StaticValue::String(variant.preferred_name.clone()))
                })
            }) {
                continue;
            }

            let method_name = family.method_name.as_str();
            let is_message = method_name == "name"
                || method_name.ends_with("_name")
                || method_name
                    .trim_start_matches("r#")
                    .split('_')
                    .any(|component| matches!(component, "message" | "label"));
            let provider = if is_message {
                "EnumMessage"
            } else {
                "EnumProperty"
            };

            // Existing authored Strum metadata already supplies the generated replacement.
            if contract.is_some_and(|contract| {
                if is_message {
                    contract.has_authored_message_metadata()
                } else {
                    contract.has_authored_property(method_name)
                }
            }) {
                continue;
            }

            Violation {
                owner: family.owner,
                span: family.span,
                enum_name: cx.tcx.item_name(family.enum_def.to_def_id()),
                provider,
                is_public: family.is_public,
                is_wrapper: false,
            }
            .emit(cx);
        }

        for wrapper in self.wrappers.drain(..) {
            let Some(contract) = contracts.iter().find(|contract| {
                contract.def_id == wrapper.enum_def
                    && contract.has_derive(StrumDerive::EnumMessage)
                    && contract.has_complete_message_metadata()
            }) else {
                continue;
            };
            Violation {
                owner: wrapper.owner,
                span: wrapper.span,
                enum_name: contract.name,
                provider: "EnumMessage",
                is_public: wrapper.is_public,
                is_wrapper: true,
            }
            .emit(cx);
        }
    }
}
