extern crate rustc_hir;
extern crate rustc_lint;
extern crate rustc_middle;
extern crate rustc_span;

use rustc_hir::def::{DefKind, Res};
use rustc_hir::{Expr, ExprKind};
use rustc_lint::LateContext;
use rustc_middle::ty::{self, Ty};
use rustc_span::symbol::sym;

// -----------------------------------------------------------------------------
// ResultContract: Resolved success and failure types
// -----------------------------------------------------------------------------

/// Exact standard `Result<Success, Error>` contract carried by an expression.
#[derive(Clone, Copy)]
pub struct ResultContract<'tcx> {
    /// Value preserved by the successful branch.
    success: Ty<'tcx>,
    /// Failure information erased by the operation under review.
    error: Ty<'tcx>,
}

impl<'tcx> ResultContract<'tcx> {
    /// Resolves an expression whose concrete type is the standard `Result`.
    pub fn from_expression(cx: &LateContext<'tcx>, expression: &Expr<'_>) -> Option<Self> {
        Self::from_type(cx, cx.typeck_results().expr_ty(expression))
    }

    /// Resolves a standard `Result` type and its two semantic arguments.
    fn from_type(cx: &LateContext<'tcx>, expression_type: Ty<'tcx>) -> Option<Self> {
        // Require the standard result definition before reading its arguments.
        let ty::Adt(definition, arguments) = expression_type.peel_refs().kind() else {
            return None;
        };
        if !cx.tcx.is_diagnostic_item(sym::Result, definition.did()) {
            return None;
        }

        // Recover the success and error identities in their declared order.
        let mut types = arguments.types();
        Some(Self {
            success: types.next()?,
            error: types.next()?,
        })
    }

    /// Formats the successful value type for remediation guidance.
    pub fn success_name(self) -> String {
        self.success.to_string()
    }

    /// Formats the erased error type for diagnostic context.
    pub fn error_name(self) -> String {
        self.error.to_string()
    }
}

// -----------------------------------------------------------------------------
// ResultOperation: Standard failure consuming operation
// -----------------------------------------------------------------------------

/// Supported inherent operation on standard Result.
#[derive(Clone, Copy)]
pub enum ResultOperation {
    /// Converts success to `Some` and failure to `None`.
    Ok,
    /// Returns success or an eagerly supplied fallback.
    UnwrapOr,
    /// Returns success or invokes a fallback with the error.
    UnwrapOrElse,
    /// Returns success or the success type's default value.
    UnwrapOrDefault,
}

impl ResultOperation {
    /// Returns the standard associated item name used for resolution.
    const fn name(self) -> &'static str {
        match self {
            Self::Ok => "ok",
            Self::UnwrapOr => "unwrap_or",
            Self::UnwrapOrElse => "unwrap_or_else",
            Self::UnwrapOrDefault => "unwrap_or_default",
        }
    }
}

// -----------------------------------------------------------------------------
// ResultCallParts: Resolved call syntax
// -----------------------------------------------------------------------------

/// Syntax-independent components of one candidate result operation.
struct ResultCallParts<'hir> {
    /// Associated operation selected by method resolution or UFCS.
    definition: rustc_hir::def_id::DefId,
    /// First semantic argument acting as the result receiver.
    receiver: &'hir Expr<'hir>,
    /// Remaining explicit arguments in semantic order.
    arguments: &'hir [Expr<'hir>],
}

// -----------------------------------------------------------------------------
// ResolvedResultCall: Standard result method invocation
// -----------------------------------------------------------------------------

/// Method or UFCS call resolved to one inherent operation on standard `Result`.
pub struct ResolvedResultCall<'hir, 'tcx> {
    /// Result receiver whose failure branch is consumed by the operation.
    receiver: &'hir Expr<'hir>,
    /// Explicit arguments after the receiver in semantic call order.
    arguments: &'hir [Expr<'hir>],
    /// Resolved success and failure types of the receiver.
    contract: ResultContract<'tcx>,
}

impl<'hir, 'tcx> ResolvedResultCall<'hir, 'tcx> {
    /// Returns the Result receiver used as diagnostic evidence.
    pub const fn receiver(&self) -> &'hir Expr<'hir> {
        self.receiver
    }

    /// Returns explicit arguments excluding the semantic receiver.
    pub const fn arguments(&self) -> &'hir [Expr<'hir>] {
        self.arguments
    }

    /// Returns the receiver's resolved success and failure types.
    pub const fn contract(&self) -> ResultContract<'tcx> {
        self.contract
    }
}

// -----------------------------------------------------------------------------
// ResultLossAnalyzer: Failure erasure recognition
// -----------------------------------------------------------------------------

/// Context-bound semantic analysis for standard result failure-erasure operations.
pub struct ResultLossAnalyzer<'analysis, 'tcx> {
    /// Compiler context used for type and definition resolution.
    cx: &'analysis LateContext<'tcx>,
}

impl<'analysis, 'tcx> ResultLossAnalyzer<'analysis, 'tcx> {
    /// Binds result-loss analysis to the active compiler context.
    pub const fn for_context(cx: &'analysis LateContext<'tcx>) -> Self {
        Self { cx }
    }

    /// Resolves an expression whose concrete type is standard Result.
    pub fn contract(&self, expression: &Expr<'_>) -> Option<ResultContract<'tcx>> {
        ResultContract::from_expression(self.cx, expression)
    }

    /// Normalizes one type-dependent method call into semantic call parts.
    fn method_call_parts<'hir>(
        &self,
        expression: &'hir Expr<'hir>,
        operation: ResultOperation,
    ) -> Option<ResultCallParts<'hir>> {
        // Require method syntax selecting the expected operation name.
        let ExprKind::MethodCall(segment, receiver, arguments, _) = expression.kind else {
            return None;
        };
        if segment.ident.name.as_str() != operation.name() {
            return None;
        }

        // Resolve the compiler-selected item for this type-dependent call.
        let definition = self
            .cx
            .typeck_results()
            .type_dependent_def_id(expression.hir_id)?;

        // Preserve the selected item and semantic call arguments.
        Some(ResultCallParts {
            definition,
            receiver,
            arguments,
        })
    }

    /// Resolves a direct call expression to the invoked definition.
    fn resolved_path_definition(&self, expression: &Expr<'_>) -> Option<rustc_hir::def_id::DefId> {
        let ExprKind::Path(path) = expression.kind else {
            return None;
        };
        let Res::Def(DefKind::Fn | DefKind::AssocFn, definition) =
            self.cx.qpath_res(&path, expression.hir_id)
        else {
            return None;
        };
        Some(definition)
    }

    /// Returns whether an expression directly calls standard `Default::default`.
    pub fn is_default_value(&self, expression: &Expr<'_>) -> bool {
        // Resolve a direct zero-argument associated function call.
        let ExprKind::Call(callee, []) = expression.peel_blocks().kind else {
            return false;
        };
        let Some(definition) = self.resolved_path_definition(callee) else {
            return false;
        };

        // Require the selected item to be the standard default trait operation.
        let trait_definition = self.cx.tcx.parent(definition);
        self.cx.tcx.item_name(definition).as_str() == "default"
            && self
                .cx
                .tcx
                .is_diagnostic_item(sym::Default, trait_definition)
    }

    /// Returns whether a closure ignores its error and directly yields a default value.
    pub fn is_defaulting_closure(&self, expression: &Expr<'_>) -> bool {
        let ExprKind::Closure(closure) = expression.kind else {
            return false;
        };
        let body = self.cx.tcx.hir_body(closure.body);
        self.is_default_value(body.value)
    }

    /// Recognizes an explicit call to standard `drop` with a Result argument.
    pub fn dropped_result(&self, expression: &Expr<'_>) -> Option<ResultContract<'tcx>> {
        let ExprKind::Call(callee, [argument]) = expression.kind else {
            return None;
        };
        let definition = self.resolved_path_definition(callee)?;
        if !self.cx.tcx.is_diagnostic_item(sym::mem_drop, definition) {
            return None;
        }
        self.contract(argument)
    }

    /// Normalizes one direct UFCS call into semantic call parts.
    fn ufcs_call_parts<'hir>(&self, expression: &'hir Expr<'hir>) -> Option<ResultCallParts<'hir>> {
        // Separate the direct callee from its semantic receiver and arguments.
        let ExprKind::Call(callee, arguments) = expression.kind else {
            return None;
        };
        let (receiver, arguments) = arguments.split_first()?;

        // Preserve the compiler-selected item and semantic call arguments.
        Some(ResultCallParts {
            definition: self.resolved_path_definition(callee)?,
            receiver,
            arguments,
        })
    }

    /// Returns whether an associated item belongs to standard result's inherent impl.
    fn is_result_inherent_method(&self, definition: rustc_hir::def_id::DefId) -> bool {
        let Some(implementation) = self.cx.tcx.impl_of_assoc(definition) else {
            return false;
        };
        let self_type = self.cx.tcx.type_of(implementation).instantiate_identity();
        let ty::Adt(result, _) = self_type.kind() else {
            return false;
        };
        self.cx.tcx.is_diagnostic_item(sym::Result, result.did())
    }

    /// Recognizes one inherent Result operation in method or UFCS syntax.
    pub fn call<'hir>(
        &self,
        expression: &'hir Expr<'hir>,
        operation: ResultOperation,
    ) -> Option<ResolvedResultCall<'hir, 'tcx>> {
        // Normalize method and UFCS syntax into one semantic call shape.
        let parts = match expression.kind {
            ExprKind::MethodCall(..) => self.method_call_parts(expression, operation)?,
            ExprKind::Call(..) => self.ufcs_call_parts(expression)?,
            _ => return None,
        };

        // Require the selected item to belong to result's inherent implementation.
        if self.cx.tcx.item_name(parts.definition).as_str() != operation.name()
            || !self.is_result_inherent_method(parts.definition)
        {
            return None;
        }

        // Preserve the resolved receiver contract and explicit arguments.
        Some(ResolvedResultCall {
            receiver: parts.receiver,
            arguments: parts.arguments,
            contract: self.contract(parts.receiver)?,
        })
    }
}
