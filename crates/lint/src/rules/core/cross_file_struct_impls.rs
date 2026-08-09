extern crate rustc_errors;
extern crate rustc_hir;
extern crate rustc_span;

use rustc_errors::DiagDecorator;
use rustc_hir::{Item, ItemKind};
use rustc_lint::{LateContext, LateLintPass, LintContext};

use crate::utils::impl_target::ImplTargetExt;

// -----------------------------------------------------------------------------
// ImplPlacement: Cross file placement model
// -----------------------------------------------------------------------------

/// One direct struct impl whose physical source can be compared with its struct definition.
struct ImplPlacement<'hir> {
    /// Authored direct inherent implementation being checked.
    implementation: &'hir Item<'hir>,
    /// Implemented struct name shown in diagnostics.
    struct_name: rustc_span::Symbol,
    /// Struct definition span used to compare physical files.
    struct_span: rustc_span::Span,
}

impl<'hir> ImplPlacement<'hir> {
    /// Recognizes an authored direct impl whose local struct can be resolved.
    fn discover(cx: &LateContext<'hir>, item: &'hir Item<'hir>) -> Option<Self> {
        // Require an authored direct implementation item.
        if !matches!(item.kind, ItemKind::Impl(_))
            || item.span.in_external_macro(cx.sess().source_map())
        {
            return None;
        }

        // Resolve an authored local struct definition from the implementation.
        let struct_def_id = item.direct_struct(cx)?;
        let struct_span = cx.tcx.def_span(struct_def_id);
        if struct_span.in_external_macro(cx.sess().source_map()) {
            return None;
        }

        // Retain the source facts needed to compare implementation placement.
        Some(Self {
            implementation: item,
            struct_name: cx.tcx.item_name(struct_def_id.to_def_id()),
            struct_span,
        })
    }

    /// Returns whether implementation and struct definition share a physical file.
    fn is_colocated(&self, cx: &LateContext<'_>) -> bool {
        let source_map = cx.sess().source_map();
        source_map.span_to_filename(self.implementation.span)
            == source_map.span_to_filename(self.struct_span)
    }

    /// Emits a diagnostic connecting both physical source locations.
    fn emit(&self, cx: &LateContext<'_>) {
        // Resolve compact filenames for both ends of the misplaced relationship.
        let source_map = cx.sess().source_map();
        let implementation_file = source_map.span_to_filename(self.implementation.span);
        let struct_file = source_map.span_to_filename(self.struct_span);
        let implementation_file = implementation_file.short();
        let struct_file = struct_file.short();

        // Connect the implementation and definition in one actionable diagnostic.
        cx.tcx.emit_node_span_lint(
            CROSS_FILE_STRUCT_IMPLS,
            self.implementation.hir_id(),
            self.implementation.span,
            DiagDecorator(|diag| {
                diag.primary_message(format!(
                    "impl block for `{}` is not colocated with its definition",
                    self.struct_name
                ));
                diag.span_label(
                    self.implementation.span,
                    format!("this impl is defined in `{implementation_file}`"),
                );
                diag.span_label(
                    self.struct_span,
                    format!("`{}` is defined in `{struct_file}`", self.struct_name),
                );
                diag.help(format!(
                    "move this complete impl block into `{struct_file}` beside `{}`",
                    self.struct_name
                ));
            }),
        );
    }
}

// -----------------------------------------------------------------------------
// CrossFileStructImpls: Lint pass
// -----------------------------------------------------------------------------

/// Late lint pass that keeps direct impls in their struct's physical file.
struct CrossFileStructImpls;

dylint_linting::impl_late_lint! {
    /// ### What it does
    ///
    /// Checks that every direct impl block for a local struct is defined in the same physical file
    /// as the struct.
    ///
    /// ### Why is this bad?
    ///
    /// Keeping a struct and all of its behavior in one file makes the type understandable without
    /// searching through unrelated modules. The definition-order lint separately ensures that impl
    /// blocks in the struct's module immediately follow its definition.
    ///
    /// For example, this layout separates a type from its behavior:
    ///
    /// ```rust,ignore
    /// // model.rs
    /// pub struct User;
    ///
    /// // service.rs
    /// impl User {
    ///     pub fn name(&self) -> &str { "Ada" }
    /// }
    /// ```
    ///
    /// Keeping both declarations in the owning file makes the type self-contained:
    ///
    /// ```rust
    /// pub struct User;
    ///
    /// impl User {
    ///     pub fn name(&self) -> &str { "Ada" }
    /// }
    /// ```
    pub CROSS_FILE_STRUCT_IMPLS,
    Warn,
    "enforces struct definitions and their impl blocks are in the same file",
    CrossFileStructImpls
}

impl<'tcx> LateLintPass<'tcx> for CrossFileStructImpls {
    fn check_item(&mut self, cx: &LateContext<'tcx>, item: &'tcx Item<'tcx>) {
        let Some(placement) = ImplPlacement::discover(cx, item) else {
            return;
        };
        if placement.is_colocated(cx) {
            return;
        }
        placement.emit(cx);
    }
}
