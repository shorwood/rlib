extern crate rustc_abi;
extern crate rustc_errors;
extern crate rustc_hir;
extern crate rustc_middle;
extern crate rustc_span;

use std::borrow::Cow;

use rustc_abi::ExternAbi;
use rustc_errors::DiagDecorator;
use rustc_hir::def::DefKind;
use rustc_hir::{
    Constness, FnHeader, FnRetTy, HirId, ImplItem, ImplItemKind, Item, ItemKind, TraitItem,
    TraitItemKind,
};
use rustc_lint::{LateContext, LateLintPass, LintContext};
use rustc_middle::ty::{self, Ty};
use rustc_span::def_id::{DefId, LocalDefId};
use rustc_span::{Span, Symbol, sym};

use crate::utils::diagnostic::LateViolation;
use crate::utils::source_provenance::{ItemProvenanceExt, SpanProvenanceExt};

// -----------------------------------------------------------------------------
// CallableKind: Response boundary ownership
// -----------------------------------------------------------------------------

/// Authored API form that exposes one explicit Axum response contract.
#[derive(Clone, Copy)]
enum CallableKind {
    /// Module or function-local free function.
    Function,
    /// Associated function or method declared by an inherent implementation.
    InherentMethod,
    /// Method contract declared by a trait owned by the current crate.
    TraitMethod,
}

// -----------------------------------------------------------------------------
// Violation: Ad hoc response-producing API
// -----------------------------------------------------------------------------

/// Synchronous response helper whose rendering behavior lacks a type owner.
struct Violation {
    /// Callable node used to honor item-level lint attributes.
    hir_id: HirId,
    /// Authored callable identifier highlighted by the diagnostic.
    span: Span,
    /// Declared return type carrying the response contract.
    output_span: Span,
    /// Callable name shown in the primary message.
    name: Symbol,
    /// Callable form used to tailor replacement guidance.
    kind: CallableKind,
}

impl LateViolation for Violation {
    fn primary_message(&self) -> Cow<'_, str> {
        Cow::Owned(format!(
            "Axum response-producing callable `{}` is an ad hoc response wrapper",
            self.name
        ))
    }

    fn rationale_message(&self) -> Cow<'_, str> {
        Cow::Borrowed(
            "`IntoResponse` gives response rendering a type owner that handlers and generic Axum APIs can compose directly",
        )
    }

    fn remediation_message(&self) -> Cow<'_, str> {
        match self.kind {
            CallableKind::Function => Cow::Borrowed(
                "define a local wrapper containing the required inputs and implement `IntoResponse` for it",
            ),
            CallableKind::InherentMethod => Cow::Borrowed(
                "replace this method with an `IntoResponse` implementation and move any additional arguments into the wrapper state",
            ),
            CallableKind::TraitMethod => Cow::Borrowed(
                "replace this trait response method with `IntoResponse` implementations on the concrete response types",
            ),
        }
    }

    fn emit(self, cx: &LateContext<'_>) {
        cx.tcx.emit_node_span_lint(
            AXUM_AD_HOC_RESPONSE_WRAPPERS,
            self.hir_id,
            self.span,
            DiagDecorator(|diag| {
                diag.primary_message(self.primary_message().into_owned());
                diag.span_label(
                    self.output_span,
                    "this return contract exposes Axum response rendering",
                );
                diag.note(self.rationale_message().into_owned());
                diag.help(self.remediation_message().into_owned());
            }),
        );
    }
}

// -----------------------------------------------------------------------------
// ResponseContract: Explicit Axum response recognition
// -----------------------------------------------------------------------------

/// Semantic classification of explicitly authored Axum response contracts.
struct ResponseContract;

impl ResponseContract {
    /// Returns whether the root output is direct or a supported fallible response contract.
    fn is_output<'tcx>(cx: &LateContext<'tcx>, output: Ty<'tcx>) -> bool {
        Self::is_direct(cx, output) || Self::is_result_response_contract(cx, output)
    }

    /// Returns whether a direct output names Axum's response type or trait.
    fn is_direct<'tcx>(cx: &LateContext<'tcx>, output: Ty<'tcx>) -> bool {
        Self::is_axum_response(cx, output) || Self::is_opaque_into_response(cx, output)
    }

    /// Searches a top-level standard result through nested result and option branches.
    fn is_result_response_contract<'tcx>(cx: &LateContext<'tcx>, output: Ty<'tcx>) -> bool {
        // Only nominal result types can establish the fallible root this traversal requires.
        let ty::Adt(definition, arguments) = output.kind() else {
            return false;
        };
        cx.tcx.is_diagnostic_item(sym::Result, definition.did())
            && arguments
                .types()
                .any(|branch| Self::is_response_in_result_branch(cx, branch))
    }

    /// Recurses only through sum-shaped result and option branches beneath a result root.
    fn is_response_in_result_branch<'tcx>(cx: &LateContext<'tcx>, branch: Ty<'tcx>) -> bool {
        // A direct response completes the recursive search without inspecting unrelated bounds.
        if Self::is_direct(cx, branch) {
            return true;
        }

        // Scalars, tuples, aliases without response bounds, and other shapes end this branch.
        let ty::Adt(definition, arguments) = branch.kind() else {
            return false;
        };
        (cx.tcx.is_diagnostic_item(sym::Result, definition.did())
            || cx.tcx.is_diagnostic_item(sym::Option, definition.did()))
            && arguments
                .types()
                .any(|nested| Self::is_response_in_result_branch(cx, nested))
    }

    /// Recognizes Axum's default `http::Response<axum_core::body::Body>` alias target.
    fn is_axum_response<'tcx>(cx: &LateContext<'tcx>, output: Ty<'tcx>) -> bool {
        // Non-ADT outputs cannot be the concrete HTTP response definition.
        let ty::Adt(definition, arguments) = output.kind() else {
            return false;
        };
        Self::is_http_response(cx, definition.did())
            && arguments
                .types()
                .next()
                .is_some_and(|body| Self::is_axum_body(cx, body))
    }

    /// Recognizes an opaque return type explicitly bounded by Axum's `IntoResponse`.
    fn is_opaque_into_response<'tcx>(cx: &LateContext<'tcx>, output: Ty<'tcx>) -> bool {
        // Concrete types do not carry an authored opaque trait contract to inspect.
        let ty::Alias(alias) = output.kind() else {
            return false;
        };
        cx.tcx
            .explicit_item_bounds(alias.kind.def_id())
            .iter_instantiated_copied(cx.tcx, alias.args)
            .any(|(clause, _)| {
                matches!(
                    clause.kind().skip_binder(),
                    ty::ClauseKind::Trait(predicate)
                        if Self::is_into_response_trait(cx, predicate.trait_ref.def_id)
                )
            })
    }

    /// Recognizes the concrete HTTP response definition behind Axum's public alias.
    fn is_http_response(cx: &LateContext<'_>, definition: DefId) -> bool {
        cx.tcx.crate_name(definition.krate).as_str() == "http"
            && cx.tcx.item_name(definition).as_str() == "Response"
    }

    /// Recognizes Axum's default erased response body.
    fn is_axum_body<'tcx>(cx: &LateContext<'tcx>, body: Ty<'tcx>) -> bool {
        matches!(body.kind(), ty::Adt(definition, _) if {
            let definition = definition.did();
            cx.tcx.crate_name(definition.krate).as_str() == "axum_core"
                && cx.tcx.item_name(definition).as_str() == "Body"
        })
    }

    /// Recognizes the exact Axum trait that owns response conversion.
    fn is_into_response_trait(cx: &LateContext<'_>, definition: DefId) -> bool {
        cx.tcx.crate_name(definition.krate).as_str() == "axum_core"
            && cx.tcx.item_name(definition).as_str() == "IntoResponse"
    }
}

// -----------------------------------------------------------------------------
// CallableDeclaration: Shared callable classification
// -----------------------------------------------------------------------------

/// Resolved inputs shared by free, inherent, and local trait callables.
struct CallableDeclaration {
    /// Declaration node used for item-level lint attributes.
    hir_id: HirId,
    /// Local definition whose resolved signature is classified.
    definition: LocalDefId,
    /// Authored callable name.
    name: Symbol,
    /// Callable identifier highlighted by the diagnostic.
    name_span: Span,
    /// Declared output type labeled by the diagnostic.
    output_span: Span,
    /// Complete declaration span used for source provenance.
    item_span: Span,
    /// Authored function semantics that must survive a trait migration.
    header: FnHeader,
    /// Callable form used for remediation guidance.
    kind: CallableKind,
    /// Whether framework glue owns the apparent declaration.
    is_framework_generated: bool,
}

impl CallableDeclaration {
    /// Returns whether this is ordinary synchronous safe Rust callable syntax.
    fn is_ordinary(&self) -> bool {
        self.header.abi == ExternAbi::Rust
            && !self.header.is_async()
            && !self.header.is_unsafe()
            && self.header.constness != Constness::Const
    }
}

// -----------------------------------------------------------------------------
// AxumAdHocResponseWrappers: Typed response ownership policy
// -----------------------------------------------------------------------------

/// Requires explicit synchronous Axum response contracts to be owned by responder types.
struct AxumAdHocResponseWrappers;

crate::impl_late_lint! {
    #[doc = include_str!("README.md")]
    pub AXUM_AD_HOC_RESPONSE_WRAPPERS,
    Warn,
    "requires synchronous Axum response wrappers to implement IntoResponse",
    AxumAdHocResponseWrappers
}

impl AxumAdHocResponseWrappers {
    /// Classifies one owned callable contract and emits its response-wrapper diagnostic.
    fn check_callable(cx: &LateContext<'_>, callable: &CallableDeclaration) {
        let source_map = cx.sess().source_map();

        // Nonordinary and generated declarations cannot migrate to an ordinary authored impl.
        if !callable.is_ordinary()
            || callable.item_span.in_external_macro(source_map)
            || callable.item_span.is_build_generated(cx)
            || callable.is_framework_generated
        {
            return;
        }
        let output = cx
            .tcx
            .fn_sig(callable.definition)
            .instantiate_identity()
            .skip_binder()
            .output();

        // Outputs outside the explicit Axum contract require no response ownership diagnostic.
        if !ResponseContract::is_output(cx, output) {
            return;
        }
        Violation {
            hir_id: callable.hir_id,
            span: callable.name_span,
            output_span: callable.output_span,
            name: callable.name,
            kind: callable.kind,
        }
        .emit(cx);
    }

    /// Extracts the explicit output span from one non-default return declaration.
    const fn output_span(output: FnRetTy<'_>) -> Option<Span> {
        match output {
            FnRetTy::Return(output) => Some(output.span),
            FnRetTy::DefaultReturn(_) => None,
        }
    }
}

impl LateLintPass<'_> for AxumAdHocResponseWrappers {
    fn check_item(&mut self, cx: &LateContext<'_>, item: &Item<'_>) {
        // Items other than body-bearing free functions expose no owned callable contract here.
        let ItemKind::Fn { sig, has_body, .. } = item.kind else {
            return;
        };

        // Implicit unit outputs cannot carry an explicit Axum response contract.
        let Some(output_span) = Self::output_span(sig.decl.output) else {
            return;
        };

        // Bodyless synthetic declarations offer no authored free-function implementation to move.
        if !has_body {
            return;
        }

        // Preserve the authored declaration data before applying the shared response policy.
        let callable = CallableDeclaration {
            hir_id: item.hir_id(),
            definition: item.owner_id.def_id,
            name: cx.tcx.item_name(item.owner_id.to_def_id()),
            name_span: cx
                .tcx
                .def_ident_span(item.owner_id.to_def_id())
                .unwrap_or(item.span),
            output_span,
            item_span: item.span,
            header: sig.header,
            kind: CallableKind::Function,
            is_framework_generated: item.is_framework_generated(),
        };
        Self::check_callable(cx, &callable);
    }

    fn check_impl_item(&mut self, cx: &LateContext<'_>, item: &ImplItem<'_>) {
        // Associated constants and types expose no response-producing callable contract.
        let ImplItemKind::Fn(signature, _) = item.kind else {
            return;
        };
        let parent = cx.tcx.parent(item.owner_id.to_def_id());

        // Trait implementations inherit the contract checked once at a local trait declaration.
        if cx.tcx.def_kind(parent) == (DefKind::Impl { of_trait: true }) {
            return;
        }

        // Implicit unit outputs cannot carry an explicit Axum response contract.
        let Some(output_span) = Self::output_span(signature.decl.output) else {
            return;
        };

        // Preserve the authored declaration data before applying the shared response policy.
        let callable = CallableDeclaration {
            hir_id: item.hir_id(),
            definition: item.owner_id.def_id,
            name: item.ident.name,
            name_span: item.ident.span,
            output_span,
            item_span: item.span,
            header: signature.header,
            kind: CallableKind::InherentMethod,
            is_framework_generated: false,
        };
        Self::check_callable(cx, &callable);
    }

    fn check_trait_item(&mut self, cx: &LateContext<'_>, item: &TraitItem<'_>) {
        // Associated constants and types expose no locally owned trait method contract.
        let TraitItemKind::Fn(signature, _) = item.kind else {
            return;
        };

        // Implicit unit outputs cannot carry an explicit Axum response contract.
        let Some(output_span) = Self::output_span(signature.decl.output) else {
            return;
        };

        // Preserve the authored declaration data before applying the shared response policy.
        let callable = CallableDeclaration {
            hir_id: item.hir_id(),
            definition: item.owner_id.def_id,
            name: item.ident.name,
            name_span: item.ident.span,
            output_span,
            item_span: item.span,
            header: signature.header,
            kind: CallableKind::TraitMethod,
            is_framework_generated: false,
        };
        Self::check_callable(cx, &callable);
    }
}
