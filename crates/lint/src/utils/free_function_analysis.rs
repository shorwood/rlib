extern crate rustc_abi;
extern crate rustc_hir;
extern crate rustc_lint;

use rustc_abi::ExternAbi;
use rustc_hir::def::DefKind;
use rustc_hir::{Item, ItemKind};
use rustc_lint::{LateContext, LintContext};

use super::source_provenance::ItemProvenanceExt;

// -----------------------------------------------------------------------------
// FreeFunctionExt: Shared free function eligibility
// -----------------------------------------------------------------------------

/// Shared semantic classification for HIR items inspected by ownership-oriented lints.
pub trait FreeFunctionExt {
    /// Returns whether this is an authored, top-level free function using Rust's ordinary ABI.
    ///
    /// Generated framework glue, external macro output, nested declarations, associated
    /// functions, and foreign integration entry points stay outside every adopting analysis.
    fn is_authored_rust_free_function(&self, cx: &LateContext<'_>) -> bool;

    /// Returns whether another module can name this authored free function.
    fn is_visible_outside_module(&self, cx: &LateContext<'_>) -> bool;

    /// Returns whether this function exports a fixed integration symbol rather than an API.
    fn has_external_symbol(&self, cx: &LateContext<'_>) -> bool;
}

impl FreeFunctionExt for Item<'_> {
    fn is_authored_rust_free_function(&self, cx: &LateContext<'_>) -> bool {
        // Require an ordinary function body before consulting ownership semantics.
        let ItemKind::Fn { sig, has_body, .. } = self.kind else {
            return false;
        };
        if !has_body {
            return false;
        }

        // Confirm that the function belongs directly to an authored module.
        let def_id = self.owner_id.def_id;
        let is_top_level = cx
            .tcx
            .opt_local_parent(def_id)
            .is_some_and(|parent| cx.tcx.def_kind(parent) == DefKind::Mod);

        // Exclude non-Rust integration boundaries and generated source ownership.
        is_top_level
            && sig.header.abi == ExternAbi::Rust
            && !self.span.in_external_macro(cx.sess().source_map())
            && !self.is_framework_generated()
    }

    fn is_visible_outside_module(&self, cx: &LateContext<'_>) -> bool {
        if self.vis_span.is_empty() {
            return false;
        }
        let source_map = cx.sess().source_map();
        source_map
            .span_to_snippet(self.vis_span)
            .is_ok_and(|visibility| visibility.trim() != "pub(self)")
    }

    fn has_external_symbol(&self, cx: &LateContext<'_>) -> bool {
        cx.tcx
            .codegen_fn_attrs(self.owner_id.def_id)
            .contains_extern_indicator()
    }
}
