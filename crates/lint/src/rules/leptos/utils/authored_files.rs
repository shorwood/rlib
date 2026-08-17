extern crate rustc_ast;
extern crate rustc_lint;
extern crate rustc_span;

use std::collections::BTreeSet;
use std::ops::Range;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use rustc_ast::ast::Item;
use rustc_lint::{EarlyContext, LintContext};
use rustc_span::{BytePos, SourceFile, Span};

// -----------------------------------------------------------------------------
// SourceDocument: File-backed diagnostics and fixes
// -----------------------------------------------------------------------------

/// One authored file loaded into rustc's source map.
pub struct SourceDocument {
    /// Canonical local path used for pairing and deduplication.
    pub(crate) path: PathBuf,
    /// Complete authored source.
    pub(crate) source: String,
    /// rustc source file used to construct editable spans.
    file: Arc<SourceFile>,
}

impl SourceDocument {
    /// Loads an authored file through rustc so diagnostics can edit it.
    pub(crate) fn load(cx: &EarlyContext<'_>, path: &Path) -> Option<Self> {
        // Files without a canonical local path cannot participate in stable deduplication.
        let Ok(path) = path.canonicalize() else {
            // Canonicalization failure leaves no stable local identity for the document.
            return None;
        };

        // Files rustc cannot load cannot provide diagnostic spans or authored source.
        let Ok(file) = cx.sess().source_map().load_file(&path) else {
            // Loading failure leaves no source map entry from which to construct spans.
            return None;
        };
        let source = file.src.as_ref()?.as_ref().clone();
        Some(Self { path, source, file })
    }

    /// Returns an exact root-context span for a byte range in this file.
    pub(crate) fn span(&self, range: Range<usize>) -> Span {
        let source_len = self.source.len();
        let start = range.start.min(source_len);
        let end = range.end.min(source_len).max(start);
        Span::with_root_ctxt(
            self.file.start_pos + BytePos(u32::try_from(start).expect("source offset fits")),
            self.file.start_pos + BytePos(u32::try_from(end).expect("source offset fits")),
        )
    }

    /// Returns the span of the complete file.
    pub(crate) fn complete_span(&self) -> Span {
        self.span(0..self.source.len())
    }
}

// -----------------------------------------------------------------------------
// AuthoredFiles: Macro-origin file collection
// -----------------------------------------------------------------------------

/// Files containing authored macros relevant to one source-oriented lint pass.
#[derive(Default)]
pub struct AuthoredFiles {
    /// Canonical Rust source paths observed by this lint pass.
    paths: BTreeSet<PathBuf>,
}

impl AuthoredFiles {
    /// Loads every observed source file in deterministic path order.
    pub(crate) fn documents(&self, cx: &EarlyContext<'_>) -> Vec<SourceDocument> {
        self.paths
            .iter()
            .filter_map(|path| SourceDocument::load(cx, path))
            .collect()
    }

    /// Records the local Rust source containing one expanded crate item.
    pub(crate) fn observe_item(&mut self, cx: &EarlyContext<'_>, item: &Item) {
        // Out-of-line modules are loaded only after pre-expansion linting, so an
        // ordinary early pass must recover their authored files from item spans.
        let Some(path) = cx
            .sess()
            .source_map()
            .span_to_filename(item.span)
            .into_local_path()
        else {
            return;
        };

        // Only Rust source files participate in authored macro analysis.
        if path.extension().is_none_or(|extension| extension != "rs") {
            return;
        }
        self.paths.insert(path.canonicalize().unwrap_or(path));
    }
}
