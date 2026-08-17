extern crate rustc_ast;
extern crate rustc_errors;
extern crate rustc_span;

use std::borrow::Cow;
use std::path::{Path, PathBuf};

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
        // An unreadable directory is conservatively treated as containing source to avoid noise.
        let Ok(entries) = directory.read_dir() else {
            return true;
        };
        entries.filter_map(Result::ok).any(|entry| {
            let path = entry.path();
            (path.is_file() && path.extension().is_some_and(|extension| extension == "rs"))
                || (path.is_dir() && Self::directory_contains_rust_source(&path))
        })
    }

    /// Resolves the unique conventional flat source candidate for a module declaration.
    fn flat_module_source(parent_source: &Path, name: &str) -> Option<PathBuf> {
        let parent_directory = parent_source.parent()?;
        let mut roots = vec![parent_directory.to_owned()];

        // A conventionally loaded `parent.rs` owns children under `parent/`. A `#[path]` parent
        // instead searches beside its file; considering both and requiring uniqueness avoids
        // guessing when early expansion has not exposed directory-ownership metadata.
        let parent_name = parent_source.file_name()?.to_str()?;
        if !matches!(parent_name, "lib.rs" | "main.rs" | "mod.rs") {
            roots.push(parent_source.with_extension(""));
        }

        let mut candidates = roots
            .into_iter()
            .map(|root| root.join(format!("{name}.rs")))
            .filter(|source| source.is_file());
        let candidate = candidates.next()?;
        candidates.next().is_none().then_some(candidate)
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

        // Inline modules do not select a separate conventional source-file layout.
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

        // Resolve the declaring file and the unique conventional flat source candidate.
        let source_map = cx.sess().source_map();

        // Non-local source names cannot be mapped to a filesystem companion directory.
        let Some(parent_source) = source_map.span_to_filename(item.span).into_local_path() else {
            return;
        };
        let name = ident.name.as_str();

        // Declarations without a matching flat module file cannot form the mixed layout.
        let Some(module_source) = Self::flat_module_source(&parent_source, name) else {
            return;
        };

        // Diagnose only a flat source plus a same-name non-Rust companion directory.
        let companion_directory = module_source.with_extension("");

        // Missing companions and companions already containing Rust source are conventional layouts.
        if !companion_directory.is_dir()
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
