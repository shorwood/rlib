extern crate rustc_abi;
extern crate rustc_errors;
extern crate rustc_hir;
extern crate rustc_middle;
extern crate rustc_span;

use std::borrow::Cow;

use rustc_abi::ExternAbi;
use rustc_errors::DiagDecorator;
use rustc_hir::def::DefKind;
use rustc_hir::{HirId, ImplItem, ImplItemKind, Item, ItemKind, TraitItem, TraitItemKind};
use rustc_lint::{LateContext, LateLintPass, LintContext};
use rustc_middle::ty::{self, Ty};
use rustc_span::def_id::LocalDefId;
use rustc_span::{Span, Symbol, sym};

use crate::config::core::BooleanPredicateConfig;
use crate::config::store::ConfigStore;
use crate::utils::diagnostic::LateViolation;
use crate::utils::source_provenance::{ItemProvenanceExt, SpanProvenanceExt};

// -----------------------------------------------------------------------------
// Violation: Boolean-result callable naming diagnostic
// -----------------------------------------------------------------------------

/// Boolean-result callable whose name does not read as a predicate.
struct Violation {
    /// Callable declaration used for item-level lint attributes.
    hir_id: HirId,
    /// Authored callable identifier highlighted by the diagnostic.
    span: Span,
    /// Callable name used in diagnostic guidance.
    name: Symbol,
    /// Configuration-backed forms accepted by the callable policy.
    expectation: String,
}

impl LateViolation for Violation {
    fn primary_message(&self) -> Cow<'_, str> {
        Cow::Owned(format!(
            "boolean-result function or method `{}` should use {}",
            self.name, self.expectation
        ))
    }

    fn rationale_message(&self) -> Cow<'_, str> {
        Cow::Owned(format!(
            "`{}` otherwise reads like a value or operation instead of a yes-or-no query",
            self.name
        ))
    }

    fn remediation_message(&self) -> Cow<'_, str> {
        Cow::Borrowed(
            "choose a predicate-style name that describes the condition returned on the success path",
        )
    }

    fn emit(self, cx: &LateContext<'_>) {
        cx.tcx.emit_node_span_lint(
            BOOL_RETURNING_FUNCTIONS_WITHOUT_PREDICATE_PREFIX,
            self.hir_id,
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
// BoolReturningFunctionsWithoutPredicatePrefix: Callable query naming policy
// -----------------------------------------------------------------------------

/// Resolved declaration inputs shared by free, inherent, and trait callables.
#[derive(Clone, Copy)]
struct CallableDeclaration {
    /// Declaration node used for item-level lint attributes.
    hir_id: HirId,
    /// Local definition whose resolved signature is classified.
    definition: LocalDefId,
    /// Authored callable name.
    name: Symbol,
    /// Identifier span highlighted by the diagnostic.
    name_span: Span,
    /// Complete declaration span used for generated-source classification.
    item_span: Span,
    /// Authored ABI, which distinguishes ordinary Rust declarations from FFI.
    abi: ExternAbi,
    /// Whether a framework expansion owns this declaration.
    is_framework_generated: bool,
}

/// Late lint pass that requires boolean-result callable names to read as predicates.
struct BoolReturningFunctionsWithoutPredicatePrefix {
    /// Shared property prefixes and callable-only standard query roots.
    config: BooleanPredicateConfig,
}

impl BoolReturningFunctionsWithoutPredicatePrefix {
    /// Builds the pass from validated project configuration.
    fn new() -> Self {
        Self {
            config: ConfigStore::get().boolean_predicates.clone(),
        }
    }

    /// Returns whether a resolved type reaches bool through standard success containers.
    fn is_boolean_result(cx: &LateContext<'_>, output: Ty<'_>) -> bool {
        // Direct boolean outputs complete the recursive classification.
        if output.is_bool() {
            return true;
        }

        // Other scalar and user-defined outputs are outside the structural policy.
        let ty::Adt(definition, arguments) = output.kind() else {
            return false;
        };
        (cx.tcx.is_diagnostic_item(sym::Option, definition.did())
            || cx.tcx.is_diagnostic_item(sym::Result, definition.did()))
            && Self::is_boolean_result(cx, arguments.type_at(0))
    }

    /// Resolves the authored output hidden by async function lowering.
    fn output<'tcx>(cx: &LateContext<'tcx>, definition: LocalDefId) -> Option<Ty<'tcx>> {
        let output = cx
            .tcx
            .fn_sig(definition)
            .instantiate_identity()
            .skip_binder()
            .output();

        // Synchronous signatures expose their authored return type directly.
        if !cx.tcx.asyncness(definition).is_async() {
            return Some(output);
        }

        // Async functions expose an opaque or associated future in their resolved signature.
        // Its explicit Future::Output projection retains the fully resolved authored type.
        let ty::Alias(alias) = output.kind() else {
            return None;
        };
        let future_output = cx.tcx.lang_items().future_output()?;
        cx.tcx
            .explicit_item_bounds(alias.kind.def_id())
            .iter_instantiated_copied(cx.tcx, alias.args)
            .find_map(|(clause, _)| match clause.kind().skip_binder() {
                ty::ClauseKind::Projection(projection)
                    if projection.projection_term.def_id == future_output =>
                {
                    projection.term.as_type()
                }
                _ => None,
            })
    }

    /// Returns whether a name is ordinary authored snake-case Rust syntax.
    fn is_snake_name(name: &str) -> bool {
        !name.is_empty()
            && name
                .bytes()
                .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'_')
            && name.as_bytes().first().is_some_and(u8::is_ascii_lowercase)
    }

    /// Checks one structurally eligible callable declaration.
    fn check_callable(&self, cx: &LateContext<'_>, declaration: CallableDeclaration) {
        let CallableDeclaration {
            hir_id,
            definition,
            name,
            name_span,
            item_span,
            abi,
            is_framework_generated,
        } = declaration;
        let source_map = cx.sess().source_map();
        let text = name.as_str();

        // Generated, foreign, and noncanonical declarations do not express authored API policy.
        if abi != ExternAbi::Rust
            || name_span.in_external_macro(source_map)
            || item_span.is_build_generated(cx)
            || is_framework_generated
            || text.starts_with("__component_")
            || text.starts_with("__orig_")
            || !Self::is_snake_name(text)
        {
            return;
        }

        // Unsupported lowered signatures cannot be classified without guessing semantics.
        let Some(output) = Self::output(cx, definition) else {
            return;
        };

        // Accepted names and nonboolean success paths require no diagnostic.
        if !Self::is_boolean_result(cx, output) || self.config.is_callable_name(text) {
            return;
        }
        Violation {
            hir_id,
            span: name_span,
            name,
            expectation: self.config.callable_expectation(),
        }
        .emit(cx);
    }
}

crate::impl_late_lint! {
    #[doc = include_str!("README.md")]
    pub BOOL_RETURNING_FUNCTIONS_WITHOUT_PREDICATE_PREFIX,
    Warn,
    "enforces predicate names for functions and methods with boolean results",
    BoolReturningFunctionsWithoutPredicatePrefix::new()
}

impl LateLintPass<'_> for BoolReturningFunctionsWithoutPredicatePrefix {
    fn check_item(&mut self, cx: &LateContext<'_>, item: &Item<'_>) {
        // Only authored free-function declarations participate in callable naming.
        let ItemKind::Fn { sig, .. } = item.kind else {
            return;
        };

        // Classify the resolved declaration using its authored syntax and provenance.
        self.check_callable(
            cx,
            CallableDeclaration {
                hir_id: item.hir_id(),
                definition: item.owner_id.def_id,
                name: cx.tcx.item_name(item.owner_id.to_def_id()),
                name_span: cx
                    .tcx
                    .def_ident_span(item.owner_id.to_def_id())
                    .unwrap_or(item.span),
                item_span: item.span,
                abi: sig.header.abi,
                is_framework_generated: item.is_framework_generated(),
            },
        );
    }

    fn check_impl_item(&mut self, cx: &LateContext<'_>, item: &ImplItem<'_>) {
        // Associated constants and types do not have callable return contracts.
        let ImplItemKind::Fn(signature, _) = item.kind else {
            return;
        };
        let parent = cx.tcx.parent(item.owner_id.to_def_id());

        // Trait implementations inherit the name checked once at the local trait contract.
        if cx.tcx.def_kind(parent) == (DefKind::Impl { of_trait: true }) {
            return;
        }
        self.check_callable(
            cx,
            CallableDeclaration {
                hir_id: item.hir_id(),
                definition: item.owner_id.def_id,
                name: item.ident.name,
                name_span: item.ident.span,
                item_span: item.span,
                abi: signature.header.abi,
                is_framework_generated: false,
            },
        );
    }

    fn check_trait_item(&mut self, cx: &LateContext<'_>, item: &TraitItem<'_>) {
        // Associated constants and types do not have callable return contracts.
        let TraitItemKind::Fn(signature, _) = item.kind else {
            return;
        };
        self.check_callable(
            cx,
            CallableDeclaration {
                hir_id: item.hir_id(),
                definition: item.owner_id.def_id,
                name: item.ident.name,
                name_span: item.ident.span,
                item_span: item.span,
                abi: signature.header.abi,
                is_framework_generated: false,
            },
        );
    }
}
