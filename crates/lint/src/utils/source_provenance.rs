extern crate rustc_hir;
extern crate rustc_lint;
extern crate rustc_span;

use std::env;

use rustc_hir::{FieldDef, Item};
use rustc_lint::{LateContext, LintContext};
use rustc_span::Span;

// -----------------------------------------------------------------------------
// SpanProvenanceExt: Build generated span classification
// -----------------------------------------------------------------------------

/// Build-output provenance available directly on compiler spans.
pub trait SpanProvenanceExt {
    /// Returns whether this span belongs to build-script output rather than package source.
    fn is_build_generated(&self, cx: &LateContext<'_>) -> bool;
}

impl SpanProvenanceExt for Span {
    fn is_build_generated(&self, cx: &LateContext<'_>) -> bool {
        let Some(output_directory) = env::var_os("OUT_DIR") else {
            return false;
        };
        let filename = cx.sess().source_map().span_to_filename(*self);
        filename
            .into_local_path()
            .is_some_and(|path| path.starts_with(output_directory))
    }
}

// -----------------------------------------------------------------------------
// ItemProvenanceExt: Framework generated item classification
// -----------------------------------------------------------------------------

/// Framework provenance available directly on HIR items.
pub trait ItemProvenanceExt {
    /// Returns whether this item is synthetic framework glue at an authored call site.
    fn is_framework_generated(&self) -> bool;
}

impl ItemProvenanceExt for Item<'_> {
    fn is_framework_generated(&self) -> bool {
        self.kind
            .ident()
            .is_some_and(|identifier| identifier.name.as_str().starts_with("__component_"))
    }
}

// -----------------------------------------------------------------------------
// FieldProvenanceExt: Framework generated field classification
// -----------------------------------------------------------------------------

/// Framework provenance available directly on apparent HIR fields.
pub trait FieldProvenanceExt {
    /// Returns whether this field is proc-macro glue mapped onto an authored function.
    ///
    /// Attribute macros such as Leptos components synthesize props structs while mapping each
    /// field back to the complete component function. Such fields have no independently editable
    /// declaration even when their call-site spans no longer report an expansion context.
    fn is_framework_generated(&self, cx: &LateContext<'_>) -> bool;
}

impl FieldProvenanceExt for FieldDef<'_> {
    fn is_framework_generated(&self, cx: &LateContext<'_>) -> bool {
        // Require the conventional generated props container before inspecting mapped source.
        let parent = cx.tcx.parent(self.def_id.to_def_id());
        if cx.tcx.def_kind(parent) != rustc_hir::def::DefKind::Struct {
            return false;
        }
        let parent_name = cx.tcx.item_name(parent);
        if !parent_name.as_str().ends_with("Props") {
            return false;
        }

        // A complete function mapped onto one field proves that no field syntax is editable.
        let source_map = cx.sess().source_map();
        source_map
            .span_to_snippet(self.span)
            .is_ok_and(|source| source.contains("fn "))
    }
}
