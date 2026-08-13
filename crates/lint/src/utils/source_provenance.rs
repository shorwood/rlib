extern crate rustc_hir;
extern crate rustc_lint;
extern crate rustc_span;

use std::env;

use rustc_hir::def::DefKind;
use rustc_hir::{FieldDef, Item};
use rustc_lint::{LateContext, LintContext};
use rustc_span::Span;

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

/// Source text for one item, including every compiler-recorded outer attribute.
#[derive(derive_more::Deref)]
pub struct AuthoredItemSource(
    /// Complete parseable source for the item.
    String,
);

impl AuthoredItemSource {
    /// Returns an authored item together with its immediately preceding outer attributes.
    pub(crate) fn for_item(cx: &LateContext<'_>, item: &Item<'_>) -> Option<Self> {
        // Prepare the values used by this stage.
        let source_map = cx.tcx.sess.source_map();
        let item_source = match source_map.span_to_snippet(item.span) {
            Ok(source) => source,
            Err(_error) => return None,
        };
        let location = source_map.lookup_char_pos(item.span.lo());
        let file_source = location.file.src.as_deref()?;
        let offset = match usize::try_from(item.span.lo().0.checked_sub(location.file.start_pos.0)?)
        {
            Ok(offset) => offset,
            Err(_error) => return None,
        };
        let bytes = file_source.as_bytes();
        let mut start = offset;

        // Include adjacent attributes and doc comments without querying spans of parsed attributes.
        loop {
            while start > 0 && bytes[start - 1].is_ascii_whitespace() {
                start -= 1;
            }
            if start == 0 {
                break;
            }

            if bytes[start - 1] == b']' {
                let mut cursor = start - 1;
                let mut depth = 1_u32;
                while cursor > 0 && depth > 0 {
                    cursor -= 1;
                    match bytes[cursor] {
                        b']' => depth += 1,
                        b'[' => depth -= 1,
                        _ => {}
                    }
                }
                if depth == 0 && cursor > 0 && bytes[cursor - 1] == b'#' {
                    start = cursor - 1;
                    continue;
                }
            }

            let line_start = file_source[..start]
                .rfind('\n')
                .map_or(0, |newline| newline + 1);
            let line = file_source[line_start..start].trim_start();
            if line.starts_with("///") || line.starts_with("//!") {
                start = line_start;
                continue;
            }
            break;
        }

        // Return the completed analysis result.
        Some(Self(format!(
            "{}{}",
            &file_source[start..offset],
            item_source
        )))
    }
}

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
        if cx.tcx.def_kind(parent) != DefKind::Struct {
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
