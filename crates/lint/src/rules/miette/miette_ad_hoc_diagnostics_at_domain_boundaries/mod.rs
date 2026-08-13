extern crate rustc_errors;
extern crate rustc_hir;
extern crate rustc_middle;
extern crate rustc_span;

use std::borrow::Cow;
use std::collections::{BTreeMap, BTreeSet};

use rustc_errors::DiagDecorator;
use rustc_hir::{ImplItem, ImplItemKind, Item, ItemKind};
use rustc_lint::{LateContext, LateLintPass, LintContext};
use rustc_middle::ty::{self, Ty};
use rustc_span::Span;
use syn::parse::Parser;
use syn::punctuated::Punctuated;
use syn::visit::Visit;

use crate::utils::diagnostic::LateViolation;
use crate::utils::source_provenance::authored_item_source;

struct MessageUse {
    span: Span,
    function: String,
    public: bool,
}

struct Violation {
    message: String,
    uses: Vec<MessageUse>,
}

impl LateViolation for Violation {
    fn primary_message(&self) -> Cow<'_, str> {
        Cow::Owned(format!(
            "ad-hoc Miette diagnostic `{}` is repeated at domain boundaries",
            self.message
        ))
    }

    fn rationale_message(&self) -> Cow<'_, str> {
        Cow::Owned(format!(
            "the same anonymous failure is propagated by {}",
            self.uses
                .iter()
                .map(|usage| format!("`{}`", usage.function))
                .collect::<Vec<_>>()
                .join(", ")
        ))
    }

    fn remediation_message(&self) -> Cow<'_, str> {
        Cow::Borrowed(
            "introduce one typed error deriving `thiserror::Error` and `miette::Diagnostic`, then reuse it at each boundary",
        )
    }

    fn emit(self, cx: &LateContext<'_>) {
        let primary = self.uses[0].span;
        cx.emit_span_lint(
            MIETTE_AD_HOC_DIAGNOSTICS_AT_DOMAIN_BOUNDARIES,
            primary,
            DiagDecorator(|diag| {
                diag.primary_message(self.primary_message().into_owned());
                for usage in &self.uses {
                    diag.span_label(
                        usage.span,
                        format!("`{}` constructs this anonymous failure", usage.function),
                    );
                }
                diag.note(self.rationale_message().into_owned());
                diag.help(self.remediation_message().into_owned());
            }),
        );
    }
}

#[derive(Default)]
struct MietteAdHocDiagnosticsAtDomainBoundaries {
    messages: BTreeMap<String, Vec<MessageUse>>,
}

dylint_linting::impl_late_lint! {
    #[doc = include_str!("README.md")]
    pub MIETTE_AD_HOC_DIAGNOSTICS_AT_DOMAIN_BOUNDARIES,
    Warn,
    "finds repeated ad-hoc Miette diagnostics crossing domain boundaries",
    MietteAdHocDiagnosticsAtDomainBoundaries::default()
}

impl LateLintPass<'_> for MietteAdHocDiagnosticsAtDomainBoundaries {
    fn check_item(&mut self, cx: &LateContext<'_>, item: &Item<'_>) {
        if item.span.from_expansion() || !matches!(item.kind, ItemKind::Fn { .. }) {
            return;
        }
        let output = cx
            .tcx
            .fn_sig(item.owner_id.def_id)
            .instantiate_identity()
            .skip_binder()
            .output();
        if !contains_miette_report(cx, output) {
            return;
        }
        let Some(source) = authored_item_source(cx, item) else {
            return;
        };
        let Ok(function) = syn::parse_str::<syn::ItemFn>(&source) else {
            return;
        };
        let mut visitor = StaticMessageVisitor::default();
        visitor.visit_item_fn(&function);
        self.record_messages(
            visitor.messages,
            item.span,
            cx.tcx.item_name(item.owner_id.def_id).to_string(),
            cx.tcx.visibility(item.owner_id.def_id).is_public(),
        );
    }

    fn check_impl_item(&mut self, cx: &LateContext<'_>, item: &ImplItem<'_>) {
        if item.span.from_expansion() || !matches!(item.kind, ImplItemKind::Fn(..)) {
            return;
        }
        let output = cx
            .tcx
            .fn_sig(item.owner_id.def_id)
            .instantiate_identity()
            .skip_binder()
            .output();
        if !contains_miette_report(cx, output) {
            return;
        }
        let Ok(source) = cx.sess().source_map().span_to_snippet(item.span) else {
            return;
        };
        let Ok(method) = syn::parse_str::<syn::ImplItemFn>(&source) else {
            return;
        };
        let mut visitor = StaticMessageVisitor::default();
        visitor.visit_impl_item_fn(&method);
        self.record_messages(
            visitor.messages,
            item.span,
            cx.tcx.def_path_str(item.owner_id.def_id),
            cx.tcx.visibility(item.owner_id.def_id).is_public(),
        );
    }

    fn check_crate_post(&mut self, cx: &LateContext<'_>) {
        for (message, uses) in std::mem::take(&mut self.messages) {
            if uses.len() >= 2 && uses.iter().any(|usage| usage.public) {
                Violation { message, uses }.emit(cx);
            }
        }
    }
}

impl MietteAdHocDiagnosticsAtDomainBoundaries {
    fn record_messages(
        &mut self,
        messages: BTreeSet<String>,
        span: Span,
        function: String,
        public: bool,
    ) {
        for message in messages {
            self.messages.entry(message).or_default().push(MessageUse {
                span,
                function: function.clone(),
                public,
            });
        }
    }
}

#[derive(Default)]
struct StaticMessageVisitor {
    messages: BTreeSet<String>,
}

impl<'ast> Visit<'ast> for StaticMessageVisitor {
    fn visit_macro(&mut self, invocation: &'ast syn::Macro) {
        let Some(name) = invocation
            .path
            .segments
            .last()
            .map(|segment| &segment.ident)
        else {
            return;
        };
        if name != "miette" && name != "diagnostic" {
            return;
        }
        let parser = Punctuated::<syn::Expr, syn::Token![,]>::parse_terminated;
        let Ok(arguments) = parser.parse2(invocation.tokens.clone()) else {
            return;
        };
        if arguments.iter().any(is_code_assignment) {
            return;
        }
        let literals = arguments
            .iter()
            .filter_map(static_string)
            .collect::<Vec<_>>();
        if literals.len() == 1 {
            self.messages.insert(literals[0].clone());
        }
    }

    fn visit_expr_method_call(&mut self, call: &'ast syn::ExprMethodCall) {
        if call.method == "wrap_err"
            && call.args.len() == 1
            && let Some(message) = call.args.first().and_then(static_string)
        {
            self.messages.insert(message);
        }
        syn::visit::visit_expr_method_call(self, call);
    }
}

fn is_code_assignment(expression: &syn::Expr) -> bool {
    matches!(
        expression,
        syn::Expr::Assign(assignment)
            if matches!(assignment.left.as_ref(), syn::Expr::Path(path) if path.path.is_ident("code"))
    )
}

fn static_string(expression: &syn::Expr) -> Option<String> {
    let syn::Expr::Lit(syn::ExprLit {
        lit: syn::Lit::Str(literal),
        ..
    }) = expression
    else {
        return None;
    };
    let value = literal.value();
    (!value.contains('{') && !value.contains('}')).then_some(value)
}

fn contains_miette_report(cx: &LateContext<'_>, ty: Ty<'_>) -> bool {
    let ty::Adt(definition, arguments) = ty.kind() else {
        return false;
    };
    if cx.tcx.crate_name(definition.did().krate).as_str() == "miette"
        && cx.tcx.item_name(definition.did()).as_str() == "Report"
    {
        return true;
    }
    cx.tcx
        .is_diagnostic_item(rustc_span::sym::Result, definition.did())
        && arguments.len() == 2
        && contains_miette_report(cx, arguments.type_at(1))
}
