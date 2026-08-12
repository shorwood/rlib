extern crate rustc_ast;
extern crate rustc_errors;
extern crate rustc_span;

use std::borrow::Cow;
use std::path::Path;

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

impl MixedModuleFileLayouts {
    /// Returns whether a companion directory contains Rust source at any depth.
    fn directory_contains_rust_source(directory: &Path) -> bool {
        let Ok(entries) = directory.read_dir() else {
            return true;
        };
        entries.filter_map(Result::ok).any(|entry| {
            let path = entry.path();
            (path.is_file() && path.extension().is_some_and(|extension| extension == "rs"))
                || (path.is_dir() && Self::directory_contains_rust_source(&path))
        })
    }
}

dylint_linting::impl_pre_expansion_lint! {
    #[doc = include_str!("README.md")]
    pub MIXED_MODULE_FILE_LAYOUTS,
    Warn,
    "rejects modules split between a flat source file and a same-name directory",
    MixedModuleFileLayouts
}

impl EarlyLintPass for MixedModuleFileLayouts {
    fn check_item(&mut self, cx: &EarlyContext<'_>, item: &Item) {
        // Restrict the rule to authored, conventionally located out-of-line modules.
        let ItemKind::Mod(_, ident, kind) = &item.kind else {
            return;
        };
        if matches!(kind, ModKind::Loaded(_, Inline::Yes, _)) {
            return;
        }

        // Preserve generated modules and declarations with intentional custom source paths.
        if item.span.from_expansion()
            || item
                .attrs
                .iter()
                .any(|attribute| attribute.has_name(sym::path))
        {
            return;
        }

        // Resolve the declaring file so both conventional module locations can be checked.
        let source_map = cx.sess().source_map();
        let Some(parent_source) = source_map.span_to_filename(item.span).into_local_path() else {
            return;
        };
        let Some(parent_directory) = parent_source.parent() else {
            return;
        };

        // Diagnose only the exact `name.rs` plus `name/` split, not unrelated sibling styles.
        let name = ident.name.as_str();
        let companion_directory = parent_directory.join(name);
        if !parent_directory.join(format!("{name}.rs")).is_file()
            || !companion_directory.is_dir()
            || Self::directory_contains_rust_source(&companion_directory)
        {
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
