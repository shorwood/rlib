extern crate rustc_errors;
extern crate rustc_hir;
extern crate rustc_span;

use std::borrow::Cow;

use rustc_errors::DiagDecorator;
use rustc_hir::{Expr, ExprKind, Node};
use rustc_lint::{LateContext, LateLintPass};
use rustc_span::Span;

use crate::utils::diagnostic::LateViolation;

/// Carries the `Violation` state used by this analysis.
struct Violation {
    /// Stores the `owner` value used by this analysis.
    owner: rustc_hir::HirId,
    /// Stores the `span` value used by this analysis.
    span: Span,
    /// Stores the `sink` value used by this analysis.
    sink: String,
}

impl LateViolation for Violation {
    fn primary_message(&self) -> Cow<'_, str> {
        Cow::Borrowed("enum documentation is used as externally observed message text")
    }

    fn rationale_message(&self) -> Cow<'_, str> {
        Cow::Owned(format!(
            "developer documentation flows directly into `{}`",
            self.sink
        ))
    }

    fn remediation_message(&self) -> Cow<'_, str> {
        Cow::Borrowed(
            "use explicit Strum message metadata or a dedicated presentation/localization key",
        )
    }

    fn emit(self, cx: &LateContext<'_>) {
        cx.tcx.emit_node_span_lint(
            STRUM_DOCUMENTATION_USED_AS_ENUM_MESSAGES,
            self.owner,
            self.span,
            DiagDecorator(|diag| {
                diag.primary_message(self.primary_message().into_owned());
                diag.note(self.rationale_message().into_owned());
                diag.help(self.remediation_message().into_owned());
            }),
        );
    }
}

/// Performs the `callable_name` step of the lint analysis.
fn callable_name(cx: &LateContext<'_>, callee: &Expr<'_>) -> Option<String> {
    let ExprKind::Path(path) = callee.kind else {
        return None;
    };
    let definition = cx.qpath_res(&path, callee.hir_id).opt_def_id()?;
    Some(cx.tcx.item_name(definition).as_str().to_owned())
}

/// Performs the `is_observed_name` step of the lint analysis.
fn is_observed_name(name: &str) -> bool {
    [
        "show", "display", "render", "response", "error", "message", "notify", "alert",
    ]
    .into_iter()
    .any(|token| name.contains(token))
}

/// Performs the `observed_sink` step of the lint analysis.
fn observed_sink(cx: &LateContext<'_>, mut hir_id: rustc_hir::HirId) -> Option<String> {
    /// Maximum parent expressions inspected for an observed message sink.
    const MAXIMUM_OBSERVED_SINK_ANCESTORS: usize = 4;
    for _ in 0..MAXIMUM_OBSERVED_SINK_ANCESTORS {
        // Prepare the values used by this stage.
        let parent = cx.tcx.parent_hir_node(hir_id);
        let Node::Expr(expression) = parent else {
            return None;
        };

        // Classify the current analyze_candidate.
        match expression.kind {
            ExprKind::MethodCall(segment, ..)
                if matches!(
                    segment.ident.name.as_str(),
                    "unwrap" | "expect" | "to_owned" | "to_string"
                ) =>
            {
                hir_id = expression.hir_id;
            }
            ExprKind::Call(callee, arguments)
                if arguments.iter().any(|argument| argument.hir_id == hir_id) =>
            {
                let name = callable_name(cx, callee)?;
                return is_observed_name(&name).then_some(name);
            }
            ExprKind::MethodCall(segment, receiver, arguments, _)
                if receiver.hir_id != hir_id
                    && arguments.iter().any(|argument| argument.hir_id == hir_id) =>
            {
                let name = segment.ident.name.as_str().to_owned();
                return is_observed_name(&name).then_some(name);
            }
            _ => return None,
        }
    }
    None
}

/// Carries the `StrumDocumentationUsedAsEnumMessages` state used by this analysis.
struct StrumDocumentationUsedAsEnumMessages;

dylint_linting::impl_late_lint! {
    #[doc = include_str!("README.md")]
    pub STRUM_DOCUMENTATION_USED_AS_ENUM_MESSAGES,
    Warn,
    "finds Strum enum documentation used as message text",
    StrumDocumentationUsedAsEnumMessages
}

impl LateLintPass<'_> for StrumDocumentationUsedAsEnumMessages {
    fn check_expr(&mut self, cx: &LateContext<'_>, expression: &Expr<'_>) {
        // Prepare the values used by this stage.
        let ExprKind::MethodCall(segment, ..) = expression.kind else {
            return;
        };
        if segment.ident.name.as_str() != "get_documentation" {
            return;
        }

        // Prepare the values used by this stage.
        let Some(method) = cx.typeck_results().type_dependent_def_id(expression.hir_id) else {
            return;
        };

        // Prepare the values used by this stage.
        let contract = cx
            .tcx
            .associated_item(method)
            .trait_item_def_id()
            .unwrap_or(method);

        // Reject inputs that do not satisfy this stage.
        if !cx
            .tcx
            .def_path_str(contract)
            .contains("strum::EnumMessage::get_documentation")
        {
            return;
        }

        // Prepare the values used by this stage.
        let Some(sink) = observed_sink(cx, expression.hir_id) else {
            return;
        };

        // Perform the next step of the analysis.
        Violation {
            owner: expression.hir_id,
            span: expression.span.source_callsite(),
            sink,
        }
        .emit(cx);
    }
}
