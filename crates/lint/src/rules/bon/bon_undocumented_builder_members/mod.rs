extern crate rustc_ast;
extern crate rustc_errors;
extern crate rustc_span;

use std::borrow::Cow;
use std::collections::HashSet;

use rustc_ast::ast::{AssocItemKind, FieldDef, Item, ItemKind, Param, VisibilityKind};
use rustc_errors::DiagDecorator;
use rustc_lint::{EarlyContext, EarlyLintPass, LintContext};
use rustc_span::Span;

use super::utils::attributes::{BonAttributeAnalysis, OptionType};
use crate::utils::diagnostic::EarlyViolation;

// -----------------------------------------------------------------------------
// Violation: Public member with undocumented generated behavior
// -----------------------------------------------------------------------------

/// Public builder member whose non-obvious setter contract lacks documentation.
struct Violation {
    /// Member type receiving the diagnostic.
    span: Span,
    /// Public member named in the diagnostic.
    member: String,
    /// Optional, default, conversion, or hidden behavior requiring explanation.
    behavior: &'static str,
}

impl Violation {
    /// Classifies undocumented behavior attached to one builder member.
    fn member_violation(
        cx: &EarlyContext<'_>,
        attributes: &[rustc_ast::Attribute],
        ty_span: Span,
        member: &str,
    ) -> Option<Self> {
        let authored_documents = attributes.iter().any(|attribute| {
            attribute
                .doc_str()
                .is_some_and(|documentation| !documentation.as_str().trim().is_empty())
        });
        if authored_documents || Self::has_substantive_builder_docs(cx, attributes) {
            return None;
        }

        let ty = match cx.sess().source_map().span_to_snippet(ty_span) {
            Ok(ty) => ty,
            Err(_error) => return None,
        };
        let behavior = if OptionType::is_option(&ty)
            && !BonAttributeAnalysis::builder_has_option(cx, attributes, "required")
        {
            "optional"
        } else if BonAttributeAnalysis::builder_has_option(cx, attributes, "default") {
            "default"
        } else if BonAttributeAnalysis::builder_has_option(cx, attributes, "into")
            || BonAttributeAnalysis::builder_has_option(cx, attributes, "with")
        {
            "conversion"
        } else {
            if !BonAttributeAnalysis::builder_has_option(cx, attributes, "skip")
                && !BonAttributeAnalysis::builder_has_option(cx, attributes, "field")
            {
                return None;
            }
            "hidden initialization"
        };

        Some(Self {
            span: ty_span,
            member: member.trim().to_owned(),
            behavior,
        })
    }

    /// Recognizes an explicit nonempty `doc { ... }` payload in nested Bon setter policy.
    fn has_substantive_builder_docs(
        cx: &EarlyContext<'_>,
        attributes: &[rustc_ast::Attribute],
    ) -> bool {
        let Some(attribute) = BonAttributeAnalysis::builder(attributes) else {
            return false;
        };
        let Ok(source) = BonAttributeAnalysis::source(cx, attribute) else {
            return false;
        };
        let bytes = source.as_bytes();
        let mut search = 0_usize;
        while let Some(relative) = source[search..].find("doc") {
            let start = search + relative;
            let end = start + 3;
            let left_boundary =
                start == 0 || !bytes[start - 1].is_ascii_alphanumeric() && bytes[start - 1] != b'_';
            let right_boundary =
                end == bytes.len() || !bytes[end].is_ascii_alphanumeric() && bytes[end] != b'_';
            if left_boundary && right_boundary {
                let tail = source[end..].trim_start();
                if let Some(payload) = tail.strip_prefix('{') {
                    if let Some(close) = payload.find('}') {
                        if !payload[..close].trim().is_empty() {
                            return true;
                        }
                    }
                }
            }
            search = end;
        }
        false
    }
}

impl EarlyViolation for Violation {
    fn primary_message(&self) -> Cow<'_, str> {
        Cow::Owned(format!(
            "public Bon member `{}` has undocumented {} behavior",
            self.member, self.behavior
        ))
    }

    fn rationale_message(&self) -> Cow<'_, str> {
        Cow::Borrowed(
            "generated setter signatures do not explain the domain meaning of optionality, defaults, conversions, or hidden initialization",
        )
    }

    fn remediation_message(&self) -> Cow<'_, str> {
        Cow::Borrowed(
            "document the member's caller-visible policy with a doc comment or explicit Bon setter documentation",
        )
    }

    fn emit(self, cx: &EarlyContext<'_>) {
        cx.emit_span_lint(
            BON_UNDOCUMENTED_BUILDER_MEMBERS,
            self.span,
            DiagDecorator(|diag| {
                diag.primary_message(self.primary_message().into_owned());
                diag.span_label(self.span, "this non-obvious builder policy is undocumented");
                diag.note(self.rationale_message().into_owned());
                diag.help(self.remediation_message().into_owned());
            }),
        );
    }
}

// -----------------------------------------------------------------------------
// BonUndocumentedBuilderMembers: Public setter documentation policy
// -----------------------------------------------------------------------------

/// Requires public generated setters to explain non-obvious behavior.
#[derive(Default)]
struct BonUndocumentedBuilderMembers {
    /// Depth below a non-public enclosing module.
    private_module_depth: usize,
    /// Public nominal types that can expose associated builder setters.
    public_types: HashSet<String>,
    /// Associated member findings deferred until all type declarations are known.
    associated: Vec<(String, Violation)>,
}

dylint_linting::impl_pre_expansion_lint! {
    #[doc = include_str!("README.md")]
    pub BON_UNDOCUMENTED_BUILDER_MEMBERS,
    Warn,
    "requires documentation for non-obvious public Bon member policy",
    BonUndocumentedBuilderMembers::default()
}

impl BonUndocumentedBuilderMembers {
    /// Checks documentation on a builder function parameter.
    fn parameter_violation(cx: &EarlyContext<'_>, parameter: &Param) -> Option<Violation> {
        let member = match cx.sess().source_map().span_to_snippet(parameter.pat.span) {
            Ok(member) => member,
            Err(_error) => return None,
        };
        Violation::member_violation(cx, &parameter.attrs, parameter.ty.span, &member)
    }

    /// Checks documentation on a derived builder field.
    fn field_violation(cx: &EarlyContext<'_>, field: &FieldDef) -> Option<Violation> {
        Violation::member_violation(
            cx,
            &field.attrs,
            field.ty.span,
            &field.ident?.name.to_string(),
        )
    }
}
impl EarlyLintPass for BonUndocumentedBuilderMembers {
    fn check_item(&mut self, cx: &EarlyContext<'_>, item: &Item) {
        if matches!(item.kind, ItemKind::Mod(..)) {
            if !matches!(item.vis.kind, VisibilityKind::Public) {
                self.private_module_depth += 1;
            }
            return;
        }
        if self.private_module_depth > 0 {
            return;
        }
        if matches!(
            item.kind,
            ItemKind::Struct(..) | ItemKind::Enum(..) | ItemKind::Union(..)
        ) && matches!(item.vis.kind, VisibilityKind::Public)
        {
            if let Some(identifier) = item.kind.ident() {
                self.public_types.insert(identifier.name.to_string());
            }
        }
        match &item.kind {
            ItemKind::Fn(function)
                if matches!(item.vis.kind, VisibilityKind::Public)
                    && BonAttributeAnalysis::builder(&item.attrs).is_some() =>
            {
                for parameter in &function.sig.decl.inputs {
                    let Some(violation) = Self::parameter_violation(cx, parameter) else {
                        continue;
                    };
                    violation.emit(cx);
                }
            }
            ItemKind::Struct(_, _, data)
                if matches!(item.vis.kind, VisibilityKind::Public)
                    && BonAttributeAnalysis::derives_builder(cx, &item.attrs) =>
            {
                for field in data.fields() {
                    let Some(violation) = Self::field_violation(cx, field) else {
                        continue;
                    };
                    violation.emit(cx);
                }
            }
            ItemKind::Impl(implementation) => {
                let Ok(owner) = cx
                    .sess()
                    .source_map()
                    .span_to_snippet(implementation.self_ty.span)
                else {
                    return;
                };
                let owner = owner
                    .split('<')
                    .next()
                    .unwrap_or(&owner)
                    .trim()
                    .rsplit("::")
                    .next()
                    .unwrap_or_default()
                    .to_owned();
                for associated in &implementation.items {
                    let AssocItemKind::Fn(function) = &associated.kind else {
                        continue;
                    };
                    if !matches!(associated.vis.kind, VisibilityKind::Public)
                        || BonAttributeAnalysis::builder(&associated.attrs).is_none()
                    {
                        continue;
                    }
                    for parameter in &function.sig.decl.inputs {
                        let Some(violation) = Self::parameter_violation(cx, parameter) else {
                            continue;
                        };
                        self.associated.push((owner.clone(), violation));
                    }
                }
            }
            _ => {}
        }
    }

    fn check_item_post(&mut self, _: &EarlyContext<'_>, item: &Item) {
        if matches!(item.kind, ItemKind::Mod(..))
            && !matches!(item.vis.kind, VisibilityKind::Public)
        {
            self.private_module_depth -= 1;
        }
    }

    fn check_crate_post(&mut self, cx: &EarlyContext<'_>, _: &rustc_ast::Crate) {
        for (owner, violation) in self.associated.drain(..) {
            if self.public_types.contains(&owner) {
                violation.emit(cx);
            }
        }
    }
}
