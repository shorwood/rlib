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
use super::utils::naming::{CASES, apply_case};
use crate::utils::diagnostic::LateViolation;
use crate::utils::name_policy::{CandidatePolicy, factor_names, standalone_attribute_span};
use crate::utils::source_provenance::AuthoredItemSource;

#[derive(Default)]
struct DisplayAttributes {
    rename_all: Option<String>,
    static_format: Option<String>,
    has_format: bool,
}

fn display_attributes(attributes: &[syn::Attribute]) -> DisplayAttributes {
    let mut output = DisplayAttributes::default();
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
        let _ = attribute.parse_nested_meta(|meta| {
            if meta.path.is_ident("rename_all") {
                output.rename_all = Some(meta.value()?.parse::<syn::LitStr>()?.value());
            }
            Ok(())
        });
    }
    output
}

struct VariantSource {
    name: String,
    span: Span,
    attributes: DisplayAttributes,
    is_unit: bool,
}

struct Pending {
    definition: LocalDefId,
    owner: HirId,
    span: Span,
    container: DisplayAttributes,
    variants: Vec<VariantSource>,
}

struct Violation {
    owner: HirId,
    span: Span,
    detail: String,
    remediation: String,
    suggestion: Option<(Span, String)>,
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
struct DeriveMoreNoncanonicalDisplayPolicies {
    catalog: DeriveMoreContractCatalog,
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
    fn analyze(cx: &LateContext<'_>, pending: &Pending) {
        let units = pending
            .variants
            .iter()
            .filter(|variant| variant.is_unit)
            .collect::<Vec<_>>();
        for variant in &units {
            let inherited = apply_case(&variant.name, pending.container.rename_all.as_deref());
            let expected = if variant.attributes.static_format.as_deref() == Some(&inherited) {
                Some(format!("#[display(\"{inherited}\")]"))
            } else if let Some(case) = &variant.attributes.rename_all
                && apply_case(&variant.name, Some(case)) == inherited
            {
                Some(format!("#[display(rename_all=\"{case}\")]"))
            } else {
                None
            };
            let Some(expected) = expected else { continue };
            let suggestion = standalone_attribute_span(cx, pending.span, &expected)
                .map(|span| (span, String::new()));
            Violation {
                owner: pending.owner,
                span: suggestion.as_ref().map_or(variant.span, |(span, _)| *span),
                detail: format!(
                    "`{}` already displays as `{inherited}` without this leaf directive",
                    variant.name
                ),
                remediation: "remove the redundant Display directive".to_owned(),
                suggestion,
            }
            .emit(cx);
        }

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
                    apply_case(&variant.name, variant.attributes.rename_all.as_deref())
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
        let policies = CASES.map(|case| CandidatePolicy {
            name: case,
            names: units
                .iter()
                .map(|variant| apply_case(&variant.name, Some(case)))
                .collect(),
            directive_cost: 1,
        });
        let Some(policy) = factor_names(&effective, authored, policies) else {
            return;
        };
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
}

impl LateLintPass<'_> for DeriveMoreNoncanonicalDisplayPolicies {
    fn check_item(&mut self, cx: &LateContext<'_>, item: &Item<'_>) {
        self.catalog.check_item(cx, item);
        if item.span.from_expansion() {
            return;
        }
        let ItemKind::Enum(_, _, definition) = item.kind else {
            return;
        };
        let Some(source) = AuthoredItemSource::for_item(cx, item) else {
            return;
        };
        let Ok(enumeration) = syn::parse_str::<syn::ItemEnum>(&source) else {
            return;
        };
        let variants = enumeration
            .variants
            .iter()
            .zip(definition.variants)
            .map(|(variant, hir)| VariantSource {
                name: variant.ident.to_string(),
                span: hir.span,
                attributes: display_attributes(&variant.attrs),
                is_unit: variant.fields.is_empty(),
            })
            .collect();
        self.pending.push(Pending {
            definition: item.owner_id.def_id,
            owner: item.hir_id(),
            span: item.span,
            container: display_attributes(&enumeration.attrs),
            variants,
        });
    }

    fn check_crate_post(&mut self, cx: &LateContext<'_>) {
        for pending in self.pending.drain(..) {
            if self
                .catalog
                .derived_type(pending.definition, "Display")
                .is_some()
            {
                Self::analyze(cx, &pending);
            }
        }
    }
}
