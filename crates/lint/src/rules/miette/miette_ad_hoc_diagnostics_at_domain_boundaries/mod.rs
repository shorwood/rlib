extern crate rustc_errors;
extern crate rustc_hir;
extern crate rustc_middle;
extern crate rustc_span;

use std::borrow::Cow;
use std::collections::{BTreeMap, BTreeSet};
use std::mem;

use rustc_errors::DiagDecorator;
use rustc_hir::{ImplItem, ImplItemKind, Item, ItemKind};
use rustc_lint::{LateContext, LateLintPass, LintContext};
use rustc_middle::ty::{self, Ty};
use rustc_span::{Span, sym};
use syn::parse::Parser;
use syn::punctuated::Punctuated;
use syn::visit::{Visit, visit_expr_method_call};

use crate::utils::diagnostic::LateViolation;
use crate::utils::source_provenance::AuthoredItemSource;

/// Carries the `MessageUse` state used by this analysis.
#[derive(Clone)]
struct MessageUse {
    /// Stores the `span` value used by this analysis.
    span: Span,
    /// Stores the `function` value used by this analysis.
    function: String,
    /// Stores the `is_public` value used by this analysis.
    is_public: bool,
}

/// Carries the `Violation` state used by this analysis.
struct Violation {
    /// Stores the `message` value used by this analysis.
    message: String,
    /// Stores the `uses` value used by this analysis.
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

/// Performs the `is_code_assignment` step of the lint analysis.
fn is_code_assignment(expression: &syn::Expr) -> bool {
    matches!(
        expression,
        syn::Expr::Assign(assignment)
            if matches!(assignment.left.as_ref(), syn::Expr::Path(path) if path.path.is_ident("code"))
    )
}

/// Performs the `static_string` step of the lint analysis.
fn static_string(expression: &syn::Expr) -> Option<String> {
    // Prepare the values used by this stage.
    let syn::Expr::Lit(syn::ExprLit {
        lit: syn::Lit::Str(literal),
        ..
    }) = expression
    else {
        return None;
    };

    // Prepare the values used by this stage.
    let value = literal.value();
    (!value.contains('{') && !value.contains('}')).then_some(value)
}

#[derive(Default)]
/// Carries the `StaticMessageVisitor` state used by this analysis.
struct StaticMessageVisitor {
    /// Stores the `messages` value used by this analysis.
    messages: BTreeSet<String>,
}

impl<'ast> Visit<'ast> for StaticMessageVisitor {
    fn visit_macro(&mut self, invocation: &'ast syn::Macro) {
        // Prepare the values used by this stage.
        let Some(name) = invocation
            .path
            .segments
            .last()
            .map(|segment| &segment.ident)
        // Perform the next step of the analysis.
        else {
            return;
        };
        if name != "miette" && name != "diagnostic" {
            return;
        }
        let parser = Punctuated::<syn::Expr, syn::Token![,]>::parse_terminated;

        // Prepare the values used by this stage.
        let Ok(arguments) = parser.parse2(invocation.tokens.clone()) else {
            return;
        };
        if arguments.iter().any(is_code_assignment) {
            return;
        }

        // Prepare the values used by this stage.
        let literals = arguments
            .iter()
            .filter_map(static_string)
            .collect::<Vec<_>>();
        if literals.len() != 1 {
            return;
        }
        self.messages.insert(literals[0].clone());
    }

    fn visit_expr_method_call(&mut self, call: &'ast syn::ExprMethodCall) {
        if call.method == "wrap_err"
            && call.args.len() == 1
            && let Some(message) = call.args.first().and_then(static_string)
        {
            self.messages.insert(message);
        }
        visit_expr_method_call(self, call);
    }
}

/// Performs the `contains_miette_report` step of the lint analysis.
fn contains_miette_report(cx: &LateContext<'_>, ty: Ty<'_>) -> bool {
    let ty::Adt(definition, arguments) = ty.kind() else {
        return false;
    };
    if cx.tcx.crate_name(definition.did().krate).as_str() == "miette"
        && cx.tcx.item_name(definition.did()).as_str() == "Report"
    {
        return true;
    }
    cx.tcx.is_diagnostic_item(sym::Result, definition.did())
        && arguments.len() == 2
        && contains_miette_report(cx, arguments.type_at(1))
}

#[derive(Default)]
/// Carries the `MietteAdHocDiagnosticsAtDomainBoundaries` state used by this analysis.
struct MietteAdHocDiagnosticsAtDomainBoundaries {
    /// Stores the `messages` value used by this analysis.
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
        // Reject inputs that do not satisfy this stage.
        if item.span.from_expansion() || !matches!(item.kind, ItemKind::Fn { .. }) {
            return;
        }

        // Prepare the values used by this stage.
        let output = cx
            .tcx
            .fn_sig(item.owner_id.def_id)
            .instantiate_identity()
            .skip_binder()
            .output();

        // Reject inputs that do not satisfy this stage.
        if !contains_miette_report(cx, output) {
            return;
        }
        let Some(source) = AuthoredItemSource::for_item(cx, item) else {
            return;
        };

        // Prepare the values used by this stage.
        let Ok(function) = syn::parse_str::<syn::ItemFn>(&source) else {
            return;
        };
        let mut visitor = StaticMessageVisitor::default();
        visitor.visit_item_fn(&function);

        // Update the accumulated analysis state.
        self.record_messages(
            visitor.messages,
            &MessageUse {
                span: item.span,
                function: cx.tcx.item_name(item.owner_id.def_id).to_string(),
                is_public: cx.tcx.visibility(item.owner_id.def_id).is_public(),
            },
        );
    }

    fn check_impl_item(&mut self, cx: &LateContext<'_>, item: &ImplItem<'_>) {
        // Reject inputs that do not satisfy this stage.
        if item.span.from_expansion() || !matches!(item.kind, ImplItemKind::Fn(..)) {
            return;
        }

        // Prepare the values used by this stage.
        let output = cx
            .tcx
            .fn_sig(item.owner_id.def_id)
            .instantiate_identity()
            .skip_binder()
            .output();

        // Reject inputs that do not satisfy this stage.
        if !contains_miette_report(cx, output) {
            return;
        }
        let Ok(source) = cx.sess().source_map().span_to_snippet(item.span) else {
            return;
        };

        // Prepare the values used by this stage.
        let Ok(method) = syn::parse_str::<syn::ImplItemFn>(&source) else {
            return;
        };
        let mut visitor = StaticMessageVisitor::default();
        visitor.visit_impl_item_fn(&method);

        // Update the accumulated analysis state.
        self.record_messages(
            visitor.messages,
            &MessageUse {
                span: item.span,
                function: cx.tcx.def_path_str(item.owner_id.def_id),
                is_public: cx.tcx.visibility(item.owner_id.def_id).is_public(),
            },
        );
    }

    fn check_crate_post(&mut self, cx: &LateContext<'_>) {
        /// Smallest use count that proves a message is repeated.
        const MINIMUM_REPEATED_MESSAGE_USES: usize = 2;

        for (message, uses) in mem::take(&mut self.messages) {
            if !(uses.len() >= MINIMUM_REPEATED_MESSAGE_USES
                && uses.iter().any(|usage| usage.is_public))
            {
                continue;
            }
            Violation { message, uses }.emit(cx);
        }
    }
}

impl MietteAdHocDiagnosticsAtDomainBoundaries {
    /// Performs the `record_messages` operation for this value.
    fn record_messages(&mut self, messages: BTreeSet<String>, usage: &MessageUse) {
        for message in messages {
            self.messages
                .entry(message)
                .or_default()
                .push(usage.clone());
        }
    }
}
