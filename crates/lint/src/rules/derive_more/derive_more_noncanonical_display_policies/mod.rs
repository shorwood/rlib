extern crate rustc_errors;
extern crate rustc_hir;
extern crate rustc_span;

use std::borrow::Cow;

use rustc_errors::{Applicability, DiagDecorator};
use rustc_hir::{HirId, Item, ItemKind};
use rustc_lint::{LateContext, LateLintPass};
use rustc_span::Span;
use rustc_span::def_id::LocalDefId;

use super::utils::contracts::DeriveMoreContractCatalog;
use super::utils::naming::{CASES, OptionalCaseExt as _};
use crate::utils::diagnostic::LateViolation;
use crate::utils::name_policy::{
    AuthoredAttributeSpanExt as _, NamingPolicyCandidate, NamingPolicyFactor,
};
use crate::utils::source_provenance::AuthoredItemSource;

// -----------------------------------------------------------------------------
// DisplayAttributes: Relevant authored Display configuration
// -----------------------------------------------------------------------------

/// Display directives that influence a variant's generated text.
#[derive(Default)]
struct DisplayAttributes {
    /// Optional case policy applied to an enum or variant.
    rename_all: Option<String>,
    /// Direct literal format used by a unit variant.
    static_format: Option<String>,
    /// Whether any explicit format contract was authored.
    has_format: bool,
}

impl DisplayAttributes {
    /// Reads the Display directives represented by an authored attribute list.
    fn from_attributes(attributes: &[syn::Attribute]) -> Self {
        let mut output = Self::default();
        for attribute in attributes
            .iter()
            .filter(|attribute| attribute.path().is_ident("display"))
        {
            if let Ok(format) = attribute.parse_args::<syn::LitStr>() {
                output.static_format = Some(format.value());
                output.has_format = true;
                continue;
            }
            if let syn::Meta::List(list) = &attribute.meta {
                output.has_format |= list.tokens.to_string().trim_start().starts_with('"');
            }
            match attribute.parse_nested_meta(|meta| {
                if meta.path.is_ident("rename_all") {
                    output.rename_all = Some(meta.value()?.parse::<syn::LitStr>()?.value());
                }
                Ok(())
            }) {
                Ok(()) => {}
                Err(_malformed_attribute) => {
                    // Malformed derive_more syntax is reported by the derive macro itself.
                    output.has_format = true;
                }
            }
        }
        output
    }
}

// -----------------------------------------------------------------------------
// VariantSource: Variant naming input
// -----------------------------------------------------------------------------

/// Authored source facts needed to analyze one enum variant.
struct VariantSource {
    /// Rust variant identifier.
    name: String,
    /// HIR span used as the diagnostic fallback.
    span: Span,
    /// Display directives authored on the variant.
    attributes: DisplayAttributes,
    /// Whether the variant has no payload fields.
    is_unit: bool,
}

// -----------------------------------------------------------------------------
// Pending: Deferred enum analysis
// -----------------------------------------------------------------------------

/// Enum source retained until derive_more's generated implementations are known.
struct Pending {
    /// Local definition used to query the derive contract catalog.
    definition: LocalDefId,
    /// HIR owner receiving emitted diagnostics.
    owner: HirId,
    /// Complete enum span used to find removable attributes.
    span: Span,
    /// Display directives authored on the enum.
    container: DisplayAttributes,
    /// Declaration-order variant inputs.
    variants: Vec<VariantSource>,
}

impl Pending {
    /// Builds deferred analysis from the authored enum and its HIR definition.
    fn from_enum(
        item: &Item<'_>,
        enumeration: &syn::ItemEnum,
        definition: &rustc_hir::EnumDef<'_>,
    ) -> Self {
        let variants = enumeration
            .variants
            .iter()
            .zip(definition.variants)
            .map(|(variant, hir)| VariantSource {
                name: variant.ident.to_string(),
                span: hir.span,
                attributes: DisplayAttributes::from_attributes(&variant.attrs),
                is_unit: variant.fields.is_empty(),
            })
            .collect();
        Self {
            definition: item.owner_id.def_id,
            owner: item.hir_id(),
            span: item.span,
            container: DisplayAttributes::from_attributes(&enumeration.attrs),
            variants,
        }
    }
}

// -----------------------------------------------------------------------------
// Suggestion: Machine-applicable source edit
// -----------------------------------------------------------------------------

/// Source replacement attached to a violation when its target is unambiguous.
struct Suggestion {
    /// Exact authored range to replace.
    span: Span,
    /// Replacement source text.
    replacement: String,
}

// -----------------------------------------------------------------------------
// Violation: Noncanonical Display naming policy
// -----------------------------------------------------------------------------

/// One redundant leaf directive or factorable enum-wide naming policy.
struct Violation {
    /// HIR owner receiving the lint.
    owner: HirId,
    /// Primary diagnostic location.
    span: Span,
    /// Explanation of the redundant authored representation.
    detail: String,
    /// Canonical representation requested from the author.
    remediation: String,
    /// Safe source edit when the redundant attribute is uniquely located.
    suggestion: Option<Suggestion>,
}

impl LateViolation for Violation {
    fn primary_message(&self) -> Cow<'_, str> {
        Cow::Borrowed("this derive_more Display naming policy is noncanonical")
    }

    fn rationale_message(&self) -> Cow<'_, str> {
        Cow::Borrowed(&self.detail)
    }

    fn remediation_message(&self) -> Cow<'_, str> {
        Cow::Borrowed(&self.remediation)
    }

    fn emit(self, cx: &LateContext<'_>) {
        cx.tcx.emit_node_span_lint(
            DERIVE_MORE_NONCANONICAL_DISPLAY_POLICIES,
            self.owner,
            self.span,
            DiagDecorator(|diag| {
                diag.primary_message(self.primary_message().into_owned());
                diag.note(self.rationale_message().into_owned());
                let remediation = self.remediation_message().into_owned();
                if let Some(suggestion) = self.suggestion {
                    diag.span_suggestion(
                        suggestion.span,
                        remediation,
                        suggestion.replacement,
                        Applicability::MachineApplicable,
                    );
                } else {
                    diag.help(remediation);
                }
            }),
        );
    }
}

// -----------------------------------------------------------------------------
// DeriveMoreNoncanonicalDisplayPolicies: Lint pass
// -----------------------------------------------------------------------------

/// Collects authored enums and compares their Display names with container policies.
#[derive(Default)]
struct DeriveMoreNoncanonicalDisplayPolicies {
    /// Generated derive_more contracts indexed by source definition.
    catalog: DeriveMoreContractCatalog,
    /// Authored enums awaiting generated-contract confirmation.
    pending: Vec<Pending>,
}

crate::impl_late_lint! {
    #[doc = include_str!("README.md")]
    pub DERIVE_MORE_NONCANONICAL_DISPLAY_POLICIES,
    Warn,
    "finds derive_more Display names that should inherit a container policy",
    DeriveMoreNoncanonicalDisplayPolicies::default()
}

impl DeriveMoreNoncanonicalDisplayPolicies {
    /// Emits violations for leaf directives already implied by the container policy.
    fn analyze_leaf_directives(cx: &LateContext<'_>, pending: &Pending, units: &[&VariantSource]) {
        for variant in units {
            let inherited = pending
                .container
                .rename_all
                .as_deref()
                .apply_to(&variant.name);
            let expected = if variant.attributes.static_format.as_deref() == Some(&inherited) {
                Some(format!("#[display(\"{inherited}\")]"))
            } else if let Some(case) = &variant.attributes.rename_all
                && Some(case.as_str()).apply_to(&variant.name) == inherited
            {
                Some(format!("#[display(rename_all=\"{case}\")]"))
            } else {
                None
            };
            let Some(expected) = expected else { continue };
            let suggestion = pending
                .span
                .standalone_attribute(cx, &expected)
                .map(|span| Suggestion {
                    span,
                    replacement: String::new(),
                });
            Violation {
                owner: pending.owner,
                span: suggestion.as_ref().map_or(variant.span, |edit| edit.span),
                detail: format!(
                    "`{}` already displays as `{inherited}` without this leaf directive",
                    variant.name
                ),
                remediation: "remove the redundant Display directive".to_owned(),
                suggestion,
            }
            .emit(cx);
        }
    }

    /// Emits a violation when leaf formats reduce to one cheaper container policy.
    fn factor_container_policy(cx: &LateContext<'_>, pending: &Pending, units: &[&VariantSource]) {
        // Existing container behavior or no participating variants leaves nothing to factor.
        if pending.container.rename_all.is_some()
            || pending.container.has_format
            || units.is_empty()
        {
            return;
        }
        let effective = units
            .iter()
            .map(|variant| {
                variant.attributes.static_format.clone().unwrap_or_else(|| {
                    variant
                        .attributes
                        .rename_all
                        .as_deref()
                        .apply_to(&variant.name)
                })
            })
            .collect::<Vec<_>>();
        let authored = units
            .iter()
            .filter(|variant| {
                variant.attributes.static_format.is_some()
                    || variant.attributes.rename_all.is_some()
            })
            .count();
        let policies = CASES.map(|case| NamingPolicyCandidate {
            name: case,
            names: units
                .iter()
                .map(|variant| Some(case).apply_to(&variant.name))
                .collect(),
            directive_cost: 1,
        });

        // Ambiguous or non-reducing mappings do not justify changing authored policy.
        let Some(policy) = NamingPolicyFactor::factor(&effective, authored, policies) else {
            return;
        };
        Self::emit_container_violation(cx, pending, units, authored, &policy);
    }

    /// Emits the canonical container policy and names any required leaf exceptions.
    fn emit_container_violation(
        cx: &LateContext<'_>,
        pending: &Pending,
        units: &[&VariantSource],
        authored: usize,
        policy: &NamingPolicyFactor,
    ) {
        let exceptions = policy
            .exceptions
            .iter()
            .map(|index| units[*index].name.as_str())
            .collect::<Vec<_>>();
        let suffix = if exceptions.is_empty() {
            String::new()
        } else {
            format!(" and retain formats only for {}", exceptions.join(", "))
        };
        Violation {
            owner: pending.owner,
            span: pending.span,
            detail: format!(
                "{authored} unit-variant directives reduce to one `{}` policy",
                policy.name
            ),
            remediation: format!(
                "declare `#[display(rename_all = \"{}\")]` on the enum{suffix}",
                policy.name
            ),
            suggestion: None,
        }
        .emit(cx);
    }

    /// Analyzes redundant leaf directives before considering a container replacement.
    fn analyze(cx: &LateContext<'_>, pending: &Pending) {
        let units = pending
            .variants
            .iter()
            .filter(|variant| variant.is_unit)
            .collect::<Vec<_>>();
        Self::analyze_leaf_directives(cx, pending, &units);
        Self::factor_container_policy(cx, pending, &units);
    }
}

impl LateLintPass<'_> for DeriveMoreNoncanonicalDisplayPolicies {
    fn check_item(&mut self, cx: &LateContext<'_>, item: &Item<'_>) {
        self.catalog.check_item(cx, item);

        // Macro-generated enums have no stable authored attribute representation.
        if item.span.from_expansion() {
            return;
        }

        // Display naming policy is defined only for enum variants.
        let ItemKind::Enum(_, _, definition) = item.kind else {
            return;
        };

        // Unavailable source cannot support syntax-aware directive analysis.
        let Some(source) = AuthoredItemSource::for_item(cx, item) else {
            return;
        };

        // Invalid syntax is owned by rustc and derive_more rather than this policy lint.
        let Ok(enumeration) = syn::parse_str::<syn::ItemEnum>(&source) else {
            return;
        };
        self.pending
            .push(Pending::from_enum(item, &enumeration, &definition));
    }

    fn check_crate_post(&mut self, cx: &LateContext<'_>) {
        for pending in self.pending.drain(..) {
            // Only enums confirmed to derive Display participate in this rule.
            if self
                .catalog
                .derived_type(pending.definition, "Display")
                .is_none()
            {
                continue;
            }
            Self::analyze(cx, &pending);
        }
    }
}
