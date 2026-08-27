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
// FromStrAttributes: Relevant parser configuration
// -----------------------------------------------------------------------------

/// Case policy authored on an enum or one of its variants.
#[derive(Default)]
struct FromStrAttributes {
    /// Optional exact case conversion selected by `rename_all`.
    rename_all: Option<String>,
}

impl FromStrAttributes {
    /// Reads parser naming policy from derive_more attributes.
    fn from_attributes(attributes: &[syn::Attribute]) -> Self {
        let mut output = Self::default();
        for attribute in attributes
            .iter()
            .filter(|attribute| attribute.path().is_ident("from_str"))
        {
            match attribute.parse_nested_meta(|meta| {
                if meta.path.is_ident("rename_all") {
                    output.rename_all = Some(meta.value()?.parse::<syn::LitStr>()?.value());
                }
                Ok(())
            }) {
                Ok(()) | Err(_) => {}
            }
        }
        output
    }
}

// -----------------------------------------------------------------------------
// VariantSource: Variant parser input
// -----------------------------------------------------------------------------

/// Authored source facts needed to analyze one unit variant.
struct VariantSource {
    /// Rust variant identifier.
    name: String,
    /// HIR span used as the diagnostic fallback.
    span: Span,
    /// Variant-level parser case policy.
    rename_all: Option<String>,
}

// -----------------------------------------------------------------------------
// Pending: Deferred enum analysis
// -----------------------------------------------------------------------------

/// Unit enum retained until derive_more's generated implementation is known.
struct Pending {
    /// Local definition used to query generated derive contracts.
    definition: LocalDefId,
    /// HIR owner receiving emitted diagnostics.
    owner: HirId,
    /// Complete enum span used to locate removable attributes.
    span: Span,
    /// Enum-level parser case policy.
    rename_all: Option<String>,
    /// Declaration-order variant inputs.
    variants: Vec<VariantSource>,
}

impl Pending {
    /// Builds deferred analysis from an authored unit enum and its HIR definition.
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
                rename_all: FromStrAttributes::from_attributes(&variant.attrs).rename_all,
            })
            .collect();
        Self {
            definition: item.owner_id.def_id,
            owner: item.hir_id(),
            span: item.span,
            rename_all: FromStrAttributes::from_attributes(&enumeration.attrs).rename_all,
            variants,
        }
    }
}

// -----------------------------------------------------------------------------
// Suggestion: Machine-applicable source edit
// -----------------------------------------------------------------------------

/// Source replacement attached when a redundant attribute is uniquely located.
struct Suggestion {
    /// Exact authored range to replace.
    span: Span,
    /// Replacement source text.
    replacement: String,
}

// -----------------------------------------------------------------------------
// Violation: Noncanonical FromStr naming policy
// -----------------------------------------------------------------------------

/// One redundant leaf directive or factorable enum-wide parser policy.
struct Violation {
    /// HIR owner receiving the lint.
    owner: HirId,
    /// Primary diagnostic location.
    span: Span,
    /// Explanation of the redundant representation.
    detail: String,
    /// Canonical representation requested from the author.
    remediation: String,
    /// Safe source edit for a uniquely located redundant attribute.
    suggestion: Option<Suggestion>,
}

impl LateViolation for Violation {
    fn primary_message(&self) -> Cow<'_, str> {
        Cow::Borrowed("this derive_more FromStr naming policy is noncanonical")
    }

    fn rationale_message(&self) -> Cow<'_, str> {
        Cow::Borrowed(&self.detail)
    }

    fn remediation_message(&self) -> Cow<'_, str> {
        Cow::Borrowed(&self.remediation)
    }

    fn emit(self, cx: &LateContext<'_>) {
        cx.tcx.emit_node_span_lint(
            DERIVE_MORE_NONCANONICAL_FROM_STR_POLICIES,
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
// DeriveMoreNoncanonicalFromStrPolicies: Lint pass
// -----------------------------------------------------------------------------

/// Collects unit enums and compares their parser spellings with container policies.
#[derive(Default)]
struct DeriveMoreNoncanonicalFromStrPolicies {
    /// Generated derive_more contracts indexed by source definition.
    catalog: DeriveMoreContractCatalog,
    /// Authored enums awaiting generated-contract confirmation.
    pending: Vec<Pending>,
}

crate::impl_late_lint! {
    #[doc = include_str!("README.md")]
    pub DERIVE_MORE_NONCANONICAL_FROM_STR_POLICIES,
    Warn,
    "finds derive_more FromStr names that should inherit a container policy",
    DeriveMoreNoncanonicalFromStrPolicies::default()
}

impl DeriveMoreNoncanonicalFromStrPolicies {
    /// Removes variant policies that exactly repeat an existing container policy.
    fn analyze_redundant_variants(cx: &LateContext<'_>, pending: &Pending, container: &str) {
        for variant in &pending.variants {
            if variant.rename_all.as_deref() != Some(container) {
                continue;
            }
            let expected = format!("#[from_str(rename_all=\"{container}\")]");
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
                    "`{}` repeats the inherited `{container}` parser policy",
                    variant.name
                ),
                remediation: "remove the redundant variant-level `from_str` directive".to_owned(),
                suggestion,
            }
            .emit(cx);
        }
    }

    /// Factors exact variant spellings into one cheaper enum-wide policy.
    fn analyze_container_policy(cx: &LateContext<'_>, pending: &Pending) {
        // Hoisting changes implicit variants from case-insensitive to exact matching.
        if pending.variants.is_empty()
            || pending
                .variants
                .iter()
                .any(|variant| variant.rename_all.is_none())
        {
            return;
        }
        let effective = pending
            .variants
            .iter()
            .map(|variant| variant.rename_all.as_deref().apply_to(&variant.name))
            .collect::<Vec<_>>();
        let policies = CASES.map(|case| NamingPolicyCandidate {
            name: case,
            names: pending
                .variants
                .iter()
                .map(|variant| Some(case).apply_to(&variant.name))
                .collect(),
            directive_cost: 1,
        });

        // Ambiguous or non-reducing mappings do not justify changing parser semantics.
        let Some(policy) = NamingPolicyFactor::factor(&effective, pending.variants.len(), policies)
        else {
            return;
        };
        Self::emit_container_violation(cx, pending, &policy);
    }

    /// Emits the canonical parser policy and names any required exact exceptions.
    fn emit_container_violation(
        cx: &LateContext<'_>,
        pending: &Pending,
        policy: &NamingPolicyFactor,
    ) {
        let exceptions = policy
            .exceptions
            .iter()
            .map(|index| pending.variants[*index].name.as_str())
            .collect::<Vec<_>>();
        let suffix = if exceptions.is_empty() {
            String::new()
        } else {
            format!(
                " and retain exact overrides only for {}",
                exceptions.join(", ")
            )
        };
        Violation {
            owner: pending.owner,
            span: pending.span,
            detail: format!(
                "variant parser casings reduce to one exact `{}` policy",
                policy.name
            ),
            remediation: format!(
                "declare `#[from_str(rename_all = \"{}\")]` on the enum{suffix}",
                policy.name
            ),
            suggestion: None,
        }
        .emit(cx);
    }

    /// Chooses redundant-leaf or container factoring analysis for one derived enum.
    fn analyze(cx: &LateContext<'_>, pending: &Pending) {
        // An existing container policy can only make matching leaf policy redundant.
        if let Some(container) = &pending.rename_all {
            Self::analyze_redundant_variants(cx, pending, container);
            return;
        }
        Self::analyze_container_policy(cx, pending);
    }
}

impl LateLintPass<'_> for DeriveMoreNoncanonicalFromStrPolicies {
    fn check_item(&mut self, cx: &LateContext<'_>, item: &Item<'_>) {
        self.catalog.check_item(cx, item);

        // Macro-generated enums have no stable authored attribute representation.
        if item.span.from_expansion() {
            return;
        }

        // FromStr naming policy is defined only for enum variants.
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

        // derive_more's FromStr contract currently supports only unit variants.
        if enumeration
            .variants
            .iter()
            .any(|variant| !variant.fields.is_empty())
        {
            return;
        }
        self.pending
            .push(Pending::from_enum(item, &enumeration, &definition));
    }

    fn check_crate_post(&mut self, cx: &LateContext<'_>) {
        for pending in self.pending.drain(..) {
            // Only enums confirmed to derive FromStr participate in this rule.
            if self
                .catalog
                .derived_type(pending.definition, "FromStr")
                .is_none()
            {
                continue;
            }
            Self::analyze(cx, &pending);
        }
    }
}
