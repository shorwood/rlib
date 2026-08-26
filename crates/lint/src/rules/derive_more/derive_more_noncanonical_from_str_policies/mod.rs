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

fn rename_all(attributes: &[syn::Attribute]) -> Option<String> {
    let mut output = None;
    for attribute in attributes
        .iter()
        .filter(|attribute| attribute.path().is_ident("from_str"))
    {
        let _ = attribute.parse_nested_meta(|meta| {
            if meta.path.is_ident("rename_all") {
                output = Some(meta.value()?.parse::<syn::LitStr>()?.value());
            }
            Ok(())
        });
    }
    output
}

struct VariantSource {
    name: String,
    span: Span,
    rename_all: Option<String>,
}

struct Pending {
    definition: LocalDefId,
    owner: HirId,
    span: Span,
    rename_all: Option<String>,
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
struct DeriveMoreNoncanonicalFromStrPolicies {
    catalog: DeriveMoreContractCatalog,
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
    fn analyze(cx: &LateContext<'_>, pending: &Pending) {
        if let Some(container) = &pending.rename_all {
            for variant in &pending.variants {
                if variant.rename_all.as_deref() != Some(container) {
                    continue;
                }
                let expected = format!("#[from_str(rename_all=\"{container}\")]");
                let suggestion = standalone_attribute_span(cx, pending.span, &expected)
                    .map(|span| (span, String::new()));
                Violation {
                    owner: pending.owner,
                    span: suggestion.as_ref().map_or(variant.span, |(span, _)| *span),
                    detail: format!(
                        "`{}` repeats the inherited `{container}` parser policy",
                        variant.name
                    ),
                    remediation: "remove the redundant variant-level `from_str` directive"
                        .to_owned(),
                    suggestion,
                }
                .emit(cx);
            }
            return;
        }

        // A top-level rename makes every variant exact-case. Do not hoist across any currently
        // implicit variant because derive_more accepts those variants case-insensitively.
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
            .map(|variant| apply_case(&variant.name, variant.rename_all.as_deref()))
            .collect::<Vec<_>>();
        let policies = CASES.map(|case| CandidatePolicy {
            name: case,
            names: pending
                .variants
                .iter()
                .map(|variant| apply_case(&variant.name, Some(case)))
                .collect(),
            directive_cost: 1,
        });
        let Some(policy) = factor_names(&effective, pending.variants.len(), policies) else {
            return;
        };
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
}

impl LateLintPass<'_> for DeriveMoreNoncanonicalFromStrPolicies {
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
        if enumeration
            .variants
            .iter()
            .any(|variant| !variant.fields.is_empty())
        {
            return;
        }
        let variants = enumeration
            .variants
            .iter()
            .zip(definition.variants)
            .map(|(variant, hir)| VariantSource {
                name: variant.ident.to_string(),
                span: hir.span,
                rename_all: rename_all(&variant.attrs),
            })
            .collect();
        self.pending.push(Pending {
            definition: item.owner_id.def_id,
            owner: item.hir_id(),
            span: item.span,
            rename_all: rename_all(&enumeration.attrs),
            variants,
        });
    }

    fn check_crate_post(&mut self, cx: &LateContext<'_>) {
        for pending in self.pending.drain(..) {
            if self
                .catalog
                .derived_type(pending.definition, "FromStr")
                .is_some()
            {
                Self::analyze(cx, &pending);
            }
        }
    }
}
