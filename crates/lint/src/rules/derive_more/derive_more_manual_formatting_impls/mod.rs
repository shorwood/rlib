extern crate rustc_errors;
extern crate rustc_hir;
extern crate rustc_middle;
extern crate rustc_span;

use std::borrow::Cow;
use std::collections::HashMap;

use rustc_errors::DiagDecorator;
use rustc_hir::{Expr, ExprKind, ImplItem, ImplItemKind, ItemKind, Mutability, Node};
use rustc_lint::{LateContext, LateLintPass, LintContext};
use rustc_middle::ty;
use rustc_span::def_id::LocalDefId;
use rustc_span::Span;
use syn::parse::Parser;

use crate::utils::diagnostic::LateViolation;
use crate::utils::direct_forwarding::DirectForwarding;

struct Family {
    span: Span,
    name: String,
    traits: Vec<&'static str>,
}

struct Violation(Family);

impl LateViolation for Violation {
    fn primary_message(&self) -> Cow<'_, str> {
        Cow::Owned(format!(
            "manual formatting for `{}` is derivable",
            self.0.name
        ))
    }

    fn rationale_message(&self) -> Cow<'_, str> {
        Cow::Borrowed(
            "each implementation is one exact field delegation or one single-field format invocation without policy-bearing control flow",
        )
    }

    fn remediation_message(&self) -> Cow<'_, str> {
        let derives = self
            .0
            .traits
            .iter()
            .map(|name| format!("derive_more::{name}"))
            .collect::<Vec<_>>()
            .join(", ");
        Cow::Owned(format!(
            "replace this family with `#[derive({derives})]`, preserving exact format strings in derive_more attributes"
        ))
    }

    fn emit(self, cx: &LateContext<'_>) {
        cx.emit_span_lint(
            DERIVE_MORE_MANUAL_FORMATTING_IMPLS,
            self.0.span,
            DiagDecorator(|diag| {
                diag.primary_message(self.primary_message().into_owned());
                diag.span_label(self.0.span, "this type owns derivable formatting plumbing");
                diag.note(self.rationale_message().into_owned());
                diag.help(self.remediation_message().into_owned());
            }),
        );
    }
}

#[derive(Default)]
struct DeriveMoreManualFormattingImpls {
    families: HashMap<LocalDefId, Family>,
}

dylint_linting::impl_late_lint! {
    #[doc = include_str!("README.md")]
    pub DERIVE_MORE_MANUAL_FORMATTING_IMPLS,
    Warn,
    "finds formatting implementations reproducible by derive_more",
    DeriveMoreManualFormattingImpls::default()
}

impl<'tcx> LateLintPass<'tcx> for DeriveMoreManualFormattingImpls {
    fn check_impl_item(&mut self, cx: &LateContext<'tcx>, item: &'tcx ImplItem<'tcx>) {
        let Some((definition, trait_name)) = exact_formatting(cx, item) else {
            return;
        };
        let family = self.families.entry(definition).or_insert_with(|| Family {
            span: cx.tcx.def_span(definition),
            name: cx.tcx.item_name(definition).to_string(),
            traits: Vec::new(),
        });
        if !family.traits.contains(&trait_name) {
            family.traits.push(trait_name);
        }
    }

    fn check_crate_post(&mut self, cx: &LateContext<'tcx>) {
        for (_, mut family) in self.families.drain() {
            family.traits.sort_unstable();
            Violation(family).emit(cx);
        }
    }
}

fn exact_formatting(
    cx: &LateContext<'_>,
    item: &ImplItem<'_>,
) -> Option<(LocalDefId, &'static str)> {
    let ImplItemKind::Fn(signature, body_id) = item.kind else {
        return None;
    };
    if item.ident.name.as_str() != "fmt" || item.span.from_expansion() {
        return None;
    }
    let implementation = cx.tcx.local_parent(item.owner_id.def_id);
    let Node::Item(parent) = cx.tcx.hir_node_by_def_id(implementation) else {
        return None;
    };
    let ItemKind::Impl(implementation_item) = parent.kind else {
        return None;
    };
    let trait_id = implementation_item.of_trait?.trait_ref.trait_def_id()?;
    if cx.tcx.crate_name(trait_id.krate).as_str() != "core" {
        return None;
    }
    let trait_name = formatting_trait(cx.tcx.item_name(trait_id).as_str())?;
    let trait_ref = cx.tcx.impl_trait_ref(implementation).instantiate_identity();
    let ty::Adt(definition, _) = trait_ref.self_ty().kind() else {
        return None;
    };
    if !definition.is_struct() {
        return None;
    }
    let definition = definition.did().as_local()?;
    let body = cx.tcx.hir_body(body_id);
    let forwarding = DirectForwarding::expression(cx, item.owner_id.def_id, signature.header, body)?;
    let [self_binding, formatter_binding] = forwarding.bindings.as_slice() else {
        return None;
    };
    if direct_trait_delegation(
        cx,
        forwarding.typeck_owner,
        forwarding.forwarded,
        *self_binding,
        *formatter_binding,
        trait_id,
    ) || single_field_write(cx, item)
    {
        Some((definition, trait_name))
    } else {
        None
    }
}

fn formatting_trait(name: &str) -> Option<&'static str> {
    match name {
        "Debug" => Some("Debug"),
        "Display" => Some("Display"),
        "Binary" => Some("Binary"),
        "Octal" => Some("Octal"),
        "LowerHex" => Some("LowerHex"),
        "UpperHex" => Some("UpperHex"),
        "LowerExp" => Some("LowerExp"),
        "UpperExp" => Some("UpperExp"),
        "Pointer" => Some("Pointer"),
        _ => None,
    }
}

fn direct_trait_delegation(
    cx: &LateContext<'_>,
    owner: LocalDefId,
    expression: &Expr<'_>,
    self_binding: rustc_hir::HirId,
    formatter_binding: rustc_hir::HirId,
    trait_id: rustc_hir::def_id::DefId,
) -> bool {
    let Some(call) = DirectForwarding::call(cx, owner, expression) else {
        return false;
    };
    let [value, formatter] = call.arguments.as_slice() else {
        return false;
    };
    cx.tcx.trait_of_assoc(call.target) == Some(trait_id)
        && field_reference(cx, value, self_binding)
        && DirectForwarding::is_binding(cx, formatter, formatter_binding)
}

fn field_reference(
    cx: &LateContext<'_>,
    expression: &Expr<'_>,
    binding: rustc_hir::HirId,
) -> bool {
    let expression = match expression.kind {
        ExprKind::AddrOf(_, Mutability::Not, inner) => inner,
        _ => expression,
    };
    let ExprKind::Field(base, _) = expression.kind else {
        return false;
    };
    DirectForwarding::is_binding(cx, base, binding)
}

fn single_field_write(cx: &LateContext<'_>, item: &ImplItem<'_>) -> bool {
    let Ok(source) = cx.sess().source_map().span_to_snippet(item.span) else {
        return false;
    };
    let Ok(method) = syn::parse_str::<syn::ImplItemFn>(&source) else {
        return false;
    };
    let [syn::Stmt::Expr(syn::Expr::Macro(invocation), _)] = method.block.stmts.as_slice() else {
        return false;
    };
    if !invocation.mac.path.is_ident("write") {
        return false;
    }
    let inputs = method.sig.inputs.iter().collect::<Vec<_>>();
    let [syn::FnArg::Receiver(_), syn::FnArg::Typed(formatter_parameter)] = inputs.as_slice()
    else {
        return false;
    };
    let syn::Pat::Ident(formatter_parameter) = formatter_parameter.pat.as_ref() else {
        return false;
    };
    let parser = syn::punctuated::Punctuated::<syn::Expr, syn::Token![,]>::parse_terminated;
    let Ok(arguments) = parser.parse2(invocation.mac.tokens.clone()) else {
        return false;
    };
    let arguments = arguments.iter().collect::<Vec<_>>();
    let [formatter, syn::Expr::Lit(format), field] = arguments.as_slice() else {
        return false;
    };
    let syn::Expr::Path(formatter) = formatter else {
        return false;
    };
    let syn::Lit::Str(format) = &format.lit else {
        return false;
    };
    let syn::Expr::Field(field) = field else {
        return false;
    };
    formatter.path.is_ident(&formatter_parameter.ident)
        && matches!(field.base.as_ref(), syn::Expr::Path(base) if base.path.is_ident("self"))
        && has_one_placeholder(&format.value())
}

fn has_one_placeholder(format: &str) -> bool {
    let mut placeholders = 0;
    let mut characters = format.chars().peekable();
    while let Some(character) = characters.next() {
        if character != '{' {
            continue;
        }
        if characters.peek() == Some(&'{') {
            characters.next();
            continue;
        }
        placeholders += 1;
    }
    placeholders == 1
}
