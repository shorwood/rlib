extern crate rustc_ast;
extern crate rustc_errors;
extern crate rustc_span;

use std::borrow::Cow;

use rustc_ast::ast::{Inline, Item, ItemKind, ModKind};
use rustc_errors::DiagDecorator;
use rustc_lint::{EarlyContext, EarlyLintPass, LintContext};
use rustc_span::Span;
use rustc_span::symbol::sym;

use crate::utils::diagnostic::EarlyViolation;

// -----------------------------------------------------------------------------
// Violation: Split module root and companion directory
// -----------------------------------------------------------------------------

/// Flat module source that also owns a same-name companion directory.
struct Violation {
    /// Out-of-line module declaration used as the diagnostic location.
    span: Span,
    /// Authored module name used to render both competing layouts.
    name: String,
}

impl EarlyViolation for Violation {
    fn primary_message(&self) -> Cow<'_, str> {
        Cow::Owned(format!(
            "module `{}` splits its files across two source roots",
            self.name
        ))
    }

    fn rationale_message(&self) -> Cow<'_, str> {
        Cow::Owned(format!(
            "`{}.rs` and `{}/` make readers check two locations for one module",
            self.name, self.name
        ))
    }

    fn remediation_message(&self) -> Cow<'_, str> {
        Cow::Owned(format!(
            "move `{}.rs` to `{}/mod.rs` so the module and its companion files share one directory",
            self.name, self.name
        ))
    }

    fn emit(self, cx: &EarlyContext<'_>) {
        cx.emit_span_lint(
            MIXED_MODULE_FILE_LAYOUTS,
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
// MixedModuleFileLayouts: Module source location policy
// -----------------------------------------------------------------------------

/// Rejects a flat module source beside a same-name companion directory.
struct MixedModuleFileLayouts;

crate::impl_early_lint! {
    #[doc = include_str!("README.md")]
    pub MIXED_MODULE_FILE_LAYOUTS,
    Warn,
    "rejects modules split between a flat source file and a same-name directory",
    MixedModuleFileLayouts
}

impl EarlyLintPass for MixedModuleFileLayouts {
    fn check_item(&mut self, cx: &EarlyContext<'_>, item: &Item) {
        // Restrict the rule to authored, conventionally located out-of-line modules.
        let ItemKind::Mod(_, ident, ModKind::Loaded(_, Inline::No { .. }, spans)) = &item.kind
        else {
            return;
        };

        // Preserve generated modules and declarations with intentional custom source paths.
        if item.span.from_expansion()
            || item
                .attrs
                .iter()
                .any(|attribute| attribute.has_name(sym::path))
        {
            return;
        }

        // Resolve the actual loaded module file rather than reconstructing nested search roots.
        let source_map = cx.sess().source_map();

        // Non-local module sources cannot be mapped to a filesystem companion directory.
        let Some(module_source) = source_map
            .span_to_filename(spans.inner_span)
            .into_local_path()
        else {
            return;
        };
        let name = ident.name.as_str();

        // Directory-root modules and custom filenames do not select the flat `name.rs` layout.
        if module_source
            .extension()
            .is_none_or(|extension| extension != "rs")
            || module_source.file_stem().and_then(|stem| stem.to_str()) != Some(name)
        {
            return;
        }

        // Every same-name directory creates a second physical root, independent of its contents.
        let companion_directory = module_source.with_extension("");

        // A standalone flat source has only one physical module root.
        if !companion_directory.is_dir() {
            return;
        }

        // Report the declaration without attempting a cross-file automatic move.
        Violation {
            span: item.span,
            name: name.to_owned(),
        }
        .emit(cx);
    }
}
