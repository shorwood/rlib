extern crate rustc_errors;
extern crate rustc_hir;
extern crate rustc_span;

use std::borrow::Cow;
use std::collections::HashSet;

use heck::{
    ToKebabCase, ToLowerCamelCase, ToShoutyKebabCase, ToShoutySnakeCase, ToSnakeCase, ToTitleCase,
    ToTrainCase, ToUpperCamelCase,
};
use rustc_errors::{Applicability, DiagDecorator};
use rustc_hir::{HirId, Item};
use rustc_lint::{LateContext, LateLintPass};
use rustc_span::Span;
use rustc_span::def_id::LocalDefId;

use super::utils::contracts::{ContractCatalog, EnumContract, StrumDerive, VariantContract};
use crate::utils::diagnostic::LateViolation;
use crate::utils::name_policy::{CandidatePolicy, factor_names, standalone_attribute_span};
use crate::utils::source_provenance::AuthoredItemSource;

const CASES: [&str; 11] = [
    "camelCase",
    "kebab-case",
    "lowercase",
    "mixed_case",
    "PascalCase",
    "SCREAMING-KEBAB-CASE",
    "SCREAMING_SNAKE_CASE",
    "snake_case",
    "title_case",
    "Train-Case",
    "UPPERCASE",
];

fn apply_case(value: &str, case: &str) -> String {
    match case {
        "camelCase" => value.to_lower_camel_case(),
        "kebab-case" => value.to_kebab_case(),
        "lowercase" => value.to_lowercase(),
        "mixed_case" | "snake_case" => value.to_snake_case(),
        "PascalCase" => value.to_upper_camel_case(),
        "SCREAMING-KEBAB-CASE" => value.to_shouty_kebab_case(),
        "SCREAMING_SNAKE_CASE" => value.to_shouty_snake_case(),
        "title_case" => value.to_title_case(),
        "Train-Case" => value.to_train_case(),
        "UPPERCASE" => value.to_uppercase(),
        _ => value.to_owned(),
    }
}

#[derive(Default)]
struct TypeAttributes {
    serialize_all: Option<String>,
    prefix: Option<String>,
    suffix: Option<String>,
}

#[derive(Default)]
struct VariantAttributes {
    serializations: Vec<String>,
    to_string: Option<String>,
    disabled: bool,
    transparent: bool,
}

fn parse_type(attributes: &[syn::Attribute]) -> TypeAttributes {
    let mut output = TypeAttributes::default();
    for attribute in attributes
        .iter()
        .filter(|attribute| attribute.path().is_ident("strum"))
    {
        let _ = attribute.parse_nested_meta(|meta| {
            let value = || Ok::<_, syn::Error>(meta.value()?.parse::<syn::LitStr>()?.value());
            if meta.path.is_ident("serialize_all") {
                output.serialize_all = Some(value()?);
            } else if meta.path.is_ident("prefix") {
                output.prefix = Some(value()?);
            } else if meta.path.is_ident("suffix") {
                output.suffix = Some(value()?);
            }
            Ok(())
        });
    }
    output
}

fn parse_variant(attributes: &[syn::Attribute]) -> VariantAttributes {
    let mut output = VariantAttributes::default();
    for attribute in attributes
        .iter()
        .filter(|attribute| attribute.path().is_ident("strum"))
    {
        let _ = attribute.parse_nested_meta(|meta| {
            if meta.path.is_ident("serialize") {
                output
                    .serializations
                    .push(meta.value()?.parse::<syn::LitStr>()?.value());
            } else if meta.path.is_ident("to_string") {
                output.to_string = Some(meta.value()?.parse::<syn::LitStr>()?.value());
            } else if meta.path.is_ident("disabled") {
                output.disabled = true;
            } else if meta.path.is_ident("transparent") {
                output.transparent = true;
            }
            Ok(())
        });
    }
    output
}

struct Pending {
    definition: LocalDefId,
    owner: HirId,
    span: Span,
    enumeration: syn::ItemEnum,
}

type ActiveVariant<'a> = (
    (&'a syn::Variant, &'a VariantAttributes),
    &'a VariantContract,
);

struct Violation {
    owner: HirId,
    span: Span,
    detail: String,
    remediation: String,
    suggestion: Option<(Span, String)>,
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
                if let Some((span, replacement)) = self.suggestion {
                    diag.span_suggestion(
                        span,
                        remediation,
                        replacement,
                        Applicability::MachineApplicable,
                    );
                } else {
                    diag.help(remediation);
                }
            }),
        );
    }
}

#[derive(Default)]
struct StrumNoncanonicalStringPolicies {
    catalog: ContractCatalog,
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
    fn analyze_variants(
        cx: &LateContext<'_>,
        pending: &Pending,
        ty: &TypeAttributes,
        active: &[ActiveVariant<'_>],
    ) {
        for ((variant, attributes), variant_contract) in active {
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
                let suggestion = standalone_attribute_span(cx, pending.span, &expected)
                    .map(|span| (span, String::new()));
                Violation {
                    owner: pending.owner,
                    span: suggestion
                        .as_ref()
                        .map_or(variant_contract.span, |(span, _)| *span),
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

            if let Some(output) = &attributes.to_string
                && attributes
                    .serializations
                    .iter()
                    .any(|serialization| serialization == output)
                && attributes
                    .serializations
                    .iter()
                    .max_by_key(|serialization| serialization.len())
                    == Some(output)
            {
                Violation {
                    owner: pending.owner,
                    span: variant_contract.span,
                    detail: format!(
                        "`to_string = \"{output}\"` repeats the unambiguous longest serialization"
                    ),
                    remediation: "remove the redundant `to_string` directive".to_owned(),
                    suggestion: None,
                }
                .emit(cx);
            }
        }
    }

    fn factor_case(
        cx: &LateContext<'_>,
        pending: &Pending,
        active: &[ActiveVariant<'_>],
        names: &[String],
        authored: usize,
    ) {
        let policies = CASES.map(|case| CandidatePolicy {
            name: case,
            names: active
                .iter()
                .map(|((variant, _), _)| apply_case(&variant.ident.to_string(), case))
                .collect(),
            directive_cost: 1,
        });
        if let Some(policy) = factor_names(names, authored, policies) {
            let exceptions = policy
                .exceptions
                .iter()
                .map(|index| active[*index].0.0.ident.to_string())
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
    }

    fn affix_policy(
        active: &[ActiveVariant<'_>],
        names: &[String],
        case: &'static str,
    ) -> Option<(&'static str, String, String, usize, Vec<String>)> {
        let converted = active
            .iter()
            .map(|((variant, _), _)| apply_case(&variant.ident.to_string(), case))
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
        Some((case, prefix.to_owned(), suffix.to_owned(), cost, generated))
    }

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
            .map(|(case, _, _, directive_cost, generated)| CandidatePolicy {
                name: case,
                names: generated.clone(),
                directive_cost: *directive_cost,
            });
        let Some(policy) = factor_names(names, authored, policies) else {
            return;
        };
        let Some((case, prefix, suffix, _, _)) = affix_policies
            .iter()
            .find(|(case, _, _, _, _)| *case == policy.name)
        else {
            return;
        };
        let prefix = if prefix.is_empty() {
            String::new()
        } else {
            format!(", prefix = \"{prefix}\"")
        };
        let suffix = if suffix.is_empty() {
            String::new()
        } else {
            format!(", suffix = \"{suffix}\"")
        };
        Violation {
            owner: pending.owner,
            span: pending.span,
            detail: "variant output spellings repeat one case policy and common affixes".to_owned(),
            remediation: format!(
                "declare `serialize_all = \"{case}\"`{prefix}{suffix} on the enum"
            ),
            suggestion: None,
        }
        .emit(cx);
    }

    fn analyze(cx: &LateContext<'_>, pending: &Pending, contract: &EnumContract) {
        let has_string_derive = [
            StrumDerive::AsRefStr,
            StrumDerive::Display,
            StrumDerive::EnumString,
            StrumDerive::IntoStaticStr,
            StrumDerive::VariantNames,
        ]
        .into_iter()
        .any(|derive| contract.has_derive(derive));
        if !has_string_derive {
            return;
        }
        let ty = parse_type(&pending.enumeration.attrs);
        let parsed = pending
            .enumeration
            .variants
            .iter()
            .map(|variant| parse_variant(&variant.attrs))
            .collect::<Vec<_>>();
        let active = pending
            .enumeration
            .variants
            .iter()
            .zip(&parsed)
            .zip(&contract.variants)
            .filter(|((_, attributes), contract)| {
                !attributes.disabled && !attributes.transparent && !contract.has_payload
            })
            .collect::<Vec<_>>();
        Self::analyze_variants(cx, pending, &ty, &active);
        if ty.serialize_all.is_some() || active.is_empty() {
            return;
        }
        let names = active
            .iter()
            .map(|((variant, attributes), _)| {
                attributes
                    .serializations
                    .first()
                    .cloned()
                    .unwrap_or_else(|| variant.ident.to_string())
            })
            .collect::<Vec<_>>();
        let authored = active
            .iter()
            .filter(|((_, attributes), _)| {
                attributes.serializations.len() == 1 && attributes.to_string.is_none()
            })
            .count();
        Self::factor_case(cx, pending, &active, &names, authored);
        // Prefix and suffix affect output derives but not EnumString's parser language.
        if !contract.has_derive(StrumDerive::EnumString)
            && ty.prefix.is_none()
            && ty.suffix.is_none()
        {
            Self::factor_affixes(cx, pending, &active, &names, authored);
        }
    }
}

impl LateLintPass<'_> for StrumNoncanonicalStringPolicies {
    fn check_item(&mut self, cx: &LateContext<'_>, item: &Item<'_>) {
        self.catalog.check_item(cx, item);
        if item.span.from_expansion() {
            return;
        }
        let Some(source) = AuthoredItemSource::for_item(cx, item) else {
            return;
        };
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
