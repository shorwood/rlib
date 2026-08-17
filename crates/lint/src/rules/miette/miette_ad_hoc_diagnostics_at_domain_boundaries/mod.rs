extern crate rustc_ast;
extern crate rustc_errors;
extern crate rustc_hir;
extern crate rustc_middle;
extern crate rustc_span;

use std::borrow::Cow;
use std::collections::{BTreeMap, BTreeSet};
use std::mem;

use rustc_errors::DiagDecorator;
use rustc_hir::intravisit::{self, Visitor as HirVisitor};
use rustc_hir::{BodyId, Expr, ExprKind, ImplItem, ImplItemKind, Item, ItemKind};
use rustc_lint::{LateContext, LateLintPass, LintContext};
use rustc_middle::ty::{self, Ty};
use rustc_span::{Span, sym};
use syn::parse::Parser;
use syn::punctuated::Punctuated;
use syn::visit::Visit;

use crate::utils::diagnostic::LateViolation;
use crate::utils::source_provenance::AuthoredItemSource;

/// Smallest use count that proves a message is repeated.
const MINIMUM_REPEATED_MESSAGE_USES: usize = 2;

// -----------------------------------------------------------------------------
// Violation: Repeated anonymous diagnostic at a domain boundary
// -----------------------------------------------------------------------------

/// One function constructing a static anonymous diagnostic message.
#[derive(Clone)]
struct ViolationUse {
    /// Function declaration containing the construction.
    span: Span,
    /// Function name shown to the author.
    function: String,
    /// Whether this use crosses a public domain boundary.
    is_public: bool,
}

/// Repeated static failure text that has acquired a shared domain meaning.
struct Violation {
    /// Anonymous message repeated across functions.
    message: String,
    /// Construction sites sharing that message.
    uses: Vec<ViolationUse>,
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

// -----------------------------------------------------------------------------
// SemanticContextVisitor: Resolved context-message discovery
// -----------------------------------------------------------------------------

/// Finds literal context messages on resolved Miette `WrapErr` methods.
struct SemanticContextVisitor<'analysis, 'tcx> {
    /// Compiler context used for method identity and nested closure bodies.
    cx: &'analysis LateContext<'tcx>,
    /// Distinct static context messages in the function.
    messages: BTreeSet<String>,
}

impl<'analysis, 'tcx> SemanticContextVisitor<'analysis, 'tcx> {
    /// Starts semantic context collection.
    const fn new(cx: &'analysis LateContext<'tcx>) -> Self {
        Self {
            cx,
            messages: BTreeSet::new(),
        }
    }

    /// Extracts a literal string directly or from a lazy context closure.
    fn literal_message(&self, expression: &'tcx Expr<'tcx>) -> Option<String> {
        let expression = match expression.kind {
            ExprKind::Closure(closure) => self.cx.tcx.hir_body(closure.body).value,
            _ => expression,
        };

        // Non-literal context expressions do not define a stable shared message.
        let ExprKind::Lit(literal) = expression.kind else {
            return None;
        };

        // Only string literals can name an anonymous diagnostic consistently.
        let rustc_ast::LitKind::Str(message, _) = literal.node else {
            return None;
        };
        Some(message.to_string())
    }
}

impl<'tcx> HirVisitor<'tcx> for SemanticContextVisitor<'_, 'tcx> {
    fn visit_nested_body(&mut self, body: BodyId) {
        self.visit_body(self.cx.tcx.hir_body(body));
    }

    fn visit_expr(&mut self, expression: &'tcx Expr<'tcx>) {
        if let ExprKind::MethodCall(_, _, [argument], _) = expression.kind {
            let owner = self.cx.tcx.hir_enclosing_body_owner(expression.hir_id);
            if let Some(method) = self
                .cx
                .tcx
                .typeck(owner)
                .type_dependent_def_id(expression.hir_id)
                && self.cx.tcx.crate_name(method.krate).as_str() == "miette"
                && matches!(
                    self.cx.tcx.item_name(method).as_str(),
                    "wrap_err" | "wrap_err_with"
                )
                && self
                    .cx
                    .tcx
                    .trait_of_assoc(method)
                    .is_some_and(|trait_id| self.cx.tcx.item_name(trait_id).as_str() == "WrapErr")
                && let Some(message) = self.literal_message(argument)
            {
                self.messages.insert(message);
            }
        }
        intravisit::walk_expr(self, expression);
    }
}

// -----------------------------------------------------------------------------
// MietteAdHocDiagnosticsAtDomainBoundaries: Typed domain failure policy
// -----------------------------------------------------------------------------

/// Correlates repeated anonymous messages across report-returning functions.
#[derive(Default)]
struct MietteAdHocDiagnosticsAtDomainBoundaries {
    /// Construction sites grouped by static message text.
    messages: BTreeMap<String, Vec<ViolationUse>>,
}

dylint_linting::impl_late_lint! {
    #[doc = include_str!("README.md")]
    pub MIETTE_AD_HOC_DIAGNOSTICS_AT_DOMAIN_BOUNDARIES,
    Warn,
    "finds repeated ad-hoc Miette diagnostics crossing domain boundaries",
    MietteAdHocDiagnosticsAtDomainBoundaries::default()
}

impl MietteAdHocDiagnosticsAtDomainBoundaries {
    /// Returns whether a macro argument gives the diagnostic a stable code.
    fn is_code_assignment(expression: &syn::Expr) -> bool {
        matches!(
            expression,
            syn::Expr::Assign(assignment)
                if matches!(assignment.left.as_ref(), syn::Expr::Path(path) if path.path.is_ident("code"))
        )
    }

    /// Extracts literal text with no formatting placeholders.
    fn static_string(expression: &syn::Expr) -> Option<String> {
        // Non-string expressions cannot provide a static diagnostic message.
        let syn::Expr::Lit(syn::ExprLit {
            lit: syn::Lit::Str(literal),
            ..
        }) = expression
        else {
            return None;
        };

        Self::static_format_text(&literal.value())
    }

    /// Resolves escaped braces while rejecting actual format replacement fields.
    fn static_format_text(value: &str) -> Option<String> {
        let mut characters = value.chars().peekable();
        let mut rendered = String::with_capacity(value.len());
        while let Some(character) = characters.next() {
            if matches!(character, '{' | '}') {
                characters.next_if_eq(&character)?;
            }
            rendered.push(character);
        }
        Some(rendered)
    }

    /// Finds a Miette report directly or in a result error position.
    fn contains_miette_report(cx: &LateContext<'_>, ty: Ty<'_>) -> bool {
        // Non-ADT types cannot be a report or a result containing one.
        let ty::Adt(definition, arguments) = ty.kind() else {
            return false;
        };

        // A direct report type completes the boundary classification.
        if cx.tcx.crate_name(definition.did().krate).as_str() == "miette"
            && cx.tcx.item_name(definition.did()).as_str() == "Report"
        {
            return true;
        }
        cx.tcx.is_diagnostic_item(sym::Result, definition.did())
            && arguments.len() == 2
            && Self::contains_miette_report(cx, arguments.type_at(1))
    }
}

impl LateLintPass<'_> for MietteAdHocDiagnosticsAtDomainBoundaries {
    fn check_item(&mut self, cx: &LateContext<'_>, item: &Item<'_>) {
        // Only authored free functions can establish this kind of domain boundary.
        if item.span.from_expansion() || !matches!(item.kind, ItemKind::Fn { .. }) {
            return;
        }

        let output = cx
            .tcx
            .fn_sig(item.owner_id.def_id)
            .instantiate_identity()
            .skip_binder()
            .output();

        // Functions not returning a report cannot propagate an anonymous Miette diagnostic.
        if !Self::contains_miette_report(cx, output) {
            return;
        }

        // Functions without recoverable authored source cannot be inspected for diagnostic macros.
        let Some(source) = AuthoredItemSource::for_item(cx, item) else {
            return;
        };

        // Unparseable source cannot establish authored static diagnostic messages.
        let Ok(function) = syn::parse_str::<syn::ItemFn>(&source) else {
            return;
        };
        let mut visitor = StaticMessageVisitor::default();
        visitor.visit_item_fn(&function);
        if let ItemKind::Fn { body, .. } = item.kind {
            let mut contexts = SemanticContextVisitor::new(cx);
            contexts.visit_body(cx.tcx.hir_body(body));
            visitor.messages.extend(contexts.messages);
        }

        self.record_messages(
            visitor.messages,
            &ViolationUse {
                span: item.span,
                function: cx.tcx.item_name(item.owner_id.def_id).to_string(),
                is_public: cx.tcx.visibility(item.owner_id.def_id).is_public(),
            },
        );
    }

    fn check_impl_item(&mut self, cx: &LateContext<'_>, item: &ImplItem<'_>) {
        // Only authored implementation methods can establish this kind of domain boundary.
        if item.span.from_expansion() || !matches!(item.kind, ImplItemKind::Fn(..)) {
            return;
        }

        let output = cx
            .tcx
            .fn_sig(item.owner_id.def_id)
            .instantiate_identity()
            .skip_binder()
            .output();

        // Methods not returning a report cannot propagate an anonymous Miette diagnostic.
        if !Self::contains_miette_report(cx, output) {
            return;
        }

        // Methods without recoverable source cannot be inspected for diagnostic macros.
        let Ok(source) = cx.sess().source_map().span_to_snippet(item.span) else {
            return;
        };

        // Unparseable source cannot establish authored static diagnostic messages.
        let Ok(method) = syn::parse_str::<syn::ImplItemFn>(&source) else {
            return;
        };
        let mut visitor = StaticMessageVisitor::default();
        visitor.visit_impl_item_fn(&method);
        if let ImplItemKind::Fn(_, body) = item.kind {
            let mut contexts = SemanticContextVisitor::new(cx);
            contexts.visit_body(cx.tcx.hir_body(body));
            visitor.messages.extend(contexts.messages);
        }

        self.record_messages(
            visitor.messages,
            &ViolationUse {
                span: item.span,
                function: cx.tcx.def_path_str(item.owner_id.def_id),
                is_public: cx.tcx.visibility(item.owner_id.def_id).is_public(),
            },
        );
    }

    fn check_crate_post(&mut self, cx: &LateContext<'_>) {
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
    /// Associates every message found in a function with that boundary.
    fn record_messages(&mut self, messages: BTreeSet<String>, usage: &ViolationUse) {
        for message in messages {
            self.messages
                .entry(message)
                .or_default()
                .push(usage.clone());
        }
    }
}

/// Finds uncoded literal messages in Miette macros and wrapping calls.
#[derive(Default)]
struct StaticMessageVisitor {
    /// Distinct static messages found in one function.
    messages: BTreeSet<String>,
}

impl<'ast> Visit<'ast> for StaticMessageVisitor {
    fn visit_macro(&mut self, invocation: &'ast syn::Macro) {
        // Macros without a terminal name cannot match a Miette constructor.
        let Some(name) = invocation
            .path
            .segments
            .last()
            .map(|segment| &segment.ident)
        else {
            return;
        };

        // Other macros do not create the anonymous diagnostics governed here.
        if name != "miette" && name != "diagnostic" {
            return;
        }
        let parser = Punctuated::<syn::Expr, syn::Token![,]>::parse_terminated;

        // Unparseable arguments cannot prove a static anonymous diagnostic contract.
        let Ok(arguments) = parser.parse2(invocation.tokens.clone()) else {
            return;
        };

        // A stable diagnostic code already gives this failure a reusable identity.
        if arguments
            .iter()
            .any(MietteAdHocDiagnosticsAtDomainBoundaries::is_code_assignment)
        {
            return;
        }

        let literals = arguments
            .iter()
            .filter_map(MietteAdHocDiagnosticsAtDomainBoundaries::static_string)
            .collect::<Vec<_>>();

        // Exactly one static literal is required to identify the anonymous message.
        if literals.len() != 1 {
            return;
        }
        self.messages.insert(literals[0].clone());
    }
}
