extern crate rustc_errors;
extern crate rustc_hir;

use std::borrow::Cow;

use rustc_errors::{Applicability, DiagDecorator};
use rustc_hir::{HirId, Item, ItemKind, Mod};
use rustc_lint::{LateContext, LateLintPass, LintContext};

use crate::utils::diagnostic::LateViolation;
use crate::utils::item_separation::{Analyzer, Finding, Source};
use crate::utils::source_provenance::{ItemProvenanceExt, SpanProvenanceExt};

/// Adjacent module declarations lacking a visually empty line between them.
struct Violation(Finding);

impl LateViolation for Violation {
    fn primary_message(&self) -> Cow<'_, str> {
        Cow::Owned(format!(
            "module-level declarations `{}` and `{}` are not separated by a blank line",
            self.0.previous_name, self.0.following_name
        ))
    }

    fn rationale_message(&self) -> Cow<'_, str> {
        Cow::Borrowed(
            "visually separating module-level declarations makes ownership and declaration boundaries easier to scan",
        )
    }

    fn remediation_message(&self) -> Cow<'_, str> {
        Cow::Owned(format!(
            "insert one blank line before `{}`",
            self.0.following_name
        ))
    }

    fn emit(self, cx: &LateContext<'_>) {
        let primary = self.primary_message().into_owned();
        let remediation = self.remediation_message().into_owned();
        cx.tcx.emit_node_span_lint(
            UNSEPARATED_MODULE_ITEMS,
            self.0.hir_id,
            self.0.span,
            DiagDecorator(|diag| {
                diag.primary_message(primary);
                diag.span_label(self.0.span, "this declaration needs visual separation");
                diag.note(self.rationale_message().into_owned());
                if let Some(insertion) = self.0.insertion {
                    diag.span_suggestion(
                        insertion,
                        remediation,
                        "\n",
                        Applicability::MachineApplicable,
                    );
                } else {
                    diag.help(remediation);
                }
            }),
        );
    }
}

/// Late lint pass requiring one visually empty line between module declarations.
struct UnseparatedModuleItems;

dylint_linting::impl_late_lint! {
    #[doc = include_str!("README.md")]
    pub UNSEPARATED_MODULE_ITEMS,
    Warn,
    "rejects module-level declarations without a separating blank line",
    UnseparatedModuleItems
}

impl UnseparatedModuleItems {
    /// Returns whether an item participates in declaration spacing.
    const fn is_declaration(item: &Item<'_>) -> bool {
        !matches!(
            item.kind,
            ItemKind::ExternCrate(..) | ItemKind::Use(..) | ItemKind::Macro(..) | ItemKind::Mod(..)
        )
    }

    /// Produces a stable human-readable name for one declaration.
    fn name(cx: &LateContext<'_>, item: &Item<'_>) -> String {
        if matches!(item.kind, ItemKind::ForeignMod { .. }) {
            return "extern block".to_owned();
        }
        if matches!(item.kind, ItemKind::Impl(_)) {
            return format!(
                "impl {}",
                cx.tcx.type_of(item.owner_id).instantiate_identity()
            );
        }
        item.kind
            .ident()
            .map_or_else(|| "declaration".to_owned(), |ident| ident.name.to_string())
    }

    /// Emits findings for one uninterrupted run of eligible declarations.
    fn check_run(cx: &LateContext<'_>, run: &[Source]) {
        for finding in Analyzer::findings(cx, run) {
            Violation(finding).emit(cx);
        }
    }
}

impl<'tcx> LateLintPass<'tcx> for UnseparatedModuleItems {
    fn check_mod(&mut self, cx: &LateContext<'tcx>, module: &'tcx Mod<'tcx>, _: HirId) {
        let mut run = Vec::new();
        let source_map = cx.sess().source_map();
        for id in module.item_ids {
            let item = cx.tcx.hir_item(*id);
            let is_authored = !item.span.in_external_macro(source_map)
                && !item.span.is_build_generated(cx)
                && !item.is_framework_generated();
            // Generated declarations are invisible in authored source and cannot break adjacency.
            if !is_authored {
                continue;
            }
            // Authored imports, module declarations, and macros bound separate declaration runs.
            if !Self::is_declaration(item) {
                Self::check_run(cx, &run);
                run.clear();
                continue;
            }
            run.push(Source::new(item.hir_id(), item.span, Self::name(cx, item)));
        }
        Self::check_run(cx, &run);
    }
}
