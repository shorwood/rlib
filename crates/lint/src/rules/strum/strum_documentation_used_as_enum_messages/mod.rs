extern crate rustc_errors;
extern crate rustc_hir;
extern crate rustc_span;

use std::borrow::Cow;

use rustc_errors::DiagDecorator;
use rustc_hir::{Expr, ExprKind, Node};
use rustc_lint::{LateContext, LateLintPass};
use rustc_span::Span;

use crate::utils::diagnostic::LateViolation;

// -----------------------------------------------------------------------------
// Violation: Documentation reused as a runtime message
// -----------------------------------------------------------------------------

/// Enum message contract coupled to authored API documentation.
struct Violation {
    /// Declaration whose lint level governs this finding.
    owner: rustc_hir::HirId,
    /// Authored declaration or expression range used as the diagnostic anchor.
    span: Span,
    /// User-visible consumer reached by the documentation-derived message.
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

// -----------------------------------------------------------------------------
// StrumDocumentationUsedAsEnumMessages: Separate message policy
// -----------------------------------------------------------------------------

/// Detects Strum message derivation from documentation comments.
struct StrumDocumentationUsedAsEnumMessages;

dylint_linting::impl_late_lint! {
    #[doc = include_str!("README.md")]
    pub STRUM_DOCUMENTATION_USED_AS_ENUM_MESSAGES,
    Warn,
    "finds Strum enum documentation used as message text",
    StrumDocumentationUsedAsEnumMessages
}

impl StrumDocumentationUsedAsEnumMessages {
    /// Resolves the authored name of a directly called function or method.
    fn callable_name(cx: &LateContext<'_>, callee: &Expr<'_>) -> Option<String> {
        let ExprKind::Path(path) = callee.kind else {
            return None;
        };
        let definition = cx.qpath_res(&path, callee.hir_id).opt_def_id()?;
        Some(cx.tcx.item_name(definition).as_str().to_owned())
    }

    /// Recognizes call names that conventionally expose text to a user.
    fn is_observed_name(name: &str) -> bool {
        [
            "show", "display", "render", "response", "error", "message", "notify", "alert",
        ]
        .into_iter()
        .any(|token| name.contains(token))
    }

    /// Finds the nearby user-visible call that consumes a generated enum message.
    fn observed_sink(cx: &LateContext<'_>, mut hir_id: rustc_hir::HirId) -> Option<String> {
        /// Maximum parent expressions inspected for an observed message sink.
        const MAXIMUM_OBSERVED_SINK_ANCESTORS: usize = 4;
        for _ in 0..MAXIMUM_OBSERVED_SINK_ANCESTORS {
            let parent = cx.tcx.parent_hir_node(hir_id);
            let Node::Expr(expression) = parent else {
                return None;
            };

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
                    let name = Self::callable_name(cx, callee)?;
                    return Self::is_observed_name(&name).then_some(name);
                }
                ExprKind::MethodCall(segment, receiver, arguments, _)
                    if receiver.hir_id != hir_id
                        && arguments.iter().any(|argument| argument.hir_id == hir_id) =>
                {
                    let name = segment.ident.name.as_str().to_owned();
                    return Self::is_observed_name(&name).then_some(name);
                }
                _ => return None,
            }
        }
        None
    }
}
impl LateLintPass<'_> for StrumDocumentationUsedAsEnumMessages {
    fn check_expr(&mut self, cx: &LateContext<'_>, expression: &Expr<'_>) {
        let ExprKind::MethodCall(segment, ..) = expression.kind else {
            return;
        };
        if segment.ident.name.as_str() != "get_documentation" {
            return;
        }

        let Some(method) = cx.typeck_results().type_dependent_def_id(expression.hir_id) else {
            return;
        };

        let contract = cx
            .tcx
            .associated_item(method)
            .trait_item_def_id()
            .unwrap_or(method);

        if !cx
            .tcx
            .def_path_str(contract)
            .contains("strum::EnumMessage::get_documentation")
        {
            return;
        }

        let Some(sink) = Self::observed_sink(cx, expression.hir_id) else {
            return;
        };

        Violation {
            owner: expression.hir_id,
            span: expression.span.source_callsite(),
            sink,
        }
        .emit(cx);
    }
}
