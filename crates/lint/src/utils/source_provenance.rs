extern crate rustc_hir;
extern crate rustc_lint;
extern crate rustc_span;

use std::env;

use rustc_hir::{FieldDef, Item};
use rustc_lint::{LateContext, LintContext};
use rustc_span::Span;

/// Returns whether a span belongs to build-script output rather than authored package source.
pub fn is_build_generated(cx: &LateContext<'_>, span: Span) -> bool {
    let Some(output_directory) = env::var_os("OUT_DIR") else {
        return false;
    };
    let filename = cx.sess().source_map().span_to_filename(span);
    filename
        .into_local_path()
        .is_some_and(|path| path.starts_with(output_directory))
}

/// Returns whether an item is synthetic framework glue represented at an authored call site.
pub fn is_framework_generated_item(item: &Item<'_>) -> bool {
    item.kind
        .ident()
        .is_some_and(|identifier| identifier.name.as_str().starts_with("__component_"))
}

/// Returns whether an apparent field is proc-macro glue mapped onto an authored function.
///
/// Attribute macros such as Leptos components synthesize props structs while mapping each field
/// back to the complete component function. Such fields have no independently editable source
/// declaration even though their call-site spans no longer report an expansion context.
pub fn is_framework_generated_field(cx: &LateContext<'_>, field: &FieldDef<'_>) -> bool {
    let parent = cx.tcx.parent(field.def_id.to_def_id());
    if cx.tcx.def_kind(parent) != rustc_hir::def::DefKind::Struct {
        return false;
    }
    let parent_name = cx.tcx.item_name(parent);
    if !parent_name.as_str().ends_with("Props") {
        return false;
    }
    cx.sess()
        .source_map()
        .span_to_snippet(field.span)
        .is_ok_and(|source| source.contains("fn "))
}
