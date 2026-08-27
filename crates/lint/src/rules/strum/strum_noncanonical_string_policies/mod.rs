extern crate rustc_errors;
extern crate rustc_hir;
extern crate rustc_span;

use std::borrow::Cow;
use std::collections::HashSet;

use rustc_errors::{Applicability, DiagDecorator};
use rustc_hir::{HirId, Item};
use rustc_lint::{LateContext, LateLintPass};
use rustc_span::Span;
use rustc_span::def_id::LocalDefId;

use super::utils::contracts::{
    ContractCatalog, EnumContract, StrumDerive, VariantContract, apply_case,
};
use crate::utils::diagnostic::LateViolation;
use crate::utils::name_policy::{
    AuthoredAttributeSpanExt as _, NamingPolicyCandidate, NamingPolicyFactor,
};
use crate::utils::source_provenance::AuthoredItemSource;

// -----------------------------------------------------------------------------
// Cases: Strum naming policies
// -----------------------------------------------------------------------------

/// Strum naming policies considered for container factoring.
const CASES: [&str; 10] = [
    "camelCase",
    "kebab-case",
    "lowercase",
    "PascalCase",
    "SCREAMING-KEBAB-CASE",
    "SCREAMING_SNAKE_CASE",
    "snake_case",
    "title_case",
    "Train-Case",
    "UPPERCASE",
];

// -----------------------------------------------------------------------------
// TypeAttributes: Relevant enum policy
// -----------------------------------------------------------------------------

/// String policy authored on a Strum enum.
#[derive(Default)]
struct TypeAttributes {
    /// Case conversion applied to participating variants.
    serialize_all: Option<String>,
    /// Common output prefix.
    prefix: Option<String>,
    /// Common output suffix.
    suffix: Option<String>,
}

impl TypeAttributes {
    /// Reads relevant Strum policy from enum attributes.
    fn from_attributes(attributes: &[syn::Attribute]) -> Self {
        let mut output = Self::default();
        for attribute in attributes
            .iter()
            .filter(|attribute| attribute.path().is_ident("strum"))
        {
            match attribute.parse_nested_meta(|meta| {
                let value = || Ok::<_, syn::Error>(meta.value()?.parse::<syn::LitStr>()?.value());
                if meta.path.is_ident("serialize_all") {
                    output.serialize_all = Some(value()?);
                } else if meta.path.is_ident("prefix") {
                    output.prefix = Some(value()?);
                } else if meta.path.is_ident("suffix") {
                    output.suffix = Some(value()?);
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
// VariantAttributes: Relevant variant policy
// -----------------------------------------------------------------------------

/// String spellings and participation flags authored on one Strum variant.
#[derive(Default)]
struct VariantAttributes {
    /// Parser and output spellings in authored order.
    serializations: Vec<String>,
    /// Explicit preferred output spelling.
    to_string: Option<String>,
    /// Whether the variant is excluded from generated string contracts.
    is_disabled: bool,
    /// Whether string behavior delegates to a payload field.
    is_transparent: bool,
}

impl VariantAttributes {
    /// Reads relevant Strum policy from variant attributes.
    fn from_attributes(attributes: &[syn::Attribute]) -> Self {
        let mut output = Self::default();
        for attribute in attributes
            .iter()
            .filter(|attribute| attribute.path().is_ident("strum"))
        {
            match attribute.parse_nested_meta(|meta| {
                if meta.path.is_ident("serialize") {
                    output
                        .serializations
                        .push(meta.value()?.parse::<syn::LitStr>()?.value());
                } else if meta.path.is_ident("to_string") {
                    output.to_string = Some(meta.value()?.parse::<syn::LitStr>()?.value());
                } else if meta.path.is_ident("disabled") {
                    output.is_disabled = true;
                } else if meta.path.is_ident("transparent") {
                    output.is_transparent = true;
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
// Pending: Deferred enum analysis
// -----------------------------------------------------------------------------

/// Authored enum retained until its generated Strum contract is known.
struct Pending {
    /// Local definition used for generated contract lookup.
    definition: LocalDefId,
    /// HIR owner receiving diagnostics.
    owner: HirId,
    /// Authored enum span.
    span: Span,
    /// Parsed authored enum syntax.
    enumeration: syn::ItemEnum,
}

// -----------------------------------------------------------------------------
// ActiveVariant: Participating variant state
// -----------------------------------------------------------------------------

/// Authored and generated facts for one variant participating in string contracts.
struct ActiveVariant<'a> {
    /// Authored variant syntax.
    variant: &'a syn::Variant,
    /// Parsed authored Strum policy.
    attributes: &'a VariantAttributes,
    /// Generated Strum contract.
    contract: &'a VariantContract,
}

// -----------------------------------------------------------------------------
// FactorInputs: Container factoring evidence
// -----------------------------------------------------------------------------

/// Effective variant spellings and authored costs used by policy factoring.
struct FactorInputs {
    /// Effective output name for each active variant.
    names: Vec<String>,
    /// Variants whose extra spellings require a leaf exception.
    required_exceptions: Vec<bool>,
    /// Number of authored leaf directives represented by the input.
    authored: usize,
}

impl From<&[ActiveVariant<'_>]> for FactorInputs {
    fn from(active: &[ActiveVariant<'_>]) -> Self {
        let names = active
            .iter()
            .map(|active| {
                active
                    .attributes
                    .serializations
                    .first()
                    .cloned()
                    .unwrap_or_else(|| active.variant.ident.to_string())
            })
            .collect();
        let required_exceptions = active
            .iter()
            .map(|active| {
                active.attributes.serializations.len() > 1 || active.attributes.to_string.is_some()
            })
            .collect();
        let authored = active
            .iter()
            .filter(|active| {
                !active.attributes.serializations.is_empty()
                    || active.attributes.to_string.is_some()
            })
            .count();
        Self {
            names,
            required_exceptions,
            authored,
        }
    }
}

// -----------------------------------------------------------------------------
// AffixPolicy: Factored case and affixes
// -----------------------------------------------------------------------------

/// Candidate container policy reproducing every active variant spelling.
struct AffixPolicy {
    /// Strum case conversion name.
    case: &'static str,
    /// Common output prefix.
    prefix: String,
    /// Common output suffix.
    suffix: String,
    /// Number of required container directives.
    directive_cost: usize,
    /// Generated names in declaration order.
    names: Vec<String>,
}

// -----------------------------------------------------------------------------
// Suggestion: Machine-applicable source edit
// -----------------------------------------------------------------------------

/// Exact source replacement for a redundant Strum attribute.
struct Suggestion {
    /// Authored source range to replace.
    span: Span,
    /// Replacement source text.
    replacement: String,
}

// -----------------------------------------------------------------------------
// Violation: Noncanonical Strum string policy
// -----------------------------------------------------------------------------

/// One redundant spelling or factorable enum-wide policy.
struct Violation {
    /// HIR owner receiving the lint.
    owner: HirId,
    /// Primary diagnostic span.
    span: Span,
    /// Explanation of the redundant representation.
    detail: String,
    /// Canonical representation requested from the author.
    remediation: String,
    /// Safe edit for a uniquely located redundant attribute.
    suggestion: Option<Suggestion>,
}

impl LateViolation for Violation {
    fn primary_message(&self) -> Cow<'_, str> {
        Cow::Borrowed("this Strum string policy is noncanonical")
    }

    fn rationale_message(&self) -> Cow<'_, str> {
        Cow::Borrowed(&self.detail)
    }

    fn remediation_message(&self) -> Cow<'_, str> {
        Cow::Borrowed(&self.remediation)
    }

    fn emit(self, cx: &LateContext<'_>) {
        cx.tcx.emit_node_span_lint(
            STRUM_NONCANONICAL_STRING_POLICIES,
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
// StrumNoncanonicalStringPolicies: Lint pass
// -----------------------------------------------------------------------------

/// Collects authored enums and compares spellings with generated Strum contracts.
#[derive(Default)]
struct StrumNoncanonicalStringPolicies {
    /// Generated Strum contract catalog.
    catalog: ContractCatalog,
    /// Authored enums awaiting generated-contract correlation.
    pending: Vec<Pending>,
}

crate::impl_late_lint! {
    #[doc = include_str!("README.md")]
    pub STRUM_NONCANONICAL_STRING_POLICIES,
    Warn,
    "finds Strum variant strings that should inherit a type naming policy",
    StrumNoncanonicalStringPolicies::default()
}

impl StrumNoncanonicalStringPolicies {
    /// Returns whether Strum already selects this explicit output implicitly.
    fn is_implicit_output(attributes: &VariantAttributes, output: &str) -> bool {
        let serializations = &attributes.serializations;
        serializations
            .iter()
            .any(|serialization| serialization == output)
            && serializations
                .iter()
                .max_by_key(|serialization| serialization.len())
                .is_some_and(|longest| longest == output)
    }

    /// Reports a redundant explicit output selected by Strum's longest-name rule.
    fn analyze_output(cx: &LateContext<'_>, pending: &Pending, active: &ActiveVariant<'_>) {
        // Variants without an explicit output have no leaf directive to remove.
        let Some(output) = &active.attributes.to_string else {
            return;
        };

        // Only the unambiguous longest parser spelling is an implicit output.
        if !Self::is_implicit_output(active.attributes, output) {
            return;
        }
        Violation {
            owner: pending.owner,
            span: active.contract.span,
            detail: format!(
                "`to_string = \"{output}\"` repeats the unambiguous longest serialization"
            ),
            remediation: "remove the redundant `to_string` directive".to_owned(),
            suggestion: None,
        }
        .emit(cx);
    }

    /// Reports redundant and duplicate leaf spellings for participating variants.
    fn analyze_variants(
        cx: &LateContext<'_>,
        pending: &Pending,
        ty: &TypeAttributes,
        active: &[ActiveVariant<'_>],
    ) {
        for active_variant in active {
            let variant = active_variant.variant;
            let attributes = active_variant.attributes;
            let variant_contract = active_variant.contract;
            let implicit = apply_case(
                &variant.ident.to_string(),
                ty.serialize_all.as_deref().unwrap_or(""),
            );
            if attributes.serializations.len() == 1
                && attributes.to_string.is_none()
                && attributes.serializations[0] == implicit
            {
                let value = &attributes.serializations[0];
                let expected = format!("#[strum(serialize=\"{value}\")]");
                let suggestion = pending
                    .span
                    .standalone_attribute(cx, &expected)
                    .map(|span| Suggestion {
                        span,
                        replacement: String::new(),
                    });
                Violation {
                    owner: pending.owner,
                    span: suggestion
                        .as_ref()
                        .map_or(variant_contract.span, |edit| edit.span),
                    detail: format!(
                        "`{}` already receives `{value}` from its inherited spelling",
                        variant.ident
                    ),
                    remediation: "remove the redundant `serialize` directive".to_owned(),
                    suggestion,
                }
                .emit(cx);
            }

            let mut seen = HashSet::new();
            for serialization in &attributes.serializations {
                if seen.insert(serialization) {
                    continue;
                }
                Violation {
                    owner: pending.owner,
                    span: variant_contract.span,
                    detail: format!(
                        "`{}` repeats the parser spelling `{serialization}`",
                        variant.ident
                    ),
                    remediation: "remove the duplicate `serialize` directive".to_owned(),
                    suggestion: None,
                }
                .emit(cx);
            }

            Self::analyze_output(cx, pending, active_variant);
        }
    }

    /// Emits the selected case policy and names retained leaf exceptions.
    fn emit_case_factor(
        cx: &LateContext<'_>,
        pending: &Pending,
        active: &[ActiveVariant<'_>],
        authored: usize,
        policy: &NamingPolicyFactor,
    ) {
        let exceptions = policy
            .exceptions
            .iter()
            .map(|index| active[*index].variant.ident.to_string())
            .collect::<Vec<_>>();
        let suffix = if exceptions.is_empty() {
            String::new()
        } else {
            format!("; retain overrides only for {}", exceptions.join(", "))
        };
        Violation {
            owner: pending.owner,
            span: pending.span,
            detail: format!(
                "{authored} variant spellings reduce to one `{}` policy",
                policy.name
            ),
            remediation: format!(
                "declare `#[strum(serialize_all = \"{}\")]`{suffix}",
                policy.name
            ),
            suggestion: None,
        }
        .emit(cx);
    }

    /// Factors variant spellings into one case policy while retaining required exceptions.
    fn factor_case(
        cx: &LateContext<'_>,
        pending: &Pending,
        active: &[ActiveVariant<'_>],
        names: &[String],
        authored: usize,
        required_exceptions: &[bool],
    ) {
        let policies = CASES.map(|case| NamingPolicyCandidate {
            name: case,
            names: active
                .iter()
                .map(|active| apply_case(&active.variant.ident.to_string(), case))
                .collect(),
            directive_cost: 1,
        });

        // Ambiguous or non-reducing mappings do not justify changing authored policy.
        let Some(policy) = NamingPolicyFactor::factor_with_required(
            names,
            authored,
            required_exceptions,
            policies,
        ) else {
            return;
        };
        Self::emit_case_factor(cx, pending, active, authored, &policy);
    }

    /// Builds a container policy when all names share one case conversion and affix pair.
    fn affix_policy(
        active: &[ActiveVariant<'_>],
        names: &[String],
        case: &'static str,
    ) -> Option<AffixPolicy> {
        let converted = active
            .iter()
            .map(|active| apply_case(&active.variant.ident.to_string(), case))
            .collect::<Vec<_>>();
        let pairs = names
            .iter()
            .zip(&converted)
            .filter_map(|(name, base)| {
                let offset = name.find(base)?;
                Some((&name[..offset], &name[offset + base.len()..]))
            })
            .collect::<Vec<_>>();
        let (prefix, suffix) = *pairs.first()?;

        // Partial or identity affixes do not establish one useful container policy.
        if pairs.len() != names.len()
            || (prefix.is_empty() && suffix.is_empty())
            || !pairs.iter().all(|pair| *pair == (prefix, suffix))
        {
            return None;
        }
        let cost = 1 + usize::from(!prefix.is_empty()) + usize::from(!suffix.is_empty());
        let generated = converted
            .iter()
            .map(|base| format!("{prefix}{base}{suffix}"))
            .collect();
        Some(AffixPolicy {
            case,
            prefix: prefix.to_owned(),
            suffix: suffix.to_owned(),
            directive_cost: cost,
            names: generated,
        })
    }

    /// Emits the selected case policy and its common affixes.
    fn emit_affix_factor(cx: &LateContext<'_>, pending: &Pending, affixes: &AffixPolicy) {
        let prefix = if affixes.prefix.is_empty() {
            String::new()
        } else {
            format!(", prefix = \"{}\"", affixes.prefix)
        };
        let suffix = if affixes.suffix.is_empty() {
            String::new()
        } else {
            format!(", suffix = \"{}\"", affixes.suffix)
        };
        Violation {
            owner: pending.owner,
            span: pending.span,
            detail: "variant output spellings repeat one case policy and common affixes".to_owned(),
            remediation: format!(
                "declare `serialize_all = \"{}\"`{prefix}{suffix} on the enum",
                affixes.case
            ),
            suggestion: None,
        }
        .emit(cx);
    }

    /// Factors variant spellings into one case policy with common prefix and suffix.
    fn factor_affixes(
        cx: &LateContext<'_>,
        pending: &Pending,
        active: &[ActiveVariant<'_>],
        names: &[String],
        authored: usize,
    ) {
        let affix_policies = CASES
            .into_iter()
            .filter_map(|case| Self::affix_policy(active, names, case))
            .collect::<Vec<_>>();
        let policies = affix_policies
            .iter()
            .map(|candidate| NamingPolicyCandidate {
                name: candidate.case,
                names: candidate.names.clone(),
                directive_cost: candidate.directive_cost,
            });

        // Ambiguous or non-reducing mappings do not justify changing authored policy.
        let Some(policy) = NamingPolicyFactor::factor(names, authored, policies) else {
            return;
        };

        // The selected factor must retain its corresponding affix representation.
        let Some(affixes) = affix_policies
            .iter()
            .find(|candidate| candidate.case == policy.name)
        else {
            return;
        };
        Self::emit_affix_factor(cx, pending, affixes);
    }

    /// Returns whether generated behavior includes a Strum string contract.
    fn has_string_derive(contract: &EnumContract) -> bool {
        [
            StrumDerive::AsRefStr,
            StrumDerive::Display,
            StrumDerive::EnumString,
            StrumDerive::IntoStaticStr,
            StrumDerive::VariantNames,
        ]
        .into_iter()
        .any(|derive| contract.has_derive(derive))
    }

    /// Selects variants whose shape participates in type-wide string policy.
    fn active_variants<'a>(
        pending: &'a Pending,
        parsed: &'a [VariantAttributes],
        contract: &'a EnumContract,
    ) -> Vec<ActiveVariant<'a>> {
        pending
            .enumeration
            .variants
            .iter()
            .zip(parsed)
            .zip(&contract.variants)
            .filter_map(|((variant, attributes), contract)| {
                (!attributes.is_disabled && !attributes.is_transparent && !contract.has_payload)
                    .then_some(ActiveVariant {
                        variant,
                        attributes,
                        contract,
                    })
            })
            .collect()
    }

    /// Correlates one authored enum with generated Strum string behavior.
    fn analyze(cx: &LateContext<'_>, pending: &Pending, contract: &EnumContract) {
        // Enums without a string-producing or parsing derive have no Strum spelling contract.
        if !Self::has_string_derive(contract) {
            return;
        }
        let ty = TypeAttributes::from_attributes(&pending.enumeration.attrs);
        let parsed = pending
            .enumeration
            .variants
            .iter()
            .map(|variant| VariantAttributes::from_attributes(&variant.attrs))
            .collect::<Vec<_>>();
        let active = Self::active_variants(pending, &parsed, contract);
        Self::analyze_variants(cx, pending, &ty, &active);

        // Existing container policy or no participating variants leaves nothing to factor.
        if ty.serialize_all.is_some() || active.is_empty() {
            return;
        }

        // A type-level policy reaches every participating variant. Only factor when the entire
        // generated contract is represented by the spelling model below.
        if parsed
            .iter()
            .zip(&contract.variants)
            .any(|(attributes, variant)| {
                attributes.is_disabled || attributes.is_transparent || variant.has_payload
            })
        {
            return;
        }
        let inputs = FactorInputs::from(active.as_slice());
        Self::factor_case(
            cx,
            pending,
            &active,
            &inputs.names,
            inputs.authored,
            &inputs.required_exceptions,
        );

        // Prefix and suffix affect output derives but not EnumString's parser language.
        if contract.has_derive(StrumDerive::EnumString)
            || ty.prefix.is_some()
            || ty.suffix.is_some()
            || inputs.required_exceptions.iter().any(|required| *required)
        {
            return;
        }
        Self::factor_affixes(cx, pending, &active, &inputs.names, inputs.authored);
    }
}

impl LateLintPass<'_> for StrumNoncanonicalStringPolicies {
    fn check_item(&mut self, cx: &LateContext<'_>, item: &Item<'_>) {
        self.catalog.check_item(cx, item);

        // Macro-generated enums have no stable authored attribute representation.
        if item.span.from_expansion() {
            return;
        }

        // Unavailable source cannot support syntax-aware attribute analysis.
        let Some(source) = AuthoredItemSource::for_item(cx, item) else {
            return;
        };

        // Non-enum and invalid syntax is outside Strum's enum string policy.
        let Ok(enumeration) = syn::parse_str::<syn::ItemEnum>(&source) else {
            return;
        };
        self.pending.push(Pending {
            definition: item.owner_id.def_id,
            owner: item.hir_id(),
            span: item.span,
            enumeration,
        });
    }

    fn check_crate_post(&mut self, cx: &LateContext<'_>) {
        let contracts = self.catalog.contracts();
        for pending in self.pending.drain(..) {
            let Some(contract) = contracts
                .iter()
                .find(|contract| contract.def_id == pending.definition)
            else {
                continue;
            };
            Self::analyze(cx, &pending, contract);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::apply_case;

    #[test]
    fn mixed_case_matches_strums_lower_camel_contract() {
        assert_eq!(apply_case("UserCreated", "mixed_case"), "userCreated");
    }
}
