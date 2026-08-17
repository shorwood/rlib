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
        // Only direct paths expose a stable resolved callable name.
        let ExprKind::Path(path) = callee.kind else {
            return None;
        };
        let definition = cx.qpath_res(&path, callee.hir_id).opt_def_id()?;
        Some(cx.tcx.item_name(definition).as_str().to_owned())
    }

    /// Recognizes call names that conventionally expose text to a user.
    fn is_observed_name(name: &str) -> bool {
        let vocabulary = [
            "show", "display", "render", "response", "error", "message", "notify", "alert",
        ];
        name.trim_start_matches("r#")
            .split('_')
            .any(|component| vocabulary.contains(&component))
    }

    /// Returns whether the receiver enum contains documentation that this API can expose.
    fn receiver_has_documentation(cx: &LateContext<'_>, receiver: &Expr<'_>) -> bool {
        cx.typeck_results()
            .expr_ty(receiver)
            .peel_refs()
            .ty_adt_def()
            .is_some_and(|definition| {
                definition.variants().iter().any(|variant| {
                    variant.def_id.as_local().is_some_and(|definition| {
                        cx.tcx
                            .hir_attrs(cx.tcx.local_def_id_to_hir_id(definition))
                            .iter()
                            .any(|attribute| attribute.doc_str().is_some())
                    })
                })
            })
    }

    /// Finds the nearby user-visible call that consumes a generated enum message.
    fn observed_sink(cx: &LateContext<'_>, mut hir_id: rustc_hir::HirId) -> Option<String> {
        /// Maximum parent expressions inspected for an observed message sink.
        const MAXIMUM_OBSERVED_SINK_ANCESTORS: usize = 4;
        for _ in 0..MAXIMUM_OBSERVED_SINK_ANCESTORS {
            let parent = cx.tcx.parent_hir_node(hir_id);

            // Leaving expression ancestry means no nearby call consumes the message.
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
                // A consuming free call is the terminal sink for the tracked message value.
                ExprKind::Call(callee, arguments)
                    if arguments.iter().any(|argument| argument.hir_id == hir_id) =>
                {
                    let name = Self::callable_name(cx, callee)?;

                    // A consuming free call completes the bounded sink search.
                    return Self::is_observed_name(&name).then_some(name);
                }
                // A consuming method argument is the terminal sink for the tracked message value.
                ExprKind::MethodCall(segment, receiver, arguments, _)
                    if receiver.hir_id != hir_id
                        && arguments.iter().any(|argument| argument.hir_id == hir_id) =>
                {
                    let name = segment.ident.name.as_str().to_owned();

                    // A consuming method call completes the bounded sink search.
                    return Self::is_observed_name(&name).then_some(name);
                }
                // Other parent shapes break the direct message-to-sink flow.
                _ => return None,
            }
        }
        None
    }
}
impl LateLintPass<'_> for StrumDocumentationUsedAsEnumMessages {
    fn check_expr(&mut self, cx: &LateContext<'_>, expression: &Expr<'_>) {
        // Documentation retrieval must be expressed as a receiver method call.
        let ExprKind::MethodCall(segment, receiver, ..) = expression.kind else {
            return;
        };

        // Retain only authored documentation reads from documented enum variants.
        if expression.span.from_expansion()
            || segment.ident.name.as_str() != "get_documentation"
            || !Self::receiver_has_documentation(cx, receiver)
        {
            return;
        }

        // Unresolved methods cannot establish the Strum message contract.
        let Some(method) = cx.typeck_results().type_dependent_def_id(expression.hir_id) else {
            return;
        };

        let contract = cx
            .tcx
            .associated_item(method)
            .trait_item_def_id()
            .unwrap_or(method);

        // Exclude similarly named methods outside Strum's documentation interface.
        if !cx
            .tcx
            .def_path_str(contract)
            .contains("strum::EnumMessage::get_documentation")
        {
            return;
        }

        // Documentation that never reaches an observed sink remains developer-facing.
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
